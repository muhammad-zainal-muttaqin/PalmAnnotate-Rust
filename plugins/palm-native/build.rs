const COMMANDS: &[&str] = &[
    "camera_status",
    "camera_start",
    "camera_capture",
    "camera_stop",
    "temp_delete",
    "orbbec_status",
    "orbbec_list",
    "orbbec_request_permission",
    "orbbec_open",
    "orbbec_capture",
    "orbbec_close",
    "saf_pick_folder",
    "saf_release_folder",
    "saf_list",
    "saf_read_to_temp",
    "saf_copy_tree_to_temp",
    "saf_copy_from_path",
    "saf_write",
    "saf_exists",
    "saf_delete",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
