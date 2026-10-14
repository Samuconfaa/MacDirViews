//! Pannello laterale: classifica delle 10 cartelle piu' grandi.

use std::path::PathBuf;

use eframe::egui;

use super::folder_menu::folder_menu;
use crate::app::MacDirViewsApp;
use crate::utils::format::fmt_size;

impl MacDirViewsApp {
    pub(in crate::app) fn side_panel_ui(&mut self, ctx: &egui::Context, scanning: bool) {
        let mut clicked_path: Option<PathBuf> = None;
        let mut side_open: Option<PathBuf> = None;
        let mut side_del: Option<(PathBuf, bool)> = None;
        let can_modify = !scanning;
        egui::SidePanel::left("top10_panel")
            .min_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Top 10 cartelle");
                ui.label(format!("in: {}", self.view_dir.display()));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.top10.is_empty() {
                        ui.label("Nessun dato.");
                    }
                    for (i, (path, size)) in self.top10.iter().enumerate() {
                        let text = format!("{}. {}\n{}", i + 1, fmt_size(*size), path.display());
                        let is_sel = self.selected.as_ref() == Some(path);
                        let resp = ui.selectable_label(is_sel, text);
                        if resp.clicked() {
                            clicked_path = Some(path.clone());
                        }
                        // Tasto destro anche sulle voci della Top 10.
                        resp.context_menu(|ui| {
                            folder_menu(ui, path, can_modify, &mut side_open, &mut side_del);
                        });
                        ui.add_space(2.0);
                    }
                });
            });
        if let Some(p) = clicked_path {
            self.selected = Some(p);
        }
        if let Some(p) = side_open {
            self.open_in_finder(&p);
        }
        if let Some(d) = side_del {
            self.pending_delete = Some(d);
        }
    }
}
