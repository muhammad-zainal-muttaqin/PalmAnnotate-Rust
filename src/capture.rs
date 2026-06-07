use super::*;

const CAPTURE_VIDEO_ID: &str = "pa-capture-video";

/// Open the device camera as an in-page `getUserMedia` stream and attach it to
/// the live `<video>` element. This mirrors the JS app (capture-source.js): the
/// preview renders directly in the WebView, which the Android WebChromeClient
/// already grants camera permission for — no fragile native preview pump.
async fn open_web_camera() -> Result<web_sys::MediaStream, String> {
    let window = web_sys::window().ok_or("Browser window unavailable.")?;
    let media = window
        .navigator()
        .media_devices()
        .map_err(|_| "This WebView has no camera access (getUserMedia).".to_string())?;

    let video_constraints = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &video_constraints,
        &JsValue::from_str("facingMode"),
        &JsValue::from_str("environment"),
    );
    let width = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&width, &JsValue::from_str("ideal"), &JsValue::from_f64(1920.0));
    let _ = js_sys::Reflect::set(&video_constraints, &JsValue::from_str("width"), &width);
    let height = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&height, &JsValue::from_str("ideal"), &JsValue::from_f64(1080.0));
    let _ = js_sys::Reflect::set(&video_constraints, &JsValue::from_str("height"), &height);

    let constraints = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&constraints, &JsValue::from_str("audio"), &JsValue::FALSE);
    let _ = js_sys::Reflect::set(&constraints, &JsValue::from_str("video"), &video_constraints);
    let constraints: web_sys::MediaStreamConstraints = constraints.unchecked_into();

    let promise = media
        .get_user_media_with_constraints(&constraints)
        .map_err(|_| "Could not start the camera.".to_string())?;
    let stream_value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|_| "Camera permission was denied or no camera is available.".to_string())?;
    let stream: web_sys::MediaStream = stream_value
        .dyn_into()
        .map_err(|_| "The camera returned an unexpected stream.".to_string())?;

    if let Some(video) = web_camera_element() {
        video.set_src_object(Some(&stream));
        video.set_muted(true);
        let _ = video.play();
    }
    Ok(stream)
}

fn web_camera_element() -> Option<web_sys::HtmlVideoElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id(CAPTURE_VIDEO_ID)?
        .dyn_into::<web_sys::HtmlVideoElement>()
        .ok()
}

/// Grab the current live frame to an offscreen canvas and return base64 JPEG
/// plus the intrinsic dimensions (matches JS `grab()` at videoWidth/Height).
fn grab_web_frame() -> Result<(String, u32, u32), String> {
    let video = web_camera_element().ok_or("Camera preview is not ready.")?;
    let width = video.video_width();
    let height = video.video_height();
    if width == 0 || height == 0 {
        return Err("Camera is still warming up — try again.".into());
    }
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("Browser document unavailable.")?;
    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "Could not allocate a capture canvas.".to_string())?
        .dyn_into()
        .map_err(|_| "Could not allocate a capture canvas.".to_string())?;
    canvas.set_width(width);
    canvas.set_height(height);
    let context = canvas
        .get_context("2d")
        .map_err(|_| "Could not get a 2D drawing context.".to_string())?
        .ok_or("Could not get a 2D drawing context.")?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(|_| "Could not get a 2D drawing context.".to_string())?;
    context
        .draw_image_with_html_video_element_and_dw_and_dh(
            &video,
            0.0,
            0.0,
            f64::from(width),
            f64::from(height),
        )
        .map_err(|_| "Could not read a frame from the camera.".to_string())?;
    let data_url = canvas
        .to_data_url_with_type_and_encoder_options("image/jpeg", &JsValue::from_f64(0.92))
        .map_err(|_| "Could not encode the captured frame.".to_string())?;
    let base64 = data_url
        .split_once(',')
        .map(|(_, payload)| payload.to_string())
        .unwrap_or(data_url);
    Ok((base64, width, height))
}

fn stop_web_camera(stream: &web_sys::MediaStream) {
    let tracks = stream.get_tracks();
    for index in 0..tracks.length() {
        if let Ok(track) = tracks.get(index).dyn_into::<web_sys::MediaStreamTrack>() {
            track.stop();
        }
    }
    if let Some(video) = web_camera_element() {
        video.set_src_object(None);
    }
}

