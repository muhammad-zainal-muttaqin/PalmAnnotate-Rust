#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthDisplayRange {
    pub minimum_mm: f32,
    pub maximum_mm: f32,
    pub median_mm: f32,
    pub valid: usize,
    pub total: usize,
}

pub const DEPTH_FLOOR_MM: f32 = 250.0;
pub const DEPTH_CEILING_MM: f32 = 7000.0;

pub fn depth_display_range(values: &[u16], value_scale: f32) -> DepthDisplayRange {
    let mut valid = values
        .iter()
        .copied()
        .filter(|value| !matches!(*value, 0 | u16::MAX))
        .map(|value| f32::from(value) * value_scale)
        .filter(|value| (DEPTH_FLOOR_MM..=DEPTH_CEILING_MM).contains(value))
        .collect::<Vec<_>>();
    valid.sort_by(f32::total_cmp);
    if valid.is_empty() {
        return DepthDisplayRange {
            minimum_mm: DEPTH_FLOOR_MM,
            maximum_mm: DEPTH_CEILING_MM,
            median_mm: 0.0,
            valid: 0,
            total: values.len(),
        };
    }
    let minimum_mm = percentile(&valid, 0.02);
    let maximum_mm = percentile(&valid, 0.98).max(minimum_mm + 1.0);
    DepthDisplayRange {
        minimum_mm,
        maximum_mm,
        median_mm: percentile(&valid, 0.5),
        valid: valid.len(),
        total: values.len(),
    }
}

pub fn depth_color(value_mm: f32, minimum_mm: f32, maximum_mm: f32) -> [u8; 3] {
    if !(DEPTH_FLOOR_MM..=DEPTH_CEILING_MM).contains(&value_mm) {
        return [0, 0, 0];
    }
    let span = (maximum_mm - minimum_mm).max(1.0);
    let value = ((value_mm - minimum_mm) / span).clamp(0.0, 1.0);
    let red = (1.5 - (4.0 * value - 3.0).abs()).clamp(0.0, 1.0);
    let green = (1.5 - (4.0 * value - 2.0).abs()).clamp(0.0, 1.0);
    let blue = (1.5 - (4.0 * value - 1.0).abs()).clamp(0.0, 1.0);
    [
        (red * 255.0).round() as u8,
        (green * 255.0).round() as u8,
        (blue * 255.0).round() as u8,
    ]
}

fn percentile(sorted: &[f32], percentile: f32) -> f32 {
    let index = ((sorted.len() - 1) as f32 * percentile).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}
