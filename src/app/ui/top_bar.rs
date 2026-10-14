//! Barra superiore: percorso, sfoglia, avvio/arresto scansione, selettore volume,
//! stato amministratore e guida Full Disk Access.

use eframe::egui;
use egui::Color32;

use crate::app::MacDirViewsApp;
use crate::system::protection::list_volumes;

impl MacDirViewsApp {
    pub(in crate::app) fn top_bar_ui(&mut self, ctx: &egui::Context, scanning: bool) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Percorso:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("vuoto = cartella Home")
                        .desired_width(280.0),
                );
                // Pulsante Sfoglia.
                if ui.add_enabled(!scanning, egui::Button::new("📁 Sfoglia...")).clicked() {
                    self.browse_folder();
                }
                // Pulsante Start / Stop.
                if scanning {
                    if ui.button("⏹ Ferma scansione").clicked() {
                        self.stop_scan();
                    }
                } else {
                    if ui.button("▶ Start Scan").clicked() {
                        self.start_scan(ctx);
                    }
                }
                if ui
                    .add_enabled(!scanning, egui::Button::new("Scansiona tutto il disco (/)"))
                    .clicked()
                {
                    self.path_input = "/".to_string();
                    self.start_scan(ctx);
                }

                // Menu volumi esterni.
                ui.menu_button("💾 Volumi...", |ui| {
                    let volumes = list_volumes();
                    if volumes.is_empty() {
                        ui.label("Nessun volume trovato in /Volumes");
                    }
                    for (name, path) in &volumes {
                        if ui.button(format!("{} ({})", name, path.display())).clicked() {
                            self.path_input = path.display().to_string();
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    ui.label("Seleziona un volume e premi Start.");
                });

                ui.separator();
                if self.is_admin {
                    ui.colored_label(Color32::LIGHT_GREEN, "Admin: attivo");
                } else {
                    ui.colored_label(Color32::YELLOW, "Admin: non attivo");
                    if ui.add_enabled(!scanning, egui::Button::new("Riavvia come amministratore")).clicked() {
                        self.relaunch_as_admin(ctx);
                    }
                    // Guida Full Disk Access.
                    ui.menu_button("ℹ Full Disk Access", |ui| {
                        ui.label("Per leggere Mail, Foto e altre cartelle protette:");
                        ui.add_space(4.0);
                        ui.label("1. Apri Impostazioni di Sistema");
                        ui.label("2. Privacy e Sicurezza → Full Disk Access");
                        ui.label("3. Clicca '+' e aggiungi MacDirViews");
                        ui.label("4. Riavvia l'app");
                        ui.add_space(4.0);
                        ui.label("Questo e' piu' sicuro di lanciare l'app come root.");
                        ui.add_space(4.0);
                        if ui.button("Apri Impostazioni di Sistema").clicked() {
                            let _ = std::process::Command::new("open")
                                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
                                .spawn();
                            ui.close_menu();
                        }
                    });
                }
            });
            ui.add_space(2.0);
            // Informazioni sulle cartelle trascinate.
            ui.horizontal(|ui| {
                ui.small("💡 Puoi trascinare una cartella sulla finestra per scansionarla.");
            });
            ui.add_space(4.0);
        });
    }
}