/// Persist a web-captured frame to a temp file via the Rust backend, returning a
/// `CapturedFrame` identical in shape to a native capture.
async fn save_web_frame(base64: String, width: u32, height: u32) -> Result<CapturedFrame, String> {
    let args = to_invoke_args(&serde_json::json!({
        "payload": { "base64": base64, "width": width, "height": height }
    }))
    .map_err(|error| error.to_string())?;
    let value = invoke("camera_save_frame", args).await.map_err(js_error)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

async fn close_capture_source(orbbec: bool) {
    let command = if orbbec {
        "plugin:palm-native|orbbec_close"
    } else {
        "plugin:palm-native|camera_stop"
    };
    let _ = native_empty::<serde_json::Value>(command).await;
}

async fn find_orbbec() -> Result<bool, String> {
    let value = match native_empty::<serde_json::Value>("plugin:palm-native|orbbec_refresh").await {
        Ok(value) => value,
        Err(_) => native_empty::<serde_json::Value>("plugin:palm-native|orbbec_status").await?,
    };
    Ok(value
        .get("available")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or_else(|| {
            value
                .get("count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                > 0
        }))
}

#[component]
pub(super) fn Capture(
    session: Option<Session>,
    existing: Option<PendingCapture>,
    retake_side: Option<usize>,
    on_cancel: EventHandler<MouseEvent>,
    on_complete: EventHandler<PendingCapture>,
) -> Element {
    let initial_frames = existing
        .as_ref()
        .map(|capture| capture.frames.clone())
        .unwrap_or_default();
    let mut frames = use_signal(move || initial_frames);
    let mut opened = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut capture_error = use_signal(|| None::<String>);
    let mut use_orbbec = use_signal(|| false);
    let mut orbbec_available = use_signal(|| false);
    let mut checking_sources = use_signal(|| false);
    let initial_tree_number = existing
        .as_ref()
        .map(|capture| capture.tree_number.to_string())
        .or_else(|| {
            session
                .as_ref()
                .map(|value| value.next_id.max(1).to_string())
        })
        .unwrap_or_else(|| "1".into());
    let mut manual_tree_number = use_signal(move || initial_tree_number);
    let mut preview = use_signal(|| None::<String>);
    let mut depth_preview = use_signal(|| None::<String>);
    let mut preview_callback = use_signal(|| None::<Closure<dyn FnMut(JsValue)>>);
    let mut preview_unlisten = use_signal(|| None::<js_sys::Function>);
    let mut orbbec_preview_callback = use_signal(|| None::<Closure<dyn FnMut(JsValue)>>);
    let mut orbbec_preview_unlisten = use_signal(|| None::<js_sys::Function>);
    let mut device_callback = use_signal(|| None::<Closure<dyn FnMut(JsValue)>>);
    let mut device_unlisten = use_signal(|| None::<js_sys::Function>);
    // Live device-camera stream (getUserMedia). Orbbec keeps the native pump.
    let mut web_stream = use_signal(|| None::<web_sys::MediaStream>);
    let expected = session.as_ref().map(|value| value.side_count).unwrap_or(0);
    let has_session = session.is_some();
    let manual_mode = session.as_ref().is_some_and(|value| !value.auto_id);
    let is_retake = retake_side.is_some();
    let current_side = retake_side.unwrap_or_else(|| frames.read().len());
    let existing_gps = existing.as_ref().and_then(|capture| capture.gps.clone());
    let retake_tree_number = existing
        .as_ref()
        .map(|capture| capture.tree_number)
        .unwrap_or(1);
    let start_session = session.clone();
    let shoot_session = session.clone();

    use_effect(move || {
        checking_sources.set(true);
        spawn(async move {
            match find_orbbec().await {
                Ok(available) => orbbec_available.set(available),
                Err(_) => orbbec_available.set(false),
            }
            checking_sources.set(false);
        });
    });

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

    use_effect(move || {
        let callback = Closure::<dyn FnMut(JsValue)>::new(move |value| {
            let Ok(event) = serde_wasm_bindgen::from_value::<serde_json::Value>(value) else {
                return;
            };
            let payload = event.get("payload").unwrap_or(&event);
            let available = payload
                .get("attached")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || payload
                    .get("count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    > 0;
            orbbec_available.set(available);
            if !available {
                use_orbbec.set(false);
                opened.set(false);
                preview.set(None);
                depth_preview.set(None);
                spawn(async move {
                    close_capture_source(true).await;
                });
            }
        });
        let function = callback
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        spawn(async move {
            if let Ok(value) = listen("orbbec-device-change", &function).await {
                if let Ok(unlisten) = value.dyn_into::<js_sys::Function>() {
                    device_unlisten.set(Some(unlisten));
                }
                device_callback.set(Some(callback));
            }
        });
    });

    use_drop(move || {
        if let Some(stream) = web_stream.write().take() {
            stop_web_camera(&stream);
        }
        if let Some(unlisten) = preview_unlisten.read().as_ref() {
            let _ = unlisten.call0(&JsValue::UNDEFINED);
        }
        if let Some(unlisten) = orbbec_preview_unlisten.read().as_ref() {
            let _ = unlisten.call0(&JsValue::UNDEFINED);
        }
        if let Some(unlisten) = device_unlisten.read().as_ref() {
            let _ = unlisten.call0(&JsValue::UNDEFINED);
        }
        wasm_bindgen_futures::spawn_local(async {
            close_capture_source(false).await;
            close_capture_source(true).await;
        });
    });

    let refresh_sources = move |_| {
        checking_sources.set(true);
        capture_error.set(None);
        spawn(async move {
            match find_orbbec().await {
                Ok(available) => {
                    orbbec_available.set(available);
                    if !available {
                        use_orbbec.set(false);
                    }
                }
                Err(message) => capture_error.set(Some(message)),
            }
            checking_sources.set(false);
        });
    };

    let start = move |_| {
        if start_session.as_ref().is_some_and(|value| !value.auto_id)
            && manual_tree_number
                .read()
                .parse::<usize>()
                .ok()
                .filter(|value| *value > 0)
                .is_none()
        {
            capture_error.set(Some("Enter a positive tree ID.".into()));
            return;
        }
        busy.set(true);
        capture_error.set(None);
        preview.set(None);
        depth_preview.set(None);
        spawn(async move {
            if *use_orbbec.read() {
                let result = async {
                    if !find_orbbec().await? {
                        return Err("No Orbbec camera found.".into());
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
                        return Err("Orbbec USB permission denied.".into());
                    }
                    native_empty::<serde_json::Value>("plugin:palm-native|orbbec_open").await
                }
                .await;
                match result {
                    Ok(_) => opened.set(true),
                    Err(message) => capture_error.set(Some(message)),
                }
            } else {
                match open_web_camera().await {
                    Ok(stream) => {
                        web_stream.set(Some(stream));
                        opened.set(true);
                    }
                    Err(message) => capture_error.set(Some(message)),
                }
            }
            busy.set(false);
        });
    };

    let shoot = move |_| {
        let Some(session) = shoot_session.clone() else {
            capture_error.set(Some("Open a session first.".into()));
            return;
        };
        let source_is_orbbec = *use_orbbec.read();
        let preserved_gps = existing_gps.clone();
        busy.set(true);
        capture_error.set(None);
        spawn(async move {
            let frame_result: Result<CapturedFrame, String> = if source_is_orbbec {
                native_empty::<CapturedFrame>("plugin:palm-native|orbbec_capture").await
            } else {
                match grab_web_frame() {
                    Ok((base64, width, height)) => save_web_frame(base64, width, height).await,
                    Err(message) => Err(message),
                }
            };
            match frame_result {
                Ok(frame) => {
                    if let Some(index) = retake_side {
                        if index >= frames.read().len() {
                            capture_error.set(Some("The selected side no longer exists.".into()));
                        } else {
                            let old = frames.read()[index].clone();
                            frames.write()[index] = frame;
                            close_capture_source(source_is_orbbec).await;
                            if !source_is_orbbec {
                                if let Some(stream) = web_stream.write().take() {
                                    stop_web_camera(&stream);
                                }
                            }
                            opened.set(false);
                            delete_temporary_frames(vec![old]).await;
                            on_complete.call(PendingCapture {
                                session,
                                tree_number: retake_tree_number,
                                frames: frames.read().clone(),
                                gps: preserved_gps,
                            });
                        }
                    } else {
                        frames.write().push(frame);
                        if frames.read().len() == session.side_count {
                            close_capture_source(source_is_orbbec).await;
                            if !source_is_orbbec {
                                if let Some(stream) = web_stream.write().take() {
                                    stop_web_camera(&stream);
                                }
                            }
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
                }
                Err(message) => capture_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let cancel = move |event: MouseEvent| {
        let source_is_orbbec = *use_orbbec.read();
        if let Some(stream) = web_stream.write().take() {
            stop_web_camera(&stream);
        }
        let temporary = if is_retake {
            Vec::new()
        } else {
            frames.read().clone()
        };
        spawn(async move {
            close_capture_source(source_is_orbbec).await;
            delete_temporary_frames(temporary).await;
            on_cancel.call(event);
        });
    };

    rsx! {
        div { class: "capture-screen",
            div { class: "capture-toolbar",
                button { class: "button media", disabled: *busy.read(), onclick: cancel, "Cancel" }
                strong { if is_retake { "Retake side {current_side + 1}" } else { "Side {current_side + 1} / {expected}" } }
                button {
                    class: "button media",
                    disabled: *opened.read() || *checking_sources.read(),
                    onclick: refresh_sources,
                    if *checking_sources.read() { "Finding..." } else { "Find camera" }
                }
            }
            if let Some(message) = capture_error.read().as_ref() {
                div { class: "capture-error", "{message}" }
            }
            div { class: "capture-live",
                if *use_orbbec.read() {
                    if let Some(source) = preview.read().as_ref() {
                        img { class: "capture-rgb", src: "{source}", alt: "Camera preview" }
                    }
                    if let Some(source) = depth_preview.read().as_ref() {
                        img { class: "capture-depth", src: "{source}", alt: "Depth preview" }
                    }
                    if preview.read().is_none() {
                        div { class: "capture-empty", if *opened.read() { "Camera ready" } else { "Camera" } }
                    }
                } else {
                    video {
                        id: CAPTURE_VIDEO_ID,
                        class: "capture-rgb",
                        autoplay: true,
                        muted: true,
                        "playsinline": "true",
                    }
                    if !*opened.read() {
                        div { class: "capture-empty", "Camera" }
                    }
                }
                div { class: "capture-controls",
                    select {
                        disabled: *opened.read() || *busy.read(),
                        value: if *use_orbbec.read() { "orbbec" } else { "camerax" },
                        onchange: move |event| use_orbbec.set(event.value() == "orbbec"),
                        option { value: "camerax", "Device camera" }
                        if *orbbec_available.read() {
                            option { value: "orbbec", "Orbbec RGB-D" }
                        }
                    }
                    if manual_mode {
                        input {
                            class: "capture-idinput",
                            r#type: "number",
                            min: "1",
                            disabled: *opened.read(),
                            value: "{manual_tree_number}",
                            aria_label: "Tree ID",
                            oninput: move |event| manual_tree_number.set(event.value())
                        }
                    }
                    if !*opened.read() {
                        button {
                            class: "shutter open",
                            disabled: *busy.read() || !has_session,
                            onclick: start,
                            "Open"
                        }
                    } else {
                        button {
                            class: "shutter",
                            disabled: *busy.read(),
                            onclick: shoot,
                            span {}
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn Review(
    capture: Option<PendingCapture>,
    data_root: String,
    on_retake: EventHandler<usize>,
    on_retake_all: EventHandler<MouseEvent>,
    on_cancel: EventHandler<MouseEvent>,
    on_committed: EventHandler<CommitOutcome>,
) -> Element {
    let initial_gps = capture.as_ref().and_then(|value| value.gps.clone());
    let mut gps = use_signal(move || initial_gps);
    let mut active_side = use_signal(|| 0_usize);
    let mut busy = use_signal(|| false);
    let mut review_error = use_signal(|| None::<String>);
    let mut swipe = use_signal(|| None::<SwipeGesture>);
    let frames = capture
        .as_ref()
        .map(|value| value.frames.clone())
        .unwrap_or_default();
    let count = frames.len();
    let expected = capture
        .as_ref()
        .map(|value| value.session.side_count)
        .unwrap_or(0);
    let has_capture = capture.is_some();
    let issue_count = frames
        .iter()
        .filter(|frame| {
            frame.width == 0
                || frame.height == 0
                || (frame.source.to_ascii_lowercase().contains("orbbec")
                    && (frame.depth_path.is_none()
                        || frame.depth_width.is_none()
                        || frame.depth_height.is_none()))
        })
        .count()
        + usize::from(count != expected);
    let cleanup_frames = frames.clone();
    let discard_frames = cleanup_frames.clone();
    let discard = move |event: MouseEvent| {
        let frames = discard_frames.clone();
        spawn(async move {
            delete_temporary_frames(frames).await;
            on_cancel.call(event);
        });
    };
    let retake_all = move |event: MouseEvent| {
        let frames = cleanup_frames.clone();
        spawn(async move {
            delete_temporary_frames(frames).await;
            on_retake_all.call(event);
        });
    };
    let retry_gps = move |_| {
        busy.set(true);
        review_error.set(None);
        spawn(async move {
            gps.set(optional_gps().await);
            busy.set(false);
        });
    };
    let commit = move |_| {
        let Some(mut pending) = capture.clone() else {
            review_error.set(Some("No capture to save.".into()));
            return;
        };
        if pending.frames.len() != pending.session.side_count {
            review_error.set(Some("Capture every side first.".into()));
            return;
        }
        pending.gps = gps.read().clone();
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
    let active_frame = frames.get(*active_side.read()).cloned();
    let frame_url = active_frame
        .as_ref()
        .map(|frame| convert_file_src(&frame.path));
    let review_side_number = *active_side.read() + 1;

    let review_down = move |event: PointerEvent| {
        let point = event.data().element_coordinates();
        swipe.set(Some(SwipeGesture {
            pointer_id: event.data().pointer_id(),
            start_x: point.x,
            start_y: point.y,
        }));
    };
    let review_up = move |event: PointerEvent| {
        let Some(start) = *swipe.read() else {
            return;
        };
        swipe.set(None);
        if start.pointer_id != event.data().pointer_id() || count <= 1 {
            return;
        }
        let point = event.data().element_coordinates();
        let dx = point.x - start.start_x;
        let dy = point.y - start.start_y;
        if dx.abs() > 60.0 && dx.abs() > dy.abs() * 1.2 {
            let current = *active_side.read();
            active_side.set(if dx < 0.0 {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            });
        }
    };

    rsx! {
        div { class: "review-screen",
            div { class: "review-toolbar",
                button { class: "button media", disabled: *busy.read(), onclick: discard, "Cancel" }
                strong { "Review {review_side_number} / {count}" }
                button { class: "button media", disabled: *busy.read(), onclick: retake_all, "Retake all" }
            }
            if let Some(message) = review_error.read().as_ref() {
                div { class: "capture-error", "{message}" }
            }
            div {
                class: "review-stage",
                onpointerdown: review_down,
                onpointerup: review_up,
                if let Some(url) = frame_url {
                    img { src: "{url}", alt: "Captured side" }
                }
                button {
                    class: "review-retake",
                    disabled: *busy.read(),
                    onclick: move |_| on_retake.call(*active_side.read()),
                    "Retake"
                }
            }
            div { class: "review-bottom",
                div { class: "side-dots",
                    for index in 0..count {
                        button {
                            class: if *active_side.read() == index { "active" } else { "" },
                            aria_label: "Side {index + 1}",
                            onclick: move |_| active_side.set(index),
                            span {}
                        }
                    }
                }
                div { class: "review-checks",
                    span { if issue_count == 0 { "Ready" } else { "{issue_count} issue(s)" } }
                    button { class: "text-button", disabled: *busy.read(), onclick: retry_gps,
                        if gps.read().is_some() { "GPS OK" } else { "Retry GPS" }
                    }
                }
                button {
                    class: "button primary review-save",
                    disabled: *busy.read() || !has_capture || issue_count > 0,
                    onclick: commit,
                    if *busy.read() { "Saving..." } else { "Save" }
                }
            }
        }
    }
}
