use super::*;

#[allow(clippy::too_many_arguments)]
fn switch_side(
    next: usize,
    mut active_side: Signal<usize>,
    mut selected_bbox: Signal<Option<String>>,
    mut viewport: Signal<CanvasViewport>,
    mut contacts: Signal<Vec<PointerContact>>,
    mut pinch: Signal<Option<PinchGesture>>,
    mut swipe: Signal<Option<SwipeGesture>>,
    mut box_gesture: Signal<Option<BoxGesture>>,
) {
    active_side.set(next);
    selected_bbox.set(None);
    viewport.set(CanvasViewport::reset());
    contacts.set(Vec::new());
    pinch.set(None);
    swipe.set(None);
    box_gesture.set(None);
}

fn tree_dimensions(
    tree: &Signal<Option<TreeData>>,
    active_side: &Signal<usize>,
) -> Option<(f64, f64)> {
    tree.read()
        .as_ref()?
        .sides
        .get(*active_side.read())
        .map(|side| {
            (
                f64::from(side.image_width.max(1)),
                f64::from(side.image_height.max(1)),
            )
        })
}

fn status_for_tree(tree: &TreeData) -> &'static str {
    if tree
        .sides
        .iter()
        .flat_map(|side| &side.bboxes)
        .all(|bbox| (0..=3).contains(&bbox.class_id))
    {
        "annotated"
    } else {
        "captured"
    }
}

