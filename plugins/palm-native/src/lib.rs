use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;
#[cfg(desktop)]
mod desktop;
mod error;
#[cfg(mobile)]
mod mobile;
mod models;

pub use error::{Error, Result};
pub use models::*;

#[cfg(desktop)]
use desktop::PalmNative;
#[cfg(mobile)]
use mobile::PalmNative;

pub trait PalmNativeExt<R: Runtime> {
    fn palm_native(&self) -> &PalmNative<R>;
}

impl<R: Runtime, T: Manager<R>> PalmNativeExt<R> for T {
    fn palm_native(&self) -> &PalmNative<R> {
        self.state::<PalmNative<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("palm-native")
        .invoke_handler(tauri::generate_handler![
            commands::camera_status,
            commands::camera_start,
            commands::camera_capture,
            commands::camera_stop,
            commands::temp_delete,
            commands::orbbec_status,
            commands::orbbec_list,
            commands::orbbec_request_permission,
            commands::orbbec_open,
            commands::orbbec_capture,
            commands::orbbec_close,
            commands::orbbec_refresh,
            commands::saf_pick_folder,
            commands::saf_pick_json,
            commands::saf_release_folder,
            commands::saf_validate,
            commands::saf_list,
            commands::saf_read_to_temp,
            commands::saf_copy_tree_to_temp,
            commands::saf_copy_from_path,
            commands::saf_write,
            commands::saf_exists,
            commands::saf_delete
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let plugin = mobile::init(app, api)?;
            #[cfg(desktop)]
            let plugin = desktop::init(app, api)?;
            app.manage(plugin);
            Ok(())
        })
        .build()
}
