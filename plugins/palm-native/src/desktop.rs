use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::{Error, JsonResponse, Result};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<PalmNative<R>> {
    Ok(PalmNative(app.clone()))
}

pub struct PalmNative<R: Runtime>(#[allow(dead_code)] AppHandle<R>);

impl<R: Runtime> PalmNative<R> {
    pub fn run<T: serde::Serialize>(&self, command: &str, _payload: T) -> Result<JsonResponse> {
        Err(Error::Unavailable(format!(
            "{command} is only available on Android."
        )))
    }
}
