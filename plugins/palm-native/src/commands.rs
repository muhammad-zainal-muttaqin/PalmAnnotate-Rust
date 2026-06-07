use tauri::{AppHandle, Runtime};

use crate::{
    Empty, JsonResponse, PalmNativeExt, PathRequest, Result, SafCopyRequest, SafPathRequest,
    TreeRequest,
};

macro_rules! empty_command {
    ($name:ident) => {
        #[tauri::command]
        pub(crate) async fn $name<R: Runtime>(app: AppHandle<R>) -> Result<JsonResponse> {
            app.palm_native().run(stringify!($name), Empty::default())
        }
    };
}

empty_command!(camera_status);
empty_command!(camera_start);
empty_command!(camera_capture);
empty_command!(camera_stop);
empty_command!(orbbec_status);
empty_command!(orbbec_list);
empty_command!(orbbec_request_permission);
empty_command!(orbbec_open);
empty_command!(orbbec_capture);
empty_command!(orbbec_close);
empty_command!(orbbec_refresh);
empty_command!(saf_pick_folder);
empty_command!(saf_pick_json);

#[tauri::command]
pub(crate) async fn temp_delete<R: Runtime>(
    app: AppHandle<R>,
    payload: PathRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("temp_delete", payload)
}

#[tauri::command]
pub(crate) async fn saf_release_folder<R: Runtime>(
    app: AppHandle<R>,
    payload: TreeRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_release_folder", payload)
}

#[tauri::command]
pub(crate) async fn saf_validate<R: Runtime>(
    app: AppHandle<R>,
    payload: TreeRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_validate", payload)
}

#[tauri::command]
pub(crate) async fn saf_list<R: Runtime>(
    app: AppHandle<R>,
    payload: TreeRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_list", payload)
}

#[tauri::command]
pub(crate) async fn saf_read_to_temp<R: Runtime>(
    app: AppHandle<R>,
    payload: SafPathRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_read_to_temp", payload)
}

#[tauri::command]
pub(crate) async fn saf_copy_tree_to_temp<R: Runtime>(
    app: AppHandle<R>,
    payload: TreeRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_copy_tree_to_temp", payload)
}

#[tauri::command]
pub(crate) async fn saf_copy_from_path<R: Runtime>(
    app: AppHandle<R>,
    payload: SafCopyRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_copy_from_path", payload)
}

#[tauri::command]
pub(crate) async fn saf_write<R: Runtime>(
    app: AppHandle<R>,
    payload: SafCopyRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_write", payload)
}

#[tauri::command]
pub(crate) async fn saf_exists<R: Runtime>(
    app: AppHandle<R>,
    payload: SafPathRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_exists", payload)
}

#[tauri::command]
pub(crate) async fn saf_delete<R: Runtime>(
    app: AppHandle<R>,
    payload: SafPathRequest,
) -> Result<JsonResponse> {
    app.palm_native().run("saf_delete", payload)
}
