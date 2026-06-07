use palmannotate_core::{
    build_output_v4, check_tree, compute_results, load_output_v4, parse_yolo, suggest_tree,
    AppError, AppStore, ErrorPayload, LinkSuggestion, OutputV4, QualityReport, Session, Side, Tree,
    TreeMetadata, TreeStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};

struct AppState {
    store: Mutex<AppStore>,
}

type CommandResult<T> = Result<T, ErrorPayload>;

fn command<T>(result: Result<T, AppError>) -> CommandResult<T> {
    result.map_err(|error| error.payload())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    product_name: &'static str,
    schema_version: u8,
    data_root: String,
    sessions: Vec<Session>,
    platform: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureCommit {
    tree: Tree,
    temporary_files: Vec<TemporaryFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemporaryFile {
    source_path: String,
    relative_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputeResponse {
    result: palmannotate_core::ComputationResult,
    quality: QualityReport,
    output: OutputV4,
    export_uri: String,
    export_files: Vec<ExportFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportFile {
    relative_path: String,
    source_path: String,
    mime_type: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DetectorResponse {
    boxes: Vec<palmannotate_core::BBox>,
    model: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DepthRenderResponse {
    path: String,
    width: u32,
    height: u32,
    minimum: f32,
    maximum: f32,
}

#[tauri::command]
fn bootstrap(state: State<'_, AppState>) -> CommandResult<Bootstrap> {
    let store = state.store.lock().map_err(|_| ErrorPayload {
        code: "state_poisoned",
        message: "Application storage lock is unavailable.".into(),
        recoverable: true,
    })?;
    command(store.list_sessions()).map(|sessions| Bootstrap {
        product_name: "PalmAnnotate",
        schema_version: palmannotate_core::SCHEMA_VERSION,
        data_root: store.root().to_string_lossy().into_owned(),
        sessions,
        platform: if cfg!(target_os = "android") {
            "android"
        } else {
            "desktop"
        },
    })
}

#[tauri::command]
fn session_list(state: State<'_, AppState>) -> CommandResult<Vec<Session>> {
    let store = lock_store(&state)?;
    command(store.list_sessions())
}

#[tauri::command]
fn session_save(state: State<'_, AppState>, session: Session) -> CommandResult<Session> {
    let store = lock_store(&state)?;
    command(store.save_session(&session))
}

#[tauri::command]
fn session_delete(state: State<'_, AppState>, session_id: String) -> CommandResult<Vec<Session>> {
    let store = lock_store(&state)?;
    command(store.delete_session(&session_id))
}

#[tauri::command]
fn sessions_import(
    state: State<'_, AppState>,
    sessions: Vec<Session>,
) -> CommandResult<Vec<Session>> {
    let store = lock_store(&state)?;
    command(store.import_sessions(sessions))
}

#[tauri::command]
fn sessions_import_folder(
    state: State<'_, AppState>,
    folder_path: String,
    export_uri: String,
) -> CommandResult<Vec<Session>> {
    let store = lock_store(&state)?;
    command(import_folder(&store, Path::new(&folder_path), &export_uri))
}

#[tauri::command]
fn tree_load(state: State<'_, AppState>, tree_id: String) -> CommandResult<Tree> {
    let store = lock_store(&state)?;
    command(store.load_tree(&tree_id))
}

#[tauri::command]
fn tree_save(state: State<'_, AppState>, tree: Tree) -> CommandResult<Tree> {
    let store = lock_store(&state)?;
    command(store.save_tree(&tree))?;
    Ok(tree)
}

#[tauri::command]
fn tree_delete(state: State<'_, AppState>, tree_id: String) -> CommandResult<()> {
    let store = lock_store(&state)?;
    command(store.delete_tree(&tree_id))
}

#[tauri::command]
fn capture_commit(state: State<'_, AppState>, mut request: CaptureCommit) -> CommandResult<Tree> {
    let store = lock_store(&state)?;
    if request.temporary_files.len() < request.tree.side_count {
        return Err(ErrorPayload {
            code: "capture_side_mismatch",
            message: format!(
                "Tree declares {} sides but capture supplied only {} files.",
                request.tree.side_count,
                request.temporary_files.len()
            ),
            recoverable: true,
        });
    }
    let dataset = store.root().join("dataset");
    let staging = store
        .root()
        .join("snapshots")
        .join(format!("capture-{}", request.tree.id));
    if staging.exists() {
        command(fs::remove_dir_all(&staging).map_err(AppError::from))?;
    }
    command(fs::create_dir_all(&staging).map_err(AppError::from))?;

    let mut supplied = HashSet::new();
    let mut staged = Vec::with_capacity(request.temporary_files.len());
    for (index, item) in request.temporary_files.iter().enumerate() {
        let relative = safe_relative_path(&item.relative_path)?;
        if !supplied.insert(relative.clone()) {
            let _ = fs::remove_dir_all(&staging);
            return Err(ErrorPayload {
                code: "capture_path_duplicate",
                message: format!("Capture supplied {} more than once.", item.relative_path),
                recoverable: true,
            });
        }
        let source = Path::new(&item.source_path);
        if !source.is_file() {
            let _ = fs::remove_dir_all(&staging);
            return Err(ErrorPayload {
                code: "capture_file_missing",
                message: format!("Captured side {} is no longer available.", index + 1),
                recoverable: true,
            });
        }
        let staged_path = staging.join(format!("side-{index}.tmp"));
        if let Err(error) = fs::copy(source, &staged_path) {
            let _ = fs::remove_dir_all(&staging);
            return command(Err(AppError::from(error)));
        }
        staged.push((source.to_path_buf(), staged_path, dataset.join(relative)));
    }
    for side in &request.tree.sides {
        let image = safe_relative_path(&side.image_path)?;
        if !supplied.contains(&image) {
            let _ = fs::remove_dir_all(&staging);
            return Err(ErrorPayload {
                code: "capture_image_missing",
                message: format!(
                    "Side {} has no matching captured image.",
                    side.side_index + 1
                ),
                recoverable: true,
            });
        }
        if let Some(depth_path) = &side.depth_path {
            let depth = safe_relative_path(depth_path)?;
            let depth_metadata = PathBuf::from(format!("{depth_path}.json"));
            if !supplied.contains(&depth) || !supplied.contains(&depth_metadata) {
                let _ = fs::remove_dir_all(&staging);
                return Err(ErrorPayload {
                    code: "capture_depth_missing",
                    message: format!(
                        "Side {} depth RAW or metadata file is missing.",
                        side.side_index + 1
                    ),
                    recoverable: true,
                });
            }
        }
    }

    let old_tree = store.load_tree(&request.tree.id).ok();
    let old_dataset = if let Some(tree) = old_tree.as_ref() {
        command(backup_tree_dataset(&dataset, tree, &staging).map_err(AppError::from))?
    } else {
        Vec::new()
    };
    if old_tree.is_some() {
        command(store.delete_tree(&request.tree.id))?;
    }

    // Stamp a fresh per-capture cache-bust token on every side so a reused tree
    // id can never surface a stale WebView-cached photo (matches JS capture-flow).
    let bust = chrono::Utc::now().timestamp_millis();
    for side in &mut request.tree.sides {
        side.cache_bust = Some(format!("{bust}_{}", side.side_index + 1));
    }

    let mut installed = Vec::with_capacity(staged.len());
    let install_result = (|| -> Result<(), AppError> {
        for (_, staged_path, destination) in &staged {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            if destination.exists() {
                return Err(AppError::Conflict(format!(
                    "Capture destination {} already exists.",
                    destination.display()
                )));
            }
            fs::rename(staged_path, destination)?;
            installed.push(destination.clone());
        }
        store.save_tree(&request.tree)
    })();
    if let Err(error) = install_result {
        for path in installed {
            let _ = fs::remove_file(path);
        }
        if let Some(tree) = old_tree {
            for (backup, destination) in old_dataset {
                if let Some(parent) = destination.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::copy(backup, destination);
            }
            let _ = store.save_tree(&tree);
        }
        let _ = fs::remove_dir_all(&staging);
        return command(Err(error));
    }
    for (source, _, _) in staged {
        let _ = fs::remove_file(source);
    }
    let _ = fs::remove_dir_all(staging);
    Ok(request.tree)
}

fn backup_tree_dataset(
    dataset: &Path,
    tree: &Tree,
    staging: &Path,
) -> std::io::Result<Vec<(PathBuf, PathBuf)>> {
    let mut relative_paths = HashSet::new();
    for side in &tree.sides {
        relative_paths.insert(PathBuf::from(&side.image_path));
        if let Some(depth_path) = &side.depth_path {
            relative_paths.insert(PathBuf::from(depth_path));
            relative_paths.insert(PathBuf::from(format!("{depth_path}.json")));
        }
    }
    let mut backups = Vec::new();
    for relative in relative_paths {
        let source = dataset.join(&relative);
        if source.is_file() {
            let backup = staging.join("old-dataset").join(&relative);
            if let Some(parent) = backup.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source, &backup)?;
            backups.push((backup, source));
        }
    }
    Ok(backups)
}

#[tauri::command]
fn tree_compute(state: State<'_, AppState>, tree_id: String) -> CommandResult<ComputeResponse> {
    let store = lock_store(&state)?;
    let mut tree = command(store.load_tree(&tree_id))?;
    let result = compute_results(&tree);
    let quality = check_tree(&tree);
    if quality.ready && result.unassigned_count == 0 {
        tree.status = palmannotate_core::TreeStatus::Complete;
        command(store.save_tree(&tree))?;
    }
    let sessions = command(store.list_sessions())?;
    let export_uri = sessions
        .iter()
        .find(|session| session.id == tree.session_id)
        .map(|session| session.export_uri.clone())
        .ok_or_else(|| ErrorPayload {
            code: "export_session_missing",
            message: "The tree session is missing; exports remain local.".into(),
            recoverable: true,
        })?;
    let export_files = command(write_tree_exports(&store, &tree, &result))?;
    Ok(ComputeResponse {
        result,
        quality,
        output: build_output_v4(&tree),
        export_uri,
        export_files,
    })
}

fn write_tree_exports(
    store: &AppStore,
    tree: &Tree,
    result: &palmannotate_core::ComputationResult,
) -> Result<Vec<ExportFile>, AppError> {
    let mut files = Vec::new();
    let csv = format!(
        "tree_name,split,unique,raw,B1,B2,B3,B4\n{},{},{},{},{},{},{},{}",
        tree.tree_name,
        tree.split,
        result.unique_count,
        result.raw_count,
        result.class_counts.get("B1").copied().unwrap_or(0),
        result.class_counts.get("B2").copied().unwrap_or(0),
        result.class_counts.get("B3").copied().unwrap_or(0),
        result.class_counts.get("B4").copied().unwrap_or(0),
    );
    save_export_file(
        store,
        &mut files,
        format!("{}_result.csv", tree.tree_name),
        csv.as_bytes(),
        "text/csv",
    )?;

    let session_json = serde_json::to_vec_pretty(&serde_json::json!({
        "version": tree.version,
        "tree": tree,
        "result": result,
        "exportedAt": chrono::Utc::now().to_rfc3339(),
    }))?;
    save_export_file(
        store,
        &mut files,
        format!("{}_session.json", tree.tree_name),
        &session_json,
        "application/json",
    )?;

    let bunches = result
        .clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            let detections = cluster
                .members
                .iter()
                .map(|&(side_index, box_index)| {
                    let side = &tree.sides[side_index];
                    let bbox = &side.bboxes[box_index];
                    serde_json::json!({
                        "side": side_index,
                        "sideName": side.label,
                        "bboxId": bbox.id,
                        "class": bbox.class_name,
                        "coords": [bbox.x1, bbox.y1, bbox.x2, bbox.y2],
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "id": index + 1,
                "classMismatch": cluster.class_mismatch,
                "detections": detections,
            })
        })
        .collect::<Vec<_>>();
    let identity = serde_json::to_vec_pretty(&serde_json::json!({
        "tree_name": tree.tree_name,
        "exportedAt": chrono::Utc::now().to_rfc3339(),
        "totalUniqueBunches": bunches.len(),
        "classMismatchCount": result.clusters.iter().filter(|cluster| cluster.class_mismatch).count(),
        "bunches": bunches,
    }))?;
    save_export_file(
        store,
        &mut files,
        format!("{}_identity.json", tree.tree_name),
        &identity,
        "application/json",
    )?;

    let mismatch_members = result
        .clusters
        .iter()
        .filter(|cluster| cluster.class_mismatch)
        .flat_map(|cluster| cluster.members.iter().copied())
        .collect::<HashSet<_>>();
    for side in &tree.sides {
        let (normal, mismatch): (Vec<_>, Vec<_>) =
            side.bboxes.iter().enumerate().partition(|(box_index, _)| {
                !mismatch_members.contains(&(side.side_index, *box_index))
            });
        let normal = normal
            .into_iter()
            .map(|(_, bbox)| bbox.clone())
            .collect::<Vec<_>>();
        let normal_text =
            palmannotate_core::serialize_yolo(&normal, side.image_width, side.image_height);
        save_export_file(
            store,
            &mut files,
            format!("{}_{}.txt", tree.tree_name, side.side_index + 1),
            normal_text.as_bytes(),
            "text/plain",
        )?;
        if !mismatch.is_empty() {
            let mismatch = mismatch
                .into_iter()
                .map(|(_, bbox)| bbox.clone())
                .collect::<Vec<_>>();
            let mismatch_text =
                palmannotate_core::serialize_yolo(&mismatch, side.image_width, side.image_height);
            save_export_file(
                store,
                &mut files,
                format!("{}_{}_mismatch.txt", tree.tree_name, side.side_index + 1),
                mismatch_text.as_bytes(),
                "text/plain",
            )?;
        }
    }

    files.push(ExportFile {
        relative_path: format!("Output JSON/{}.json", tree.tree_name),
        source_path: store
            .root()
            .join("Output JSON")
            .join(format!("{}.json", tree.tree_name))
            .to_string_lossy()
            .into_owned(),
        mime_type: "application/json",
    });
    for side in &tree.sides {
        files.push(ExportFile {
            relative_path: format!("Output TXT/{}_{}.txt", tree.tree_name, side.side_index + 1),
            source_path: store
                .root()
                .join("Output TXT")
                .join(format!("{}_{}.txt", tree.tree_name, side.side_index + 1))
                .to_string_lossy()
                .into_owned(),
            mime_type: "text/plain",
        });
    }
    Ok(files)
}

fn save_export_file(
    store: &AppStore,
    files: &mut Vec<ExportFile>,
    filename: String,
    data: &[u8],
    mime_type: &'static str,
) -> Result<(), AppError> {
    let path = store.save_export(&filename, data)?;
    files.push(ExportFile {
        relative_path: format!("exports/{filename}"),
        source_path: path.to_string_lossy().into_owned(),
        mime_type,
    });
    Ok(())
}

#[tauri::command]
fn tree_suggest(state: State<'_, AppState>, tree_id: String) -> CommandResult<Vec<LinkSuggestion>> {
    let store = lock_store(&state)?;
    let tree = command(store.load_tree(&tree_id))?;
    Ok(suggest_tree(&tree))
}

#[tauri::command]
async fn detector_run(
    state: State<'_, AppState>,
    image_path: String,
) -> CommandResult<DetectorResponse> {
    let path = {
        let store = lock_store(&state)?;
        let relative = safe_relative_path(&image_path)?;
        store.root().join("dataset").join(relative)
    };
    if !path.is_file() {
        return Err(ErrorPayload {
            code: "detector_image_missing",
            message: "The selected tree image is missing from local storage.".into(),
            recoverable: true,
        });
    }
    #[cfg(target_os = "android")]
    {
        tauri::async_runtime::spawn_blocking(move || detector::run(&path.to_string_lossy()))
            .await
            .map_err(background_error)?
    }
    #[cfg(not(target_os = "android"))]
    {
        Err(ErrorPayload {
            code: "detector_unavailable",
            message: "Offline detector is packaged for the Android arm64 build.".into(),
            recoverable: true,
        })
    }
}

#[tauri::command]
async fn depth_render(
    state: State<'_, AppState>,
    tree_id: String,
    side_index: usize,
) -> CommandResult<DepthRenderResponse> {
    let (source, output, width, height, value_scale) = {
        let store = lock_store(&state)?;
        let tree = command(store.load_tree(&tree_id))?;
        let side = tree.sides.get(side_index).ok_or_else(|| ErrorPayload {
            code: "depth_side_missing",
            message: "The selected tree side does not exist.".into(),
            recoverable: true,
        })?;
        let depth_path = side.depth_path.as_ref().ok_or_else(|| ErrorPayload {
            code: "depth_missing",
            message: "The selected side has no depth capture.".into(),
            recoverable: true,
        })?;
        let metadata = side.depth.as_ref().ok_or_else(|| ErrorPayload {
            code: "depth_metadata_missing",
            message: "Depth dimensions and scale are missing.".into(),
            recoverable: true,
        })?;
        let relative = safe_relative_path(depth_path)?;
        (
            store.root().join("dataset").join(relative),
            store.root().join("snapshots").join(format!(
                "{}-side-{}-depth.png",
                tree.id,
                side_index + 1
            )),
            metadata.width,
            metadata.height,
            metadata.value_scale,
        )
    };
    #[cfg(target_os = "android")]
    {
        tauri::async_runtime::spawn_blocking(move || {
            render_depth_response(source, output, width, height, value_scale)
        })
        .await
        .map_err(background_error)?
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (source, output, width, height, value_scale);
        Err(ErrorPayload {
            code: "depth_unavailable",
            message: "Depth rendering is available in the Android build.".into(),
            recoverable: true,
        })
    }
}

#[cfg(target_os = "android")]
fn render_depth_response(
    source: PathBuf,
    output: PathBuf,
    width: u32,
    height: u32,
    value_scale: f32,
) -> CommandResult<DepthRenderResponse> {
    let range = render_depth_png(&source, &output, width, height, value_scale)?;
    Ok(DepthRenderResponse {
        path: output.to_string_lossy().into_owned(),
        width,
        height,
        minimum: range.minimum_mm,
        maximum: range.maximum_mm,
    })
}

#[cfg(target_os = "android")]
fn background_error(error: impl std::fmt::Display) -> ErrorPayload {
    ErrorPayload {
        code: "background_task_failed",
        message: format!("Background operation failed: {error}"),
        recoverable: true,
    }
}

#[cfg(target_os = "android")]
fn render_depth_png(
    source: &Path,
    output: &Path,
    width: u32,
    height: u32,
    value_scale: f32,
) -> CommandResult<palmannotate_core::DepthDisplayRange> {
    let bytes = command(fs::read(source).map_err(AppError::from))?;
    let expected = width as usize * height as usize;
    if bytes.len() < expected * 2 {
        return Err(ErrorPayload {
            code: "depth_truncated",
            message: "Depth RAW file is shorter than its declared dimensions.".into(),
            recoverable: true,
        });
    }
    let values = bytes
        .chunks_exact(2)
        .take(expected)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let range = palmannotate_core::depth_display_range(&values, value_scale);
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        let value = values[(y * width + x) as usize];
        image::Rgb(palmannotate_core::depth_color(
            f32::from(value) * value_scale,
            range.minimum_mm,
            range.maximum_mm,
        ))
    });
    command(
        image
            .save(output)
            .map_err(|error| AppError::Io(std::io::Error::other(error.to_string()))),
    )?;
    Ok(range)
}

fn safe_relative_path(value: &str) -> CommandResult<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(ErrorPayload {
            code: "invalid_path",
            message: "Relative path must stay inside PalmAnnotate storage.".into(),
            recoverable: true,
        });
    }
    Ok(path.to_path_buf())
}

fn import_folder(
    store: &AppStore,
    source: &Path,
    export_uri: &str,
) -> Result<Vec<Session>, AppError> {
    if export_uri.trim().is_empty() {
        return Err(AppError::Validation(
            "Imported sessions require a writable SAF folder.".into(),
        ));
    }
    let manifest_path = source.join("sessions.json");
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let session_values = manifest
        .as_array()
        .or_else(|| {
            manifest
                .get("sessions")
                .and_then(serde_json::Value::as_array)
        })
        .ok_or_else(|| AppError::Validation("sessions.json has no sessions array.".into()))?;
    let existing = store.list_sessions()?;
    let mut imported_sessions = Vec::new();
    let mut imported_trees = Vec::new();
    let mut copies = Vec::<(PathBuf, PathBuf)>::new();

    for value in session_values {
        let id = json_string(value, &["id"])
            .ok_or_else(|| AppError::Validation("Imported session is missing id.".into()))?;
        if existing.iter().any(|session| session.id == id)
            || imported_sessions
                .iter()
                .any(|session: &Session| session.id == id)
        {
            return Err(AppError::Conflict(format!(
                "Session id {id} already exists; import does not overwrite."
            )));
        }
        let variety = json_string(value, &["variety"]).unwrap_or_else(|| "UNKNOWN".into());
        let block = json_string(value, &["block", "blok"]).unwrap_or_default();
        let side_count = json_usize(value, &["sideCount", "side_count"]).unwrap_or(4);
        let tree_values = value
            .get("trees")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut session = Session {
            id: id.clone(),
            name: json_string(value, &["name"]).unwrap_or_else(|| format!("{variety} / {block}")),
            variety: variety.clone(),
            block: block.clone(),
            group_key: String::new(),
            side_count,
            auto_id: value
                .get("autoId")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
            next_id: json_usize(value, &["nextId"]).unwrap_or(1),
            operator: json_string(value, &["operator"]).unwrap_or_default(),
            export_uri: export_uri.into(),
            created_at: json_string(value, &["createdAt"]).unwrap_or_default(),
            updated_at: json_string(value, &["updatedAt"]).unwrap_or_default(),
            trees: vec![],
        };

        for (position, tree_value) in tree_values.iter().enumerate() {
            let tree_name = json_string(tree_value, &["treeName", "name"])
                .ok_or_else(|| AppError::Validation("Imported tree is missing name.".into()))?;
            let tree_number = json_usize(tree_value, &["treeId"]).unwrap_or(position + 1);
            let tree_id = json_string(tree_value, &["id"])
                .unwrap_or_else(|| format!("{}-tree-{tree_number}", session.id));
            if store.load_tree(&tree_id).is_ok()
                || imported_trees.iter().any(|tree: &Tree| tree.id == tree_id)
            {
                return Err(AppError::Conflict(format!(
                    "Tree id {tree_id} already exists; import does not overwrite."
                )));
            }
            let output_path = source.join("Output JSON").join(format!("{tree_name}.json"));
            let mut tree = if output_path.is_file() {
                let output: OutputV4 = serde_json::from_slice(&fs::read(output_path)?)?;
                load_output_v4(output, tree_id, session.id.clone())?
            } else {
                import_unannotated_tree(source, &session, tree_value, tree_id, &tree_name)?
            };
            tree.metadata.variety = variety.clone();
            tree.metadata.block = block.clone();
            if tree.metadata.operator.is_empty() {
                tree.metadata.operator = session.operator.clone();
            }
            for side in &mut tree.sides {
                let relative = format!("images/field/{tree_name}_{}.jpg", side.side_index + 1);
                let source_image = source.join("dataset").join(&relative);
                if !source_image.is_file() {
                    return Err(AppError::NotFound(format!(
                        "Imported tree {tree_name} references missing {relative}."
                    )));
                }
                side.image_path = relative.clone();
                copies.push((source_image, store.root().join("dataset").join(relative)));
                let depth_relative = format!("depth/field/{tree_name}_{}.raw", side.side_index + 1);
                let source_depth = source.join("dataset").join(&depth_relative);
                if source_depth.is_file() {
                    side.depth_path = Some(depth_relative.clone());
                    copies.push((
                        source_depth,
                        store.root().join("dataset").join(&depth_relative),
                    ));
                    let metadata_relative = format!("{depth_relative}.json");
                    let source_metadata = source.join("dataset").join(&metadata_relative);
                    if source_metadata.is_file() {
                        copies.push((
                            source_metadata,
                            store.root().join("dataset").join(metadata_relative),
                        ));
                    }
                }
            }
            imported_trees.push(tree);
        }
        session.next_id = imported_trees
            .iter()
            .filter(|tree| tree.session_id == session.id)
            .map(|tree| {
                tree.tree_name
                    .rsplit('_')
                    .next()
                    .and_then(|part| part.parse().ok())
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
            + 1;
        imported_sessions.push(session);
    }

    for (source_path, destination) in &copies {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source_path, destination)?;
    }
    store.import_sessions(imported_sessions)?;
    for tree in imported_trees {
        store.save_tree(&tree)?;
    }
    store.list_sessions()
}

fn import_unannotated_tree(
    source: &Path,
    session: &Session,
    value: &serde_json::Value,
    id: String,
    tree_name: &str,
) -> Result<Tree, AppError> {
    let side_count = json_usize(value, &["sideCount"]).unwrap_or(session.side_count);
    let mut sides = Vec::with_capacity(side_count);
    for side_index in 0..side_count {
        let relative = format!("images/field/{tree_name}_{}.jpg", side_index + 1);
        let image_path = source.join("dataset").join(&relative);
        let (width, height) = image::image_dimensions(&image_path)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let label_path = source
            .join("dataset")
            .join("labels")
            .join("field")
            .join(format!("{tree_name}_{}.txt", side_index + 1));
        let bboxes = if label_path.is_file() {
            parse_yolo(&fs::read_to_string(label_path)?, width, height)
        } else {
            vec![]
        };
        sides.push(Side {
            side_index,
            label: format!("Side {}", side_index + 1),
            image_path: relative,
            image_width: width,
            image_height: height,
            depth_path: None,
            depth: None,
            original_bboxes: Vec::new(),
            cache_bust: None,
            bboxes,
        });
    }
    Ok(Tree {
        version: palmannotate_core::SCHEMA_VERSION,
        id,
        session_id: session.id.clone(),
        tree_name: tree_name.into(),
        split: "field".into(),
        side_count,
        metadata: TreeMetadata {
            variety: session.variety.clone(),
            block: session.block.clone(),
            operator: session.operator.clone(),
            timestamp: json_string(value, &["capturedAt", "createdAt"]).unwrap_or_default(),
            gps: None,
        },
        sides,
        confirmed_links: vec![],
        status: TreeStatus::Captured,
    })
}

fn json_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key)?.as_str().map(str::to_owned))
}

