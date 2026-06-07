#![allow(non_snake_case)]

#[path = "annotate.rs"]
mod annotate;
#[path = "capture.rs"]
mod capture;
#[path = "workflows.rs"]
mod workflows;

use dioxus::prelude::*;
use dioxus_web::WebEventExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use annotate::Annotate;
use capture::{Capture, Review};
use workflows::{Dedup, DepthViewer, Results};

/// Bundled stylesheet. Using the `asset!` macro makes Dioxus emit a correctly
/// resolved (hashed) `<link>` in the built app. A bare `href: "styles.css"` 404s
/// inside the Tauri/WebView bundle, which left the whole UI rendering as
/// unstyled plain HTML.
const STYLES: Asset = asset!("/assets/styles.css");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = convertFileSrc)]
    fn convert_file_src(file_path: &str) -> String;

    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: &js_sys::Function) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    NewSession,
    SessionDetail,
    Capture,
    Review,
    Annotate,
    Dedup,
    Results,
    DepthViewer,
    Settings,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    product_name: String,
    schema_version: u8,
    data_root: String,
    settings: AppSettings,
    sessions: Vec<Session>,
    platform: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    #[serde(default)]
    export_uri: String,
    #[serde(default)]
    export_name: String,
    #[serde(default)]
    recent_varieties: Vec<String>,
    #[serde(default)]
    recent_blocks: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Session {
    id: String,
    name: String,
    #[serde(default)]
    variety: String,
    #[serde(default)]
    block: String,
    #[serde(default)]
    group_key: String,
    #[serde(default = "default_side_count")]
    side_count: usize,
    #[serde(default = "default_true")]
    auto_id: bool,
    #[serde(default = "default_next_id")]
    next_id: usize,
    operator: String,
    export_uri: String,
    created_at: String,
    updated_at: String,
    trees: Vec<TreeSummary>,
}

const fn default_side_count() -> usize {
    4
}

const fn default_true() -> bool {
    true
}

