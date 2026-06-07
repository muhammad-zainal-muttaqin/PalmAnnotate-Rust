use palmannotate_core::{decode_yolo, DetectorConfig, Letterbox, UNASSIGNED_CLASS_ID};

fn set_channels_first(data: &mut [f32], rows: usize, row: usize, values: [f32; 5]) {
    for (attribute, value) in values.into_iter().enumerate() {
        data[attribute * rows + row] = value;
    }
}

#[test]
fn detector_matches_legacy_threshold_letterbox_nms_and_class_contract() {
    let rows = 20;
    let attributes = 5;
    let mut data = vec![0.0; attributes * rows];
    set_channels_first(&mut data, rows, 0, [320.0, 240.0, 100.0, 80.0, 0.9]);
    set_channels_first(&mut data, rows, 1, [325.0, 245.0, 100.0, 80.0, 0.8]);
    set_channels_first(&mut data, rows, 2, [100.0, 400.0, 60.0, 60.0, 0.5]);
    set_channels_first(&mut data, rows, 3, [500.0, 300.0, 50.0, 50.0, 0.04]);
    set_channels_first(&mut data, rows, 4, [200.0, 200.0, 0.4, 0.4, 0.9]);

    let letterbox = Letterbox::new(1280, 960, 640).unwrap();
    let boxes = decode_yolo(
        &data,
        &[1, attributes, rows],
        letterbox,
        &DetectorConfig::default(),
    );

    assert_eq!(boxes.len(), 3);
    assert_eq!(boxes[0].id, "det0");
    assert_eq!(boxes[0].class_id, UNASSIGNED_CLASS_ID);
    assert_eq!(boxes[0].class_name, "U");
    assert_eq!(
        [boxes[0].x1, boxes[0].y1, boxes[0].x2, boxes[0].y2],
        [540.0, 240.0, 740.0, 400.0]
    );
    assert_eq!(
        [boxes[1].x1, boxes[1].y1, boxes[1].x2, boxes[1].y2],
        [140.0, 580.0, 260.0, 700.0]
    );
    assert_eq!(
        [boxes[2].x1, boxes[2].y1, boxes[2].x2, boxes[2].y2],
        [950.0, 390.0, 1050.0, 490.0]
    );
}

#[test]
fn detector_accepts_transposed_output_orientation() {
    let rows = 20;
    let attributes = 5;
    let mut data = vec![0.0; attributes * rows];
    data[..5].copy_from_slice(&[320.0, 240.0, 100.0, 80.0, 0.9]);

    let boxes = decode_yolo(
        &data,
        &[1, rows, attributes],
        Letterbox::new(1280, 960, 640).unwrap(),
        &DetectorConfig::default(),
    );

    assert_eq!(boxes.len(), 1);
    assert_eq!(
        [boxes[0].x1, boxes[0].y1, boxes[0].x2, boxes[0].y2],
        [540.0, 240.0, 740.0, 400.0]
    );
}

#[test]
fn detector_config_deserializes_the_committed_contract() {
    let config: DetectorConfig =
        serde_json::from_str(include_str!("../../../models/detector.config.json")).unwrap();
    assert_eq!(config.model_file, "ffb-detector.onnx");
    assert_eq!(config.input_size, 640);
    assert_eq!(config.conf_threshold, 0.01);
    assert_eq!(config.iou_threshold, 0.30);
    assert_eq!(config.max_boxes, 300);
    assert!(!config.class_aware);
}
