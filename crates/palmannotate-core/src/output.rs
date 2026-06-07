use crate::{
    compute_results, AppError, AppResult, BBox, OutputAnnotation, OutputAppearance, OutputBunch,
    OutputImage, OutputMetadata, OutputSummary, OutputV4, Side, Tree, SCHEMA_VERSION,
};
use chrono::Utc;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

fn side_key(index: usize) -> String {
    format!("side_{}", index + 1)
}

fn is_adjacent(a: usize, b: usize, total: usize) -> bool {
    total == 2 || a.abs_diff(b) == 1 || (total > 2 && a.abs_diff(b) == total - 1)
}

fn yolo(bbox: &BBox, width: u32, height: u32) -> [f64; 4] {
    let width = f64::from(width);
    let height = f64::from(height);
    [
        (((bbox.x1 + bbox.x2) / 2.0) / width * 1_000_000.0).round() / 1_000_000.0,
        (((bbox.y1 + bbox.y2) / 2.0) / height * 1_000_000.0).round() / 1_000_000.0,
        ((bbox.x2 - bbox.x1) / width * 1_000_000.0).round() / 1_000_000.0,
        ((bbox.y2 - bbox.y1) / height * 1_000_000.0).round() / 1_000_000.0,
    ]
}

pub fn build_output_v4(tree: &Tree) -> OutputV4 {
    let result = compute_results(tree);
    let mut images = BTreeMap::new();
    for side in &tree.sides {
        let filename = Path::new(&side.image_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        let label_file = Path::new(&filename)
            .with_extension("txt")
            .to_string_lossy()
            .into_owned();
        let annotations = side
            .bboxes
            .iter()
            .enumerate()
            .map(|(box_index, bbox)| OutputAnnotation {
                box_index,
                class_id: bbox.class_id,
                class_name: bbox.class_name.clone(),
                bbox_yolo: yolo(bbox, side.image_width, side.image_height),
                bbox_pixel: [
                    bbox.x1.round() as i64,
                    bbox.y1.round() as i64,
                    bbox.x2.round() as i64,
                    bbox.y2.round() as i64,
                ],
            })
            .collect::<Vec<_>>();
        images.insert(
            side_key(side.side_index),
            OutputImage {
                filename,
                label_file,
                side_index: side.side_index,
                side_label: side.label.clone(),
                width: side.image_width,
                height: side.image_height,
                bbox_count: annotations.len(),
                annotations,
            },
        );
    }

    let bunches = result
        .clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            let mut appearances = cluster
                .members
                .iter()
                .map(|&(side_index, box_index)| {
                    let bbox = &tree.sides[side_index].bboxes[box_index];
                    OutputAppearance {
                        side: side_key(side_index),
                        side_index,
                        box_index,
                        class_name: bbox.class_name.clone(),
                        bbox_pixel: [
                            bbox.x1.round() as i64,
                            bbox.y1.round() as i64,
                            bbox.x2.round() as i64,
                            bbox.y2.round() as i64,
                        ],
                    }
                })
                .collect::<Vec<_>>();
            appearances.sort_by_key(|item| item.side_index);
            OutputBunch {
                bunch_id: index + 1,
                class: cluster.dominant_class.clone(),
                class_mismatch: cluster.class_mismatch,
                appearance_count: appearances.len(),
                appearances,
            }
        })
        .collect();

    let bbox_indices = tree
        .sides
        .iter()
        .flat_map(|side| {
            side.bboxes
                .iter()
                .enumerate()
                .map(move |(index, bbox)| ((side.side_index, bbox.id.as_str()), index))
        })
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let confirmed_links = tree
        .confirmed_links
        .iter()
        .filter_map(|link| {
            let &a = bbox_indices.get(&(link.side_a, link.bbox_id_a.as_str()))?;
            let &b = bbox_indices.get(&(link.side_b, link.bbox_id_b.as_str()))?;
            if !is_adjacent(link.side_a, link.side_b, tree.sides.len()) {
                return None;
            }
            let mut out = link.clone();
            out.bbox_id_a = format!("b{a}");
            out.bbox_id_b = format!("b{b}");
            let key = if (out.side_a, &out.bbox_id_a) <= (out.side_b, &out.bbox_id_b) {
                format!(
                    "{}:{}|{}:{}",
                    out.side_a, out.bbox_id_a, out.side_b, out.bbox_id_b
                )
            } else {
                format!(
                    "{}:{}|{}:{}",
                    out.side_b, out.bbox_id_b, out.side_a, out.bbox_id_a
                )
            };
            seen.insert(key).then_some(out)
        })
        .collect();

    let by_side = tree
        .sides
        .iter()
        .map(|side| (side_key(side.side_index), side.bboxes.len()))
        .collect();
    OutputV4 {
        version: SCHEMA_VERSION,
        tree_id: tree.tree_name.clone(),
        tree_name: tree.tree_name.clone(),
        split: tree.split.clone(),
        metadata: OutputMetadata {
            variety: if tree.metadata.variety.trim().is_empty() {
                tree.tree_name
                    .split('_')
                    .next()
                    .unwrap_or("UNKNOWN")
                    .to_uppercase()
            } else {
                tree.metadata.variety.trim().into()
            },
            generated_at: Utc::now().to_rfc3339(),
        },
        images,
        bunches,
        confirmed_links,
        summary: OutputSummary {
            total_unique_bunches: result.unique_count,
            total_detections: result.raw_count,
            duplicates_linked: result.linked_count,
            by_class: result.class_counts,
            by_side,
        },
    }
}

pub fn load_output_v4(output: OutputV4, id: String, session_id: String) -> AppResult<Tree> {
    if output.version != SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "Unsupported output schema version {}",
            output.version
        )));
    }
    let mut sides = output
        .images
        .into_values()
        .map(|image| Side {
            side_index: image.side_index,
            label: image.side_label.replace("Sisi", "Side"),
            image_path: image.filename,
            image_width: image.width,
            image_height: image.height,
            depth_path: None,
            depth: None,
            original_bboxes: Vec::new(),
            cache_bust: None,
            bboxes: image
                .annotations
                .into_iter()
                .enumerate()
                .map(|(index, annotation)| BBox {
                    id: format!("b{}", annotation.box_index.max(index)),
                    class_id: annotation.class_id,
                    class_name: annotation.class_name,
                    x1: annotation.bbox_pixel[0] as f64,
                    y1: annotation.bbox_pixel[1] as f64,
                    x2: annotation.bbox_pixel[2] as f64,
                    y2: annotation.bbox_pixel[3] as f64,
                    confidence: None,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    sides.sort_by_key(|side| side.side_index);
    Ok(Tree {
        version: SCHEMA_VERSION,
        id,
        session_id,
        tree_name: output.tree_name,
        split: output.split,
        side_count: sides.len(),
        metadata: crate::TreeMetadata {
            variety: output.metadata.variety,
            ..Default::default()
        },
        sides,
        confirmed_links: output.confirmed_links,
        status: crate::TreeStatus::Annotated,
    })
}
