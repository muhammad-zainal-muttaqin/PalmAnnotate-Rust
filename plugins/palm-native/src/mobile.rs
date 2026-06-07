use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::{JsonResponse, Result};

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<PalmNative<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(
        "dev.sawitulm.palmannotate.rust.nativebridge",
        "PalmNativePlugin",
    )?;
    Ok(PalmNative(handle))
}

pub struct PalmNative<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> PalmNative<R> {
    pub fn run<T: serde::Serialize>(&self, command: &str, payload: T) -> Result<JsonResponse> {
        self.0
            .run_mobile_plugin(command, payload)
            .map_err(Into::into)
    }
}
