//! Barra superiore: percorso, sfoglia, avvio/arresto scansione, selettore volume,
//! barra di stato della memoria disco (Disk Usage Bar) e strumenti di pulizia/duplicati.

use eframe::egui;
use egui::Color32;

use crate::app::{ActiveTool, MacDirViewsApp};
use crate::system::protection::{list_volumes, scan_all_label, system_root_path};
use crate::utils::format::fmt_size;

impl MacDirViewsApp {
    pub(in crate::app) fn top_bar_ui(&mut self, ctx: &egui::Context, scanning: bool) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Percorso:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("vuoto = cartella Home")
                        .desired_width(240.0),
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
                    .add_enabled(!scanning, egui::Button::new(scan_all_label()))
                    .clicked()
                {
                    self.path_input = system_root_path().display().to_string();
                    self.start_scan(ctx);
                }

                // Menu volumi / unità esterne.
                ui.menu_button("💾 Volumi...", |ui| {
                    let volumes = list_volumes();
                    if volumes.is_empty() {
                        if cfg!(windows) {
                            ui.label("Nessuna unità disco trovata");
                        } else {
                            ui.label("Nessun volume trovato in /Volumes");
                        }
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

                // Nuovi strumenti rapidi
                let junk_tooltip = if cfg!(windows) {
                    "Pulisci cache temporanee (%TEMP%), dump di crash e package manager"
                } else {
                    "Pulisci cache temporanee, Xcode DerivedData e package manager"
                };

                if ui
                    .button("🧹 Pulizia Rapida")
                    .on_hover_text(junk_tooltip)
                    .clicked()
                {
                    self.active_tool = ActiveTool::JunkCleaner;
                    if self.junk_items.is_empty() {
                        self.start_junk_scan(ctx);
                    }
                }

                if ui
                    .button("👯 File Duplicati")
                    .on_hover_text("Trova file duplicati identici per liberare spazio")
                    .clicked()
                {
                    self.active_tool = ActiveTool::DuplicateFinder;
                    if self.duplicate_groups.is_empty() {
                        self.start_duplicate_scan(ctx);
                    }
                }

                ui.separator();

                if self.is_admin {
                    ui.colored_label(Color32::LIGHT_GREEN, "Admin: attivo");
                } else {
                    ui.colored_label(Color32::YELLOW, "Admin: no");
                    if ui.add_enabled(!scanning, egui::Button::new("Riavvia come admin")).clicked() {
                        self.relaunch_as_admin(ctx);
                    }

                    // Guida permessi elevati specifica per OS
                    #[cfg(target_os = "macos")]
                    ui.menu_button("ℹ Full Disk Access", |ui| {
                        ui.label("Per leggere Mail, Foto e altre cartelle protette:");
                        ui.add_space(4.0);
                        ui.label("1. Apri Impostazioni di Sistema");
                        ui.label("2. Privacy e Sicurezza → Accesso completo al disco");
                        ui.label("3. Clicca '+' e aggiungi MacDirViews");
                        ui.label("4. Riavvia l'app");
                        ui.add_space(4.0);
                        if ui.button("Apri Impostazioni di Sistema").clicked() {
                            let _ = std::process::Command::new("open")
                                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
                                .spawn();
                            ui.close_menu();
                        }
                    });

                    #[cfg(windows)]
                    ui.menu_button("ℹ Privilegi Admin (UAC)", |ui| {
                        ui.label("Per scansionare cartelle di sistema protette:");
                        ui.add_space(4.0);
                        ui.label("1. Clicca su 'Riavvia come admin'");
                        ui.label("2. Conferma la richiesta UAC di Windows");
                        ui.label("3. L'app si riavvierà con diritti completi");
                    });
                }
            });

            ui.add_space(4.0);

            let mut do_refresh_disk = false;
            if let Some(ref usage) = self.disk_usage {
                let ratio = (usage.used_bytes as f64 / usage.total_bytes.max(1) as f64) as f32;
                let pct = ratio * 100.0;
                ui.horizontal(|ui| {
                    ui.label(format!("Capienza {}:", usage.volume_name));
                    let bar = egui::ProgressBar::new(ratio)
                        .text(format!(
                            "{:.1}% usato ({} / {}) — {} liberi",
                            pct,
                            fmt_size(usage.used_bytes),
                            fmt_size(usage.total_bytes),
                            fmt_size(usage.free_bytes)
                        ))
                        .desired_width(380.0);
                    ui.add(bar);

                    if ui.small_button("🔄").on_hover_text("Aggiorna spazio disco").clicked() {
                        do_refresh_disk = true;
                    }
                });
            } else {
                // Tenta di caricare le informazioni sul disco se non presenti
                if self.last_disk_refresh.elapsed() >= std::time::Duration::from_secs(10) {
                    do_refresh_disk = true;
                }
            }

            if do_refresh_disk {
                self.refresh_disk_usage(ctx);
            }

            ui.add_space(2.0);
        });
    }
}
