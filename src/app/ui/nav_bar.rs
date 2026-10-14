//! Barra di navigazione (Indietro / Avanti / Su) con percorso corrente.

use eframe::egui;

use crate::app::MacDirViewsApp;

impl MacDirViewsApp {
    pub(in crate::app) fn nav_bar_ui(&mut self, ui: &mut egui::Ui) {
        let mut go_up = false;
        let mut go_back = false;
        let mut go_fwd = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.back_stack.is_empty(), egui::Button::new("< Indietro"))
                .clicked()
            {
                go_back = true;
            }
            if ui
                .add_enabled(!self.fwd_stack.is_empty(), egui::Button::new("Avanti >"))
                .clicked()
            {
                go_fwd = true;
            }
            let can_up = self.view_dir != self.scan_root
                && self.view_dir.parent().is_some()
                && !self.scan_root.as_os_str().is_empty();
            if ui.add_enabled(can_up, egui::Button::new("Su")).clicked() {
                go_up = true;
            }
            ui.label(self.view_dir.display().to_string());
        });
        // Tasto "indietro" del mouse (se presente).
        if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1)) {
            go_back = true;
        }
        if go_back {
            self.go_back();
        } else if go_fwd {
            self.go_forward();
        } else if go_up {
            if let Some(parent) = self.view_dir.parent().map(|p| p.to_path_buf()) {
                self.navigate_to(parent);
            }
        }
    }
}
