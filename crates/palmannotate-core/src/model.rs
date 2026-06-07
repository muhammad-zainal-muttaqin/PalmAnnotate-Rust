use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u8 = 4;
pub const UNASSIGNED_CLASS_ID: i32 = -1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub variety: String,
    #[serde(default, alias = "blok")]
    pub block: String,
    #[serde(default)]
    pub group_key: String,
    #[serde(default = "default_session_side_count")]
    pub side_count: usize,
    #[serde(default = "default_true")]
    pub auto_id: bool,
    #[serde(default = "default_next_id")]
    pub next_id: usize,
    #[serde(default)]
    pub operator: String,
    pub export_uri: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub trees: Vec<TreeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TreeSummary {
    pub id: String,
    pub tree_name: String,
    #[serde(default)]
    pub tree_id: usize,
    pub side_count: usize,
    pub status: TreeStatus,
    #[serde(default)]
    pub updated_at: String,
}

const fn default_session_side_count() -> usize {
    4
}

const fn default_true() -> bool {
    true
}

const fn default_next_id() -> usize {
    1
}

pub fn group_key_for(variety: &str, block: &str) -> String {
    fn normalize(value: &str) -> String {
        value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_uppercase)
            .collect()
    }
    format!("{}__{}", normalize(variety), normalize(block))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TreeStatus {
    #[default]
    Draft,
    Captured,
    Annotated,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TreeMetadata {
    #[serde(default)]
    pub variety: String,
    #[serde(default, alias = "blok")]
    pub block: String,
    #[serde(default)]
    pub operator: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub gps: Option<GpsPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpsPoint {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tree {
    pub version: u8,
    pub id: String,
    pub session_id: String,
    pub tree_name: String,
    #[serde(default = "default_split")]
    pub split: String,
    pub side_count: usize,
    #[serde(default)]
    pub metadata: TreeMetadata,
    #[serde(default)]
    pub sides: Vec<Side>,
    #[serde(default, rename = "_confirmedLinks")]
    pub confirmed_links: Vec<ConfirmedLink>,
    #[serde(default)]
    pub status: TreeStatus,
}

fn default_split() -> String {
    "field".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Side {
    pub side_index: usize,
    pub label: String,
    pub image_path: String,
    pub image_width: u32,
    pub image_height: u32,
    #[serde(default)]
    pub depth_path: Option<String>,
    #[serde(default)]
    pub depth: Option<DepthMetadata>,
    #[serde(default)]
    pub bboxes: Vec<BBox>,
    /// Detector baseline kept for the annotation behavior log (suggestions vs
    /// final). Mirrors the JS `side.originalBboxes`. Empty until the detector runs.
    #[serde(default)]
    pub original_bboxes: Vec<BBox>,
    /// Per-capture token appended to image URLs so reusing a tree id cannot show
    /// a stale WebView-cached photo. Mirrors the JS `side.cacheBust`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_bust: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DepthMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub value_scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BBox {
    pub id: String,
    pub class_id: i32,
    pub class_name: String,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    #[serde(default)]
    pub confidence: Option<f32>,
}

impl BBox {
    pub fn class_name_for(id: i32) -> &'static str {
        match id {
            0 => "B1",
            1 => "B2",
            2 => "B3",
            3 => "B4",
            _ => "U",
        }
    }

    pub fn is_assigned(&self) -> bool {
        (0..=3).contains(&self.class_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedLink {
    pub link_id: String,
    pub side_a: usize,
    pub bbox_id_a: String,
    pub side_b: usize,
    pub bbox_id_b: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputV4 {
    pub version: u8,
    pub tree_id: String,
    pub tree_name: String,
    pub split: String,
    pub metadata: OutputMetadata,
    pub images: BTreeMap<String, OutputImage>,
    pub bunches: Vec<OutputBunch>,
    #[serde(rename = "_confirmedLinks")]
    pub confirmed_links: Vec<ConfirmedLink>,
    pub summary: OutputSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputMetadata {
    pub variety: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputImage {
    pub filename: String,
    pub label_file: String,
    pub side_index: usize,
    pub side_label: String,
    pub width: u32,
    pub height: u32,
    pub bbox_count: usize,
    pub annotations: Vec<OutputAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputAnnotation {
    pub box_index: usize,
    pub class_id: i32,
    pub class_name: String,
    pub bbox_yolo: [f64; 4],
    pub bbox_pixel: [i64; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputBunch {
    pub bunch_id: usize,
    pub class: String,
    pub class_mismatch: bool,
    pub appearance_count: usize,
    pub appearances: Vec<OutputAppearance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputAppearance {
    pub side: String,
    pub side_index: usize,
    pub box_index: usize,
    pub class_name: String,
    pub bbox_pixel: [i64; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputSummary {
    pub total_unique_bunches: usize,
    pub total_detections: usize,
    pub duplicates_linked: usize,
    pub by_class: BTreeMap<String, usize>,
    pub by_side: BTreeMap<String, usize>,
}
