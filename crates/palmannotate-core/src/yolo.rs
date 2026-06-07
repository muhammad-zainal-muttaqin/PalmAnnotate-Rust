use crate::{BBox, UNASSIGNED_CLASS_ID};

pub fn parse_yolo(text: &str, image_width: u32, image_height: u32) -> Vec<BBox> {
    let width = f64::from(image_width);
    let height = f64::from(image_height);
    text.lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 5 {
                return None;
            }
            let class_id: i32 = fields[0].parse().ok()?;
            if !(0..=3).contains(&class_id) {
                return None;
            }
            let cx: f64 = fields[1].parse().ok()?;
            let cy: f64 = fields[2].parse().ok()?;
            let w: f64 = fields[3].parse().ok()?;
            let h: f64 = fields[4].parse().ok()?;
            Some((class_id, cx, cy, w, h))
        })
        .enumerate()
        .map(|(index, (class_id, cx, cy, w, h))| BBox {
            id: format!("b{index}"),
            class_id,
            class_name: BBox::class_name_for(class_id).into(),
            x1: ((cx - w / 2.0) * width).max(0.0),
            y1: ((cy - h / 2.0) * height).max(0.0),
            x2: ((cx + w / 2.0) * width).min(width),
            y2: ((cy + h / 2.0) * height).min(height),
            confidence: None,
        })
        .collect()
}

pub fn serialize_yolo(bboxes: &[BBox], image_width: u32, image_height: u32) -> String {
    let width = f64::from(image_width);
    let height = f64::from(image_height);
    bboxes
        .iter()
        .filter(|bbox| bbox.class_id != UNASSIGNED_CLASS_ID && bbox.is_assigned())
        .map(|bbox| {
            let cx = ((bbox.x1 + bbox.x2) / 2.0) / width;
            let cy = ((bbox.y1 + bbox.y2) / 2.0) / height;
            let w = (bbox.x2 - bbox.x1) / width;
            let h = (bbox.y2 - bbox.y1) / height;
            format!("{} {cx:.6} {cy:.6} {w:.6} {h:.6}", bbox.class_id)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
