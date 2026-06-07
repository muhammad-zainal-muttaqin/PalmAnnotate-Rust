#![allow(non_snake_case)]

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

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

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Home => "Field sessions",
            Self::NewSession => "New session",
            Self::SessionDetail => "Session detail",
            Self::Capture => "Capture",
            Self::Review => "Review",
            Self::Annotate => "Annotate",
            Self::Dedup => "Dedup",
            Self::Results => "Results",
            Self::DepthViewer => "Depth viewer",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    product_name: String,
    schema_version: u8,
    data_root: String,
    sessions: Vec<Session>,
    platform: String,
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
struct SafCopyArgs {
    tree_uri: String,
    relative_path: String,
    source_path: String,
    mime_type: String,
}

#[derive(Serialize)]
struct TempPathArgs {
    path: String,
}

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| js_sys::JSON::stringify(&value).ok()?.as_string())
        .unwrap_or_else(|| "Native command failed.".into())
}

async fn load_bootstrap() -> Result<Bootstrap, String> {
    let value = invoke("bootstrap", JsValue::UNDEFINED)
        .await
        .map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn save_session(session: Session) -> Result<Session, String> {
    let args = serde_wasm_bindgen::to_value(&SessionArgs { session })
        .map_err(|error| error.to_string())?;
    let value = invoke("session_save", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn delete_session(session_id: String) -> Result<Vec<Session>, String> {
    let args = serde_wasm_bindgen::to_value(&SessionIdArgs { session_id })
        .map_err(|error| error.to_string())?;
    let value = invoke("session_delete", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn list_sessions() -> Result<Vec<Session>, String> {
    let value = invoke("session_list", JsValue::UNDEFINED)
        .await
        .map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn delete_tree(tree_id: String) -> Result<(), String> {
    let args =
        serde_wasm_bindgen::to_value(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    invoke("tree_delete", args).await.map_err(js_error)?;
    Ok(())
}

async fn load_tree(tree_id: String) -> Result<TreeData, String> {
    let args =
        serde_wasm_bindgen::to_value(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    let value = invoke("tree_load", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn save_tree(tree: TreeData) -> Result<TreeData, String> {
    let args =
        serde_wasm_bindgen::to_value(&TreeSaveArgs { tree }).map_err(|error| error.to_string())?;
    let value = invoke("tree_save", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn run_detector(image_path: String) -> Result<Vec<BoxData>, String> {
    let args = serde_wasm_bindgen::to_value(&DetectorArgs { image_path })
        .map_err(|error| error.to_string())?;
    let value = invoke("detector_run", args).await.map_err(js_error)?;
    let response: DetectorData =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    Ok(response.boxes)
}

async fn compute_tree(tree_id: String) -> Result<ComputeData, String> {
    let args =
        serde_wasm_bindgen::to_value(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    let value = invoke("tree_compute", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn suggest_tree_links(tree_id: String) -> Result<Vec<LinkSuggestionData>, String> {
    let args =
        serde_wasm_bindgen::to_value(&TreeIdArgs { tree_id }).map_err(|error| error.to_string())?;
    let value = invoke("tree_suggest", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn render_depth(tree_id: String, side_index: usize) -> Result<DepthRenderData, String> {
    let args = serde_wasm_bindgen::to_value(&DepthRenderArgs {
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
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "payload": SafCopyArgs {
            tree_uri: tree_uri.into(),
            relative_path: relative_path.into(),
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
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "payload": { "treeUri": tree_uri, "relativePath": relative_path }
    }))
    .map_err(|error| error.to_string())?;
    invoke("plugin:palm-native|saf_delete", args)
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
            if let Ok(args) =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "payload": { "path": path } }))
            {
                let _ = invoke("plugin:palm-native|temp_delete", args).await;
            }
        }
    }
}

async fn optional_gps() -> Option<GpsData> {
    let permission_args = serde_wasm_bindgen::to_value(&serde_json::json!({
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
    let position_args = serde_wasm_bindgen::to_value(&serde_json::json!({
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
        serde_wasm_bindgen::to_value(&serde_json::json!({})).map_err(|error| error.to_string())?,
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

async fn import_saf_folder() -> Result<Option<Vec<Session>>, String> {
    let Some(folder) = pick_saf_folder().await? else {
        return Ok(None);
    };
    let tree_args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "payload": { "treeUri": folder.uri.clone() }
    }))
    .map_err(|error| error.to_string())?;
    let value = invoke("plugin:palm-native|saf_copy_tree_to_temp", tree_args)
        .await
        .map_err(js_error)?;
    let staged: NativePath =
        serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())?;
    let args = serde_wasm_bindgen::to_value(&ImportFolderArgs {
        folder_path: staged.path,
        export_uri: folder.uri,
    })
    .map_err(|error| error.to_string())?;
    let value = invoke("sessions_import_folder", args)
        .await
        .map_err(js_error)?;
    serde_wasm_bindgen::from_value(value)
        .map(Some)
        .map_err(|error| error.to_string())
}

async fn native_empty<T>(command: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let args =
        serde_wasm_bindgen::to_value(&serde_json::json!({})).map_err(|error| error.to_string())?;
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
    let args = serde_wasm_bindgen::to_value(&CaptureCommitArgs { request })
        .map_err(|error| error.to_string())?;
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
            format!("Output TXT/{}_{}.txt", tree.tree_name, side.side_index + 1),
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

    use_effect(move || {
        spawn(async move {
            match load_bootstrap().await {
                Ok(value) => bootstrap.set(Some(value)),
                Err(message) => error.set(Some(message)),
            }
            loading.set(false);
        });
    });

    rsx! {
        document::Stylesheet { href: STYLES }
        div { class: "app-shell",
            Sidebar { page, on_navigate: move |next| page.set(next) }
            main { class: "workspace",
                if *page.read() != Page::Home {
                    Header { page, bootstrap: bootstrap.read().clone() }
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
                                    on_new: move |_| page.set(Page::NewSession),
                                    on_import: move |_| {
                                        loading.set(true);
                                        notice.set(None);
                                        spawn(async move {
                                            match import_saf_folder().await {
                                                Ok(Some(sessions)) => {
                                                    if let Some(value) = bootstrap.write().as_mut() {
                                                        value.sessions = sessions;
                                                    }
                                                }
                                                Ok(None) => {}
                                                Err(message) => notice.set(Some(message)),
                                            }
                                            loading.set(false);
                                        });
                                    },
                                    on_open: move |session: Session| {
                                        selected_session.set(Some(session));
                                        page.set(Page::SessionDetail);
                                    }
                                }
                            },
                            Page::NewSession => rsx! {
                                NewSession {
                                    data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
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
                                    on_capture: move |_| page.set(Page::Capture),
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
                                        page.set(Page::Home);
                                    }
                                }
                            },
                            Page::Capture => rsx! {
                                Capture {
                                    session: selected_session.read().clone(),
                                    on_cancel: move |_| page.set(Page::SessionDetail),
                                    on_complete: move |capture: PendingCapture| {
                                        pending_capture.set(Some(capture));
                                        page.set(Page::Review);
                                    }
                                }
                            },
                            Page::Review => rsx! {
                                Review {
                                    capture: pending_capture.read().clone(),
                                    data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                    on_retake: move |_| page.set(Page::Capture),
                                    on_cancel: move |_| {
                                        pending_capture.set(None);
                                        page.set(Page::SessionDetail);
                                    },
                                    on_committed: move |outcome: CommitOutcome| {
                                        selected_tree_id.set(Some(outcome.tree_id));
                                        notice.set(outcome.mirror_warning);
                                        pending_capture.set(None);
                                        page.set(Page::Annotate);
                                    }
                                }
                            },
                            Page::Annotate => rsx! {
                                Annotate {
                                    tree_id: selected_tree_id.read().clone(),
                                    data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default(),
                                    on_next: move |_| page.set(Page::Dedup)
                                }
                            },
                            Page::Dedup => rsx! {
                                Dedup {
                                    tree_id: selected_tree_id.read().clone(),
                                    on_results: move |_| page.set(Page::Results)
                                }
                            },
                            Page::Results => rsx! {
                                Results { tree_id: selected_tree_id.read().clone() }
                            },
                            Page::DepthViewer => rsx! {
                                DepthViewer { tree_id: selected_tree_id.read().clone() }
                            },
                            Page::Settings => rsx! {
                                Settings {
                                    data_root: bootstrap.read().as_ref().map(|b| b.data_root.clone()).unwrap_or_default()
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Sidebar(page: Signal<Page>, on_navigate: EventHandler<Page>) -> Element {
    let primary = [
        (Page::Home, "Sessions", "grid"),
        (Page::Capture, "Capture", "camera"),
        (Page::Review, "Review", "review"),
        (Page::Annotate, "Annotate", "box"),
        (Page::Dedup, "Dedup", "link"),
        (Page::Results, "Results", "chart"),
    ];
    rsx! {
        aside { class: "sidebar",
            div { class: "brand",
                div { class: "brand-mark", "PA" }
                div { class: "brand-copy",
                    strong { "PalmAnnotate" }
                    span { "Field workspace" }
                }
            }
            nav { class: "nav-list", aria_label: "Primary",
                for (target, label, icon) in primary {
                    button {
                        class: if *page.read() == target { "nav-item active" } else { "nav-item" },
                        onclick: move |_| on_navigate.call(target),
                        Icon { name: icon }
                        span { "{label}" }
                    }
                }
            }
            div { class: "sidebar-bottom",
                button {
                    class: if *page.read() == Page::DepthViewer { "nav-item active" } else { "nav-item" },
                    onclick: move |_| on_navigate.call(Page::DepthViewer),
                    Icon { name: "depth" }
                    span { "Depth" }
                }
                button {
                    class: if *page.read() == Page::Settings { "nav-item active" } else { "nav-item" },
                    onclick: move |_| on_navigate.call(Page::Settings),
                    Icon { name: "settings" }
                    span { "Settings" }
                }
                div { class: "sync-state",
                    span { class: "status-dot" }
                    div { strong { "Offline ready" } span { "Primary data local" } }
                }
            }
        }
    }
}

#[component]
fn Header(page: Signal<Page>, bootstrap: Option<Bootstrap>) -> Element {
    rsx! {
        header { class: "topbar",
            div {
                p { class: "eyebrow", "PALMANNOTATE / FIELD" }
                h1 { "{page.read().title()}" }
            }
            div { class: "device-strip",
                DeviceState { label: "Camera", state: "Ready", active: true }
                DeviceState { label: "Orbbec", state: "Not attached", active: false }
                div { class: "schema-pill", "Schema v{bootstrap.as_ref().map(|b| b.schema_version).unwrap_or(4)}" }
            }
        }
    }
}

#[component]
fn DeviceState(label: &'static str, state: &'static str, active: bool) -> Element {
    rsx! {
        div { class: "device-state",
            span { class: if active { "status-dot" } else { "status-dot muted" } }
            div { span { "{label}" } strong { "{state}" } }
        }
    }
}

#[component]
fn Home(
    sessions: Vec<Session>,
    on_new: EventHandler<MouseEvent>,
    on_import: EventHandler<MouseEvent>,
    on_open: EventHandler<Session>,
) -> Element {
    let total_trees: usize = sessions.iter().map(|session| session.trees.len()).sum();
    let mut groups: Vec<String> = sessions
        .iter()
        .map(|session| format!("{}|{}", session.variety, session.block))
        .collect();
    groups.sort_unstable();
    groups.dedup();
    let total_groups = groups.len();
    rsx! {
        div { class: "home",
            header { class: "home-head",
                h1 { "PalmAnnotate" }
                p { "Fresh fruit bunch documentation — work session by session" }
            }
            div { class: "stat-cards",
                div { class: "stat-card",
                    strong { class: "stat-trees", "{total_trees}" }
                    span { "TOTAL TREES" }
                }
                div { class: "stat-card",
                    strong { class: "stat-groups", "{total_groups}" }
                    span { "TOTAL GROUPS" }
                }
            }
            button { class: "button primary block", onclick: on_new,
                Icon { name: "plus" } "New Session"
            }
            p { class: "section-label", "RECENT SESSIONS" }
            if sessions.is_empty() {
                div { class: "empty-simple",
                    "No sessions yet. Choose an export folder, then create one."
                }
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
                                strong { "{session.variety} · {session.block}" }
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
            }
        }
    }
}

#[component]
fn NewSession(
    data_root: String,
    on_cancel: EventHandler<MouseEvent>,
    on_warning: EventHandler<String>,
    on_saved: EventHandler<Session>,
) -> Element {
    let mut variety = use_signal(String::new);
    let mut block = use_signal(String::new);
    let mut operator = use_signal(String::new);
    let mut export_uri = use_signal(String::new);
    let mut export_name = use_signal(String::new);
    let mut side_count = use_signal(|| 4_usize);
    let mut auto_id = use_signal(|| true);
    let mut form_error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let mut choosing_folder = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if variety.read().trim().is_empty()
            || block.read().trim().is_empty()
            || export_uri.read().trim().is_empty()
        {
            form_error.set(Some(
                "Variety, block, and SAF export folder are required.".into(),
            ));
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
            export_uri: export_uri.read().trim().into(),
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
                        placeholder: "Example: DAMIMAS",
                        value: "{variety}",
                        oninput: move |event| variety.set(event.value())
                    }
                    small { "Locked for every tree in this session." }
                }
                label { class: "field",
                    span { "Block" }
                    input {
                        placeholder: "Example: A21B",
                        value: "{block}",
                        oninput: move |event| block.set(event.value())
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
                    span { "SAF export folder URI" }
                    div { class: "input-action",
                        input {
                            placeholder: "content://...",
                            value: "{export_uri}",
                            readonly: true
                        }
                        button {
                            class: "button secondary",
                            r#type: "button",
                            disabled: *choosing_folder.read(),
                            onclick: move |_| {
                                choosing_folder.set(true);
                                form_error.set(None);
                                spawn(async move {
                                    match pick_saf_folder().await {
                                        Ok(Some(folder)) => {
                                            export_uri.set(folder.uri);
                                            export_name.set(folder.name);
                                        }
                                        Ok(None) => {}
                                        Err(message) => form_error.set(Some(message)),
                                    }
                                    choosing_folder.set(false);
                                });
                            },
                            if *choosing_folder.read() { "Opening..." } else { "Choose" }
                        }
                    }
                    if !export_name.read().is_empty() {
                        small { "Selected: {export_name}" }
                    }
                    small { "Required before any tree is created. Mirror failures never discard local primary data." }
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
            aside { class: "form-aside",
                h3 { "Local first, export visible" }
                ul {
                    li { strong { "Primary" } span { "Tauri app data / PalmAnnotate" } }
                    li { strong { "Mirror" } span { "Selected SAF folder" } }
                    li { strong { "Output" } span { "JSON v4 and YOLO TXT" } }
                    li { strong { "Conflict" } span { "Import refuses silent overwrite" } }
                }
            }
        }
    }
}

#[component]
fn SessionDetail(
    session: Option<Session>,
    data_root: String,
    on_capture: EventHandler<MouseEvent>,
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
    let data_root_for_delete = data_root.clone();
    let delete = move |_| {
        let target = session_for_delete.clone();
        busy.set(true);
        detail_error.set(None);
        let data_root = data_root_for_delete.clone();
        spawn(async move {
            match delete_session(target.id.clone()).await {
                Ok(sessions) => {
                    let mut mirror_failed = None;
                    for tree in &target.trees {
                        let mut paths = vec![
                            format!("trees/{}.json", tree.id),
                            format!("Output JSON/{}.json", tree.tree_name),
                        ];
                        for side in 1..=tree.side_count {
                            paths.push(format!("Output TXT/{}_{}.txt", tree.tree_name, side));
                            paths.push(format!(
                                "dataset/images/field/{}_{}.jpg",
                                tree.tree_name, side
                            ));
                            paths.push(format!(
                                "dataset/depth/field/{}_{}.raw",
                                tree.tree_name, side
                            ));
                            paths.push(format!(
                                "dataset/depth/field/{}_{}.raw.json",
                                tree.tree_name, side
                            ));
                        }
                        for path in paths {
                            if let Err(message) = delete_from_saf(&target.export_uri, &path).await {
                                mirror_failed = Some(message);
                                break;
                            }
                        }
                    }
                    if let Err(message) = copy_to_saf(
                        &target.export_uri,
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
    rsx! {
        div { class: "detail-grid",
            section { class: "detail-summary",
                h2 { "{session.name}" }
                p { "{session.side_count} sides per tree / next tree {session.next_id:04} / operator {session.operator}" }
                if let Some(message) = detail_error.read().as_ref() {
                    div { class: "inline-error", "{message}" }
                }
                button { class: "button primary", onclick: on_capture, Icon { name: "camera" } "Capture new tree" }
                button { class: "button ghost", disabled: *busy.read(), onclick: delete,
                    if *busy.read() { "Deleting..." } else { "Delete session and data" }
                }
            }
            WorkflowRail {}
            section { class: "empty-table",
                div { h3 { "Trees" } span { "{session.trees.len()} records" } }
                if session.trees.is_empty() {
                    p { "No tree has been captured in this session." }
                } else {
                    div { class: "placeholder-rows",
                        for tree in session.trees.clone() {
                            div {
                                span { "{tree.tree_name}" }
                                i {}
                                strong { "{tree.status}" }
                                button {
                                    class: "class-button danger",
                                    disabled: *busy.read(),
                                    onclick: {
                                        let tree = tree.clone();
                                        let export_uri = session.export_uri.clone();
                                        let data_root = data_root.clone();
                                        move |_| {
                                            let tree = tree.clone();
                                            let export_uri = export_uri.clone();
                                            let data_root = data_root.clone();
                                            busy.set(true);
                                            detail_error.set(None);
                                            spawn(async move {
                                                match delete_tree(tree.id.clone()).await {
                                                    Ok(()) => {
                                                        let mut paths = vec![
                                                            format!("trees/{}.json", tree.id),
                                                            format!("Output JSON/{}.json", tree.tree_name),
                                                        ];
                                                        for side in 1..=tree.side_count {
                                                            paths.push(format!("Output TXT/{}_{}.txt", tree.tree_name, side));
                                                            paths.push(format!("dataset/images/field/{}_{}.jpg", tree.tree_name, side));
                                                            paths.push(format!("dataset/depth/field/{}_{}.raw", tree.tree_name, side));
                                                            paths.push(format!("dataset/depth/field/{}_{}.raw.json", tree.tree_name, side));
                                                        }
                                                        for path in paths {
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
fn Capture(
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
fn Review(
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
fn Annotate(
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
fn Dedup(tree_id: Option<String>, on_results: EventHandler<MouseEvent>) -> Element {
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
fn Results(tree_id: Option<String>) -> Element {
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
fn DepthViewer(tree_id: Option<String>) -> Element {
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
fn Settings(data_root: String) -> Element {
    let mut saf = use_signal(|| "Not selected".to_string());
    let mut camera = use_signal(|| "Tap to check".to_string());
    let mut orbbec = use_signal(|| "Tap to refresh".to_string());
    let mut notice = use_signal(|| None::<String>);

    rsx! {
        div { class: "settings-list",
            if let Some(message) = notice.read().as_ref() {
                div { class: "inline-error", "{message}" }
            }
            section { h2 { "Local primary store" } code { "{data_root}" } }
            div { class: "setting-row",
                div { strong { "SAF export folder" } span { "{saf}" } }
                button { class: "button secondary",
                    onclick: move |_| {
                        notice.set(None);
                        spawn(async move {
                            match pick_saf_folder().await {
                                Ok(Some(folder)) => {
                                    let label = if folder.name.is_empty() { folder.uri } else { folder.name };
                                    saf.set(label);
                                }
                                Ok(None) => {}
                                Err(message) => notice.set(Some(message)),
                            }
                        });
                    },
                    "Choose folder"
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
                            match native_empty::<serde_json::Value>("plugin:palm-native|orbbec_status").await {
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
            section { h2 { "Offline detector" } code { "ffb-detector.onnx / 640 px" } }
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
    use super::{normalized_block, normalized_segment};

    #[test]
    fn canonical_tree_tokens_match_legacy_capture_names() {
        assert_eq!(normalized_segment("Tenera hybrid"), "TENERA_HYBRID");
        assert_eq!(normalized_block("b-07"), "B07");
        assert_eq!(normalized_block("A 21b"), "A21B");
    }
}