const fn default_next_id() -> usize {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TreeSummary {
    id: String,
    tree_name: String,
    #[serde(default)]
    tree_id: usize,
    side_count: usize,
    status: String,
    updated_at: String,
}

#[derive(Serialize)]
struct SessionArgs {
    session: Session,
}

#[derive(Serialize)]
struct SettingsArgs {
    settings: AppSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SafFolder {
    #[serde(default)]
    uri: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    cancelled: bool,
}

#[derive(Debug, Deserialize)]
struct NativePath {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PickedFile {
    #[serde(default)]
    path: String,
    #[serde(default)]
    cancelled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonImportData {
    tree_id: String,
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct SafValidation {
    valid: bool,
}

#[derive(Debug, Deserialize)]
struct GeoPermission {
    location: String,
}

#[derive(Debug, Deserialize)]
struct GeoPosition {
    coords: GeoCoordinates,
}

#[derive(Debug, Deserialize)]
struct GeoCoordinates {
    latitude: f64,
    longitude: f64,
    accuracy: f64,
}

#[derive(Debug, Deserialize)]
struct CameraPreviewEvent {
    payload: CameraPreviewPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CameraPreviewPayload {
    jpeg_base64: String,
}

#[derive(Debug, Deserialize)]
struct OrbbecPreviewEvent {
    payload: OrbbecPreviewPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrbbecPreviewPayload {
    #[serde(default)]
    rgb_jpeg_base64: Option<String>,
    #[serde(default)]
    depth_jpeg_base64: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct CapturedFrame {
    path: String,
    width: u32,
    height: u32,
    format: String,
    source: String,
    #[serde(default)]
    depth_path: Option<String>,
    #[serde(default)]
    depth_metadata_path: Option<String>,
    #[serde(default)]
    depth_width: Option<u32>,
    #[serde(default)]
    depth_height: Option<u32>,
    #[serde(default)]
    depth_format: Option<String>,
    #[serde(default)]
    depth_value_scale: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingCapture {
    session: Session,
    tree_number: usize,
    frames: Vec<CapturedFrame>,
    gps: Option<GpsData>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct GpsData {
    latitude: f64,
    longitude: f64,
    accuracy: Option<f64>,
}

#[derive(Clone, Debug)]
struct CommitOutcome {
    tree_id: String,
    mirror_warning: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TreeData {
    version: u8,
    id: String,
    session_id: String,
    tree_name: String,
    split: String,
    side_count: usize,
    metadata: serde_json::Value,
    sides: Vec<SideData>,
    #[serde(rename = "_confirmedLinks", default)]
    confirmed_links: Vec<ConfirmedLinkData>,
    status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ConfirmedLinkData {
    link_id: String,
    side_a: usize,
    bbox_id_a: String,
    side_b: usize,
    bbox_id_b: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SideData {
    side_index: usize,
    label: String,
    image_path: String,
    image_width: u32,
    image_height: u32,
    #[serde(default)]
    depth_path: Option<String>,
    #[serde(default)]
    depth: Option<serde_json::Value>,
    #[serde(default)]
    bboxes: Vec<BoxData>,
    /// Detector baseline for the annotation behavior log (suggestions vs final).
    #[serde(default)]
    original_bboxes: Vec<BoxData>,
    /// Cache-busting token so a reused tree id never shows a stale cached photo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_bust: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct BoxData {
    id: String,
    class_id: i32,
    class_name: String,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    #[serde(default)]
    confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct DetectorData {
    boxes: Vec<BoxData>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ComputeData {
    result: ComputationData,
    quality: QualityData,
    #[serde(default)]
    export_uri: String,
    #[serde(default)]
    export_files: Vec<ExportFileData>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ExportFileData {
    relative_path: String,
    source_path: String,
    mime_type: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ExportData {
    export_uri: String,
    export_files: Vec<ExportFileData>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ComputationData {
    unique_count: usize,
    raw_count: usize,
    linked_count: usize,
    unassigned_count: usize,
    class_counts: std::collections::BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct QualityData {
    ready: bool,
    issues: Vec<QualityIssueData>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct QualityIssueData {
    code: String,
    message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct DepthRenderData {
    path: String,
    width: u32,
    height: u32,
    minimum: f32,
    maximum: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct LinkSuggestionData {
    link_id: String,
    side_a: usize,
    bbox_id_a: String,
    side_b: usize,
    bbox_id_b: String,
    score: f64,
    category: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnnotationMode {
    Review,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

#[derive(Clone, Debug)]
enum BoxGesture {
    Draw {
        start_x: f64,
        start_y: f64,
        current_x: f64,
        current_y: f64,
    },
    Move {
        bbox_id: String,
        start_x: f64,
        start_y: f64,
        original: BoxData,
    },
    Resize {
        bbox_id: String,
        handle: ResizeHandle,
        original: BoxData,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CanvasViewport {
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
}

impl CanvasViewport {
    fn reset() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CanvasPoint {
    base_x: f64,
    base_y: f64,
    image_x: f64,
    image_y: f64,
}

#[derive(Clone, Copy, Debug)]
struct PointerContact {
    id: i32,
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug)]
struct PinchGesture {
    distance: f64,
    centroid_x: f64,
    centroid_y: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
}

#[derive(Clone, Copy, Debug)]
struct SwipeGesture {
    pointer_id: i32,
    start_x: f64,
    start_y: f64,
}

#[derive(Serialize)]
struct CaptureCommitArgs {
    request: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TreeIdArgs {
    tree_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TreeExportArgs {
    tree_id: String,
    export_kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdArgs {
    session_id: String,
}

#[derive(Serialize)]
struct TreeSaveArgs {
    tree: TreeData,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DetectorArgs {
    image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DepthRenderArgs {
    tree_id: String,
    side_index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportFolderArgs {
    folder_path: String,
    export_uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonImportArgs {
    request: JsonImportRequest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonImportRequest {
    file_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SafCopyArgs {
    tree_uri: String,
    relative_path: String,
    source_path: String,
    mime_type: String,
}

fn class_color(class_id: i32) -> &'static str {
    match class_id {
        0 => "#3b82f6",
        1 => "#ef4444",
        2 => "#f59e0b",
        3 => "#8b5cf6",
        _ => "#94a3b8",
    }
}

fn sides_are_adjacent(a: usize, b: usize, count: usize) -> bool {
    count == 2 || a.abs_diff(b) == 1 || (count > 2 && a.abs_diff(b) == count - 1)
}

fn connected_bbox_endpoints(
    tree: &TreeData,
    side_index: usize,
    bbox_id: &str,
) -> HashSet<(usize, String)> {
    let start = (side_index, bbox_id.to_string());
    let mut found = HashSet::from([start.clone()]);
    let mut queue = VecDeque::from([start]);
    while let Some(endpoint) = queue.pop_front() {
        for link in &tree.confirmed_links {
            let left = (link.side_a, link.bbox_id_a.clone());
            let right = (link.side_b, link.bbox_id_b.clone());
            let next = if endpoint == left {
                Some(right)
            } else if endpoint == right {
                Some(left)
            } else {
                None
            };
            if let Some(next) = next {
                if found.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
        }
    }
    found
}

fn set_connected_bbox_class(
    tree: &mut TreeData,
    side_index: usize,
    bbox_id: &str,
    class_id: i32,
) -> usize {
    if !(0..=3).contains(&class_id) {
        return 0;
    }
    let endpoints = connected_bbox_endpoints(tree, side_index, bbox_id);
    let class_name = match class_id {
        0 => "B1",
        1 => "B2",
        2 => "B3",
        3 => "B4",
        _ => unreachable!(),
    };
    let mut updated = 0;
    for (side_index, bbox_id) in endpoints {
        if let Some(bbox) = tree
            .sides
            .get_mut(side_index)
            .and_then(|side| side.bboxes.iter_mut().find(|bbox| bbox.id == bbox_id))
        {
            bbox.class_id = class_id;
            bbox.class_name = class_name.into();
            updated += 1;
        }
    }
    updated
}

fn delete_bbox(tree: &mut TreeData, side_index: usize, bbox_id: &str) -> bool {
    let Some(side) = tree.sides.get_mut(side_index) else {
        return false;
    };
    let before = side.bboxes.len();
    side.bboxes.retain(|bbox| bbox.id != bbox_id);
    if side.bboxes.len() == before {
        return false;
    }
    tree.confirmed_links.retain(|link| {
        !(link.side_a == side_index && link.bbox_id_a == bbox_id
            || link.side_b == side_index && link.bbox_id_b == bbox_id)
    });
    true
}

fn add_confirmed_link(
    tree: &mut TreeData,
    side_a: usize,
    bbox_id_a: String,
    side_b: usize,
    bbox_id_b: String,
) -> Result<(), String> {
    if !sides_are_adjacent(side_a, side_b, tree.sides.len()) {
        return Err("Only adjacent sides can be linked.".into());
    }
    let left_exists = tree
        .sides
        .get(side_a)
        .is_some_and(|side| side.bboxes.iter().any(|bbox| bbox.id == bbox_id_a));
    let right_exists = tree
        .sides
        .get(side_b)
        .is_some_and(|side| side.bboxes.iter().any(|bbox| bbox.id == bbox_id_b));
    if !left_exists || !right_exists {
        return Err("The selected box no longer exists.".into());
    }
    tree.confirmed_links.retain(|link| {
        let same_pair = (link.side_a == side_a && link.side_b == side_b)
            || (link.side_a == side_b && link.side_b == side_a);
        let uses_endpoint = (link.side_a == side_a && link.bbox_id_a == bbox_id_a)
            || (link.side_b == side_a && link.bbox_id_b == bbox_id_a)
            || (link.side_a == side_b && link.bbox_id_a == bbox_id_b)
            || (link.side_b == side_b && link.bbox_id_b == bbox_id_b);
        !(same_pair && uses_endpoint)
    });
    let link_id = format!("lnk-{side_a}-{bbox_id_a}-{side_b}-{bbox_id_b}");
    tree.confirmed_links.push(ConfirmedLinkData {
        link_id,
        side_a,
        bbox_id_a,
        side_b,
        bbox_id_b,
    });
    Ok(())
}

fn handle_points(bbox: &BoxData) -> [(ResizeHandle, f64, f64); 8] {
    let middle_x = (bbox.x1 + bbox.x2) / 2.0;
    let middle_y = (bbox.y1 + bbox.y2) / 2.0;
    [
        (ResizeHandle::NorthWest, bbox.x1, bbox.y1),
        (ResizeHandle::North, middle_x, bbox.y1),
        (ResizeHandle::NorthEast, bbox.x2, bbox.y1),
        (ResizeHandle::East, bbox.x2, middle_y),
        (ResizeHandle::SouthEast, bbox.x2, bbox.y2),
        (ResizeHandle::South, middle_x, bbox.y2),
        (ResizeHandle::SouthWest, bbox.x1, bbox.y2),
        (ResizeHandle::West, bbox.x1, middle_y),
    ]
}

fn hit_resize_handle(bbox: &BoxData, x: f64, y: f64, tolerance: f64) -> Option<ResizeHandle> {
    handle_points(bbox)
        .into_iter()
        .find(|(_, hx, hy)| (x - hx).hypot(y - hy) <= tolerance)
        .map(|(handle, _, _)| handle)
}

fn hit_bbox(boxes: &[BoxData], x: f64, y: f64) -> Option<String> {
    boxes
        .iter()
        .rev()
        .find(|bbox| x >= bbox.x1 && x <= bbox.x2 && y >= bbox.y1 && y <= bbox.y2)
        .map(|bbox| bbox.id.clone())
}

fn move_bbox(
    bbox: &mut BoxData,
    original: &BoxData,
    delta_x: f64,
    delta_y: f64,
    width: f64,
    height: f64,
) {
    let box_width = original.x2 - original.x1;
    let box_height = original.y2 - original.y1;
    bbox.x1 = (original.x1 + delta_x).clamp(0.0, (width - box_width).max(0.0));
    bbox.y1 = (original.y1 + delta_y).clamp(0.0, (height - box_height).max(0.0));
    bbox.x2 = bbox.x1 + box_width;
    bbox.y2 = bbox.y1 + box_height;
}

fn resize_bbox(
    bbox: &mut BoxData,
    original: &BoxData,
    handle: ResizeHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) {
    const MIN_SIZE: f64 = 4.0;
    let x = x.clamp(0.0, width);
    let y = y.clamp(0.0, height);
    let mut x1 = original.x1;
    let mut y1 = original.y1;
    let mut x2 = original.x2;
    let mut y2 = original.y2;
    if matches!(
        handle,
        ResizeHandle::NorthWest | ResizeHandle::West | ResizeHandle::SouthWest
    ) {
        x1 = x.min(x2 - MIN_SIZE);
    }
    if matches!(
        handle,
        ResizeHandle::NorthEast | ResizeHandle::East | ResizeHandle::SouthEast
    ) {
        x2 = x.max(x1 + MIN_SIZE);
    }
    if matches!(
        handle,
        ResizeHandle::NorthWest | ResizeHandle::North | ResizeHandle::NorthEast
    ) {
        y1 = y.min(y2 - MIN_SIZE);
    }
    if matches!(
        handle,
        ResizeHandle::SouthWest | ResizeHandle::South | ResizeHandle::SouthEast
    ) {
        y2 = y.max(y1 + MIN_SIZE);
    }
    bbox.x1 = x1.clamp(0.0, width);
    bbox.y1 = y1.clamp(0.0, height);
    bbox.x2 = x2.clamp(0.0, width);
    bbox.y2 = y2.clamp(0.0, height);
}

fn clamp_viewport(mut viewport: CanvasViewport, width: f64, height: f64) -> CanvasViewport {
    viewport.zoom = viewport.zoom.clamp(1.0, 6.0);
    let max_x = width * (viewport.zoom - 1.0) / 2.0;
    let max_y = height * (viewport.zoom - 1.0) / 2.0;
    viewport.pan_x = viewport.pan_x.clamp(-max_x, max_x);
    viewport.pan_y = viewport.pan_y.clamp(-max_y, max_y);
    viewport
}

fn pointer_canvas_point(
    event: &PointerEvent,
    width: f64,
    height: f64,
    viewport: CanvasViewport,
) -> Option<CanvasPoint> {
    let raw = event.data().as_web_event();
    let element = raw.current_target()?.dyn_into::<web_sys::Element>().ok()?;
    let rect = element.get_bounding_client_rect();
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let base_x =
        ((f64::from(raw.client_x()) - rect.left()) / rect.width() * width).clamp(0.0, width);
    let base_y =
        ((f64::from(raw.client_y()) - rect.top()) / rect.height() * height).clamp(0.0, height);
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    Some(CanvasPoint {
        base_x,
        base_y,
        image_x: ((base_x - viewport.pan_x - center_x) / viewport.zoom + center_x)
            .clamp(0.0, width),
        image_y: ((base_y - viewport.pan_y - center_y) / viewport.zoom + center_y)
            .clamp(0.0, height),
    })
}

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| js_sys::JSON::stringify(&value).ok()?.as_string())
        .unwrap_or_else(|| "Native command failed.".into())
}

fn to_invoke_args<T>(value: &T) -> Result<JsValue, serde_wasm_bindgen::Error>
where
    T: Serialize + ?Sized,
{
    value.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
}

fn confirm_action(message: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.confirm_with_message(message).ok())
        .unwrap_or(false)
}

async fn load_bootstrap() -> Result<Bootstrap, String> {
    let value = invoke("bootstrap", JsValue::UNDEFINED)
        .await
        .map_err(js_error)?;
    let mut bootstrap: Bootstrap =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    if !bootstrap.settings.export_uri.is_empty()
        && matches!(
            validate_saf_folder(&bootstrap.settings.export_uri).await,
            Ok(false)
        )
    {
        bootstrap.settings = save_app_settings(AppSettings::default()).await?;
    }
    Ok(bootstrap)
}

async fn save_app_settings(settings: AppSettings) -> Result<AppSettings, String> {
    let args = to_invoke_args(&SettingsArgs { settings }).map_err(|error| error.to_string())?;
    let value = invoke("settings_save", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn save_session(session: Session) -> Result<Session, String> {
    let args = to_invoke_args(&SessionArgs { session }).map_err(|error| error.to_string())?;
    let value = invoke("session_save", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn delete_session(session_id: String) -> Result<Vec<Session>, String> {
    let args = to_invoke_args(&SessionIdArgs { session_id }).map_err(|error| error.to_string())?;
    let value = invoke("session_delete", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn export_session(session_id: String) -> Result<ExportData, String> {
    let args = to_invoke_args(&SessionIdArgs { session_id }).map_err(|error| error.to_string())?;
    let value = invoke("session_export", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn list_sessions() -> Result<Vec<Session>, String> {
    let value = invoke("session_list", JsValue::UNDEFINED)
        .await
        .map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn delete_tree(tree_id: String) -> Result<(), String> {
    let args = to_invoke_args(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    invoke("tree_delete", args).await.map_err(js_error)?;
    Ok(())
}

async fn load_tree(tree_id: String) -> Result<TreeData, String> {
    let args = to_invoke_args(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    let value = invoke("tree_load", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn save_tree(tree: TreeData) -> Result<TreeData, String> {
    let args = to_invoke_args(&TreeSaveArgs { tree }).map_err(|error| error.to_string())?;
    let value = invoke("tree_save", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn mirror_sessions_index(data_root: &str, export_uri: &str) -> Result<(), String> {
    if export_uri.trim().is_empty() {
        return Ok(());
    }
    copy_to_saf(
        export_uri,
        "sessions.json",
        &local_path(data_root, "sessions.json"),
        "application/json",
    )
    .await
}

async fn mirror_tree_state(
    tree: &TreeData,
    data_root: &str,
    export_uri: &str,
) -> Result<(), String> {
    if export_uri.trim().is_empty() {
        return Ok(());
    }
    let mut files = vec![
        ("sessions.json".to_string(), "application/json"),
        (format!("trees/{}.json", tree.id), "application/json"),
        (
            format!("Output JSON/{}.json", tree.tree_name),
            "application/json",
        ),
    ];
    for side in &tree.sides {
        files.push((
            format!(
                "Output TXT/{}/{}_{}.txt",
                tree.split,
                tree.tree_name,
                side.side_index + 1
            ),
            "text/plain",
        ));
        files.push((
            format!(
                "dataset/annotlog/{}/{}_{}.json",
                tree.split,
                tree.tree_name,
                side.side_index + 1
            ),
            "application/json",
        ));
    }
    for (relative_path, mime_type) in files {
        copy_to_saf(
            export_uri,
            &relative_path,
            &local_path(data_root, &relative_path),
            mime_type,
        )
        .await?;
    }
    Ok(())
}

async fn save_tree_portable(
    tree: TreeData,
    data_root: &str,
    export_uri: &str,
) -> Result<(TreeData, Option<String>), String> {
    let saved = save_tree(tree).await?;
    let warning = mirror_tree_state(&saved, data_root, export_uri).await.err();
    Ok((saved, warning))
}

async fn run_detector(image_path: String) -> Result<Vec<BoxData>, String> {
    let args = to_invoke_args(&DetectorArgs { image_path }).map_err(|error| error.to_string())?;
    let value = invoke("detector_run", args).await.map_err(js_error)?;
    let response: DetectorData =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    Ok(response.boxes)
}

async fn compute_tree(tree_id: String) -> Result<ComputeData, String> {
    let args = to_invoke_args(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    let value = invoke("tree_compute", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn export_tree(tree_id: String, export_kind: &str) -> Result<ExportData, String> {
    let args = to_invoke_args(&TreeExportArgs {
        tree_id,
        export_kind: export_kind.into(),
    })
    .map_err(|error| error.to_string())?;
    let value = invoke("tree_export", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn suggest_tree_links(tree_id: String) -> Result<Vec<LinkSuggestionData>, String> {
    let args = to_invoke_args(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    let value = invoke("tree_suggest", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn render_depth(tree_id: String, side_index: usize) -> Result<DepthRenderData, String> {
    let args = to_invoke_args(&DepthRenderArgs {
        tree_id,
        side_index,
    })
    .map_err(|error| error.to_string())?;
    let value = invoke("depth_render", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn copy_to_saf(
    tree_uri: &str,
    relative_path: &str,
    source_path: &str,
    mime_type: &str,
) -> Result<(), String> {
    // Plugin commands take a single `payload` parameter, so the invoke args must
    // be wrapped under "payload" (otherwise Tauri reports "missing required key
    // payload" and the SAF mirror silently fails).
    let relative_path = format!("PalmAnnotate/{}", relative_path.trim_start_matches('/'));
    let args = to_invoke_args(&serde_json::json!({
        "payload": SafCopyArgs {
            tree_uri: tree_uri.into(),
            relative_path,
            source_path: source_path.into(),
            mime_type: mime_type.into(),
        }
    }))
    .map_err(|error| error.to_string())?;
    invoke("plugin:palm-native|saf_copy_from_path", args)
        .await
        .map_err(js_error)?;
    Ok(())
}

async fn delete_from_saf(tree_uri: &str, relative_path: &str) -> Result<(), String> {
    let relative_path = format!("PalmAnnotate/{}", relative_path.trim_start_matches('/'));
    let args = to_invoke_args(&serde_json::json!({
        "payload": { "treeUri": tree_uri, "relativePath": relative_path }
    }))
    .map_err(|error| error.to_string())?;
    invoke("plugin:palm-native|saf_delete", args)
        .await
        .map_err(js_error)?;
    Ok(())
}

async fn validate_saf_folder(tree_uri: &str) -> Result<bool, String> {
    let args = to_invoke_args(&serde_json::json!({
        "payload": { "treeUri": tree_uri }
    }))
    .map_err(|error| error.to_string())?;
    let value = invoke("plugin:palm-native|saf_validate", args)
        .await
        .map_err(js_error)?;
    let validation: SafValidation =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    Ok(validation.valid)
}

async fn release_saf_folder(tree_uri: &str) -> Result<(), String> {
    let args = to_invoke_args(&serde_json::json!({
        "payload": { "treeUri": tree_uri }
    }))
    .map_err(|error| error.to_string())?;
    invoke("plugin:palm-native|saf_release_folder", args)
        .await
        .map_err(js_error)?;
    Ok(())
}

fn local_path(root: &str, relative: &str) -> String {
    format!(
        "{}/{}",
        root.trim_end_matches(['/', '\\']),
        relative.replace('\\', "/")
    )
}

fn tree_artifact_paths(tree: &TreeSummary) -> Vec<String> {
    let mut paths = vec![
        format!("trees/{}.json", tree.id),
        format!("Output JSON/{}.json", tree.tree_name),
        format!("dataset/metadata/{}.json", tree.tree_name),
        format!("exports/{}_result.csv", tree.tree_name),
        format!("exports/{}_session.json", tree.tree_name),
        format!("exports/{}_identity.json", tree.tree_name),
    ];
    for side in 1..=tree.side_count.max(1) {
        paths.push(format!("Output TXT/{}_{}.txt", tree.tree_name, side));
        paths.push(format!("exports/{}_{}.txt", tree.tree_name, side));
        paths.push(format!("exports/{}_{}_mismatch.txt", tree.tree_name, side));
        for split in ["field", "train", "val", "test"] {
            for extension in ["jpg", "jpeg", "png"] {
                paths.push(format!(
                    "dataset/images/{split}/{}_{}.{}",
                    tree.tree_name, side, extension
                ));
            }
            for extension in ["raw", "bin", "png", "json"] {
                paths.push(format!(
                    "dataset/depth/{split}/{}_{}.{}",
                    tree.tree_name, side, extension
                ));
            }
            paths.push(format!(
                "dataset/labels/{split}/{}_{}.txt",
                tree.tree_name, side
            ));
            paths.push(format!(
                "dataset/annotlog/{split}/{}_{}.json",
                tree.tree_name, side
            ));
            paths.push(format!(
                "Output TXT/{split}/{}_{}.txt",
                tree.tree_name, side
            ));
        }
    }
    paths
}

async fn delete_temporary_frames(frames: Vec<CapturedFrame>) {
    for frame in frames {
        for path in [
            Some(frame.path),
            frame.depth_path,
            frame.depth_metadata_path,
        ]
        .into_iter()
        .flatten()
        {
            if let Ok(args) = to_invoke_args(&serde_json::json!({ "payload": { "path": path } })) {
                let _ = invoke("plugin:palm-native|temp_delete", args).await;
            }
        }
    }
}

async fn optional_gps() -> Option<GpsData> {
    let permission_args = to_invoke_args(&serde_json::json!({
        "permissions": ["location"]
    }))
    .ok()?;
    let permission_value = invoke("plugin:geolocation|request_permissions", permission_args)
        .await
        .ok()?;
    let permission: GeoPermission = serde_wasm_bindgen::from_value(permission_value).ok()?;
    if permission.location != "granted" {
        return None;
    }
    let position_args = to_invoke_args(&serde_json::json!({
        "options": {
            "enableHighAccuracy": true,
            "timeout": 15000,
            "maximumAge": 0
        }
    }))
    .ok()?;
    let position_value = invoke("plugin:geolocation|get_current_position", position_args)
        .await
        .ok()?;
    let position: GeoPosition = serde_wasm_bindgen::from_value(position_value).ok()?;
    Some(GpsData {
        latitude: position.coords.latitude,
        longitude: position.coords.longitude,
        accuracy: Some(position.coords.accuracy),
    })
}

async fn pick_saf_folder() -> Result<Option<SafFolder>, String> {
    let value = invoke(
        "plugin:palm-native|saf_pick_folder",
        to_invoke_args(&serde_json::json!({})).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(js_error)?;
    let folder: SafFolder =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    if folder.cancelled {
        Ok(None)
    } else if folder.uri.is_empty() {
        Err("Android did not return a writable SAF folder.".into())
    } else {
        Ok(Some(folder))
    }
}

async fn import_saf_folder() -> Result<Option<(Vec<Session>, AppSettings)>, String> {
    let Some(folder) = pick_saf_folder().await? else {
        return Ok(None);
    };
    let settings = save_app_settings(AppSettings {
        export_uri: folder.uri.clone(),
        export_name: folder.name,
        ..AppSettings::default()
    })
    .await?;
    let tree_args = to_invoke_args(&serde_json::json!({
        "payload": { "treeUri": folder.uri.clone() }
    }))
    .map_err(|error| error.to_string())?;
    let value = invoke("plugin:palm-native|saf_copy_tree_to_temp", tree_args)
        .await
        .map_err(js_error)?;
    let staged: NativePath =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    let args = to_invoke_args(&ImportFolderArgs {
        folder_path: staged.path,
        export_uri: folder.uri,
    })
    .map_err(|error| error.to_string())?;
    let value = invoke("sessions_import_folder", args)
        .await
        .map_err(js_error)?;
    let sessions = serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    Ok(Some((sessions, settings)))
}

async fn import_json_file() -> Result<Option<JsonImportData>, String> {
    let value = invoke(
        "plugin:palm-native|saf_pick_json",
        to_invoke_args(&serde_json::json!({})).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(js_error)?;
    let picked: PickedFile =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    if picked.cancelled {
        return Ok(None);
    }
    if picked.path.is_empty() {
        return Err("Android did not return the selected JSON file.".into());
    }
    let args = to_invoke_args(&JsonImportArgs {
        request: JsonImportRequest {
            file_path: picked.path.clone(),
        },
    })
    .map_err(|error| error.to_string())?;
    let result = invoke("tree_import_json", args).await.map_err(js_error);
    if let Ok(temp_args) =
        to_invoke_args(&serde_json::json!({ "payload": { "path": picked.path } }))
    {
        let _ = invoke("plugin:palm-native|temp_delete", temp_args).await;
    }
    let value = result?;
    serde_wasm_bindgen::from_value(value)
        .map(Some)
        .map_err(|error| error.to_string())
}

async fn native_empty<T>(command: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let args = to_invoke_args(&serde_json::json!({})).map_err(|error| error.to_string())?;
    let value = invoke(command, args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

fn normalized_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn normalized_block(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

async fn commit_capture(
    pending: PendingCapture,
    data_root: String,
) -> Result<CommitOutcome, String> {
    let number = pending.tree_number.max(1);
    let tree_name = format!(
        "{}_{}_{number:04}",
        normalized_segment(&pending.session.variety),
        normalized_block(&pending.session.block)
    );
    let tree_id = format!("{}-tree-{number}", pending.session.id);
    let timestamp: String = js_sys::Date::new_0().to_iso_string().into();
    let sides = pending
        .frames
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            let depth_path = frame
                .depth_path
                .as_ref()
                .map(|_| format!("depth/field/{tree_name}_{}.raw", index + 1));
            let depth = match (
                frame.depth_width,
                frame.depth_height,
                frame.depth_format.as_ref(),
                frame.depth_value_scale,
            ) {
                (Some(width), Some(height), Some(format), Some(value_scale)) => {
                    Some(serde_json::json!({
                        "width": width,
                        "height": height,
                        "format": format,
                        "valueScale": value_scale
                    }))
                }
                _ => None,
            };
            serde_json::json!({
                "sideIndex": index,
                "label": format!("Side {}", index + 1),
                "imagePath": format!("images/field/{tree_name}_{}.jpg", index + 1),
                "imageWidth": frame.width,
                "imageHeight": frame.height,
                "depthPath": depth_path,
                "depth": depth,
                "bboxes": []
            })
        })
        .collect::<Vec<_>>();
    let temporary_files = pending
        .frames
        .iter()
        .enumerate()
        .flat_map(|(index, frame)| {
            let mut files = vec![serde_json::json!({
                "sourcePath": frame.path,
                "relativePath": format!("images/field/{tree_name}_{}.jpg", index + 1)
            })];
            if let Some(path) = &frame.depth_path {
                files.push(serde_json::json!({
                    "sourcePath": path,
                    "relativePath": format!("depth/field/{tree_name}_{}.raw", index + 1)
                }));
            }
            if let Some(path) = &frame.depth_metadata_path {
                files.push(serde_json::json!({
                    "sourcePath": path,
                    "relativePath": format!("depth/field/{tree_name}_{}.raw.json", index + 1)
                }));
            }
            files
        })
        .collect::<Vec<_>>();
    let request = serde_json::json!({
        "tree": {
            "version": 4,
            "id": tree_id,
            "sessionId": pending.session.id,
            "treeName": tree_name,
            "split": "field",
            "sideCount": pending.frames.len(),
            "metadata": {
                "variety": pending.session.variety,
                "block": pending.session.block,
                "operator": pending.session.operator,
                "timestamp": timestamp,
                "gps": pending.gps
            },
            "sides": sides,
            "_confirmedLinks": [],
            "status": "captured"
        },
        "temporaryFiles": temporary_files
    });
    let args = to_invoke_args(&CaptureCommitArgs { request }).map_err(|error| error.to_string())?;
    let value = invoke("capture_commit", args).await.map_err(js_error)?;
    let tree: TreeData =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    let mut mirror_warning = None;
    let mut files = vec![
        ("sessions.json".to_string(), "application/json".to_string()),
        (
            format!("trees/{}.json", tree.id),
            "application/json".to_string(),
        ),
        (
            format!("Output JSON/{}.json", tree.tree_name),
            "application/json".to_string(),
        ),
    ];
    for side in &tree.sides {
        files.push((format!("dataset/{}", side.image_path), "image/jpeg".into()));
        if let Some(depth_path) = &side.depth_path {
            files.push((
                format!("dataset/{depth_path}"),
                "application/octet-stream".into(),
            ));
            files.push((
                format!("dataset/{depth_path}.json"),
                "application/json".into(),
            ));
        }
        files.push((
            format!(
                "Output TXT/{}/{}_{}.txt",
                tree.split,
                tree.tree_name,
                side.side_index + 1
            ),
            "text/plain".into(),
        ));
    }
    for (relative, mime) in files {
        if let Err(message) = copy_to_saf(
            &pending.session.export_uri,
            &relative,
            &local_path(&data_root, &relative),
            &mime,
        )
        .await
        {
            mirror_warning = Some(format!(
                "Saved locally, but SAF mirror is incomplete: {message}"
            ));
            break;
        }
    }
    Ok(CommitOutcome {
        tree_id: tree.id,
        mirror_warning,
    })
}

pub fn App() -> Element {
    let mut page = use_signal(|| Page::Home);
    let mut bootstrap = use_signal(|| None::<Bootstrap>);
    let mut error = use_signal(|| None::<String>);
    let mut notice = use_signal(|| None::<String>);
    let mut loading = use_signal(|| true);
    let mut selected_session = use_signal(|| None::<Session>);
    let mut selected_tree_id = use_signal(|| None::<String>);
    let mut pending_capture = use_signal(|| None::<PendingCapture>);
    let mut retake_side = use_signal(|| None::<usize>);

    use_effect(move || {
        spawn(async move {
            match load_bootstrap().await {
                Ok(value) => bootstrap.set(Some(value)),
                Err(message) => error.set(Some(message)),
            }
            loading.set(false);
        });
    });

    // Android hardware/gesture back: the native MainActivity calls window.__paBack()
    // and only finishes the activity when this returns "exit" (i.e. already Home).
    // Without it the default WebView behaviour closes the whole app on every back.
    let mut back_handler = use_signal(|| None::<Closure<dyn FnMut() -> JsValue>>);
    use_effect(move || {
        let closure = Closure::<dyn FnMut() -> JsValue>::new(move || -> JsValue {
            let current = *page.read();
            match current {
                Page::Home => return JsValue::from_str("exit"),
                Page::NewSession | Page::Settings => {
                    page.set(Page::Home);
                }
                Page::SessionDetail => {
                    selected_session.set(None);
                    selected_tree_id.set(None);
                    page.set(Page::Home);
                }
                Page::Capture | Page::Review => {
                    pending_capture.set(None);
                    retake_side.set(None);
                    if selected_session.read().is_some() {
                        page.set(Page::SessionDetail);
                    } else {
                        page.set(Page::Home);
                    }
                }
                Page::Annotate | Page::Dedup | Page::Results | Page::DepthViewer => {
                    selected_tree_id.set(None);
                    page.set(Page::SessionDetail);
                }
            }
            JsValue::from_str("back")
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                window.as_ref(),
                &JsValue::from_str("__paBack"),
                closure.as_ref(),
            );
        }
        back_handler.set(Some(closure));
    });

    rsx! {
        document::Stylesheet { href: STYLES }
        main { class: "app",
            if matches!(*page.read(), Page::Annotate | Page::Dedup | Page::Results | Page::DepthViewer) {
                EditorNav {
                    page,
                    tree_name: selected_session.read().as_ref()
                        .and_then(|session| selected_tree_id.read().as_ref()
                            .and_then(|id| session.trees.iter().find(|tree| &tree.id == id)))
                        .map(|tree| tree.tree_name.clone())
                        .unwrap_or_default(),
                    on_home: move |_| {
                        selected_tree_id.set(None);
                        page.set(Page::SessionDetail);
                    },
                    on_navigate: move |next| page.set(next)
                }
            }
            section { class: "page-stage",
                if let Some(message) = notice.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                if *loading.read() {
                    LoadingState {}
                } else if let Some(message) = error.read().as_ref() {
                    ErrorState { message: message.clone() }
                } else {
                    match *page.read() {
                        Page::Home => rsx! {
                            Home {
                                sessions: bootstrap.read().as_ref().map(|b| b.sessions.clone()).unwrap_or_default(),
                                settings: bootstrap.read().as_ref().map(|b| b.settings.clone()).unwrap_or_default(),
                                on_new: move |_| {
                                    let has_folder = bootstrap.read().as_ref()
                                        .is_some_and(|value| !value.settings.export_uri.is_empty());
                                    if has_folder {
                                        page.set(Page::NewSession);
                                    } else {
                                        loading.set(true);
                                        notice.set(None);
                                        let data_root = bootstrap.read().as_ref()
                                            .map(|value| value.data_root.clone())
                                            .unwrap_or_default();
                                        spawn(async move {
                                            match pick_saf_folder().await {
                                                Ok(Some(folder)) => {
                                                    match save_app_settings(AppSettings {
                                                        export_uri: folder.uri,
                                                        export_name: folder.name,
                                                        ..AppSettings::default()
                                                    }).await {
                                                        Ok(settings) => {
                                                            if let Err(message) = mirror_sessions_index(
                                                                &data_root,
                                                                &settings.export_uri,
                                                            ).await {
                                                                notice.set(Some(format!("Folder selected: {message}")));
                                                            }
                                                            if let Some(value) = bootstrap.write().as_mut() {
                                                                value.settings = settings;
                                                            }
                                                            page.set(Page::NewSession);
                                                        }
                                                        Err(message) => notice.set(Some(message)),
                                                    }
                                                }
                                                Ok(None) => {}
                                                Err(message) => notice.set(Some(message)),
                                            }
                                            loading.set(false);
                                        });
                                    }
                                },
                                on_choose_folder: move |_| {
                                    loading.set(true);
                                    notice.set(None);
                                    let data_root = bootstrap.read().as_ref()
                                        .map(|value| value.data_root.clone())
                                        .unwrap_or_default();
                                    spawn(async move {
                                        match pick_saf_folder().await {
                                            Ok(Some(folder)) => {
                                                match save_app_settings(AppSettings {
                                                    export_uri: folder.uri,
                                                    export_name: folder.name,
                                                    ..AppSettings::default()
                                                }).await {
                                                    Ok(settings) => {
                                                        if let Err(message) = mirror_sessions_index(
                                                            &data_root,
                                                            &settings.export_uri,
                                                        ).await {
                                                            notice.set(Some(format!("Folder selected: {message}")));
                                                        }
                                                        let sessions = list_sessions().await.ok();
                                                        if let Some(value) = bootstrap.write().as_mut() {
                                                            value.settings = settings;
                                                            if let Some(sessions) = sessions {
                                                                value.sessions = sessions;
                                                            }
                                                        }
                                                    }
                                                    Err(message) => notice.set(Some(message)),
                                                }
                                            }
                                            Ok(None) => {}
                                            Err(message) => notice.set(Some(message)),
                                        }
                                        loading.set(false);
                                    });
                                },
                                on_import: move |_| {
                                    loading.set(true);
                                    notice.set(None);
                                    let data_root = bootstrap.read().as_ref()
                                        .map(|value| value.data_root.clone())
                                        .unwrap_or_default();
                                    spawn(async move {
                                        match import_saf_folder().await {
                                            Ok(Some((sessions, settings))) => {
                                                if let Err(message) = mirror_sessions_index(
                                                    &data_root,
                                                    &settings.export_uri,
                                                ).await {
                                                    notice.set(Some(format!("Imported locally: {message}")));
                                                }
                                                if let Some(value) = bootstrap.write().as_mut() {
                                                    value.sessions = sessions;
                                                    value.settings = settings;
                                                }
                                            }
                                            Ok(None) => {}
                                            Err(message) => notice.set(Some(message)),
                                        }
                                        loading.set(false);
                                    });
                                },
                                on_import_json: move |_| {
                                    loading.set(true);
                                    notice.set(None);
                                    spawn(async move {
                                        match import_json_file().await {
                                            Ok(Some(imported)) => {
                                                match list_sessions().await {
                                                    Ok(sessions) => {
                                                        selected_session.set(
                                                            sessions.iter()
                                                                .find(|session| session.id == imported.session_id)
                                                                .cloned()
                                                        );
                                                        selected_tree_id.set(Some(imported.tree_id));
                                                        if let Some(value) = bootstrap.write().as_mut() {
                                                            value.sessions = sessions;
                                                        }
                                                        page.set(Page::Annotate);
                                                    }
                                                    Err(message) => notice.set(Some(message)),
                                                }
                                            }
                                            Ok(None) => {}
                                            Err(message) => notice.set(Some(message)),
                                        }
                                        loading.set(false);
                                    });
                                },
                                on_settings: move |_| page.set(Page::Settings),
                                on_open: move |session: Session| {
                                    selected_session.set(Some(session));
                                    selected_tree_id.set(None);
                                    page.set(Page::SessionDetail);
                                }
                            }
                        },
                        Page::NewSession => rsx! {
                            NewSession {
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                export_uri: bootstrap.read().as_ref().map(|b| b.settings.export_uri.clone()).unwrap_or_default(),
                                recent_varieties: bootstrap.read().as_ref().map(|b| b.settings.recent_varieties.clone()).unwrap_or_default(),
                                recent_blocks: bootstrap.read().as_ref().map(|b| b.settings.recent_blocks.clone()).unwrap_or_default(),
                                on_cancel: move |_| page.set(Page::Home),
                                on_warning: move |message: String| notice.set(Some(message)),
                                on_saved: move |session: Session| {
                                    if let Some(value) = bootstrap.write().as_mut() {
                                        value.sessions.push(session.clone());
                                    }
                                    selected_session.set(Some(session));
                                    page.set(Page::SessionDetail);
                                }
                            }
                        },
                        Page::SessionDetail => rsx! {
                            SessionDetail {
                                session: selected_session.read().clone(),
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                export_uri: bootstrap.read().as_ref().map(|b| b.settings.export_uri.clone()).unwrap_or_default(),
                                on_home: move |_| {
                                    selected_session.set(None);
                                    selected_tree_id.set(None);
                                    page.set(Page::Home);
                                },
                                on_capture: move |_| {
                                    pending_capture.set(None);
                                    retake_side.set(None);
                                    page.set(Page::Capture);
                                },
                                on_open_tree: move |tree_id: String| {
                                    selected_tree_id.set(Some(tree_id));
                                    page.set(Page::Annotate);
                                },
                                on_warning: move |message: String| notice.set(Some(message)),
                                on_sessions_updated: move |sessions: Vec<Session>| {
                                    let selected_id = selected_session.read().as_ref().map(|s| s.id.clone());
                                    selected_session.set(selected_id.and_then(|id| sessions.iter().find(|s| s.id == id).cloned()));
                                    if let Some(value) = bootstrap.write().as_mut() {
                                        value.sessions = sessions;
                                    }
                                },
                                on_deleted: move |sessions: Vec<Session>| {
                                    if let Some(value) = bootstrap.write().as_mut() {
                                        value.sessions = sessions;
                                    }
                                    selected_session.set(None);
                                    selected_tree_id.set(None);
                                    page.set(Page::Home);
                                }
                            }
                        },
                        Page::Capture => rsx! {
                            Capture {
                                session: selected_session.read().clone(),
                                existing: pending_capture.read().clone(),
                                retake_side: *retake_side.read(),
                                on_cancel: move |_| {
                                    if pending_capture.read().is_some() && retake_side.read().is_some() {
                                        retake_side.set(None);
                                        page.set(Page::Review);
                                    } else {
                                        pending_capture.set(None);
                                        retake_side.set(None);
                                        page.set(Page::SessionDetail);
                                    }
                                },
                                on_complete: move |capture: PendingCapture| {
                                    pending_capture.set(Some(capture));
                                    retake_side.set(None);
                                    page.set(Page::Review);
                                }
                            }
                        },
                        Page::Review => rsx! {
                            Review {
                                capture: pending_capture.read().clone(),
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                on_retake: move |side_index: usize| {
                                    retake_side.set(Some(side_index));
                                    page.set(Page::Capture);
                                },
                                on_retake_all: move |_| {
                                    pending_capture.set(None);
                                    retake_side.set(None);
                                    page.set(Page::Capture);
                                },
                                on_cancel: move |_| {
                                    pending_capture.set(None);
                                    retake_side.set(None);
                                    page.set(Page::SessionDetail);
                                },
                                on_committed: move |outcome: CommitOutcome| {
                                    selected_tree_id.set(Some(outcome.tree_id));
                                    notice.set(outcome.mirror_warning);
                                    pending_capture.set(None);
                                    spawn(async move {
                                        if let Ok(sessions) = list_sessions().await {
                                            let selected_id = selected_session.read().as_ref().map(|s| s.id.clone());
                                            selected_session.set(selected_id.and_then(|id| sessions.iter().find(|s| s.id == id).cloned()));
                                            if let Some(value) = bootstrap.write().as_mut() {
                                                value.sessions = sessions;
                                            }
                                        }
                                    });
                                    page.set(Page::Annotate);
                                }
                            }
                        },
                        Page::Annotate => rsx! {
                            Annotate {
                                tree_id: selected_tree_id.read().clone(),
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                export_uri: bootstrap.read().as_ref().map(|b| b.settings.export_uri.clone()).unwrap_or_default(),
                                on_next: move |_| page.set(Page::Dedup),
                                on_exit: move |_| {
                                    spawn(async move {
                                        if let Ok(sessions) = list_sessions().await {
                                            let selected_id = selected_session.read().as_ref().map(|s| s.id.clone());
                                            selected_session.set(selected_id.and_then(|id| sessions.iter().find(|s| s.id == id).cloned()));
                                            if let Some(value) = bootstrap.write().as_mut() {
                                                value.sessions = sessions;
                                            }
                                        }
                                    });
                                    page.set(Page::SessionDetail);
                                },
                                on_next_tree: move |_| {
                                    spawn(async move {
                                        if let Ok(sessions) = list_sessions().await {
                                            let selected_id = selected_session.read().as_ref().map(|s| s.id.clone());
                                            selected_session.set(selected_id.and_then(|id| sessions.iter().find(|s| s.id == id).cloned()));
                                            if let Some(value) = bootstrap.write().as_mut() {
                                                value.sessions = sessions;
                                            }
                                        }
                                    });
                                    selected_tree_id.set(None);
                                    page.set(Page::Capture);
                                }
                            }
                        },
                        Page::Dedup => rsx! {
                            Dedup {
                                tree_id: selected_tree_id.read().clone(),
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                export_uri: bootstrap.read().as_ref().map(|b| b.settings.export_uri.clone()).unwrap_or_default(),
                                on_results: move |_| page.set(Page::Results)
                            }
                        },
                        Page::Results => rsx! {
                            Results {
                                tree_id: selected_tree_id.read().clone(),
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                export_uri: bootstrap.read().as_ref().map(|b| b.settings.export_uri.clone()).unwrap_or_default()
                            }
                        },
                        Page::DepthViewer => rsx! {
                            DepthViewer { tree_id: selected_tree_id.read().clone() }
                        },
                        Page::Settings => rsx! {
                            Settings {
                                settings: bootstrap.read().as_ref().map(|b| b.settings.clone()).unwrap_or_default(),
                                data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                on_back: move |_| page.set(Page::Home),
                                on_saved: move |settings: AppSettings| {
                                    if let Some(value) = bootstrap.write().as_mut() {
                                        value.settings = settings;
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn EditorNav(
    page: Signal<Page>,
    tree_name: String,
    on_home: EventHandler<MouseEvent>,
    on_navigate: EventHandler<Page>,
) -> Element {
    let tools = [
        (Page::Annotate, "Annotate"),
        (Page::Dedup, "Dedup"),
        (Page::Results, "Results"),
        (Page::DepthViewer, "Depth"),
    ];
    rsx! {
        header { class: "editor-nav",
            button { class: "icon-button", onclick: on_home, aria_label: "Back to session",
                Icon { name: "back" }
            }
            strong { class: "editor-tree", "{tree_name}" }
            nav { class: "editor-tools", aria_label: "Tree tools",
                for (target, label) in tools {
                    button {
                        class: if *page.read() == target { "tool-tab active" } else { "tool-tab" },
                        onclick: move |_| on_navigate.call(target),
                        "{label}"
                    }
                }
            }
        }
    }
}

#[component]
fn Home(
    sessions: Vec<Session>,
    settings: AppSettings,
    on_new: EventHandler<MouseEvent>,
    on_choose_folder: EventHandler<MouseEvent>,
    on_import: EventHandler<MouseEvent>,
    on_import_json: EventHandler<MouseEvent>,
    on_settings: EventHandler<MouseEvent>,
    on_open: EventHandler<Session>,
) -> Element {
    let total_trees: usize = sessions.iter().map(|session| session.trees.len()).sum();
    let mut groups: Vec<String> = sessions
        .iter()
        .map(|session| {
            if session.group_key.is_empty() {
                format!(
                    "{}__{}",
                    normalized_segment(&session.variety),
                    normalized_block(&session.block)
                )
            } else {
                session.group_key.clone()
            }
        })
        .collect();
    groups.sort_unstable();
    groups.dedup();
    let total_groups = groups.len();
    rsx! {
        div { class: "home",
            header { class: "home-head",
                h1 { "PalmAnnotate" }
                button { class: "icon-button", onclick: on_settings, aria_label: "Settings",
                    Icon { name: "settings" }
                }
            }
            div { class: "stat-cards",
                div { class: "stat-card",
                    strong { class: "stat-trees", "{total_trees}" }
                    span { "TREES" }
                }
                div { class: "stat-card",
                    strong { class: "stat-groups", "{total_groups}" }
                    span { "GROUPS" }
                }
            }
            button { class: "button primary block", onclick: on_new,
                Icon { name: "plus" } "New Session"
            }
            div { class: "folder-row",
                div {
                    Icon { name: "folder" }
                    strong {
                        if settings.export_name.is_empty() { "Choose folder" } else { "{settings.export_name}" }
                    }
                }
                button { class: "button secondary compact", onclick: on_choose_folder, "Change" }
            }
            p { class: "section-label", "SESSIONS" }
            if sessions.is_empty() {
                div { class: "empty-simple", "No sessions" }
            } else {
                div { class: "session-list",
                    for session in sessions {
                        article {
                            class: "session-row",
                            onclick: {
                                let session = session.clone();
                                move |_| on_open.call(session.clone())
                            },
                            div { class: "session-main",
                                strong { "{session.variety} / {session.block}" }
                                span { "{session.trees.len()} trees" }
                            }
                            Icon { name: "arrow" }
                        }
                    }
                }
            }
            div { class: "home-foot",
                button { class: "button secondary", onclick: on_import,
                    Icon { name: "folder" } "Load Folder"
                }
                button { class: "button secondary", onclick: on_import_json, "Load JSON" }
            }
        }
    }
}

#[component]
fn NewSession(
    data_root: String,
    export_uri: String,
    recent_varieties: Vec<String>,
    recent_blocks: Vec<String>,
    on_cancel: EventHandler<MouseEvent>,
    on_warning: EventHandler<String>,
    on_saved: EventHandler<Session>,
) -> Element {
    let mut variety = use_signal(String::new);
    let mut block = use_signal(String::new);
    let mut operator = use_signal(String::new);
    let mut side_count = use_signal(|| 4_usize);
    let mut auto_id = use_signal(|| true);
    let mut form_error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if variety.read().trim().is_empty() || block.read().trim().is_empty() {
            form_error.set(Some("Variety and block are required.".into()));
            return;
        }
        if export_uri.trim().is_empty() {
            form_error.set(Some("Choose an export folder first.".into()));
            return;
        }
        let variety_value = variety.read().trim().to_string();
        let block_value = block.read().trim().to_string();
        let session = Session {
            id: format!("session-{}", js_sys::Date::now() as u64),
            name: format!("{variety_value} / {block_value}"),
            variety: variety_value,
            block: block_value,
            group_key: String::new(),
            side_count: *side_count.read(),
            auto_id: *auto_id.read(),
            next_id: 1,
            operator: operator.read().trim().into(),
            export_uri: export_uri.clone(),
            created_at: js_sys::Date::new_0().to_iso_string().into(),
            updated_at: js_sys::Date::new_0().to_iso_string().into(),
            trees: vec![],
        };
        saving.set(true);
        let data_root = data_root.clone();
        spawn(async move {
            match save_session(session).await {
                Ok(saved) => {
                    if let Err(message) = copy_to_saf(
                        &saved.export_uri,
                        "sessions.json",
                        &local_path(&data_root, "sessions.json"),
                        "application/json",
                    )
                    .await
                    {
                        on_warning.call(format!(
                            "Session saved locally, but SAF mirror failed: {message}"
                        ));
                    }
                    on_saved.call(saved);
                }
                Err(message) => form_error.set(Some(message)),
            }
            saving.set(false);
        });
    };

    rsx! {
        div { class: "form-layout",
            form { class: "form-panel", onsubmit: submit,
                div { class: "form-intro",
                    h2 { "New session" }
                }
                if let Some(message) = form_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                label { class: "field",
                    span { "Variety" }
                    input {
                        placeholder: "DAMIMAS",
                        list: "recent-varieties",
                        value: "{variety}",
                        oninput: move |event| variety.set(event.value())
                    }
                    datalist { id: "recent-varieties",
                        for value in recent_varieties {
                            option { value: "{value}" }
                        }
                    }
                }
                label { class: "field",
                    span { "Block" }
                    input {
                        placeholder: "A21B",
                        list: "recent-blocks",
                        value: "{block}",
                        oninput: move |event| block.set(event.value())
                    }
                    datalist { id: "recent-blocks",
                        for value in recent_blocks {
                            option { value: "{value}" }
                        }
                    }
                }
                label { class: "field",
                    span { "Operator" }
                    input {
                        placeholder: "Operator name",
                        value: "{operator}",
                        oninput: move |event| operator.set(event.value())
                    }
                }
                label { class: "field",
                    span { "Photos per tree" }
                    select {
                        value: "{side_count}",
                        onchange: move |event| {
                            side_count.set(event.value().parse().unwrap_or(4));
                        },
                        option { value: "4", "4 sides" }
                        option { value: "8", "8 sides" }
                    }
                }
                label { class: "field",
                    span { "Tree numbering" }
                    div { class: "input-action",
                        input {
                            r#type: "checkbox",
                            checked: *auto_id.read(),
                            onchange: move |event| auto_id.set(event.checked())
                        }
                        span { "Auto-increment tree ID" }
                    }
                }
                div { class: "form-actions",
                    button { class: "button ghost", r#type: "button", onclick: on_cancel, "Cancel" }
                    button { class: "button primary", r#type: "submit", disabled: *saving.read(),
                        if *saving.read() { "Creating..." } else { "Create session" }
                    }
                }
            }
        }
    }
}

#[component]
fn SessionDetail(
    session: Option<Session>,
    data_root: String,
    export_uri: String,
    on_home: EventHandler<MouseEvent>,
    on_capture: EventHandler<MouseEvent>,
    on_open_tree: EventHandler<String>,
    on_warning: EventHandler<String>,
    on_sessions_updated: EventHandler<Vec<Session>>,
    on_deleted: EventHandler<Vec<Session>>,
) -> Element {
    let Some(session) = session else {
        return rsx! {
            div { class: "error-state",
                strong { "No session selected." }
                p { "Open a field session before capturing a tree." }
            }
        };
    };
    let mut busy = use_signal(|| false);
    let mut detail_error = use_signal(|| None::<String>);
    let session_for_delete = session.clone();
    let session_for_backup = session.clone();
    let data_root_for_delete = data_root.clone();
    let export_uri_for_delete = export_uri.clone();
    let delete = move |_| {
        let target = session_for_delete.clone();
        if !confirm_action(&format!("Delete session {}?", target.name)) {
            return;
        }
        busy.set(true);
        detail_error.set(None);
        let data_root = data_root_for_delete.clone();
        let export_uri = export_uri_for_delete.clone();
        spawn(async move {
            match delete_session(target.id.clone()).await {
                Ok(sessions) => {
                    let mut mirror_failed = None;
                    for tree in &target.trees {
                        for path in tree_artifact_paths(tree) {
                            if let Err(message) = delete_from_saf(&export_uri, &path).await {
                                mirror_failed = Some(message);
                                break;
                            }
                        }
                    }
                    if let Err(message) = copy_to_saf(
                        &export_uri,
                        "sessions.json",
                        &local_path(&data_root, "sessions.json"),
                        "application/json",
                    )
                    .await
                    {
                        mirror_failed = Some(message);
                    }
                    if let Some(message) = mirror_failed {
                        on_warning.call(format!(
                            "Deleted locally, but SAF cleanup was incomplete: {message}"
                        ));
                    }
                    on_deleted.call(sessions);
                }
                Err(message) => detail_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let backup = move |_| {
        let session = session_for_backup.clone();
        busy.set(true);
        detail_error.set(None);
        spawn(async move {
            match export_session(session.id).await {
                Ok(data) => {
                    let mut copied = 0;
                    for file in &data.export_files {
                        match copy_to_saf(
                            &data.export_uri,
                            &file.relative_path,
                            &file.source_path,
                            &file.mime_type,
                        )
                        .await
                        {
                            Ok(()) => copied += 1,
                            Err(message) => {
                                detail_error.set(Some(message));
                                break;
                            }
                        }
                    }
                    if copied > 0 {
                        on_warning.call("Session JSON saved.".into());
                    }
                }
                Err(message) => detail_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    rsx! {
        div { class: "detail-grid",
            section { class: "detail-summary",
                div { class: "screen-title",
                    button { class: "icon-button", onclick: on_home, aria_label: "Back",
                        Icon { name: "back" }
                    }
                    h2 { "{session.name}" }
                }
                p { "{session.side_count} sides / next {session.next_id:04}" }
                if let Some(message) = detail_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                div { class: "form-actions",
                    button { class: "button primary", onclick: on_capture, Icon { name: "camera" } "Add Tree" }
                    button { class: "button secondary", disabled: *busy.read(), onclick: backup, "Session JSON" }
                    button { class: "button ghost danger-text", disabled: *busy.read(), onclick: delete,
                        if *busy.read() { "Deleting..." } else { "Delete Session" }
                    }
                }
            }
            section { class: "tree-list",
                div { class: "list-head", h3 { "Trees" } span { "{session.trees.len()}" } }
                if session.trees.is_empty() {
                    p { class: "empty-simple", "No trees" }
                } else {
                    div { class: "placeholder-rows",
                        for tree in session.trees.clone() {
                            div {
                                class: "tree-row",
                                onclick: {
                                    let tree_id = tree.id.clone();
                                    move |_| on_open_tree.call(tree_id.clone())
                                },
                                div {
                                    strong { "{tree.tree_name}" }
                                    span { "{tree.status}" }
                                }
                                Icon { name: "arrow" }
                                button {
                                    class: "class-button danger",
                                    disabled: *busy.read(),
                                    onclick: {
                                        let tree = tree.clone();
                                        let export_uri = export_uri.clone();
                                        let data_root = data_root.clone();
                                        move |event: MouseEvent| {
                                            event.stop_propagation();
                                            let tree = tree.clone();
                                            if !confirm_action(&format!("Delete tree {}?", tree.tree_name)) {
                                                return;
                                            }
                                            let export_uri = export_uri.clone();
                                            let data_root = data_root.clone();
                                            busy.set(true);
                                            detail_error.set(None);
                                            spawn(async move {
                                            match delete_tree(tree.id.clone()).await {
                                                Ok(()) => {
                                                    for path in tree_artifact_paths(&tree) {
                                                        if let Err(message) = delete_from_saf(&export_uri, &path).await {
                                                            on_warning.call(format!("Tree deleted locally, but SAF cleanup was incomplete: {message}"));
                                                            break;
                                                        }
                                                    }
                                                    match list_sessions().await {
                                                        Ok(sessions) => {
                                                            let _ = copy_to_saf(
                                                                &export_uri,
                                                                "sessions.json",
                                                                &local_path(&data_root, "sessions.json"),
                                                                "application/json",
                                                            ).await;
                                                            on_sessions_updated.call(sessions);
                                                        }
                                                        Err(message) => detail_error.set(Some(message)),
                                                    }
                                                }
                                                Err(message) => detail_error.set(Some(message)),
                                            }
                                            busy.set(false);
                                        });
                                        }
                                    },
                                    "Delete"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn WorkflowRail() -> Element {
    let steps = ["Capture", "Review", "Annotate", "Dedup", "Results"];
    rsx! {
        ol { class: "workflow-rail",
            for (index, step) in steps.iter().enumerate() {
                li { class: if index == 0 { "current" } else { "" },
                    span { "{index + 1}" }
                    strong { "{step}" }
                }
            }
        }
    }
}

#[component]
fn LegacyCapture(
    session: Option<Session>,
    on_cancel: EventHandler<MouseEvent>,
    on_complete: EventHandler<PendingCapture>,
) -> Element {
    let mut frames = use_signal(Vec::<CapturedFrame>::new);
    let mut opened = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut capture_error = use_signal(|| None::<String>);
    let mut use_orbbec = use_signal(|| false);
    let initial_tree_number = session
        .as_ref()
        .map(|value| value.next_id.max(1).to_string())
        .unwrap_or_else(|| "1".into());
    let mut manual_tree_number = use_signal(move || initial_tree_number);
    let mut preview = use_signal(|| None::<String>);
    let mut depth_preview = use_signal(|| None::<String>);
    let mut preview_callback = use_signal(|| None::<Closure<dyn FnMut(JsValue)>>);
    let mut preview_unlisten = use_signal(|| None::<js_sys::Function>);
    let mut orbbec_preview_callback = use_signal(|| None::<Closure<dyn FnMut(JsValue)>>);
    let mut orbbec_preview_unlisten = use_signal(|| None::<js_sys::Function>);
    let expected = session.as_ref().map(|value| value.side_count).unwrap_or(0);
    let has_session = session.is_some();
    let manual_mode = session.as_ref().is_some_and(|value| !value.auto_id);
    let start_session = session.clone();
    let shoot_session = session.clone();

    use_effect(move || {
        let callback = Closure::<dyn FnMut(JsValue)>::new(move |value| {
            if let Ok(event) = serde_wasm_bindgen::from_value::<CameraPreviewEvent>(value) {
                preview.set(Some(format!(
                    "data:image/jpeg;base64,{}",
                    event.payload.jpeg_base64
                )));
            }
        });
        let function = callback
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        spawn(async move {
            match listen("camera-preview", &function).await {
                Ok(value) => {
                    if let Ok(unlisten) = value.dyn_into::<js_sys::Function>() {
                        preview_unlisten.set(Some(unlisten));
                    }
                    preview_callback.set(Some(callback));
                }
                Err(message) => capture_error.set(Some(js_error(message))),
            }
        });
    });
    use_effect(move || {
        let callback = Closure::<dyn FnMut(JsValue)>::new(move |value| {
            if let Ok(event) = serde_wasm_bindgen::from_value::<OrbbecPreviewEvent>(value) {
                if let Some(rgb) = event.payload.rgb_jpeg_base64 {
                    preview.set(Some(format!("data:image/jpeg;base64,{rgb}")));
                }
                if let Some(depth) = event.payload.depth_jpeg_base64 {
                    depth_preview.set(Some(format!("data:image/jpeg;base64,{depth}")));
                }
            }
        });
        let function = callback
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        spawn(async move {
            match listen("orbbec-preview", &function).await {
                Ok(value) => {
                    if let Ok(unlisten) = value.dyn_into::<js_sys::Function>() {
                        orbbec_preview_unlisten.set(Some(unlisten));
                    }
                    orbbec_preview_callback.set(Some(callback));
                }
                Err(message) => capture_error.set(Some(js_error(message))),
            }
        });
    });
    use_drop(move || {
        if let Some(unlisten) = preview_unlisten.read().as_ref() {
            let _ = unlisten.call0(&JsValue::UNDEFINED);
        }
        if let Some(unlisten) = orbbec_preview_unlisten.read().as_ref() {
            let _ = unlisten.call0(&JsValue::UNDEFINED);
        }
    });

    let start = move |_| {
        if start_session.as_ref().is_some_and(|value| !value.auto_id)
            && manual_tree_number
                .read()
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
                .is_none()
        {
            capture_error.set(Some("Enter a positive numeric tree ID.".into()));
            return;
        }
        busy.set(true);
        capture_error.set(None);
        spawn(async move {
            let result = if *use_orbbec.read() {
                async {
                    let status =
                        native_empty::<serde_json::Value>("plugin:palm-native|orbbec_status")
                            .await?;
                    if !status
                        .get("available")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        return Err("No Orbbec USB camera is attached.".into());
                    }
                    let permission = native_empty::<serde_json::Value>(
                        "plugin:palm-native|orbbec_request_permission",
                    )
                    .await?;
                    if !permission
                        .get("granted")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        return Err("Orbbec USB permission was denied.".into());
                    }
                    native_empty::<serde_json::Value>("plugin:palm-native|orbbec_open").await
                }
                .await
            } else {
                native_empty::<serde_json::Value>("plugin:palm-native|camera_start").await
            };
            match result {
                Ok(_) => opened.set(true),
                Err(message) => capture_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let shoot = move |_| {
        let Some(session) = shoot_session.clone() else {
            capture_error.set(Some("Open a session before capture.".into()));
            return;
        };
        busy.set(true);
        capture_error.set(None);
        spawn(async move {
            let command = if *use_orbbec.read() {
                "plugin:palm-native|orbbec_capture"
            } else {
                "plugin:palm-native|camera_capture"
            };
            match native_empty::<CapturedFrame>(command).await {
                Ok(frame) => {
                    frames.write().push(frame);
                    if frames.read().len() == session.side_count {
                        let close_command = if *use_orbbec.read() {
                            "plugin:palm-native|orbbec_close"
                        } else {
                            "plugin:palm-native|camera_stop"
                        };
                        let _ = native_empty::<serde_json::Value>(close_command).await;
                        opened.set(false);
                        let gps = optional_gps().await;
                        let tree_number = if session.auto_id {
                            session.next_id.max(1)
                        } else {
                            manual_tree_number
                                .read()
                                .parse::<usize>()
                                .unwrap_or(1)
                                .max(1)
                        };
                        on_complete.call(PendingCapture {
                            session,
                            tree_number,
                            frames: frames.read().clone(),
                            gps,
                        });
                    }
                }
                Err(message) => capture_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let cancel = move |event: MouseEvent| {
        let temporary = frames.read().clone();
        spawn(async move {
            let _ = native_empty::<serde_json::Value>("plugin:palm-native|camera_stop").await;
            let _ = native_empty::<serde_json::Value>("plugin:palm-native|orbbec_close").await;
            delete_temporary_frames(temporary).await;
            on_cancel.call(event);
        });
    };

    rsx! {
        div { class: "work-layout",
            section { class: "work-copy",
                h2 { "Capture · {frames.read().len()} of {expected}" }
                if let Some(message) = capture_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                label { class: "field",
                    span { "Camera source" }
                    select {
                        disabled: *opened.read(),
                        value: if *use_orbbec.read() { "orbbec" } else { "camerax" },
                        onchange: move |event| use_orbbec.set(event.value() == "orbbec"),
                        option { value: "camerax", "CameraX RGB" }
                        option { value: "orbbec", "Orbbec RGB + depth" }
                    }
                }
                if manual_mode {
                    label { class: "field",
                        span { "Tree ID" }
                        input {
                            class: "capture-idinput",
                            r#type: "number",
                            min: "1",
                            disabled: *opened.read(),
                            value: "{manual_tree_number}",
                            oninput: move |event| manual_tree_number.set(event.value())
                        }
                    }
                }
                div { class: "form-actions",
                    button {
                        class: "button ghost",
                        disabled: *busy.read(),
                        onclick: cancel,
                        "Cancel"
                    }
                    if !*opened.read() {
                        button {
                            class: "button primary",
                            disabled: *busy.read() || !has_session,
                            onclick: start,
                            if *busy.read() { "Opening..." } else { "Start camera" }
                        }
                    } else {
                        button {
                            class: "button primary",
                            disabled: *busy.read() || frames.read().len() >= expected,
                            onclick: shoot,
                            if *busy.read() { "Capturing..." } else { "Capture side {frames.read().len() + 1}" }
                        }
                    }
                }
            }
            section { class: "work-panel",
                div { class: "capture-frame",
                    div { class: "frame-corners" }
                    if let Some(source) = preview.read().as_ref() {
                        img { src: "{source}", alt: "Camera preview" }
                    } else {
                        span { if *opened.read() { "Camera ready" } else { "Preview idle" } }
                    }
                    if *use_orbbec.read() {
                        if let Some(source) = depth_preview.read().as_ref() {
                            img { src: "{source}", alt: "Orbbec depth preview" }
                        }
                    }
                    small { "{frames.read().len()} full-resolution frame(s) captured." }
                }
            }
        }
    }
}

#[component]
fn LegacyReview(
    capture: Option<PendingCapture>,
    data_root: String,
    on_retake: EventHandler<MouseEvent>,
    on_cancel: EventHandler<MouseEvent>,
    on_committed: EventHandler<CommitOutcome>,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut review_error = use_signal(|| None::<String>);
    let count = capture
        .as_ref()
        .map(|value| value.frames.len())
        .unwrap_or(0);
    let has_capture = capture.is_some();
    let review_frames = capture
        .as_ref()
        .map(|value| value.frames.clone())
        .unwrap_or_default();
    let cleanup_frames = capture
        .as_ref()
        .map(|value| value.frames.clone())
        .unwrap_or_default();
    let discard_frames = cleanup_frames.clone();
    let discard = move |event: MouseEvent| {
        let frames = discard_frames.clone();
        spawn(async move {
            delete_temporary_frames(frames).await;
            on_cancel.call(event);
        });
    };
    let retake = move |event: MouseEvent| {
        let frames = cleanup_frames.clone();
        spawn(async move {
            delete_temporary_frames(frames).await;
            on_retake.call(event);
        });
    };
    let commit = move |_| {
        let Some(pending) = capture.clone() else {
            review_error.set(Some("No capture is available to commit.".into()));
            return;
        };
        busy.set(true);
        review_error.set(None);
        let data_root = data_root.clone();
        spawn(async move {
            match commit_capture(pending, data_root).await {
                Ok(outcome) => on_committed.call(outcome),
                Err(message) => review_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    rsx! {
        div { class: "work-layout",
            section { class: "work-copy",
                h2 { "Review {count} captured sides" }
                if let Some(message) = review_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                div { class: "form-actions",
                    button { class: "button ghost", disabled: *busy.read(), onclick: discard, "Discard" }
                    button { class: "button secondary", disabled: *busy.read(), onclick: retake, "Retake" }
                    button { class: "button primary", disabled: *busy.read() || !has_capture, onclick: commit,
                        if *busy.read() { "Committing..." } else { "Commit capture" }
                    }
                }
            }
            section { class: "work-panel",
                div { class: "review-grid",
                    for (index, frame) in review_frames.iter().enumerate() {
                        figure {
                            img { src: "{convert_file_src(&frame.path)}", alt: "Captured side {index + 1}" }
                            figcaption {
                                strong { "Side {index + 1}" }
                                span { "{frame.width} x {frame.height} / {frame.source}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LegacyAnnotate(
    tree_id: Option<String>,
    data_root: String,
    on_next: EventHandler<MouseEvent>,
) -> Element {
    let mut tree = use_signal(|| None::<TreeData>);
    let mut active_side = use_signal(|| 0_usize);
    let mut busy = use_signal(|| false);
    let mut annotation_error = use_signal(|| None::<String>);

    use_effect(move || {
        let Some(id) = tree_id.clone() else {
            annotation_error.set(Some("No tree selected for annotation.".into()));
            return;
        };
        busy.set(true);
        spawn(async move {
            match load_tree(id).await {
                Ok(value) => tree.set(Some(value)),
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    });

    let detect = move |_| {
        let image_path = tree
            .read()
            .as_ref()
            .and_then(|value| value.sides.get(*active_side.read()))
            .map(|side| side.image_path.clone());
        let Some(image_path) = image_path else {
            annotation_error.set(Some("The selected side has no image.".into()));
            return;
        };
        busy.set(true);
        annotation_error.set(None);
        spawn(async move {
            match run_detector(image_path).await {
                Ok(boxes) => {
                    if let Some(value) = tree.write().as_mut() {
                        if let Some(side) = value.sides.get_mut(*active_side.read()) {
                            // Keep the detector output as the behavior-log baseline
                            // (suggestions) before the expert edits the boxes.
                            side.original_bboxes = boxes.clone();
                            side.bboxes = boxes;
                        }
                    }
                }
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let save = move |_| {
        let Some(mut value) = tree.read().clone() else {
            return;
        };
        let all_assigned = value
            .sides
            .iter()
            .flat_map(|side| &side.bboxes)
            .all(|bbox| (0..=3).contains(&bbox.class_id));
        value.status = if all_assigned {
            "annotated".into()
        } else {
            "captured".into()
        };
        busy.set(true);
        annotation_error.set(None);
        spawn(async move {
            match save_tree(value).await {
                Ok(saved) => tree.set(Some(saved)),
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let ready_for_dedup = tree.read().as_ref().is_some_and(|value| {
        value
            .sides
            .iter()
            .flat_map(|side| &side.bboxes)
            .all(|bbox| (0..=3).contains(&bbox.class_id))
    });
    let image_url = tree
        .read()
        .as_ref()
        .and_then(|value| value.sides.get(*active_side.read()))
        .map(|side| {
            let mut url = convert_file_src(&format!(
                "{}/dataset/{}",
                data_root.trim_end_matches(['/', '\\']),
                side.image_path
            ));
            // Append the per-capture cache-bust token so reusing a tree id can
            // never show a stale WebView-cached photo (matches the JS adapter).
            if let Some(bust) = &side.cache_bust {
                url.push(if url.contains('?') { '&' } else { '?' });
                url.push_str("v=");
                url.push_str(bust);
            }
            url
        });
    let box_count = tree
        .read()
        .as_ref()
        .and_then(|value| value.sides.get(*active_side.read()))
        .map(|side| side.bboxes.len())
        .unwrap_or(0);
    let side_tabs = tree
        .read()
        .as_ref()
        .map(|value| {
            value
                .sides
                .iter()
                .map(|side| (side.side_index, side.label.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let visible_boxes = tree
        .read()
        .as_ref()
        .and_then(|value| value.sides.get(*active_side.read()))
        .map(|side| side.bboxes.clone())
        .unwrap_or_default();
    let active_dimensions = tree
        .read()
        .as_ref()
        .and_then(|value| value.sides.get(*active_side.read()))
        .map(|side| (side.image_width.max(1), side.image_height.max(1)))
        .unwrap_or((1, 1));
    let visible_overlays = visible_boxes
        .iter()
        .map(|bbox| {
            (
                bbox.class_id,
                bbox.class_name.clone(),
                format!(
                    "left:{:.3}%;top:{:.3}%;width:{:.3}%;height:{:.3}%",
                    bbox.x1 / active_dimensions.0 as f64 * 100.0,
                    bbox.y1 / active_dimensions.1 as f64 * 100.0,
                    (bbox.x2 - bbox.x1) / active_dimensions.0 as f64 * 100.0,
                    (bbox.y2 - bbox.y1) / active_dimensions.1 as f64 * 100.0,
                ),
            )
        })
        .collect::<Vec<_>>();
    let canvas_style = format!(
        "aspect-ratio:{} / {}",
        active_dimensions.0, active_dimensions.1
    );

    rsx! {
        div { class: "annotation-layout",
            section { class: "work-copy",
                h2 { "Assign every detector box" }
                if let Some(message) = annotation_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                div { class: "side-tabs",
                    for (side_index, side_label) in side_tabs {
                        button {
                            class: if *active_side.read() == side_index { "text-button active" } else { "text-button" },
                            onclick: move |_| active_side.set(side_index),
                            "{side_label}"
                        }
                    }
                }
                div { class: "form-actions",
                    button { class: "button secondary", disabled: *busy.read(), onclick: detect,
                        if *busy.read() { "Working..." } else { "Run detector" }
                    }
                    button { class: "button primary", disabled: *busy.read(), onclick: save, "Save annotations" }
                    button { class: "button ghost", disabled: !ready_for_dedup, onclick: on_next, "Continue to dedup" }
                }
            }
            section { class: "annotation-workspace",
                div { class: "annotation-board",
                    if let Some(url) = image_url {
                        div { class: "annotation-canvas", style: "{canvas_style}",
                            img { src: "{url}", alt: "Selected palm tree side" }
                            div { class: "bbox-layer",
                                for (class_id, class_name, overlay_style) in visible_overlays {
                                    div {
                                        class: if class_id >= 0 { "bbox-overlay assigned" } else { "bbox-overlay" },
                                        style: "{overlay_style}",
                                        span { "{class_name}" }
                                    }
                                }
                            }
                        }
                    } else if *busy.read() {
                        span { "Loading tree..." }
                    } else {
                        span { "No side image" }
                    }
                }
                div { class: "box-list",
                    h3 { "{box_count} boxes" }
                    for (box_index, bbox) in visible_boxes.iter().enumerate() {
                        div { class: "box-row",
                            code { "{bbox.id}" }
                            for (class_id, class_name) in [(0, "B1"), (1, "B2"), (2, "B3"), (3, "B4")] {
                                button {
                                    class: if bbox.class_id == class_id { "class-button active" } else { "class-button" },
                                    onclick: move |_| {
                                        if let Some(value) = tree.write().as_mut() {
                                            if let Some(side) = value.sides.get_mut(*active_side.read()) {
                                                if let Some(target) = side.bboxes.get_mut(box_index) {
                                                    target.class_id = class_id;
                                                    target.class_name = class_name.into();
                                                }
                                            }
                                        }
                                    },
                                    "{class_name}"
                                }
                            }
                            button {
                                class: "class-button danger",
                                onclick: move |_| {
                                    if let Some(value) = tree.write().as_mut() {
                                        if let Some(side) = value.sides.get_mut(*active_side.read()) {
                                            if box_index < side.bboxes.len() {
                                                side.bboxes.remove(box_index);
                                                    }
                                                }
                                            }
                                        },
                                "Delete"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LegacyDedup(tree_id: Option<String>, on_results: EventHandler<MouseEvent>) -> Element {
    let mut tree = use_signal(|| None::<TreeData>);
    let mut pair_index = use_signal(|| 0_usize);
    let mut bbox_a = use_signal(String::new);
    let mut bbox_b = use_signal(String::new);
    let mut suggestions = use_signal(Vec::<LinkSuggestionData>::new);
    let mut busy = use_signal(|| false);
    let mut dedup_error = use_signal(|| None::<String>);

    let tree_id_for_load = tree_id.clone();
    use_effect(move || {
        let Some(id) = tree_id_for_load.clone() else {
            dedup_error.set(Some("No tree selected for deduplication.".into()));
            return;
        };
        busy.set(true);
        spawn(async move {
            match load_tree(id).await {
                Ok(value) => tree.set(Some(value)),
                Err(message) => dedup_error.set(Some(message)),
            }
            busy.set(false);
        });
    });

    let side_count = tree
        .read()
        .as_ref()
        .map(|value| value.sides.len())
        .unwrap_or(0);
    let pairs = (0..side_count)
        .map(|index| (index, (index + 1) % side_count.max(1)))
        .collect::<Vec<_>>();
    let active_pair = pairs.get(*pair_index.read()).copied();
    let boxes_a = active_pair
        .and_then(|(a, _)| tree.read().as_ref()?.sides.get(a).cloned())
        .map(|side| side.bboxes)
        .unwrap_or_default();
    let boxes_b = active_pair
        .and_then(|(_, b)| tree.read().as_ref()?.sides.get(b).cloned())
        .map(|side| side.bboxes)
        .unwrap_or_default();
    let links = tree
        .read()
        .as_ref()
        .map(|value| value.confirmed_links.clone())
        .unwrap_or_default();

    let add_link = move |_| {
        let Some((side_a, side_b)) = active_pair else {
            return;
        };
        let left = bbox_a.read().clone();
        let right = bbox_b.read().clone();
        if left.is_empty() || right.is_empty() {
            dedup_error.set(Some("Select one box from each adjacent side.".into()));
            return;
        }
        if let Some(value) = tree.write().as_mut() {
            value.confirmed_links.retain(|link| {
                let same_pair = (link.side_a == side_a && link.side_b == side_b)
                    || (link.side_a == side_b && link.side_b == side_a);
                let uses_endpoint = (link.side_a == side_a && link.bbox_id_a == left)
                    || (link.side_b == side_a && link.bbox_id_b == left)
                    || (link.side_a == side_b && link.bbox_id_a == right)
                    || (link.side_b == side_b && link.bbox_id_b == right);
                !(same_pair && uses_endpoint)
            });
            value.confirmed_links.push(ConfirmedLinkData {
                link_id: format!("lnk-{}", js_sys::Date::now() as u64),
                side_a,
                bbox_id_a: left,
                side_b,
                bbox_id_b: right,
            });
        }
        bbox_a.set(String::new());
        bbox_b.set(String::new());
        dedup_error.set(None);
    };

    let save = move |_| {
        let Some(value) = tree.read().clone() else {
            return;
        };
        busy.set(true);
        dedup_error.set(None);
        spawn(async move {
            match save_tree(value).await {
                Ok(saved) => tree.set(Some(saved)),
                Err(message) => dedup_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let run_suggestions = move |_| {
        let Some(id) = tree_id.clone() else {
            return;
        };
        busy.set(true);
        dedup_error.set(None);
        spawn(async move {
            match suggest_tree_links(id).await {
                Ok(value) => suggestions.set(value),
                Err(message) => dedup_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let suggestion_rows = suggestions.read().clone();

    rsx! {
        div { class: "work-layout",
            section { class: "work-copy",
                h2 { "Confirm adjacent-side matches" }
                if let Some(message) = dedup_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                label { class: "field",
                    span { "Adjacent pair" }
                    select {
                        value: "{pair_index}",
                        onchange: move |event| {
                            pair_index.set(event.value().parse().unwrap_or(0));
                            bbox_a.set(String::new());
                            bbox_b.set(String::new());
                        },
                        for (index, (a, b)) in pairs.iter().enumerate() {
                            option { value: "{index}", "Side {a + 1} / Side {b + 1}" }
                        }
                    }
                }
                label { class: "field",
                    span { "First side box" }
                    select {
                        value: "{bbox_a}",
                        onchange: move |event| bbox_a.set(event.value()),
                        option { value: "", "Select box" }
                        for bbox in boxes_a {
                            option { value: "{bbox.id}", "{bbox.id} / {bbox.class_name}" }
                        }
                    }
                }
                label { class: "field",
                    span { "Second side box" }
                    select {
                        value: "{bbox_b}",
                        onchange: move |event| bbox_b.set(event.value()),
                        option { value: "", "Select box" }
                        for bbox in boxes_b {
                            option { value: "{bbox.id}", "{bbox.id} / {bbox.class_name}" }
                        }
                    }
                }
                div { class: "form-actions",
                    button { class: "button ghost", disabled: *busy.read(), onclick: run_suggestions, "Compute suggestions" }
                    button { class: "button secondary", onclick: add_link, "Add confirmed link" }
                    button { class: "button primary", disabled: *busy.read(), onclick: save, "Save links" }
                    button { class: "button ghost", onclick: on_results, "Results" }
                }
            }
            section { class: "work-panel",
                div { class: "placeholder-rows",
                    for suggestion in suggestion_rows {
                        div {
                            span { "S{suggestion.side_a + 1}:{suggestion.bbox_id_a}" }
                            i {}
                            strong { "S{suggestion.side_b + 1}:{suggestion.bbox_id_b} / {suggestion.category} {suggestion.score:.2}" }
                            button {
                                class: "class-button active",
                                onclick: {
                                    let suggestion = suggestion.clone();
                                    move |_| {
                                        if let Some(value) = tree.write().as_mut() {
                                            value.confirmed_links.retain(|link| {
                                                let same_pair = (link.side_a == suggestion.side_a && link.side_b == suggestion.side_b)
                                                    || (link.side_a == suggestion.side_b && link.side_b == suggestion.side_a);
                                                let endpoint = (link.side_a == suggestion.side_a && link.bbox_id_a == suggestion.bbox_id_a)
                                                    || (link.side_b == suggestion.side_a && link.bbox_id_b == suggestion.bbox_id_a)
                                                    || (link.side_a == suggestion.side_b && link.bbox_id_a == suggestion.bbox_id_b)
                                                    || (link.side_b == suggestion.side_b && link.bbox_id_b == suggestion.bbox_id_b);
                                                !(same_pair && endpoint)
                                            });
                                            value.confirmed_links.push(ConfirmedLinkData {
                                                link_id: format!("lnk-{}", js_sys::Date::now() as u64),
                                                side_a: suggestion.side_a,
                                                bbox_id_a: suggestion.bbox_id_a.clone(),
                                                side_b: suggestion.side_b,
                                                bbox_id_b: suggestion.bbox_id_b.clone(),
                                            });
                                        }
                                        suggestions.write().retain(|item| item.link_id != suggestion.link_id);
                                    }
                                },
                                "Confirm"
                            }
                            button {
                                class: "class-button danger",
                                onclick: {
                                    let link_id = suggestion.link_id.clone();
                                    move |_| suggestions.write().retain(|item| item.link_id != link_id)
                                },
                                "Reject"
                            }
                        }
                    }
                    for (index, link) in links.iter().enumerate() {
                        div {
                            span { "S{link.side_a + 1}:{link.bbox_id_a}" }
                            i {}
                            strong { "S{link.side_b + 1}:{link.bbox_id_b}" }
                            button {
                                class: "class-button danger",
                                onclick: move |_| {
                                    if let Some(value) = tree.write().as_mut() {
                                        if index < value.confirmed_links.len() {
                                            value.confirmed_links.remove(index);
                                        }
                                    }
                                },
                                "Remove"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LegacyResults(tree_id: Option<String>) -> Element {
    let mut result = use_signal(|| None::<ComputeData>);
    let mut busy = use_signal(|| false);
    let mut result_error = use_signal(|| None::<String>);
    let mut export_notice = use_signal(|| None::<String>);
    let compute = move |_| {
        let Some(id) = tree_id.clone() else {
            result_error.set(Some("No tree selected for results.".into()));
            return;
        };
        busy.set(true);
        result_error.set(None);
        export_notice.set(None);
        spawn(async move {
            match compute_tree(id).await {
                Ok(value) => {
                    let mut failed = None;
                    for file in &value.export_files {
                        if let Err(message) = copy_to_saf(
                            &value.export_uri,
                            &file.relative_path,
                            &file.source_path,
                            &file.mime_type,
                        )
                        .await
                        {
                            failed = Some(message);
                            break;
                        }
                    }
                    export_notice.set(Some(if let Some(message) = failed {
                        format!("Exports saved locally; SAF mirror is incomplete: {message}")
                    } else {
                        format!(
                            "{} final output file(s) saved locally and mirrored to SAF.",
                            value.export_files.len()
                        )
                    }));
                    result.set(Some(value));
                }
                Err(message) => result_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let metrics = result.read().as_ref().map(|value| value.result.clone());
    rsx! {
        div { class: "results-layout",
            section { class: "result-hero",
                h2 { if metrics.is_some() { "Tree result ready" } else { "Compute the selected tree" } }
                if let Some(message) = result_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                if let Some(message) = export_notice.read().as_ref() {
                    div { class: "inline-notice", "{message}" }
                }
                button { class: "button primary", disabled: *busy.read(), onclick: compute,
                    if *busy.read() { "Computing..." } else { "Compute and export" }
                }
            }
            div { class: "metric-line",
                MetricValue { value: metrics.as_ref().map(|m| m.unique_count).unwrap_or(0), label: "Unique bunches" }
                MetricValue { value: metrics.as_ref().map(|m| m.raw_count).unwrap_or(0), label: "Raw detections" }
                MetricValue { value: metrics.as_ref().map(|m| m.linked_count).unwrap_or(0), label: "Linked duplicates" }
                MetricValue { value: metrics.as_ref().map(|m| m.unassigned_count).unwrap_or(0), label: "Unassigned" }
            }
            section { class: "result-table",
                h3 { "Class distribution" }
                if let Some(value) = metrics.as_ref() {
                    for (class_name, count) in &value.class_counts {
                        div { class: "setting-row", strong { "{class_name}" } span { "{count}" } }
                    }
                }
                if let Some(value) = result.read().as_ref() {
                    p { if value.quality.ready { "Quality checks passed." } else { "Quality checks require attention." } }
                    for issue in &value.quality.issues {
                        small { "{issue.code}: {issue.message}" }
                    }
                }
            }
        }
    }
}

#[component]
fn LegacyDepthViewer(tree_id: Option<String>) -> Element {
    let mut side_index = use_signal(|| 0_usize);
    let mut preview = use_signal(|| None::<DepthRenderData>);
    let mut busy = use_signal(|| false);
    let mut depth_error = use_signal(|| None::<String>);
    let open = move |_| {
        let Some(id) = tree_id.clone() else {
            depth_error.set(Some("Open a captured tree before viewing depth.".into()));
            return;
        };
        busy.set(true);
        depth_error.set(None);
        spawn(async move {
            match render_depth(id, *side_index.read()).await {
                Ok(value) => preview.set(Some(value)),
                Err(message) => depth_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let preview_url = preview
        .read()
        .as_ref()
        .map(|value| convert_file_src(&value.path));
    rsx! {
        div { class: "work-layout",
            section { class: "work-copy",
                h2 { "Inspect raw uint16 depth" }
                label { class: "field",
                    span { "Side number" }
                    input {
                        r#type: "number",
                        min: "1",
                        value: "{*side_index.read() + 1}",
                        oninput: move |event| {
                            side_index.set(event.value().parse::<usize>().unwrap_or(1).saturating_sub(1));
                        }
                    }
                }
                if let Some(message) = depth_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                button { class: "button primary", disabled: *busy.read(), onclick: open,
                    if *busy.read() { "Rendering..." } else { "Render depth" }
                }
            }
            section { class: "work-panel",
                div { class: "depth-field",
                    if let Some(url) = preview_url {
                        img { src: "{url}", alt: "Depth preview" }
                    } else {
                        strong { "Depth preview" }
                    }
                    if let Some(value) = preview.read().as_ref() {
                        span { "{value.minimum:.0} mm" }
                        small { "{value.width} x {value.height}" }
                        span { "{value.maximum:.0} mm" }
                    }
                }
            }
        }
    }
}

#[component]
fn Settings(
    settings: AppSettings,
    data_root: String,
    on_back: EventHandler<MouseEvent>,
    on_saved: EventHandler<AppSettings>,
) -> Element {
    let initial_uri = settings.export_uri;
    let initial_folder = if settings.export_name.is_empty() {
        "Not selected".to_string()
    } else {
        settings.export_name
    };
    let mut saf_uri = use_signal(move || initial_uri);
    let mut saf = use_signal(move || initial_folder);
    let mut camera = use_signal(|| "Tap to check".to_string());
    let mut orbbec = use_signal(|| "Tap to refresh".to_string());
    let mut notice = use_signal(|| None::<String>);

    rsx! {
        div { class: "settings-list",
            div { class: "screen-title",
                button { class: "icon-button", onclick: on_back, aria_label: "Back",
                    Icon { name: "back" }
                }
                h2 { "Settings" }
            }
            if let Some(message) = notice.read().as_ref() {
                div { class: "inline-error", "{message}" }
            }
            div { class: "setting-row",
                div { strong { "Export folder" } span { "{saf}" } }
                button { class: "button secondary",
                    onclick: move |_| {
                        notice.set(None);
                        let data_root = data_root.clone();
                        spawn(async move {
                            match pick_saf_folder().await {
                                Ok(Some(folder)) => {
                                    let next = AppSettings {
                                        export_uri: folder.uri,
                                        export_name: folder.name,
                                        ..AppSettings::default()
                                    };
                                    match save_app_settings(next).await {
                                        Ok(saved) => {
                                            if let Err(message) = mirror_sessions_index(
                                                &data_root,
                                                &saved.export_uri,
                                            ).await {
                                                notice.set(Some(format!("Folder selected: {message}")));
                                            }
                                            saf.set(if saved.export_name.is_empty() {
                                                "Selected".into()
                                            } else {
                                                saved.export_name.clone()
                                            });
                                            saf_uri.set(saved.export_uri.clone());
                                            on_saved.call(saved);
                                        }
                                        Err(message) => notice.set(Some(message)),
                                    }
                                }
                                Ok(None) => {}
                                Err(message) => notice.set(Some(message)),
                            }
                        });
                    },
                    "Choose folder"
                }
                if !saf_uri.read().is_empty() {
                    button { class: "button ghost danger-text",
                        onclick: move |_| {
                            let uri = saf_uri.read().clone();
                            spawn(async move {
                                notice.set(None);
                                if let Err(message) = release_saf_folder(&uri).await {
                                    notice.set(Some(message));
                                    return;
                                }
                                match save_app_settings(AppSettings::default()).await {
                                    Ok(saved) => {
                                        saf_uri.set(String::new());
                                        saf.set("Not selected".into());
                                        on_saved.call(saved);
                                    }
                                    Err(message) => notice.set(Some(message)),
                                }
                            });
                        },
                        "Clear"
                    }
                }
            }
            div { class: "setting-row",
                div { strong { "Camera (CameraX)" } span { "{camera}" } }
                button { class: "button secondary",
                    onclick: move |_| {
                        spawn(async move {
                            match native_empty::<serde_json::Value>("plugin:palm-native|camera_status").await {
                                Ok(value) => {
                                    let granted = value.get("permission").and_then(serde_json::Value::as_bool).unwrap_or(false);
                                    camera.set(if granted { "Permission granted".into() } else { "Permission not granted".into() });
                                }
                                Err(message) => camera.set(format!("Error: {message}")),
                            }
                        });
                    },
                    "Check permission"
                }
            }
            div { class: "setting-row",
                div { strong { "Orbbec USB" } span { "{orbbec}" } }
                button { class: "button secondary",
                    onclick: move |_| {
                        spawn(async move {
                            match native_empty::<serde_json::Value>("plugin:palm-native|orbbec_refresh").await {
                                Ok(value) => {
                                    let count = value.get("count").and_then(serde_json::Value::as_u64).unwrap_or(0);
                                    orbbec.set(if count > 0 { format!("{count} device(s) attached") } else { "Not attached".into() });
                                }
                                Err(message) => orbbec.set(format!("Error: {message}")),
                            }
                        });
                    },
                    "Refresh"
                }
            }
        }
    }
}

#[component]
fn WorkPage(
    eyebrow: &'static str,
    title: &'static str,
    description: &'static str,
    action: &'static str,
    panel: Element,
) -> Element {
    rsx! {
        div { class: "work-layout",
            section { class: "work-copy",
                p { class: "eyebrow", "{eyebrow}" }
                h2 { "{title}" }
                p { "{description}" }
                button { class: "button primary", "{action}" }
            }
            section { class: "work-panel", {panel} }
        }
    }
}

#[component]
fn MetricValue(value: usize, label: &'static str) -> Element {
    rsx! { div { class: "metric", strong { "{value}" } span { "{label}" } } }
}

#[component]
fn PlaceholderRows(count: usize, label: &'static str) -> Element {
    rsx! {
        div { class: "placeholder-rows",
            for index in 0..count {
                div { span { "{label} {index + 1}" } i {} strong { "Pending" } }
            }
        }
    }
}

#[component]
fn LoadingState() -> Element {
    rsx! { div { class: "skeleton-page", div {} div {} div {} div {} } }
}

#[component]
fn ErrorState(message: String) -> Element {
    rsx! { div { class: "error-state", strong { "PalmAnnotate could not open local storage." } p { "{message}" } button { class: "button secondary", onclick: move |_| { let _ = web_sys::window().map(|w| w.location().reload()); }, "Retry" } } }
}

#[component]
fn Icon(name: &'static str) -> Element {
    let path = match name {
        "grid" => "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
        "camera" => "M4 8h3l2-3h6l2 3h3v11H4zM12 10a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z",
        "review" => "M5 4h14v16H5zM8 9l2 2 5-5M8 15h8",
        "box" => "M4 6l8-3 8 3v12l-8 3-8-3zM4 6l8 3 8-3M12 9v12",
        "link" => "M9 15l6-6M7.5 17.5l-1 1a3.5 3.5 0 0 1-5-5l3-3a3.5 3.5 0 0 1 5 0M16.5 6.5l1-1a3.5 3.5 0 0 1 5 5l-3 3a3.5 3.5 0 0 1-5 0",
        "chart" => "M5 20V10M12 20V4M19 20v-7",
        "depth" => "M3 12c4-6 14-6 18 0-4 6-14 6-18 0zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
        "settings" => "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM4 12h2M18 12h2M12 4v2M12 18v2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M17.7 6.3l-1.4 1.4M7.7 16.3l-1.4 1.4",
        "back" => "M19 12H5M10 7l-5 5 5 5",
        "plus" => "M12 5v14M5 12h14",
        "folder" => "M3 6h7l2 2h9v11H3z",
        "tree" => "M12 3l5 7h-3l4 6h-5v5h-2v-5H6l4-6H7z",
        "arrow" => "M5 12h14M14 7l5 5-5 5",
        _ => "M5 12h14",
    };
    rsx! {
        svg { class: "icon", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "{path}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        add_confirmed_link, delete_bbox, move_bbox, normalized_block, normalized_segment,
        resize_bbox, set_connected_bbox_class, sides_are_adjacent, BoxData, ResizeHandle, SideData,
        TreeData,
    };

    fn bbox(id: &str) -> BoxData {
        BoxData {
            id: id.into(),
            class_id: -1,
            class_name: "U".into(),
            x1: 10.0,
            y1: 10.0,
            x2: 30.0,
            y2: 30.0,
            confidence: None,
        }
    }

    fn tree() -> TreeData {
        let mut sides = (0..4)
            .map(|side_index| SideData {
                side_index,
                label: format!("Side {}", side_index + 1),
                image_path: format!("images/field/TREE_0001_{}.jpg", side_index + 1),
                image_width: 100,
                image_height: 100,
                depth_path: None,
                depth: None,
                bboxes: vec![bbox(&format!("b{side_index}"))],
                original_bboxes: Vec::new(),
                cache_bust: None,
            })
            .collect::<Vec<_>>();
        sides[1].bboxes.push(bbox("b1-alt"));
        TreeData {
            version: 4,
            id: "tree-1".into(),
            session_id: "session-1".into(),
            tree_name: "TREE_0001".into(),
            split: "field".into(),
            side_count: 4,
            metadata: serde_json::json!({}),
            sides,
            confirmed_links: Vec::new(),
            status: "annotated".into(),
        }
    }

    #[test]
    fn canonical_tree_tokens_match_legacy_capture_names() {
        assert_eq!(normalized_segment("Tenera hybrid"), "TENERA_HYBRID");
        assert_eq!(normalized_block("b-07"), "B07");
        assert_eq!(normalized_block("A 21b"), "A21B");
    }

    #[test]
    fn links_require_adjacent_sides_and_replace_pair_endpoints() {
        assert!(sides_are_adjacent(0, 3, 4));
        assert!(!sides_are_adjacent(0, 2, 4));

        let mut tree = tree();
        add_confirmed_link(&mut tree, 0, "b0".into(), 1, "b1".into()).unwrap();
        add_confirmed_link(&mut tree, 1, "b1".into(), 2, "b2".into()).unwrap();
        add_confirmed_link(&mut tree, 0, "b0".into(), 1, "b1-alt".into()).unwrap();

        assert_eq!(tree.confirmed_links.len(), 2);
        assert!(tree
            .confirmed_links
            .iter()
            .any(|link| link.bbox_id_b == "b1-alt"));
        assert!(add_confirmed_link(&mut tree, 0, "b0".into(), 2, "b2".into()).is_err());
    }

    #[test]
    fn class_changes_propagate_and_delete_removes_links() {
        let mut tree = tree();
        add_confirmed_link(&mut tree, 0, "b0".into(), 1, "b1".into()).unwrap();
        add_confirmed_link(&mut tree, 1, "b1".into(), 2, "b2".into()).unwrap();

        assert_eq!(set_connected_bbox_class(&mut tree, 0, "b0", 2), 3);
        assert_eq!(tree.sides[2].bboxes[0].class_name, "B3");
        assert!(delete_bbox(&mut tree, 1, "b1"));
        assert!(tree.confirmed_links.is_empty());
    }

    #[test]
    fn bbox_move_and_resize_stay_inside_image() {
        let original = bbox("box");
        let mut moved = original.clone();
        move_bbox(&mut moved, &original, 200.0, 200.0, 100.0, 100.0);
        assert_eq!(
            (moved.x1, moved.y1, moved.x2, moved.y2),
            (80.0, 80.0, 100.0, 100.0)
        );

        let mut resized = original.clone();
        resize_bbox(
            &mut resized,
            &original,
            ResizeHandle::NorthWest,
            29.0,
            29.0,
            100.0,
            100.0,
        );
        assert_eq!((resized.x1, resized.y1), (26.0, 26.0));
    }
}
