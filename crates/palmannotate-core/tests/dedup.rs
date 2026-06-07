use palmannotate_core::{suggest_tree, BBox, Side, Tree, TreeMetadata, TreeStatus, SCHEMA_VERSION};

fn bbox(id: &str, class_id: i32, center_x: f64, center_y: f64, size: f64) -> BBox {
    BBox {
        id: id.into(),
        class_id,
        class_name: BBox::class_name_for(class_id).into(),
        x1: center_x - size / 2.0,
        y1: center_y - size / 2.0,
        x2: center_x + size / 2.0,
        y2: center_y + size / 2.0,
        confidence: None,
    }
}

fn tree(a: Vec<BBox>, b: Vec<BBox>) -> Tree {
    Tree {
        version: SCHEMA_VERSION,
        id: "tree".into(),
        session_id: "session".into(),
        tree_name: "TREE_1".into(),
        split: "field".into(),
        side_count: 2,
        metadata: TreeMetadata::default(),
        sides: vec![
            Side {
                side_index: 0,
                label: "Side 1".into(),
                image_path: "a.jpg".into(),
                image_width: 1000,
                image_height: 1000,
                depth_path: None,
                depth: None,
                original_bboxes: Vec::new(),
                cache_bust: None,
                bboxes: a,
            },
            Side {
                side_index: 1,
                label: "Side 2".into(),
                image_path: "b.jpg".into(),
                image_width: 1000,
                image_height: 1000,
                depth_path: None,
                depth: None,
                original_bboxes: Vec::new(),
                cache_bust: None,
                bboxes: b,
            },
        ],
        confirmed_links: vec![],
        status: TreeStatus::Annotated,
    }
}

#[test]
fn aligned_seam_pair_is_auto() {
    let suggestions = suggest_tree(&tree(
        vec![bbox("a", 1, 100.0, 450.0, 100.0)],
        vec![bbox("b", 1, 900.0, 450.0, 100.0)],
    ));
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].bbox_id_a, "a");
    assert_eq!(suggestions[0].bbox_id_b, "b");
    assert_eq!(suggestions[0].category, "auto");
    assert!(suggestions[0].score > 0.75);
}

#[test]
fn far_edge_and_size_mismatch_are_rejected() {
    assert!(suggest_tree(&tree(
        vec![bbox("a", 1, 800.0, 450.0, 100.0)],
        vec![bbox("b", 1, 900.0, 450.0, 100.0)],
    ))
    .is_empty());
    assert!(suggest_tree(&tree(
        vec![bbox("a", 1, 100.0, 450.0, 100.0)],
        vec![bbox("b", 1, 900.0, 450.0, 20.0)],
    ))
    .is_empty());
}

#[test]
fn mutual_best_keeps_only_reciprocal_match() {
    let suggestions = suggest_tree(&tree(
        vec![
            bbox("close", 1, 100.0, 450.0, 100.0),
            bbox("far", 1, 100.0, 800.0, 100.0),
        ],
        vec![bbox("b", 1, 900.0, 450.0, 100.0)],
    ));
    assert!(suggestions.iter().any(|item| item.bbox_id_a == "close"));
    assert!(!suggestions.iter().any(|item| item.bbox_id_a == "far"));
}
