//! Barra superiore: percorso, avvio scansione e stato amministratore.

use eframe::egui;
use egui::Color32;

use crate::app::MacDirViewsApp;

impl MacDirViewsApp {
    pub(in crate::app) fn top_bar_ui(&mut self, ctx: &egui::Context, scanning: bool) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Percorso:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("vuoto = cartella Home")
                        .desired_width(320.0),
                );
                if ui.add_enabled(!scanning, egui::Button::new("Start Scan")).clicked() {
                    self.start_scan(ctx);
                }
                if ui
                    .add_enabled(!scanning, egui::Button::new("Scansiona tutto il disco (/)"))
                    .clicked()
                {
                    self.path_input = "/".to_string();
                    self.start_scan(ctx);
                }
                ui.separator();
                if self.is_admin {
                    ui.colored_label(Color32::LIGHT_GREEN, "Admin: attivo");
                } else {
                    ui.colored_label(Color32::YELLOW, "Admin: non attivo");
                    if ui.add_enabled(!scanning, egui::Button::new("Riavvia come amministratore")).clicked() {
                        self.relaunch_as_admin(ctx);
                    }
                }
            });
            ui.add_space(6.0);
        });
    }
}
