//! Finestra di conferma eliminazione.

use eframe::egui;
use egui::Color32;

use crate::app::MacDirViewsApp;
use crate::utils::format::fmt_size;

impl MacDirViewsApp {
    pub(in crate::app) fn delete_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some((path, permanent)) = self.pending_delete.clone() else {
            return;
        };
        let size = self.tree.lock().unwrap().size_of(&path);
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new("Conferma eliminazione")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(path.display().to_string());
                ui.label(format!("Dimensione: {}", fmt_size(size)));
                ui.add_space(6.0);
                if permanent {
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        "ATTENZIONE: eliminazione definitiva, non si puo' annullare.",
                    );
                } else {
                    ui.label("La cartella verra' spostata nel Cestino.");
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let label = if permanent { "Elimina definitivamente" } else { "Sposta nel Cestino" };
                    if ui.button(label).clicked() {
                        confirm = true;
                    }
                    if ui.button("Annulla").clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            self.pending_delete = None;
            self.start_delete(ctx, path, permanent);
        } else if cancel {
            self.pending_delete = None;
        }
    }
}
