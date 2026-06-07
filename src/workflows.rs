use super::*;

fn side_image_url(data_root: &str, side: &SideData) -> String {
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
}

#[component]
fn DedupCanvas(
    side_index: usize,
    image_url: String,
    width: u32,
    height: u32,
    boxes: Vec<BoxData>,
    selected: Option<String>,
    on_select: EventHandler<String>,
) -> Element {
    let width_f = f64::from(width.max(1));
    let height_f = f64::from(height.max(1));
    rsx! {
        div { class: "dedup-canvas-wrap",
            span { class: "dedup-side-label", "Side {side_index + 1}" }
            svg {
                class: "dedup-canvas",
                view_box: "0 0 {width_f} {height_f}",
                preserve_aspect_ratio: "xMidYMid meet",
                onpointerup: move |event: PointerEvent| {
                    if let Some(point) = pointer_canvas_point(
                        &event,
                        width_f,
                        height_f,
                        CanvasViewport::reset(),
                    ) {
                        if let Some(id) = hit_bbox(&boxes, point.image_x, point.image_y) {
                            on_select.call(id);
                        }
                    }
                },
                image {
                    href: "{image_url}",
                    x: "0",
                    y: "0",
                    width: "{width_f}",
                    height: "{height_f}",
                    preserve_aspect_ratio: "none"
                }
                for bbox in &boxes {
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
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn Dedup(
    tree_id: Option<String>,
    data_root: String,
    export_uri: String,
    on_results: EventHandler<MouseEvent>,
) -> Element {
    let mut tree = use_signal(|| None::<TreeData>);
    let mut pair_index = use_signal(|| 0_usize);
    let mut selected_a = use_signal(|| None::<String>);
    let mut selected_b = use_signal(|| None::<String>);
    let mut suggestions = use_signal(Vec::<LinkSuggestionData>::new);
    let mut busy = use_signal(|| false);
    let mut dedup_error = use_signal(|| None::<String>);

    let id_for_load = tree_id.clone();
    use_effect(move || {
        let Some(id) = id_for_load.clone() else {
            dedup_error.set(Some("No tree selected.".into()));
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
    let pair_count = pairs.len();
    let active_pair = pairs.get(*pair_index.read()).copied();
    let side_a = active_pair.and_then(|(index, _)| {
        tree.read()
            .as_ref()
            .and_then(|value| value.sides.get(index))
            .cloned()
    });
    let side_b = active_pair.and_then(|(_, index)| {
        tree.read()
            .as_ref()
            .and_then(|value| value.sides.get(index))
            .cloned()
    });
    let active_suggestions = active_pair
        .map(|(a, b)| {
            suggestions
                .read()
                .iter()
                .filter(|item| {
                    (item.side_a == a && item.side_b == b) || (item.side_a == b && item.side_b == a)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let links = tree
        .read()
        .as_ref()
        .map(|value| value.confirmed_links.clone())
        .unwrap_or_default();

    let save_data_root = data_root.clone();
    let save_export_uri = export_uri.clone();
    let save = move |_| {
        let Some(value) = tree.read().clone() else {
            return;
        };
        busy.set(true);
        dedup_error.set(None);
        let data_root = save_data_root.clone();
        let export_uri = save_export_uri.clone();
        spawn(async move {
            match save_tree_portable(value, &data_root, &export_uri).await {
                Ok((saved, warning)) => {
                    tree.set(Some(saved));
                    if let Some(message) = warning {
                        dedup_error.set(Some(format!("Saved locally: {message}")));
                    }
                }
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
    let selected_endpoint = selected_a
        .read()
        .as_ref()
        .and_then(|id| active_pair.map(|(side, _)| (side, id.clone())))
        .or_else(|| {
            selected_b
                .read()
                .as_ref()
                .and_then(|id| active_pair.map(|(_, side)| (side, id.clone())))
        });

    rsx! {
        div { class: "dedup-screen",
            div { class: "dedup-toolbar",
                button {
                    class: "icon-button",
                    disabled: pair_count == 0,
                    onclick: move |_| {
                        let count = pair_count.max(1);
                        let current = *pair_index.read();
                        pair_index.set((current + count - 1) % count);
                        selected_a.set(None);
                        selected_b.set(None);
                    },
                    "<"
                }
                strong {
                    if let Some((a, b)) = active_pair {
                        "Side {a + 1} / {b + 1}"
                    } else {
                        "No sides"
                    }
                }
                button {
                    class: "icon-button",
                    disabled: pair_count == 0,
                    onclick: move |_| {
                        let count = pair_count.max(1);
                        let current = *pair_index.read();
                        pair_index.set((current + 1) % count);
                        selected_a.set(None);
                        selected_b.set(None);
                    },
                    ">"
                }
                button { class: "button secondary", disabled: *busy.read(), onclick: run_suggestions, "Suggest" }
                button { class: "button ghost", disabled: *busy.read(), onclick: save, "Save" }
                button { class: "button primary", onclick: on_results, "Results" }
            }
            if let Some(message) = dedup_error.read().as_ref() {
                div { class: "inline-error compact", "{message}" }
            }
            div { class: "dedup-canvases",
                if let (Some((a, b)), Some(left), Some(right)) = (active_pair, side_a, side_b) {
                    DedupCanvas {
                        side_index: a,
                        image_url: side_image_url(&data_root, &left),
                        width: left.image_width,
                        height: left.image_height,
                        boxes: left.bboxes,
                        selected: selected_a.read().clone(),
                        on_select: move |id: String| {
                            let Some(right) = selected_b.read().clone() else {
                                selected_a.set(Some(id));
                                return;
                            };
                            if let Some(value) = tree.write().as_mut() {
                                match add_confirmed_link(value, a, id, b, right) {
                                    Ok(()) => {
                                        selected_a.set(None);
                                        selected_b.set(None);
                                        dedup_error.set(None);
                                    }
                                    Err(message) => dedup_error.set(Some(message)),
                                }
                            }
                        }
                    }
                    DedupCanvas {
                        side_index: b,
                        image_url: side_image_url(&data_root, &right),
                        width: right.image_width,
                        height: right.image_height,
                        boxes: right.bboxes,
                        selected: selected_b.read().clone(),
                        on_select: move |id: String| {
                            let Some(left) = selected_a.read().clone() else {
                                selected_b.set(Some(id));
                                return;
                            };
                            if let Some(value) = tree.write().as_mut() {
                                match add_confirmed_link(value, a, left, b, id) {
                                    Ok(()) => {
                                        selected_a.set(None);
                                        selected_b.set(None);
                                        dedup_error.set(None);
                                    }
                                    Err(message) => dedup_error.set(Some(message)),
                                }
                            }
                        }
                    }
                }
            }
            div { class: "dedup-tools",
                if let Some((side_index, bbox_id)) = selected_endpoint {
                    for (class_id, class_name) in [(0, "B1"), (1, "B2"), (2, "B3"), (3, "B4")] {
                        button {
                            class: "class-chip",
                            onclick: {
                                let bbox_id = bbox_id.clone();
                                move |_| {
                                    if let Some(value) = tree.write().as_mut() {
                                        set_connected_bbox_class(value, side_index, &bbox_id, class_id);
                                    }
                                }
                            },
                            "{class_name}"
                        }
                    }
                    button {
                        class: "tool-button danger",
                        onclick: {
                            let bbox_id = bbox_id.clone();
                            move |_| {
                                if let Some(value) = tree.write().as_mut() {
                                    delete_bbox(value, side_index, &bbox_id);
                                }
                                selected_a.set(None);
                                selected_b.set(None);
                            }
                        },
                        "Delete"
                    }
                }
            }
            if !active_suggestions.is_empty() {
                div { class: "suggestion-list",
                    div { class: "suggestion-head",
                        strong { "Suggestions" }
                        button {
                            class: "text-button",
                            onclick: {
                                let suggestions_for_auto = active_suggestions.clone();
                                move |_| {
                                let autos = suggestions_for_auto
                                    .iter()
                                    .filter(|item| item.category == "auto")
                                    .cloned()
                                    .collect::<Vec<_>>();
                                if let Some(value) = tree.write().as_mut() {
                                    for item in &autos {
                                        let _ = add_confirmed_link(
                                            value,
                                            item.side_a,
                                            item.bbox_id_a.clone(),
                                            item.side_b,
                                            item.bbox_id_b.clone(),
                                        );
                                    }
                                }
                                let accepted = autos.iter().map(|item| item.link_id.clone()).collect::<HashSet<_>>();
                                suggestions.write().retain(|item| !accepted.contains(&item.link_id));
                                }
                            },
                            "Accept auto"
                        }
                    }
                    for suggestion in active_suggestions.iter() {
                        div { class: "suggestion-row",
                            span { "S{suggestion.side_a + 1}:{suggestion.bbox_id_a}" }
                            strong { "{suggestion.score:.2}" }
                            span { "S{suggestion.side_b + 1}:{suggestion.bbox_id_b}" }
                            button {
                                class: "text-button",
                                onclick: {
                                    let item = suggestion.clone();
                                    move |_| {
                                        if let Some(value) = tree.write().as_mut() {
                                            let _ = add_confirmed_link(
                                                value,
                                                item.side_a,
                                                item.bbox_id_a.clone(),
                                                item.side_b,
                                                item.bbox_id_b.clone(),
                                            );
                                        }
                                        suggestions.write().retain(|candidate| candidate.link_id != item.link_id);
                                    }
                                },
                                "Accept"
                            }
                            button {
                                class: "text-button danger",
                                onclick: {
                                    let id = suggestion.link_id.clone();
                                    move |_| suggestions.write().retain(|item| item.link_id != id)
                                },
                                "Reject"
                            }
                        }
                    }
                }
            }
            if !links.is_empty() {
                div { class: "link-list",
                    for (index, link) in links.iter().enumerate() {
                        div {
                            span { "S{link.side_a + 1}:{link.bbox_id_a}" }
                            span { "S{link.side_b + 1}:{link.bbox_id_b}" }
                            button {
                                class: "link-remove",
                                onclick: move |_| {
                                    if let Some(value) = tree.write().as_mut() {
                                        if index < value.confirmed_links.len() {
                                            value.confirmed_links.remove(index);
                                        }
                                    }
                                },
                                "x"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn mismatch_groups(tree: &TreeData) -> Vec<Vec<(usize, String, String)>> {
    let mut seen = HashSet::new();
    let mut groups = Vec::new();
    for side in &tree.sides {
        for bbox in &side.bboxes {
            let key = (side.side_index, bbox.id.clone());
            if seen.contains(&key) {
                continue;
            }
            let endpoints = connected_bbox_endpoints(tree, side.side_index, &bbox.id);
            seen.extend(endpoints.iter().cloned());
            let members = endpoints
                .into_iter()
                .filter_map(|(side_index, bbox_id)| {
                    let bbox = tree
                        .sides
                        .get(side_index)?
                        .bboxes
                        .iter()
                        .find(|bbox| bbox.id == bbox_id)?;
                    Some((side_index, bbox_id, bbox.class_name.clone()))
                })
                .collect::<Vec<_>>();
            let classes = members
                .iter()
                .map(|(_, _, class_name)| class_name.clone())
                .collect::<HashSet<_>>();
            if members.len() > 1 && classes.len() > 1 {
                groups.push(members);
            }
        }
    }
    groups
}

async fn mirror_export(data: ExportData) -> Result<usize, String> {
    for file in &data.export_files {
        copy_to_saf(
            &data.export_uri,
            &file.relative_path,
            &file.source_path,
            &file.mime_type,
        )
        .await?;
    }
    Ok(data.export_files.len())
}

#[component]
pub(super) fn Results(tree_id: Option<String>, data_root: String, export_uri: String) -> Element {
    let mut tree = use_signal(|| None::<TreeData>);
    let mut result = use_signal(|| None::<ComputeData>);
    let mut busy = use_signal(|| false);
    let mut result_error = use_signal(|| None::<String>);
    let mut export_notice = use_signal(|| None::<String>);

    let id_for_load = tree_id.clone();
    use_effect(move || {
        let Some(id) = id_for_load.clone() else {
            result_error.set(Some("No tree selected.".into()));
            return;
        };
        spawn(async move {
            match load_tree(id).await {
                Ok(value) => tree.set(Some(value)),
                Err(message) => result_error.set(Some(message)),
            }
        });
    });

    let mismatch_rows = tree
        .read()
        .as_ref()
        .map(mismatch_groups)
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
    let has_mismatches = !mismatch_rows.is_empty();
    let compute_tree_id = tree_id.clone();
    let compute_data_root = data_root.clone();
    let compute_export_uri = export_uri.clone();
    let compute = move |_| {
        let Some(id) = compute_tree_id.clone() else {
            return;
        };
        if unassigned > 0 || has_mismatches {
            result_error.set(Some(
                "Resolve unassigned and mismatched classes first.".into(),
            ));
            return;
        }
        busy.set(true);
        result_error.set(None);
        let data_root = compute_data_root.clone();
        let export_uri = compute_export_uri.clone();
        spawn(async move {
            match compute_tree(id.clone()).await {
                Ok(value) => {
                    result.set(Some(value));
                    if let Ok(saved) = load_tree(id).await {
                        tree.set(Some(saved.clone()));
                        if let Err(message) =
                            mirror_tree_state(&saved, &data_root, &export_uri).await
                        {
                            result_error.set(Some(format!("Saved locally: {message}")));
                        }
                    }
                }
                Err(message) => result_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let class_data_root = data_root.clone();
    let class_export_uri = export_uri.clone();
    let save_class_fixes = move |_| {
        let Some(value) = tree.read().clone() else {
            return;
        };
        busy.set(true);
        let data_root = class_data_root.clone();
        let export_uri = class_export_uri.clone();
        spawn(async move {
            match save_tree_portable(value, &data_root, &export_uri).await {
                Ok((saved, warning)) => {
                    tree.set(Some(saved));
                    if let Some(message) = warning {
                        result_error.set(Some(format!("Saved locally: {message}")));
                    }
                }
                Err(message) => result_error.set(Some(message)),
            }
            busy.set(false);
        });
    };
    let metrics = result.read().as_ref().map(|value| value.result.clone());

    rsx! {
        div { class: "results-screen",
            div { class: "result-summary",
                MetricValue { value: metrics.as_ref().map(|value| value.unique_count).unwrap_or(0), label: "Unique" }
                MetricValue { value: metrics.as_ref().map(|value| value.raw_count).unwrap_or(0), label: "Raw" }
                MetricValue { value: metrics.as_ref().map(|value| value.linked_count).unwrap_or(0), label: "Linked" }
                MetricValue { value: metrics.as_ref().map(|value| value.unassigned_count).unwrap_or(unassigned), label: "U" }
            }
            if let Some(message) = result_error.read().as_ref() {
                div { class: "inline-error compact", "{message}" }
            }
            if let Some(message) = export_notice.read().as_ref() {
                div { class: "inline-notice compact", "{message}" }
            }
            if !mismatch_rows.is_empty() {
                div { class: "mismatch-list",
                    for (index, group) in mismatch_rows.iter().enumerate() {
                        div { class: "mismatch-row",
                            span { "Mismatch {index + 1}" }
                            for (_, _, class_name) in group {
                                code { "{class_name}" }
                            }
                            for (class_id, class_name) in [(0, "B1"), (1, "B2"), (2, "B3"), (3, "B4")] {
                                button {
                                    class: "class-chip",
                                    onclick: {
                                        let endpoint = group.first().cloned();
                                        move |_| {
                                            if let Some((side_index, bbox_id, _)) = endpoint.as_ref() {
                                                if let Some(value) = tree.write().as_mut() {
                                                    set_connected_bbox_class(value, *side_index, bbox_id, class_id);
                                                }
                                            }
                                        }
                                    },
                                    "{class_name}"
                                }
                            }
                        }
                    }
                    button { class: "button secondary", disabled: *busy.read(), onclick: save_class_fixes, "Save classes" }
                }
            }
            button {
                class: "button primary compute-button",
                disabled: *busy.read() || unassigned > 0 || !mismatch_rows.is_empty(),
                onclick: compute,
                if *busy.read() { "Working..." } else { "Compute & mark complete" }
            }
            if let Some(value) = result.read().as_ref() {
                div { class: "class-counts",
                    for (class_name, count) in &value.result.class_counts {
                        div { strong { "{class_name}" } span { "{count}" } }
                    }
                }
                if !value.quality.ready {
                    div { class: "quality-list",
                        for issue in &value.quality.issues {
                            span { "{issue.message}" }
                        }
                    }
                }
            }
            div { class: "export-grid",
                for (kind, label) in [
                    ("output", "Save output"),
                    ("yolo", "YOLO"),
                    ("session", "Session JSON"),
                    ("csv", "CSV"),
                    ("identity", "Identity"),
                ] {
                    button {
                        class: "button secondary",
                        disabled: *busy.read() || result.read().is_none(),
                        onclick: {
                            let id = tree_id.clone();
                            move |_| {
                                let Some(id) = id.clone() else {
                                    return;
                                };
                                busy.set(true);
                                result_error.set(None);
                                export_notice.set(None);
                                spawn(async move {
                                    match export_tree(id, kind).await {
                                        Ok(data) => match mirror_export(data).await {
                                            Ok(count) => export_notice.set(Some(format!("{count} file(s) saved."))),
                                            Err(message) => result_error.set(Some(message)),
                                        },
                                        Err(message) => result_error.set(Some(message)),
                                    }
                                    busy.set(false);
                                });
                            }
                        },
                        "{label}"
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn DepthViewer(tree_id: Option<String>) -> Element {
    let mut tree = use_signal(|| None::<TreeData>);
    let mut side_index = use_signal(|| 0_usize);
    let mut preview = use_signal(|| None::<DepthRenderData>);
    let mut busy = use_signal(|| false);
    let mut depth_error = use_signal(|| None::<String>);

    let id_for_load = tree_id.clone();
    use_effect(move || {
        let Some(id) = id_for_load.clone() else {
            depth_error.set(Some("No tree selected.".into()));
            return;
        };
        spawn(async move {
            match load_tree(id.clone()).await {
                Ok(value) => {
                    let first = value
                        .sides
                        .iter()
                        .position(|side| side.depth_path.is_some())
                        .unwrap_or(0);
                    side_index.set(first);
                    tree.set(Some(value));
                    busy.set(true);
                    match render_depth(id, first).await {
                        Ok(value) => preview.set(Some(value)),
                        Err(message) => depth_error.set(Some(message)),
                    }
                    busy.set(false);
                }
                Err(message) => depth_error.set(Some(message)),
            }
        });
    });

    let sides = tree
        .read()
        .as_ref()
        .map(|value| {
            value
                .sides
                .iter()
                .map(|side| {
                    (
                        side.side_index,
                        side.label.clone(),
                        side.depth_path.is_some(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let preview_url = preview
        .read()
        .as_ref()
        .map(|value| convert_file_src(&value.path));

    rsx! {
        div { class: "depth-screen",
            div { class: "depth-tabs",
                for (index, label, has_depth) in sides {
                    button {
                        class: if *side_index.read() == index { "active" } else { "" },
                        disabled: !has_depth || *busy.read(),
                        onclick: {
                            let id = tree_id.clone();
                            move |_| {
                                let Some(id) = id.clone() else {
                                    return;
                                };
                                side_index.set(index);
                                busy.set(true);
                                depth_error.set(None);
                                spawn(async move {
                                    match render_depth(id, index).await {
                                        Ok(value) => preview.set(Some(value)),
                                        Err(message) => depth_error.set(Some(message)),
                                    }
                                    busy.set(false);
                                });
                            }
                        },
                        "{label}"
                    }
                }
            }
            if let Some(message) = depth_error.read().as_ref() {
                div { class: "inline-error compact", "{message}" }
            }
            div { class: "depth-stage",
                if let Some(url) = preview_url {
                    img { src: "{url}", alt: "Depth preview" }
                }
                if let Some(value) = preview.read().as_ref() {
                    span { class: "depth-min", "{value.minimum:.0} mm" }
                    span { class: "depth-size", "{value.width} x {value.height}" }
                    span { class: "depth-max", "{value.maximum:.0} mm" }
                }
            }
        }
    }
}
