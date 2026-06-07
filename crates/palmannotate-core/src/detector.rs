use crate::{BBox, UNASSIGNED_CLASS_ID};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectorConfig {
    #[serde(default = "default_model_file")]
    pub model_file: String,
    #[serde(default = "default_input_size")]
    pub input_size: usize,
    #[serde(default = "default_conf_threshold")]
    pub conf_threshold: f32,
    #[serde(default = "default_iou_threshold")]
    pub iou_threshold: f32,
    #[serde(default = "default_max_boxes")]
    pub max_boxes: usize,
    #[serde(default)]
    pub class_aware: bool,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            model_file: default_model_file(),
            input_size: default_input_size(),
            conf_threshold: default_conf_threshold(),
            iou_threshold: default_iou_threshold(),
            max_boxes: default_max_boxes(),
            class_aware: false,
        }
    }
}

fn default_model_file() -> String {
    "ffb-detector.onnx".into()
}

const fn default_input_size() -> usize {
    640
}

const fn default_conf_threshold() -> f32 {
    0.01
}

const fn default_iou_threshold() -> f32 {
    0.30
}

const fn default_max_boxes() -> usize {
    300
}

#[derive(Debug, Clone, Copy)]
pub struct Letterbox {
    pub source_width: u32,
    pub source_height: u32,
    pub scale: f32,
    pub pad_x: f32,
    pub pad_y: f32,
}

impl Letterbox {
    pub fn new(source_width: u32, source_height: u32, input_size: usize) -> Option<Self> {
        if source_width == 0 || source_height == 0 || input_size == 0 {
            return None;
        }
        let size = input_size as f32;
        let scale = (size / source_width as f32).min(size / source_height as f32);
        let new_width = (source_width as f32 * scale).round();
        let new_height = (source_height as f32 * scale).round();
        Some(Self {
            source_width,
            source_height,
            scale,
            pad_x: ((size - new_width) / 2.0).floor(),
            pad_y: ((size - new_height) / 2.0).floor(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Candidate {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    score: f32,
}

pub fn decode_yolo(
    data: &[f32],
    dimensions: &[usize],
    letterbox: Letterbox,
    config: &DetectorConfig,
) -> Vec<BBox> {
    let Some((rows, attributes, channels_first)) = output_orientation(dimensions) else {
        return Vec::new();
    };
    if attributes < 5 || data.len() < rows.saturating_mul(attributes) {
        return Vec::new();
    }

    let at = |row: usize, attribute: usize| {
        if channels_first {
            data[attribute * rows + row]
        } else {
            data[row * attributes + attribute]
        }
    };
    let class_count = attributes - 4;
    let mut candidates = Vec::new();
    for row in 0..rows {
        let score = (0..class_count)
            .map(|class| at(row, 4 + class))
            .fold(0.0_f32, f32::max);
        if !score.is_finite() || score < config.conf_threshold {
            continue;
        }

        let center_x = at(row, 0);
        let center_y = at(row, 1);
        let width = at(row, 2);
        let height = at(row, 3);
        if ![center_x, center_y, width, height]
            .into_iter()
            .all(f32::is_finite)
        {
            continue;
        }

        let max_x = letterbox.source_width as f32;
        let max_y = letterbox.source_height as f32;
        let x1 = ((center_x - width / 2.0 - letterbox.pad_x) / letterbox.scale).clamp(0.0, max_x);
        let y1 = ((center_y - height / 2.0 - letterbox.pad_y) / letterbox.scale).clamp(0.0, max_y);
        let x2 = ((center_x + width / 2.0 - letterbox.pad_x) / letterbox.scale).clamp(0.0, max_x);
        let y2 = ((center_y + height / 2.0 - letterbox.pad_y) / letterbox.scale).clamp(0.0, max_y);
        if x2 - x1 < 1.0 || y2 - y1 < 1.0 {
            continue;
        }
        candidates.push(Candidate {
            x1,
            y1,
            x2,
            y2,
            score,
        });
    }

    non_maximum_suppression(candidates, config.iou_threshold, config.max_boxes)
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| BBox {
            id: format!("det{index}"),
            class_id: UNASSIGNED_CLASS_ID,
            class_name: "U".into(),
            x1: candidate.x1.into(),
            y1: candidate.y1.into(),
            x2: candidate.x2.into(),
            y2: candidate.y2.into(),
            confidence: Some(candidate.score),
        })
        .collect()
}

fn output_orientation(dimensions: &[usize]) -> Option<(usize, usize, bool)> {
    if dimensions.len() < 2 {
        return None;
    }
    let a = dimensions[dimensions.len() - 2];
    let b = dimensions[dimensions.len() - 1];
    let a_is_attributes = a >= 5 && a < b;
    let b_is_attributes = b >= 5 && b < a;
    if a_is_attributes && !b_is_attributes {
        Some((b, a, true))
    } else if b_is_attributes && !a_is_attributes {
        Some((a, b, false))
    } else {
        Some((b, a, true))
    }
}

fn non_maximum_suppression(
    mut candidates: Vec<Candidate>,
    threshold: f32,
    limit: usize,
) -> Vec<Candidate> {
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    let mut kept = Vec::new();
    for candidate in candidates {
        if kept
            .iter()
            .all(|current| intersection_over_union(candidate, *current) <= threshold)
        {
            kept.push(candidate);
            if kept.len() >= limit {
                break;
            }
        }
    }
    kept
}

fn intersection_over_union(left: Candidate, right: Candidate) -> f32 {
    let width = (left.x2.min(right.x2) - left.x1.max(right.x1)).max(0.0);
    let height = (left.y2.min(right.y2) - left.y1.max(right.y1)).max(0.0);
    let intersection = width * height;
    if intersection <= 0.0 {
        return 0.0;
    }
    let left_area = (left.x2 - left.x1) * (left.y2 - left.y1);
    let right_area = (right.x2 - right.x1) * (right.y2 - right.y1);
    let union = left_area + right_area - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}
