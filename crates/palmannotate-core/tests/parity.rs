use palmannotate_core::{
    build_output_v4, compute_results, parse_yolo, serialize_yolo, BBox, ConfirmedLink, Side, Tree,
    TreeMetadata, TreeStatus, SCHEMA_VERSION, UNASSIGNED_CLASS_ID,
};

fn bbox(id: &str, class_id: i32, coords: [f64; 4]) -> BBox {
    BBox {
        id: id.into(),
        class_id,
        class_name: BBox::class_name_for(class_id).into(),
        x1: coords[0],
        y1: coords[1],
        x2: coords[2],
        y2: coords[3],
        confidence: None,
    }
}

fn tree() -> Tree {
    Tree {
        version: SCHEMA_VERSION,
        id: "tree-1".into(),
        session_id: "session-1".into(),
        tree_name: "DAMIMAS_A21B_0001".into(),
        split: "train".into(),
        side_count: 2,
        metadata: TreeMetadata {
            variety: "DAMIMAS".into(),
            timestamp: "2026-06-06T00:00:00Z".into(),
            ..Default::default()
        },
        sides: vec![
            Side {
                side_index: 0,
                label: "Side 1".into(),
                image_path: "DAMIMAS_A21B_0001_1.jpg".into(),
                image_width: 1000,
                image_height: 1000,
                depth_path: None,
                depth: None,
                original_bboxes: Vec::new(),
                cache_bust: None,
                bboxes: vec![bbox("b0", 1, [100.0, 100.0, 300.0, 300.0])],
            },
            Side {
                side_index: 1,
                label: "Side 2".into(),
                image_path: "DAMIMAS_A21B_0001_2.jpg".into(),
                image_width: 1000,
                image_height: 1000,
                depth_path: None,
                depth: None,
                original_bboxes: Vec::new(),
                cache_bust: None,
                bboxes: vec![bbox("b0", 1, [120.0, 120.0, 320.0, 320.0])],
            },
        ],
        confirmed_links: vec![ConfirmedLink {
            link_id: "L0".into(),
            side_a: 0,
            bbox_id_a: "b0".into(),
            side_b: 1,
            bbox_id_b: "b0".into(),
        }],
        status: TreeStatus::Annotated,
    }
}

#[test]
fn yolo_round_trip_and_unassigned_filter_match_legacy_behavior() {
    let parsed = parse_yolo("1 0.5 0.5 0.2 0.4", 1000, 500);
    assert_eq!(parsed[0].x1, 400.0);
    assert_eq!(parsed[0].y1, 150.0);
    let mut boxes = parsed;
    boxes.push(bbox("b1", UNASSIGNED_CLASS_ID, [10.0, 10.0, 20.0, 20.0]));
    let text = serialize_yolo(&boxes, 1000, 500);
    assert_eq!(text.lines().count(), 1);
    assert!(text.starts_with("1 "));
}

#[test]
fn results_use_union_find_and_ignore_stale_links() {
    let result = compute_results(&tree());
    assert_eq!(result.raw_count, 2);
    assert_eq!(result.linked_count, 1);
    assert_eq!(result.unique_count, 1);
    assert_eq!(result.class_counts["B2"], 1);
}

#[test]
fn output_v4_preserves_stable_fields() {
    let output = build_output_v4(&tree());
    assert_eq!(output.version, 4);
    assert_eq!(output.tree_id, "DAMIMAS_A21B_0001");
    assert_eq!(output.images["side_1"].annotations[0].box_index, 0);
    assert_eq!(
        output.images["side_1"].annotations[0].bbox_yolo,
        [0.2, 0.2, 0.2, 0.2]
    );
    assert_eq!(output.confirmed_links[0].side_a, 0);
    assert_eq!(output.confirmed_links[0].bbox_id_a, "b0");
    assert_eq!(output.summary.total_unique_bunches, 1);
}

#[test]
fn output_drops_non_adjacent_links_in_four_side_ring() {
    let mut value = tree();
    value.side_count = 4;
    value.sides.extend([
        Side {
            side_index: 2,
            label: "Side 3".into(),
            ..value.sides[0].clone()
        },
        Side {
            side_index: 3,
            label: "Side 4".into(),
            ..value.sides[0].clone()
        },
    ]);
    value.confirmed_links.push(ConfirmedLink {
        link_id: "L1".into(),
        side_a: 0,
        bbox_id_a: "b0".into(),
        side_b: 2,
        bbox_id_b: "b0".into(),
    });
    assert_eq!(build_output_v4(&value).confirmed_links.len(), 1);
}
