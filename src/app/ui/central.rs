//! Pannello centrale: modalità di visualizzazione (Treemap, Albero, Split View),
//! navigazione a breadcrumbs, informazioni e menu contestuale.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Stroke};

use super::folder_menu::folder_menu;
use crate::app::squarify::{squarify, TreemapItem};
use crate::app::{MacDirViewsApp, ViewMode};
use crate::system::finder::open_in_file_manager_label;
use crate::utils::color::color_for;
use crate::utils::format::fmt_size;

impl MacDirViewsApp {
    pub(in crate::app) fn central_ui(&mut self, ui: &mut egui::Ui, scanning: bool) {
        // --- Barra di navigazione (Breadcrumbs interattivi) ---
        self.nav_bar_ui(ui);

        ui.horizontal(|ui| {
            if scanning {
                ui.spinner();
                ui.label(format!(
                    "Scansione in corso... {} file analizzati",
                    self.files_counter.load(Ordering::Relaxed)
                ));
            } else if self.total_size > 0 {
                ui.label(format!("Totale scansionato: {}", fmt_size(self.total_size)));
                if self.skipped_dirs > 0 {
                    ui.colored_label(
                        Color32::YELLOW,
                        format!(" ({} cartelle non leggibili)", self.skipped_dirs),
                    );
                }
            }

            // Selettore Vista: Treemap, Albero, Split View
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.selectable_value(&mut self.view_mode, ViewMode::Split, "🔀 Split View");
                ui.selectable_value(&mut self.view_mode, ViewMode::Tree, "🌲 Albero");
                ui.selectable_value(&mut self.view_mode, ViewMode::Treemap, "🗺 Treemap");
                ui.label("Modalità:");
            });
        });

        if let Some(msg) = &self.status {
            ui.colored_label(Color32::LIGHT_RED, msg);
        }

        ui.add_space(4.0);

        // Rendering in base alla modalità selezionata
        match self.view_mode {
            ViewMode::Treemap => {
                let avail = ui.available_size();
                let height = (avail.y - 45.0).max(120.0);
                self.render_treemap(ui, scanning, height);
            }
            ViewMode::Tree => {
                self.tree_view_ui(ui, scanning);
            }
            ViewMode::Split => {
                let avail = ui.available_size();
                let half_height = ((avail.y - 60.0) * 0.48).max(120.0);

                // Metà superiore: Albero gerarchico
                ui.group(|ui| {
                    ui.set_height(half_height);
                    self.tree_view_ui(ui, scanning);
                });

                ui.separator();

                // Metà inferiore: Treemap interattiva
                let remaining = ui.available_size().y - 45.0;
                self.render_treemap(ui, scanning, remaining.max(120.0));
            }
        }
    }

    /// Disegna la Treemap Squarified con painter nativo e gestisce click/hover.
    fn render_treemap(&mut self, ui: &mut egui::Ui, scanning: bool, height: f32) {
        let avail = ui.available_size();
        let size = egui::vec2(avail.x.max(50.0), height);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click());
        let rect = response.rect;

        painter.rect_filled(rect, 0.0, Color32::from_gray(30));

        let total: u64 = self.blocks.iter().map(|b| b.size).sum();
        let pointer: Option<Pos2> = response.hover_pos();
        let mut hovered: Option<usize> = None;

        if total > 0 {
            let items: Vec<TreemapItem> = self
                .blocks
                .iter()
                .enumerate()
                .map(|(i, b)| TreemapItem {
                    index: i,
                    area: b.size as f64,
                })
                .collect();

            let layout = squarify(&items, rect);

            for lr in &layout {
                let i = lr.index;
                let block = &self.blocks[i];
                let r = lr.rect;

                let is_hover = pointer.map_or(false, |p| r.contains(p));
                if is_hover {
                    hovered = Some(i);
                }
                let is_selected = self.selected.as_ref() == Some(&block.path) && block.is_dir;

                let mut color = if block.is_dir {
                    color_for(i)
                } else {
                    Color32::from_gray(150)
                };
                if is_hover {
                    color = color.gamma_multiply(1.15);
                }
                let inner = r.shrink(1.0);
                painter.rect_filled(inner, 2.0, color);
                if is_selected {
                    painter.rect_stroke(inner, 2.0, Stroke::new(3.0_f32, Color32::WHITE));
                }

                // Etichetta testuale
                let min_label_w = 50.0;
                let min_label_h = 30.0;
                if inner.width() > min_label_w && inner.height() > min_label_h {
                    let clipped = painter.with_clip_rect(inner);
                    let font_size = if inner.width() > 120.0 && inner.height() > 50.0 {
                        13.0
                    } else {
                        11.0
                    };
                    clipped.text(
                        inner.left_top() + egui::vec2(4.0, 4.0),
                        Align2::LEFT_TOP,
                        &block.label,
                        FontId::proportional(font_size),
                        Color32::BLACK,
                    );

                    if inner.height() > 40.0 {
                        clipped.text(
                            inner.left_top() + egui::vec2(4.0, 4.0 + font_size + 4.0),
                            Align2::LEFT_TOP,
                            fmt_size(block.size),
                            FontId::proportional(font_size - 1.0),
                            Color32::from_black_alpha(200),
                        );
                    }

                    if inner.width() > 80.0 && inner.height() > 55.0 {
                        let pct = block.size as f64 * 100.0 / total as f64;
                        clipped.text(
                            inner.left_top() + egui::vec2(4.0, 4.0 + font_size + 4.0 + font_size),
                            Align2::LEFT_TOP,
                            format!("{:.1}%", pct),
                            FontId::proportional(font_size - 2.0),
                            Color32::from_black_alpha(160),
                        );
                    }
                } else if inner.width() > 25.0 && inner.height() > 15.0 {
                    let short: String = block.label.chars().take(3).collect();
                    let clipped = painter.with_clip_rect(inner);
                    clipped.text(
                        inner.center(),
                        Align2::CENTER_CENTER,
                        short,
                        FontId::proportional(9.0),
                        Color32::from_black_alpha(180),
                    );
                }
            }
        } else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                if scanning {
                    "Scansione in corso..."
                } else {
                    "Premi \"Start Scan\" o trascina una cartella per iniziare"
                },
                FontId::proportional(16.0),
                Color32::GRAY,
            );
        }

        // Tasto destro: seleziona il blocco sotto il mouse
        if response.secondary_clicked() {
            self.ctx_target = hovered.and_then(|i| {
                let b = &self.blocks[i];
                if b.is_dir {
                    Some(b.path.clone())
                } else {
                    None
                }
            });
            if let Some(p) = self.ctx_target.clone() {
                self.selected = Some(p);
            }
        }

        // Click = seleziona, doppio click = naviga
        let mut new_view: Option<PathBuf> = None;
        if let Some(i) = hovered {
            let block = &self.blocks[i];
            if response.clicked() {
                self.selected = Some(block.path.clone());
            }
            if response.double_clicked() && block.is_dir {
                new_view = Some(block.path.clone());
            }
        }

        // Pannello informazioni in basso
        ui.add_space(4.0);
        ui.separator();
        let mut open_path: Option<PathBuf> = None;
        ui.horizontal(|ui| {
            let can_open = self.selected.is_some() || !self.view_dir.as_os_str().is_empty();
            if ui
                .add_enabled(can_open, egui::Button::new(open_in_file_manager_label()))
                .clicked()
            {
                open_path = self.selected.clone().or_else(|| Some(self.view_dir.clone()));
            }
            match hovered {
                Some(i) => {
                    let b = &self.blocks[i];
                    let pct = if total > 0 {
                        b.size as f64 * 100.0 / total as f64
                    } else {
                        0.0
                    };
                    ui.label(format!(
                        "{}  -  {} ({:.1}%)",
                        b.path.display(),
                        fmt_size(b.size),
                        pct
                    ));
                }
                None => match &self.selected {
                    Some(p) => {
                        ui.label(format!("Selezionato: {}", p.display()));
                    }
                    None => {
                        ui.label("Clic = seleziona, doppio clic = entra nella cartella.");
                    }
                },
            }
        });

        // Menu contestuale (tasto destro)
        let can_modify = !scanning;
        let mut del_req: Option<(PathBuf, bool)> = None;
        response.context_menu(|ui| match self.ctx_target.clone() {
            Some(p) => folder_menu(ui, &p, can_modify, &mut open_path, &mut del_req),
            None => {
                ui.label("Nessuna cartella qui.");
            }
        });
        if let Some(d) = del_req {
            self.pending_delete = Some(d);
        }

        if let Some(i) = hovered {
            let b = &self.blocks[i];
            let tip = format!("{}\n{}", b.path.display(), fmt_size(b.size));
            response.on_hover_text(tip);
        }

        if let Some(p) = open_path {
            self.open_in_finder(&p);
        }
        if let Some(dir) = new_view {
            self.navigate_to(dir);
        }
    }
}
