//! Palette colori della treemap.

use eframe::egui::Color32;

/// Colore (HSV -> RGB) distinto per ogni indice di blocco.
pub fn color_for(index: usize) -> Color32 {
    let h = (index as f32 * 0.137).fract();
    let (s, v) = (0.45_f32, 0.85_f32);
    let channel = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).min(1.0).max(0.0)
    };
    Color32::from_rgb(
        (channel(5.0) * 255.0) as u8,
        (channel(3.0) * 255.0) as u8,
        (channel(1.0) * 255.0) as u8,
    )
}
