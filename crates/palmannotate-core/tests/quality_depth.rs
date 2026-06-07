use palmannotate_core::{
    check_tree, depth_color, depth_display_range, BBox, ConfirmedLink, DepthMetadata, GpsPoint,
    QualityLevel, Side, Tree, TreeMetadata, TreeStatus, DEPTH_CEILING_MM, DEPTH_FLOOR_MM,
    SCHEMA_VERSION,
};

fn bbox(id: &str, class_id: i32) -> BBox {
    BBox {
        id: id.into(),
        class_id,
        class_name: BBox::class_name_for(class_id).into(),
        x1: 1.0,
        y1: 1.0,
        x2: 10.0,
        y2: 10.0,
        confidence: None,
    }
}

fn side(index: usize, class_id: i32, depth: bool) -> Side {
    Side {
        side_index: index,
        label: format!("Side {}", index + 1),
        image_path: format!("images/field/TREE_{}.jpg", index + 1),
        image_width: 100,
        image_height: 100,
        depth_path: depth.then(|| format!("depth/field/TREE_{}.raw", index + 1)),
        depth: depth.then_some(DepthMetadata {
            width: 100,
            height: 100,
            format: "Y16".into(),
            value_scale: 1.0,
        }),
        original_bboxes: Vec::new(),
        cache_bust: None,
        bboxes: vec![bbox(&format!("b{index}"), class_id)],
    }
}

fn tree() -> Tree {
    Tree {
        version: SCHEMA_VERSION,
        id: "tree".into(),
        session_id: "session".into(),
        tree_name: "DAMIMAS_A21B_0001".into(),
        split: "field".into(),
        side_count: 4,
        metadata: TreeMetadata {
            variety: "DAMIMAS".into(),
            block: "A21B".into(),
            operator: "Operator".into(),
            timestamp: "2026-06-07T00:00:00Z".into(),
            gps: Some(GpsPoint {
                latitude: -1.0,
                longitude: 116.0,
                accuracy: Some(8.0),
            }),
        },
        sides: vec![
            side(0, 1, true),
            side(1, 2, false),
            side(2, 1, false),
            side(3, 1, false),
        ],
        confirmed_links: vec![ConfirmedLink {
            link_id: "L0".into(),
            side_a: 0,
            bbox_id_a: "b0".into(),
            side_b: 1,
            bbox_id_b: "b1".into(),
        }],
        status: TreeStatus::Annotated,
    }
}

#[test]
fn quality_flags_depth_gaps_and_linked_class_mismatch() {
    let report = check_tree(&tree());
    assert!(!report.ready);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "tree_rgb_depth_incomplete"));
    assert!(report.issues.iter().any(|issue| {
        issue.code == "annotation_class_mismatch" && issue.level == QualityLevel::Error
    }));
}

#[test]
fn quality_treats_missing_gps_as_recoverable_warning() {
    let mut value = tree();
    value.metadata.gps = None;
    value.confirmed_links.clear();
    value.sides[1].bboxes[0] = bbox("b1", 1);
    let report = check_tree(&value);
    assert!(report
        .issues
        .iter()
        .find(|issue| issue.code == "metadata_gps_missing")
        .is_some_and(|issue| issue.level == QualityLevel::Warning));
}

#[test]
fn depth_range_filters_invalid_values_and_uses_robust_percentiles() {
    let mut values = vec![0, u16::MAX, 100, 8000];
    values.extend(300_u16..=399);
    values.push(7000);
    let range = depth_display_range(&values, 1.0);
    assert_eq!(range.valid, 101);
    assert_eq!(range.minimum_mm, 302.0);
    assert_eq!(range.maximum_mm, 398.0);
    assert_eq!(range.median_mm, 350.0);
}

#[test]
fn depth_range_falls_back_when_no_display_depth_is_valid() {
    let range = depth_display_range(&[0, 1, 100, u16::MAX], 1.0);
    assert_eq!(range.minimum_mm, DEPTH_FLOOR_MM);
    assert_eq!(range.maximum_mm, DEPTH_CEILING_MM);
    assert_eq!(range.valid, 0);
}

#[test]
fn depth_color_is_black_outside_display_range() {
    assert_eq!(depth_color(0.0, 300.0, 3000.0), [0, 0, 0]);
    assert_eq!(depth_color(8000.0, 300.0, 3000.0), [0, 0, 0]);
    assert_ne!(depth_color(1000.0, 300.0, 3000.0), [0, 0, 0]);
}