fn json_usize(value: &serde_json::Value, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|key| {
        value
            .get(*key)?
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
    })
}

fn lock_store<'a>(
    state: &'a State<'_, AppState>,
) -> CommandResult<std::sync::MutexGuard<'a, AppStore>> {
    state.store.lock().map_err(|_| ErrorPayload {
        code: "state_poisoned",
        message: "Application storage lock is unavailable.".into(),
        recoverable: true,
    })
}

#[cfg(target_os = "android")]
mod detector {
    use super::{CommandResult, DetectorResponse};
    use image::{imageops, DynamicImage, ImageBuffer, Rgb};
    use ndarray::Array4;
    use ort::{session::Session, value::TensorRef};
    use palmannotate_core::{decode_yolo, DetectorConfig, ErrorPayload, Letterbox};

    const MODEL: &[u8] = include_bytes!("../../models/ffb-detector.onnx");

    pub fn run(image_path: &str) -> CommandResult<DetectorResponse> {
        let image = image::open(image_path)
            .map_err(|error| detector_error("detector_image", error.to_string()))?;
        let config = DetectorConfig::default();
        let letterbox = Letterbox::new(image.width(), image.height(), config.input_size)
            .ok_or_else(|| detector_error("detector_image", "Image dimensions are invalid."))?;
        let input = preprocess(&image, letterbox, config.input_size);
        let mut session = Session::builder()
            .and_then(|mut builder| builder.commit_from_memory(MODEL))
            .map_err(|error| detector_error("detector_model", error.to_string()))?;
        let outputs = session
            .run(ort::inputs![TensorRef::from_array_view(&input).map_err(
                |error| detector_error("detector_input", error.to_string())
            )?])
            .map_err(|error| detector_error("detector_inference", error.to_string()))?;
        let output = outputs
            .values()
            .next()
            .ok_or_else(|| detector_error("detector_output", "Model returned no output tensor."))?;
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|error| detector_error("detector_output", error.to_string()))?;
        let dimensions = shape
            .iter()
            .map(|dimension| usize::try_from(*dimension).unwrap_or(0))
            .collect::<Vec<_>>();
        Ok(DetectorResponse {
            boxes: decode_yolo(data, &dimensions, letterbox, &config),
            model: "ffb-detector.onnx",
        })
    }

    fn preprocess(image: &DynamicImage, letterbox: Letterbox, size: usize) -> Array4<f32> {
        let rgb = image.to_rgb8();
        let resized_width = (image.width() as f32 * letterbox.scale).round() as u32;
        let resized_height = (image.height() as f32 * letterbox.scale).round() as u32;
        let resized = imageops::resize(
            &rgb,
            resized_width,
            resized_height,
            imageops::FilterType::Triangle,
        );
        let mut canvas = ImageBuffer::from_pixel(size as u32, size as u32, Rgb([114, 114, 114]));
        imageops::replace(
            &mut canvas,
            &resized,
            letterbox.pad_x as i64,
            letterbox.pad_y as i64,
        );
        let mut tensor = Array4::<f32>::zeros((1, 3, size, size));
        for (x, y, pixel) in canvas.enumerate_pixels() {
            tensor[[0, 0, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
            tensor[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            tensor[[0, 2, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
        }
        tensor
    }

    fn detector_error(code: &'static str, message: impl Into<String>) -> ErrorPayload {
        ErrorPayload {
            code,
            message: message.into(),
            recoverable: true,
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_palm_native::init())
        .plugin(tauri_plugin_geolocation::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = AppStore::new(data_dir)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            app.manage(AppState {
                store: Mutex::new(store),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            session_list,
            session_save,
            session_delete,
            sessions_import,
            sessions_import_folder,
            tree_load,
            tree_save,
            tree_delete,
            capture_commit,
            tree_compute,
            tree_suggest,
            detector_run,
            depth_render
        ])
        .run(tauri::generate_context!())
        .expect("error while running PalmAnnotate");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_tree() -> Tree {
        let sides = (0..4)
            .map(|side_index| Side {
                side_index,
                label: format!("Side {}", side_index + 1),
                image_path: format!("images/field/DAMIMAS_A21B_0001_{}.jpg", side_index + 1),
                image_width: 100,
                image_height: 100,
                depth_path: None,
                depth: None,
                original_bboxes: Vec::new(),
                cache_bust: None,
                bboxes: vec![palmannotate_core::BBox {
                    id: format!("b{side_index}"),
                    class_id: if side_index == 1 { 2 } else { 1 },
                    class_name: if side_index == 1 { "B3" } else { "B2" }.into(),
                    x1: 10.0,
                    y1: 10.0,
                    x2: 20.0,
                    y2: 20.0,
                    confidence: None,
                }],
            })
            .collect();
        Tree {
            version: palmannotate_core::SCHEMA_VERSION,
            id: "tree-1".into(),
            session_id: "session-1".into(),
            tree_name: "DAMIMAS_A21B_0001".into(),
            split: "field".into(),
            side_count: 4,
            metadata: TreeMetadata {
                variety: "DAMIMAS".into(),
                block: "A21B".into(),
                timestamp: "2026-06-07T00:00:00Z".into(),
                ..Default::default()
            },
            sides,
            confirmed_links: vec![palmannotate_core::ConfirmedLink {
                link_id: "L0".into(),
                side_a: 0,
                bbox_id_a: "b0".into(),
                side_b: 1,
                bbox_id_b: "b1".into(),
            }],
            status: TreeStatus::Annotated,
        }
    }

    #[test]
    fn writes_csv_session_identity_and_mismatch_exports() {
        let temp = tempfile::tempdir().unwrap();
        let store = AppStore::new(temp.path()).unwrap();
        store
            .save_session(&Session {
                id: "session-1".into(),
                name: "DAMIMAS / A21B".into(),
                variety: "DAMIMAS".into(),
                block: "A21B".into(),
                group_key: String::new(),
                side_count: 4,
                auto_id: true,
                next_id: 1,
                operator: String::new(),
                export_uri: "content://export".into(),
                created_at: String::new(),
                updated_at: String::new(),
                trees: vec![],
            })
            .unwrap();
        let tree = export_tree();
        store.save_tree(&tree).unwrap();
        let result = compute_results(&tree);
        let files = write_tree_exports(&store, &tree, &result).unwrap();

        assert!(files
            .iter()
            .any(|file| file.relative_path.ends_with("_result.csv")));
        assert!(files
            .iter()
            .any(|file| file.relative_path.ends_with("_session.json")));
        assert!(files
            .iter()
            .any(|file| file.relative_path.ends_with("_identity.json")));
        assert!(files
            .iter()
            .any(|file| file.relative_path.ends_with("_1_mismatch.txt")));
        let csv =
            fs::read_to_string(store.root().join("exports/DAMIMAS_A21B_0001_result.csv")).unwrap();
        assert!(csv.contains("DAMIMAS_A21B_0001,field,3,4"));
    }

    #[test]
    fn imports_legacy_saf_manifest_and_rejects_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("export");
        let images = source.join("dataset").join("images").join("field");
        fs::create_dir_all(&images).unwrap();
        for side in 1..=4 {
            image::RgbImage::from_pixel(16, 12, image::Rgb([side * 10, 20, 30]))
                .save(images.join(format!("DAMIMAS_A21B_0001_{side}.jpg")))
                .unwrap();
        }
        fs::write(
            source.join("sessions.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 1,
                "sessions": [{
                    "id": "legacy-session",
                    "variety": "DAMIMAS",
                    "blok": "A21B",
                    "sideCount": 4,
                    "operator": "Field operator",
                    "trees": [{
                        "name": "DAMIMAS_A21B_0001",
                        "treeId": 1,
                        "sideCount": 4
                    }]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let store = AppStore::new(temp.path().join("app")).unwrap();
        let sessions = import_folder(&store, &source, "content://legacy").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "legacy-session");
        assert_eq!(sessions[0].block, "A21B");
        assert_eq!(sessions[0].trees.len(), 1);
        let tree = store.load_tree("legacy-session-tree-1").unwrap();
        assert_eq!(tree.sides.len(), 4);
        assert_eq!(tree.sides[0].image_width, 16);
        assert!(store
            .root()
            .join("dataset/images/field/DAMIMAS_A21B_0001_1.jpg")
            .is_file());

        assert!(matches!(
            import_folder(&store, &source, "content://legacy"),
            Err(AppError::Conflict(_))
        ));
    }
}
