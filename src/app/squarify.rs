//! Algoritmo Squarified Treemap (Bruls, Huizing, van Wijk 2000).
//!
//! Dato un rettangolo e una lista di aree ordinate per grandezza decrescente,
//! produce un layout in cui i blocchi sono il piu' possibile quadrati.

use eframe::egui::{Pos2, Rect, Vec2};

/// Un elemento da posizionare nella treemap.
#[derive(Clone, Debug)]
pub struct TreemapItem {
    pub index: usize,
    pub area: f64,
}

/// Risultato del layout: rettangolo assegnato a ogni elemento.
#[derive(Clone, Debug)]
pub struct LayoutRect {
    pub index: usize,
    pub rect: Rect,
}

/// Calcola il layout squarified per gli elementi dati nel rettangolo `bounds`.
/// Gli elementi devono essere ordinati per `area` decrescente.
pub fn squarify(items: &[TreemapItem], bounds: Rect) -> Vec<LayoutRect> {
    if items.is_empty() || bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return Vec::new();
    }

    let total_area: f64 = items.iter().map(|i| i.area).sum();
    if total_area <= 0.0 {
        return Vec::new();
    }

    // Scala le aree per mappare sull'area del rettangolo.
    let rect_area = bounds.width() as f64 * bounds.height() as f64;
    let scale = rect_area / total_area;
    let scaled: Vec<f64> = items.iter().map(|i| i.area * scale).collect();

    let mut result = Vec::with_capacity(items.len());
    let mut remaining = bounds;
    let mut i = 0;

    while i < items.len() {
        let short_side = remaining.width().min(remaining.height()) as f64;
        if short_side <= 0.0 {
            break;
        }

        // Trova la riga che massimizza l'aspect ratio.
        let mut row: Vec<usize> = Vec::new();
        let mut row_area = 0.0;
        let mut best_ratio = f64::MAX;

        for j in i..items.len() {
            row.push(j);
            row_area += scaled[j];
            let ratio = worst_ratio(&row, &scaled, row_area, short_side);
            if ratio > best_ratio {
                // L'aggiunta peggiora: togli l'ultimo e ferma.
                row.pop();
                row_area -= scaled[j];
                break;
            }
            best_ratio = ratio;
        }

        if row.is_empty() {
            break;
        }

        // Disponi la riga nel rettangolo rimanente.
        let row_fraction = row_area / (remaining.width() as f64 * remaining.height() as f64);
        let is_horizontal = remaining.width() >= remaining.height();

        if is_horizontal {
            let row_width = (remaining.width() as f64 * row_fraction) as f32;
            let mut y = remaining.top();
            for &idx in &row {
                let h = if row_area > 0.0 {
                    (scaled[idx] / row_area * remaining.height() as f64) as f32
                } else {
                    0.0
                };
                result.push(LayoutRect {
                    index: items[idx].index,
                    rect: Rect::from_min_size(
                        Pos2::new(remaining.left(), y),
                        Vec2::new(row_width, h),
                    ),
                });
                y += h;
            }
            remaining = Rect::from_min_max(
                Pos2::new(remaining.left() + row_width, remaining.top()),
                remaining.right_bottom(),
            );
        } else {
            let row_height = (remaining.height() as f64 * row_fraction) as f32;
            let mut x = remaining.left();
            for &idx in &row {
                let w = if row_area > 0.0 {
                    (scaled[idx] / row_area * remaining.width() as f64) as f32
                } else {
                    0.0
                };
                result.push(LayoutRect {
                    index: items[idx].index,
                    rect: Rect::from_min_size(
                        Pos2::new(x, remaining.top()),
                        Vec2::new(w, row_height),
                    ),
                });
                x += w;
            }
            remaining = Rect::from_min_max(
                Pos2::new(remaining.left(), remaining.top() + row_height),
                remaining.right_bottom(),
            );
        }

        i += row.len();
    }

    result
}

/// Calcola il peggior aspect ratio nella riga corrente.
fn worst_ratio(row: &[usize], scaled: &[f64], row_area: f64, short_side: f64) -> f64 {
    if row_area <= 0.0 || short_side <= 0.0 {
        return f64::MAX;
    }
    let mut worst = 0.0_f64;
    for &idx in row {
        let area = scaled[idx];
        if area <= 0.0 {
            continue;
        }
        // Lunghezza del lato lungo del blocco.
        let long = row_area / short_side;
        let w = area / long;
        let ratio = if long > w { long / w } else { w / long };
        worst = worst.max(ratio);
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let result = squarify(&[], Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)));
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_item() {
        let items = vec![TreemapItem { index: 0, area: 100.0 }];
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        let result = squarify(&items, bounds);
        assert_eq!(result.len(), 1);
        // L'unico elemento dovrebbe occupare quasi tutto il rettangolo.
        let r = result[0].rect;
        assert!((r.width() - 100.0).abs() < 1.0);
        assert!((r.height() - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_multiple_items_cover_area() {
        let items = vec![
            TreemapItem { index: 0, area: 60.0 },
            TreemapItem { index: 1, area: 30.0 },
            TreemapItem { index: 2, area: 10.0 },
        ];
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        let result = squarify(&items, bounds);
        assert_eq!(result.len(), 3);

        // Verifica che l'area totale copra approssimativamente il rettangolo.
        let total_area: f32 = result.iter().map(|r| r.rect.width() * r.rect.height()).sum();
        assert!((total_area - 10000.0).abs() < 100.0);
    }
}
