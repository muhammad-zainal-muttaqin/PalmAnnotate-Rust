use crate::{BBox, Side, Tree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinkSuggestion {
    pub link_id: String,
    pub side_a: usize,
    pub bbox_id_a: String,
    pub side_b: usize,
    pub bbox_id_b: String,
    pub score: f64,
    pub category: String,
    pub signals: SuggestionSignals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestionSignals {
    pub seam: f64,
    pub vert: f64,
    pub size: f64,
    pub cls: f64,
    #[serde(rename = "sizeRatio")]
    pub size_ratio: f64,
}

#[derive(Clone, Copy)]
struct Normalized {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

#[derive(Clone)]
struct Scored {
    a_index: usize,
    b_index: usize,
    bbox_id_a: String,
    bbox_id_b: String,
    score: f64,
    signals: SuggestionSignals,
}

pub fn suggest_tree(tree: &Tree) -> Vec<LinkSuggestion> {
    let total = tree.sides.len();
    let mut suggestions = Vec::new();
    for side_a in 0..total {
        let side_b = (side_a + 1) % total.max(1);
        let Some(a) = tree.sides.get(side_a) else {
            continue;
        };
        let Some(b) = tree.sides.get(side_b) else {
            continue;
        };
        for pair in suggest_pair(a, b) {
            if tree.confirmed_links.iter().any(|link| {
                link.side_a == side_a
                    && link.bbox_id_a == pair.bbox_id_a
                    && link.side_b == side_b
                    && link.bbox_id_b == pair.bbox_id_b
            }) {
                continue;
            }
            suggestions.push(LinkSuggestion {
                link_id: format!("sug-{side_a}-{side_b}-{}", suggestions.len()),
                side_a,
                bbox_id_a: pair.bbox_id_a,
                side_b,
                bbox_id_b: pair.bbox_id_b,
                score: pair.score,
                category: if pair.score >= 0.75 {
                    "auto".into()
                } else {
                    "candidate".into()
                },
                signals: pair.signals,
            });
        }
    }
    suggestions
}

fn suggest_pair(side_a: &Side, side_b: &Side) -> Vec<Scored> {
    if side_a.image_width == 0
        || side_a.image_height == 0
        || side_b.image_width == 0
        || side_b.image_height == 0
    {
        return vec![];
    }
    let gated_a = gate(
        &side_a.bboxes,
        side_a.image_width,
        side_a.image_height,
        true,
    );
    let gated_b = gate(
        &side_b.bboxes,
        side_b.image_width,
        side_b.image_height,
        false,
    );
    let mut scored = Vec::new();
    for (a_index, (bbox_a, norm_a, center_a)) in gated_a.iter().enumerate() {
        for (b_index, (bbox_b, norm_b, center_b)) in gated_b.iter().enumerate() {
            let area_a = area(*norm_a).max(1e-6);
            let area_b = area(*norm_b).max(1e-6);
            let size_ratio = area_a.min(area_b) / area_a.max(area_b);
            if size_ratio < 0.30 {
                continue;
            }
            let seam_a = 1.0 - (center_a / 0.50).clamp(0.0, 1.0);
            let seam_b = 1.0 - ((1.0 - center_b) / 0.50).clamp(0.0, 1.0);
            let seam = (seam_a + seam_b) / 2.0;
            let vert = vertical_signal(*norm_a, *norm_b);
            let size = size_signal(*norm_a, *norm_b);
            let class = class_multiplier(bbox_a, bbox_b);
            let score = (0.45 * seam + 0.35 * vert + 0.20 * size) * class;
            if score < 0.50 {
                continue;
            }
            scored.push(Scored {
                a_index,
                b_index,
                bbox_id_a: bbox_a.id.clone(),
                bbox_id_b: bbox_b.id.clone(),
                score: score.clamp(0.0, 1.0),
                signals: SuggestionSignals {
                    seam: round(seam, 3),
                    vert: round(vert, 3),
                    size: round(size, 3),
                    cls: round(class, 2),
                    size_ratio: round(size_ratio, 3),
                },
            });
        }
    }

    let mut best_a: HashMap<usize, Scored> = HashMap::new();
    let mut best_b: HashMap<usize, Scored> = HashMap::new();
    for pair in scored {
        if best_a
            .get(&pair.a_index)
            .is_none_or(|current| pair.score > current.score)
        {
            best_a.insert(pair.a_index, pair.clone());
        }
        if best_b
            .get(&pair.b_index)
            .is_none_or(|current| pair.score > current.score)
        {
            best_b.insert(pair.b_index, pair);
        }
    }
    let mut chosen = best_a
        .into_values()
        .filter(|pair| {
            best_b
                .get(&pair.b_index)
                .is_some_and(|other| other.a_index == pair.a_index)
        })
        .collect::<Vec<_>>();
    chosen.sort_by(|left, right| right.score.total_cmp(&left.score));
    chosen
}

fn gate(boxes: &[BBox], width: u32, height: u32, left_edge: bool) -> Vec<(&BBox, Normalized, f64)> {
    boxes
        .iter()
        .filter_map(|bbox| {
            let norm = normalize(bbox, width, height);
            let center = (norm.x1 + norm.x2) / 2.0;
            (if left_edge {
                center <= 0.50
            } else {
                center >= 0.50
            })
            .then_some((bbox, norm, center))
        })
        .collect()
}

fn normalize(bbox: &BBox, width: u32, height: u32) -> Normalized {
    Normalized {
        x1: bbox.x1 / f64::from(width),
        y1: bbox.y1 / f64::from(height),
        x2: bbox.x2 / f64::from(width),
        y2: bbox.y2 / f64::from(height),
    }
}

fn area(value: Normalized) -> f64 {
    (value.x2 - value.x1) * (value.y2 - value.y1)
}

fn vertical_signal(a: Normalized, b: Normalized) -> f64 {
    let center_a = (a.y1 + a.y2) / 2.0;
    let center_b = (b.y1 + b.y2) / 2.0;
    1.0 - ((center_a - center_b).abs() / 0.20).clamp(0.0, 1.0)
}

fn size_signal(a: Normalized, b: Normalized) -> f64 {
    let area_a = area(a).max(1e-6);
    let area_b = area(b).max(1e-6);
    let area_similarity = 1.0 - ((area_a - area_b).abs() / area_a.max(area_b)).clamp(0.0, 1.0);
    let aspect_a = ((a.x2 - a.x1) / (a.y2 - a.y1).max(1e-6)).max(1e-6);
    let aspect_b = ((b.x2 - b.x1) / (b.y2 - b.y1).max(1e-6)).max(1e-6);
    let aspect_similarity =
        1.0 - ((aspect_a - aspect_b).abs() / aspect_a.max(aspect_b)).clamp(0.0, 1.0);
    0.6 * area_similarity + 0.4 * aspect_similarity
}

fn class_multiplier(a: &BBox, b: &BBox) -> f64 {
    if a.class_id == b.class_id {
        1.0
    } else if (a.class_id - b.class_id).abs() == 1 {
        0.85
    } else {
        0.5
    }
}

fn round(value: f64, places: i32) -> f64 {
    let scale = 10_f64.powi(places);
    (value * scale).round() / scale
}
