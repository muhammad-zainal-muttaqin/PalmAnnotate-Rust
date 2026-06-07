use crate::{compute_results, Tree, TreeStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityIssue {
    pub level: QualityLevel,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityReport {
    pub ready: bool,
    pub issues: Vec<QualityIssue>,
}

pub fn check_tree(tree: &Tree) -> QualityReport {
    let mut issues = Vec::new();
    if tree.metadata.variety.trim().is_empty() {
        issues.push(issue(
            QualityLevel::Error,
            "metadata_variety_missing",
            "Variety is required.",
        ));
    }
    if tree.metadata.block.trim().is_empty() {
        issues.push(issue(
            QualityLevel::Warning,
            "metadata_block_missing",
            "Block is missing.",
        ));
    }
    if tree.metadata.timestamp.trim().is_empty() {
        issues.push(issue(
            QualityLevel::Error,
            "metadata_timestamp_missing",
            "Capture timestamp is missing.",
        ));
    }
    if tree.metadata.operator.trim().is_empty() {
        issues.push(issue(
            QualityLevel::Info,
            "metadata_operator_missing",
            "Operator is empty.",
        ));
    }
    match &tree.metadata.gps {
        None => issues.push(issue(
            QualityLevel::Warning,
            "metadata_gps_missing",
            "GPS is unavailable; capture can continue.",
        )),
        Some(gps) if gps.accuracy.is_some_and(|accuracy| accuracy > 25.0) => issues.push(issue(
            QualityLevel::Warning,
            "metadata_gps_low_accuracy",
            "GPS accuracy is lower than 25 metres.",
        )),
        Some(_) => {}
    }
    if tree.sides.len() != tree.side_count {
        issues.push(issue(
            QualityLevel::Error,
            "tree_side_count",
            "Stored side count is incomplete.",
        ));
    }
    let mut depth_sides = 0;
    let mut total_boxes = 0;
    for side in &tree.sides {
        if side.image_path.trim().is_empty() {
            issues.push(issue(
                QualityLevel::Error,
                "tree_view_missing",
                &format!("Side {} has no RGB image.", side.side_index + 1),
            ));
        }
        if side.bboxes.is_empty() {
            issues.push(issue(
                QualityLevel::Info,
                "annotation_empty_side",
                &format!("Side {} has no bounding boxes.", side.side_index + 1),
            ));
        }
        total_boxes += side.bboxes.len();
        if side.depth_path.is_some() {
            depth_sides += 1;
            if side.depth.as_ref().is_none_or(|depth| {
                depth.width == 0 || depth.height == 0 || depth.value_scale <= 0.0
            }) {
                issues.push(issue(
                    QualityLevel::Warning,
                    "tree_depth_meta_incomplete",
                    &format!("Side {} depth metadata is incomplete.", side.side_index + 1),
                ));
            }
        }
    }
    if depth_sides > 0 && depth_sides < tree.sides.len() {
        issues.push(issue(
            QualityLevel::Warning,
            "tree_rgb_depth_incomplete",
            "RGB/depth pairs are incomplete across captured sides.",
        ));
    }
    if total_boxes == 0 {
        issues.push(issue(
            QualityLevel::Warning,
            "annotation_empty_tree",
            "The tree has no bounding boxes.",
        ));
    } else if total_boxes > 1 && tree.side_count > 1 && tree.confirmed_links.is_empty() {
        issues.push(issue(
            QualityLevel::Warning,
            "annotation_no_links",
            "No cross-view links are confirmed.",
        ));
    }
    if tree
        .sides
        .iter()
        .flat_map(|side| &side.bboxes)
        .any(|bbox| !bbox.is_assigned())
    {
        issues.push(issue(
            QualityLevel::Warning,
            "annotation_unassigned",
            "One or more detections still require a B1-B4 class.",
        ));
    }
    let mismatch_count = compute_results(tree)
        .clusters
        .iter()
        .filter(|cluster| cluster.class_mismatch)
        .count();
    if mismatch_count > 0 {
        issues.push(issue(
            QualityLevel::Error,
            "annotation_class_mismatch",
            &format!("{mismatch_count} linked object(s) have a class mismatch."),
        ));
    }
    if tree.status != TreeStatus::Complete {
        issues.push(issue(
            QualityLevel::Info,
            "result_not_computed",
            "Result has not been marked complete.",
        ));
    }
    QualityReport {
        ready: !issues.iter().any(|item| item.level == QualityLevel::Error),
        issues,
    }
}

fn issue(level: QualityLevel, code: &str, message: &str) -> QualityIssue {
    QualityIssue {
        level,
        code: code.into(),
        message: message.into(),
    }
}