#[component]
pub(super) fn Annotate(
    tree_id: Option<String>,
    data_root: String,
    export_uri: String,
    on_next: EventHandler<MouseEvent>,
    on_exit: EventHandler<MouseEvent>,
    on_next_tree: EventHandler<MouseEvent>,
) -> Element {
    let mut tree = use_signal(|| None::<TreeData>);
    let active_side = use_signal(|| 0_usize);
    let mut busy = use_signal(|| false);
    let mut annotation_error = use_signal(|| None::<String>);
    let mut mode = use_signal(|| AnnotationMode::Review);
    let mut selected_bbox = use_signal(|| None::<String>);
    let mut boxes_visible = use_signal(|| true);
    let mut link_source = use_signal(|| None::<(usize, String)>);
    let mut viewport = use_signal(CanvasViewport::reset);
    let mut contacts = use_signal(Vec::<PointerContact>::new);
    let mut pinch = use_signal(|| None::<PinchGesture>);
    let mut swipe = use_signal(|| None::<SwipeGesture>);
    let mut box_gesture = use_signal(|| None::<BoxGesture>);
    let mut last_tap = use_signal(|| 0.0_f64);

    use_effect(move || {
        let Some(id) = tree_id.clone() else {
            annotation_error.set(Some("No tree selected.".into()));
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
            annotation_error.set(Some("No image for this side.".into()));
            return;
        };
        busy.set(true);
        annotation_error.set(None);
        spawn(async move {
            match run_detector(image_path).await {
                Ok(boxes) => {
                    let side_index = *active_side.read();
                    if let Some(value) = tree.write().as_mut() {
                        if let Some(side) = value.sides.get_mut(side_index) {
                            side.original_bboxes = boxes.clone();
                            side.bboxes = boxes;
                        }
                        value
                            .confirmed_links
                            .retain(|link| link.side_a != side_index && link.side_b != side_index);
                    }
                    selected_bbox.set(None);
                }
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let save_data_root = data_root.clone();
    let save_export_uri = export_uri.clone();
    let save = move |_| {
        let Some(mut value) = tree.read().clone() else {
            return;
        };
        value.status = status_for_tree(&value).into();
        busy.set(true);
        annotation_error.set(None);
        let data_root = save_data_root.clone();
        let export_uri = save_export_uri.clone();
        spawn(async move {
            match save_tree_portable(value, &data_root, &export_uri).await {
                Ok((saved, warning)) => {
                    tree.set(Some(saved));
                    if let Some(message) = warning {
                        annotation_error.set(Some(format!("Saved locally: {message}")));
                    }
                }
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let exit_data_root = data_root.clone();
    let exit_export_uri = export_uri.clone();
    let save_and_exit = move |event: MouseEvent| {
        let Some(mut value) = tree.read().clone() else {
            return;
        };
        value.status = status_for_tree(&value).into();
        busy.set(true);
        annotation_error.set(None);
        let data_root = exit_data_root.clone();
        let export_uri = exit_export_uri.clone();
        spawn(async move {
            match save_tree_portable(value, &data_root, &export_uri).await {
                Ok((saved, _)) => {
                    tree.set(Some(saved));
                    on_exit.call(event);
                }
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let dedup_data_root = data_root.clone();
    let dedup_export_uri = export_uri.clone();
    let save_and_dedup = move |event: MouseEvent| {
        let Some(mut value) = tree.read().clone() else {
            return;
        };
        value.status = status_for_tree(&value).into();
        busy.set(true);
        annotation_error.set(None);
        let data_root = dedup_data_root.clone();
        let export_uri = dedup_export_uri.clone();
        spawn(async move {
            match save_tree_portable(value, &data_root, &export_uri).await {
                Ok((saved, warning)) => {
                    tree.set(Some(saved));
                    if let Some(message) = warning {
                        annotation_error.set(Some(format!("Saved locally: {message}")));
                    }
                    on_next.call(event);
                }
                Err(message) => annotation_error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let next_data_root = data_root.clone();
    let next_export_uri = export_uri.clone();
    let save_and_capture_next = move |event: MouseEvent| {
        let Some(mut value) = tree.read().clone() else {
            return;
        };
        if value
            .sides
            .iter()
            .flat_map(|side| &side.bboxes)
            .any(|bbox| !(0..=3).contains(&bbox.class_id))
        {
            annotation_error.set(Some("Assign every box first.".into()));
            return;
        }
        value.status = "annotated".into();
        let id = value.id.clone();
        busy.set(true);
        annotation_error.set(None);
        let data_root = next_data_root.clone();
        let export_uri = next_export_uri.clone();
        spawn(async move {
            match save_tree_portable(value, &data_root, &export_uri).await {
                Ok((saved, warning)) => match compute_tree(id.clone()).await {
                    Ok(result) if result.quality.ready && result.result.unassigned_count == 0 => {
                        let completed = load_tree(id).await.unwrap_or(saved);
                        let completion_warning =
                            mirror_tree_state(&completed, &data_root, &export_uri)
                                .await
                                .err();
                        tree.set(Some(completed));
                        if let Some(message) = completion_warning.or(warning) {
                            annotation_error.set(Some(format!("Saved locally: {message}")));
                        }
                        on_next_tree.call(event);
                    }
                    Ok(result) => {
                        annotation_error.set(Some(
                            result
                                .quality
                                .issues
                                .first()
                                .map(|issue| issue.message.clone())
                                .unwrap_or_else(|| "Tree checks are incomplete.".into()),
                        ));
                    }
                    Err(message) => annotation_error.set(Some(message)),
                },
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
    let current_side = *active_side.read();
    let image_url = tree
        .read()
        .as_ref()
        .and_then(|value| value.sides.get(current_side))
        .map(|side| {
            let mut url = convert_file_src(&format!(
                "{}/dataset/{}",
                data_root.trim_end_matches(['/', '\\']),
                side.image_path
            ));
            if let Some(bust) = &side.cache_bust {
                url.push(if url.contains('?') { '&' } else { '?' });
                url.push_str("v=");
                url.push_str(bust);
            }
            url
        });
    let visible_boxes = tree
        .read()
        .as_ref()
        .and_then(|value| value.sides.get(current_side))
        .map(|side| side.bboxes.clone())
        .unwrap_or_default();
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
    let side_count = side_tabs.len();
    let box_count = visible_boxes.len();
    let (width, height) = tree_dimensions(&tree, &active_side).unwrap_or((1.0, 1.0));
    let current_viewport = *viewport.read();
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    let canvas_transform = format!(
        "translate({} {}) translate({center_x} {center_y}) scale({}) translate({} {})",
        current_viewport.pan_x, current_viewport.pan_y, current_viewport.zoom, -center_x, -center_y
    );
    let selected = selected_bbox.read().clone();
    let selected_box = selected
        .as_ref()
        .and_then(|id| visible_boxes.iter().find(|bbox| &bbox.id == id))
        .cloned();
    let selected_handles = selected_box
        .as_ref()
        .map(|bbox| handle_points(bbox).to_vec())
        .unwrap_or_default();
    let preview_box = box_gesture
        .read()
        .as_ref()
        .and_then(|gesture| match gesture {
            BoxGesture::Draw {
                start_x,
                start_y,
                current_x,
                current_y,
            } => Some((
                start_x.min(*current_x),
                start_y.min(*current_y),
                (start_x - current_x).abs(),
                (start_y - current_y).abs(),
            )),
            _ => None,
        });
    let linked_boxes = tree
        .read()
        .as_ref()
        .map(|value| {
            value
                .confirmed_links
                .iter()
                .enumerate()
                .flat_map(|(index, link)| {
                    let number = index + 1;
                    [
                        ((link.side_a, link.bbox_id_a.clone()), number),
                        ((link.side_b, link.bbox_id_b.clone()), number),
                    ]
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();
    let current_links = tree
        .read()
        .as_ref()
        .map(|value| {
            value
                .confirmed_links
                .iter()
                .enumerate()
                .filter(|(_, link)| link.side_a == current_side || link.side_b == current_side)
                .map(|(index, link)| (index, link.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let unassigned = tree
        .read()
        .as_ref()
        .map(|value| {
            value
                .sides
                .iter()
                .flat_map(|side| &side.bboxes)
                .filter(|bbox| !(0..=3).contains(&bbox.class_id))
                .count()
        })
        .unwrap_or(0);

    let pointer_down = move |event: PointerEvent| {
        event.prevent_default();
        let Some((width, height)) = tree_dimensions(&tree, &active_side) else {
            return;
        };
        let Some(point) = pointer_canvas_point(&event, width, height, *viewport.read()) else {
            return;
        };
        let pointer_id = event.data().pointer_id();
        {
            let mut active = contacts.write();
            active.retain(|contact| contact.id != pointer_id);
            active.push(PointerContact {
                id: pointer_id,
                x: point.base_x,
                y: point.base_y,
            });
        }
        if contacts.read().len() == 2 {
            let active = contacts.read();
            let a = active[0];
            let b = active[1];
            pinch.set(Some(PinchGesture {
                distance: (a.x - b.x).hypot(a.y - b.y).max(1.0),
                centroid_x: (a.x + b.x) / 2.0,
                centroid_y: (a.y + b.y) / 2.0,
                zoom: viewport.read().zoom,
                pan_x: viewport.read().pan_x,
                pan_y: viewport.read().pan_y,
            }));
            box_gesture.set(None);
            swipe.set(None);
            return;
        }
        if *mode.read() == AnnotationMode::Review {
            swipe.set(Some(SwipeGesture {
                pointer_id,
                start_x: point.base_x,
                start_y: point.base_y,
            }));
            return;
        }
        let boxes = tree
            .read()
            .as_ref()
            .and_then(|value| value.sides.get(*active_side.read()))
            .map(|side| side.bboxes.clone())
            .unwrap_or_default();
        let tolerance = (22.0 / viewport.read().zoom.max(1.0) * width / 900.0).max(5.0);
        if let Some(id) = selected_bbox.read().clone() {
            if let Some(bbox) = boxes.iter().find(|bbox| bbox.id == id) {
                if let Some(handle) =
                    hit_resize_handle(bbox, point.image_x, point.image_y, tolerance)
                {
                    box_gesture.set(Some(BoxGesture::Resize {
                        bbox_id: id,
                        handle,
                        original: bbox.clone(),
                    }));
                    return;
                }
            }
        }
        if let Some(id) = hit_bbox(&boxes, point.image_x, point.image_y) {
            if let Some(original) = boxes.iter().find(|bbox| bbox.id == id).cloned() {
                selected_bbox.set(Some(id.clone()));
                box_gesture.set(Some(BoxGesture::Move {
                    bbox_id: id,
                    start_x: point.image_x,
                    start_y: point.image_y,
                    original,
                }));
            }
        } else {
            selected_bbox.set(None);
            box_gesture.set(Some(BoxGesture::Draw {
                start_x: point.image_x,
                start_y: point.image_y,
                current_x: point.image_x,
                current_y: point.image_y,
            }));
        }
    };

    let pointer_move = move |event: PointerEvent| {
        event.prevent_default();
        let Some((width, height)) = tree_dimensions(&tree, &active_side) else {
            return;
        };
        let Some(point) = pointer_canvas_point(&event, width, height, *viewport.read()) else {
            return;
        };
        let pointer_id = event.data().pointer_id();
        if let Some(contact) = contacts
            .write()
            .iter_mut()
            .find(|contact| contact.id == pointer_id)
        {
            contact.x = point.base_x;
            contact.y = point.base_y;
        }
        if let Some(start) = *pinch.read() {
            let active = contacts.read();
            if active.len() >= 2 {
                let a = active[0];
                let b = active[1];
                let distance = (a.x - b.x).hypot(a.y - b.y).max(1.0);
                let centroid_x = (a.x + b.x) / 2.0;
                let centroid_y = (a.y + b.y) / 2.0;
                let zoom = (start.zoom * distance / start.distance).clamp(1.0, 6.0);
                let focus_x =
                    (start.centroid_x - start.pan_x - width / 2.0) / start.zoom + width / 2.0;
                let focus_y =
                    (start.centroid_y - start.pan_y - height / 2.0) / start.zoom + height / 2.0;
                viewport.set(clamp_viewport(
                    CanvasViewport {
                        zoom,
                        pan_x: centroid_x - width / 2.0 - (focus_x - width / 2.0) * zoom,
                        pan_y: centroid_y - height / 2.0 - (focus_y - height / 2.0) * zoom,
                    },
                    width,
                    height,
                ));
            }
            return;
        }
        if *mode.read() != AnnotationMode::Edit {
            return;
        }
        let Some(gesture) = box_gesture.read().clone() else {
            return;
        };
        match gesture {
            BoxGesture::Draw {
                start_x, start_y, ..
            } => box_gesture.set(Some(BoxGesture::Draw {
                start_x,
                start_y,
                current_x: point.image_x,
                current_y: point.image_y,
            })),
            BoxGesture::Move {
                bbox_id,
                start_x,
                start_y,
                original,
            } => {
                if let Some(bbox) = tree
                    .write()
                    .as_mut()
                    .and_then(|value| value.sides.get_mut(*active_side.read()))
                    .and_then(|side| side.bboxes.iter_mut().find(|bbox| bbox.id == bbox_id))
                {
                    move_bbox(
                        bbox,
                        &original,
                        point.image_x - start_x,
                        point.image_y - start_y,
                        width,
                        height,
                    );
                }
            }
            BoxGesture::Resize {
                bbox_id,
                handle,
                original,
            } => {
                if let Some(bbox) = tree
                    .write()
                    .as_mut()
                    .and_then(|value| value.sides.get_mut(*active_side.read()))
                    .and_then(|side| side.bboxes.iter_mut().find(|bbox| bbox.id == bbox_id))
                {
                    resize_bbox(
                        bbox,
                        &original,
                        handle,
                        point.image_x,
                        point.image_y,
                        width,
                        height,
                    );
                }
            }
        }
    };

    let pointer_up = move |event: PointerEvent| {
        event.prevent_default();
        let Some((width, height)) = tree_dimensions(&tree, &active_side) else {
            return;
        };
        let point = pointer_canvas_point(&event, width, height, *viewport.read());
        let pointer_id = event.data().pointer_id();
        let was_pinching = pinch.read().is_some();
        contacts.write().retain(|contact| contact.id != pointer_id);
        if was_pinching {
            if contacts.read().len() < 2 {
                pinch.set(None);
            }
            box_gesture.set(None);
            swipe.set(None);
            return;
        }
        let Some(point) = point else {
            return;
        };
        if *mode.read() == AnnotationMode::Edit {
            if let Some(BoxGesture::Draw {
                start_x,
                start_y,
                current_x,
                current_y,
            }) = box_gesture.read().clone()
            {
                let x1 = start_x.min(current_x);
                let y1 = start_y.min(current_y);
                let x2 = start_x.max(current_x);
                let y2 = start_y.max(current_y);
                if x2 - x1 >= 4.0 && y2 - y1 >= 4.0 {
                    let id = format!("nb{}", js_sys::Date::now() as u64);
                    if let Some(side) = tree
                        .write()
                        .as_mut()
                        .and_then(|value| value.sides.get_mut(*active_side.read()))
                    {
                        side.bboxes.push(BoxData {
                            id: id.clone(),
                            class_id: -1,
                            class_name: "U".into(),
                            x1,
                            y1,
                            x2,
                            y2,
                            confidence: None,
                        });
                    }
                    selected_bbox.set(Some(id));
                }
            }
            box_gesture.set(None);
            return;
        }
        let Some(start) = *swipe.read() else {
            return;
        };
        if start.pointer_id != pointer_id {
            return;
        }
        swipe.set(None);
        let dx = point.base_x - start.start_x;
        let dy = point.base_y - start.start_y;
        let count = tree
            .read()
            .as_ref()
            .map(|value| value.sides.len())
            .unwrap_or(1)
            .max(1);
        if dx.abs() >= width * 0.07 && dx.abs() > dy.abs() * 1.2 {
            let current = *active_side.read();
            let next = if dx < 0.0 {
                (current + 1) % count
            } else {
                (current + count - 1) % count
            };
            switch_side(
                next,
                active_side,
                selected_bbox,
                viewport,
                contacts,
                pinch,
                swipe,
                box_gesture,
            );
            return;
        }
        let now = js_sys::Date::now();
        if now - *last_tap.read() < 300.0 && viewport.read().zoom > 1.0 {
            viewport.set(CanvasViewport::reset());
            last_tap.set(0.0);
            return;
        }
        last_tap.set(now);
        let boxes = tree
            .read()
            .as_ref()
            .and_then(|value| value.sides.get(*active_side.read()))
            .map(|side| side.bboxes.clone())
            .unwrap_or_default();
        let hit = hit_bbox(&boxes, point.image_x, point.image_y);
        let armed_link = link_source.read().clone();
        if let (Some((source_side, source_id)), Some(target_id)) = (armed_link, hit.clone()) {
            let target_side = *active_side.read();
            if source_side == target_side && source_id == target_id {
                link_source.set(None);
            } else if let Some(value) = tree.write().as_mut() {
                match add_confirmed_link(
                    value,
                    source_side,
                    source_id,
                    target_side,
                    target_id.clone(),
                ) {
                    Ok(()) => {
                        link_source.set(None);
                        annotation_error.set(None);
                    }
                    Err(message) => annotation_error.set(Some(message)),
                }
            }
        }
        selected_bbox.set(hit);
    };

    let pointer_cancel = move |event: PointerEvent| {
        let pointer_id = event.data().pointer_id();
        contacts.write().retain(|contact| contact.id != pointer_id);
        if contacts.read().len() < 2 {
            pinch.set(None);
        }
        swipe.set(None);
        box_gesture.set(None);
    };

    let wheel_zoom = move |event: WheelEvent| {
        event.prevent_default();
        let Some((width, height)) = tree_dimensions(&tree, &active_side) else {
            return;
        };
        let mut next = *viewport.read();
        next.zoom *= if event.data().delta().strip_units().y < 0.0 {
            1.15
        } else {
            1.0 / 1.15
        };
        viewport.set(clamp_viewport(next, width, height));
    };

    let keyboard = move |event: KeyboardEvent| {
        let key = event.key().to_string();
        match key.as_str() {
            "1" | "2" | "3" | "4" => {
                if let Some(id) = selected_bbox.read().clone() {
                    if let Some(value) = tree.write().as_mut() {
                        set_connected_bbox_class(
                            value,
                            *active_side.read(),
                            &id,
                            key.parse::<i32>().unwrap_or(1) - 1,
                        );
                    }
                }
            }
            "Delete" | "Backspace" => {
                let selected_id = selected_bbox.read().clone();
                if let Some(id) = selected_id {
                    if let Some(value) = tree.write().as_mut() {
                        delete_bbox(value, *active_side.read(), &id);
                    }
                    selected_bbox.set(None);
                }
            }
            "q" | "Q" => {
                let count = tree
                    .read()
                    .as_ref()
                    .map(|value| value.sides.len())
                    .unwrap_or(1)
                    .max(1);
                switch_side(
                    (*active_side.read() + count - 1) % count,
                    active_side,
                    selected_bbox,
                    viewport,
                    contacts,
                    pinch,
                    swipe,
                    box_gesture,
                );
            }
            "e" | "E" => {
                let count = tree
                    .read()
                    .as_ref()
                    .map(|value| value.sides.len())
                    .unwrap_or(1)
                    .max(1);
                switch_side(
                    (*active_side.read() + 1) % count,
                    active_side,
                    selected_bbox,
                    viewport,
                    contacts,
                    pinch,
                    swipe,
                    box_gesture,
                );
            }
            "Escape" => {
                selected_bbox.set(None);
                link_source.set(None);
            }
            _ => {}
        }
    };

    rsx! {
        div { class: "annotate-screen",
            div { class: "annotate-topbar",
                div { class: "segmented",
                    button {
                        class: if *mode.read() == AnnotationMode::Review { "active" } else { "" },
                        onclick: move |_| {
                            mode.set(AnnotationMode::Review);
                            box_gesture.set(None);
                        },
                        "Review"
                    }
                    button {
                        class: if *mode.read() == AnnotationMode::Edit { "active" } else { "" },
                        onclick: move |_| {
                            mode.set(AnnotationMode::Edit);
                            swipe.set(None);
                        },
                        "Edit"
                    }
                }
                strong { class: if unassigned > 0 { "side-status warning" } else { "side-status" },
                    "Side {current_side + 1} | {box_count}"
                    if unassigned > 0 { " | {unassigned} U" }
                }
                div { class: "zoom-tools",
                    button {
                        class: "icon-button",
                        aria_label: "Zoom out",
                        onclick: move |_| {
                            let mut next = *viewport.read();
                            next.zoom /= 1.25;
                            viewport.set(clamp_viewport(next, width, height));
                        },
                        "-"
                    }
                    button {
                        class: "icon-button zoom-reset",
                        aria_label: "Reset zoom",
                        onclick: move |_| viewport.set(CanvasViewport::reset()),
                        "{current_viewport.zoom:.1}x"
                    }
                    button {
                        class: "icon-button",
                        aria_label: "Zoom in",
                        onclick: move |_| {
                            let mut next = *viewport.read();
                            next.zoom *= 1.25;
                            viewport.set(clamp_viewport(next, width, height));
                        },
                        "+"
                    }
                }
            }
            if let Some(message) = annotation_error.read().as_ref() {
                div { class: "inline-error compact", "{message}" }
            }
            div { class: "annotate-stage",
                if let Some(url) = image_url {
                    svg {
                        class: "bbox-canvas",
                        view_box: "0 0 {width} {height}",
                        preserve_aspect_ratio: "xMidYMid meet",
                        tabindex: "0",
                        onpointerdown: pointer_down,
                        onpointermove: pointer_move,
                        onpointerup: pointer_up,
                        onpointercancel: pointer_cancel,
                        onwheel: wheel_zoom,
                        onkeydown: keyboard,
                        g { transform: "{canvas_transform}",
                            image {
                                href: "{url}",
                                x: "0",
                                y: "0",
                                width: "{width}",
                                height: "{height}",
                                preserve_aspect_ratio: "none"
                            }
                            if *boxes_visible.read() {
                                for bbox in &visible_boxes {
                                    rect {
                                        class: if selected.as_ref() == Some(&bbox.id) { "bbox-shape selected" } else { "bbox-shape" },
                                        x: "{bbox.x1}",
                                        y: "{bbox.y1}",
                                        width: "{bbox.x2 - bbox.x1}",
                                        height: "{bbox.y2 - bbox.y1}",
                                        stroke: "{class_color(bbox.class_id)}",
                                        fill: "{class_color(bbox.class_id)}"
                                    }
                                    text {
                                        class: "bbox-label",
                                        x: "{bbox.x1}",
                                        y: "{(bbox.y1 - 5.0).max(12.0)}",
                                        fill: "{class_color(bbox.class_id)}",
                                        "{bbox.class_name}"
                                        if let Some(number) = linked_boxes.get(&(current_side, bbox.id.clone())) {
                                            " L{number}"
                                        }
                                    }
                                }
                                if *mode.read() == AnnotationMode::Edit {
                                    for (_, x, y) in selected_handles {
                                        circle {
                                            class: "bbox-handle",
                                            cx: "{x}",
                                            cy: "{y}",
                                            r: "{7.0 / current_viewport.zoom.max(1.0)}"
                                        }
                                    }
                                    if let Some((x, y, box_width, box_height)) = preview_box {
                                        rect {
                                            class: "bbox-shape drawing",
                                            x: "{x}",
                                            y: "{y}",
                                            width: "{box_width}",
                                            height: "{box_height}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if *busy.read() {
                    div { class: "loading-plain", "Loading" }
                }
            }
            div { class: "side-dots",
                for (side_index, side_label) in side_tabs {
                    button {
                        class: if current_side == side_index { "active" } else { "" },
                        aria_label: "{side_label}",
                        onclick: move |_| {
                            switch_side(
                                side_index,
                                active_side,
                                selected_bbox,
                                viewport,
                                contacts,
                                pinch,
                                swipe,
                                box_gesture,
                            );
                        },
                        span {}
                    }
                }
            }
            div { class: "annotate-actions",
                div { class: "class-strip",
                    for (class_id, class_name) in [(0, "B1"), (1, "B2"), (2, "B3"), (3, "B4")] {
                        button {
                            class: if selected_box.as_ref().is_some_and(|bbox| bbox.class_id == class_id) {
                                format!("class-chip c{class_id} active")
                            } else {
                                format!("class-chip c{class_id}")
                            },
                            disabled: selected.is_none(),
                            onclick: move |_| {
                                if let Some(id) = selected_bbox.read().clone() {
                                    if let Some(value) = tree.write().as_mut() {
                                        set_connected_bbox_class(value, *active_side.read(), &id, class_id);
                                    }
                                }
                            },
                            "{class_name}"
                        }
                    }
                    button {
                        class: if *boxes_visible.read() { "tool-button active" } else { "tool-button" },
                        onclick: move |_| {
                            let visible = *boxes_visible.read();
                            boxes_visible.set(!visible);
                        },
                        "Boxes"
                    }
                    button {
                        class: if link_source.read().is_some() { "tool-button active" } else { "tool-button" },
                        disabled: selected.is_none() || *mode.read() != AnnotationMode::Review,
                        onclick: move |_| {
                            let selected_id = selected_bbox.read().clone();
                            if let Some(id) = selected_id {
                                link_source.set(Some((*active_side.read(), id)));
                            }
                        },
                        if link_source.read().is_some() { "Target" } else { "Link" }
                    }
                    button {
                        class: "tool-button danger",
                        disabled: selected.is_none(),
                        onclick: move |_| {
                            let selected_id = selected_bbox.read().clone();
                            if let Some(id) = selected_id {
                                if let Some(value) = tree.write().as_mut() {
                                    delete_bbox(value, *active_side.read(), &id);
                                }
                                selected_bbox.set(None);
                            }
                        },
                        "Delete"
                    }
                }
                div { class: "primary-row",
                    button { class: "button secondary", disabled: *busy.read(), onclick: detect, "Detect" }
                    button { class: "button ghost", disabled: *busy.read(), onclick: save, "Save" }
                    button { class: "button ghost", disabled: *busy.read(), onclick: save_and_exit, "Exit" }
                    button { class: "button primary", disabled: *busy.read() || !ready_for_dedup, onclick: save_and_dedup, "Dedup" }
                    button { class: "button primary", disabled: *busy.read() || !ready_for_dedup, onclick: save_and_capture_next, "Next tree" }
                }
            }
            if !current_links.is_empty() {
                div { class: "link-strip",
                    for (link_index, link) in current_links {
                        span { "L{link_index + 1}: S{link.side_a + 1} <-> S{link.side_b + 1}" }
                        button {
                            class: "link-remove",
                            aria_label: "Remove link",
                            onclick: move |_| {
                                if let Some(value) = tree.write().as_mut() {
                                    if link_index < value.confirmed_links.len() {
                                        value.confirmed_links.remove(link_index);
                                    }
                                }
                            },
                            "x"
                        }
                    }
                }
            }
            if side_count == 0 && !*busy.read() {
                div { class: "inline-error compact", "No sides" }
            }
        }
    }
}
