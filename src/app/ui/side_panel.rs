//! Pannello laterale: classifica delle 10 cartelle e dei 10 file piu' grandi,
//! ricerca per nome e filtro soglia minima.

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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // --- Ricerca ---
                    ui.heading("🔍 Ricerca e Filtri");
                    ui.horizontal(|ui| {
                        ui.label("Nome:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Cerca cartella...")
                                .desired_width(180.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Soglia minima:");
                        ui.add(
                            egui::Slider::new(&mut self.filter_min_size_mb, 0.0..=1024.0)
                                .suffix(" MB")
                                .logarithmic(true)
                                .clamping(egui::SliderClamping::Always),
                        );
                    });
                    let needs_refresh =
                        ui.button("Applica filtri").clicked() && !scanning;
                    ui.separator();

                    // --- Top 10 cartelle ---
                    ui.heading("Top 10 cartelle");
                    ui.label(format!("in: {}", self.view_dir.display()));
                    ui.separator();
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

                    ui.add_space(8.0);
                    ui.separator();

                    // --- Top file piu' grandi ---
                    ui.heading("📄 Top file più grandi");
                    if self.top_files.is_empty() {
                        ui.label("Nessun dato.");
                    }
                    for (i, file_info) in self.top_files.iter().enumerate() {
                        let name = file_info
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| file_info.path.display().to_string());
                        let text = format!(
                            "{}. {} — {}\n{}",
                            i + 1,
                            name,
                            fmt_size(file_info.size),
                            file_info.path.display()
                        );
                        let resp = ui.selectable_label(false, text);
                        // Tasto destro per aprire nel Finder.
                        resp.context_menu(|ui| {
                            if ui.button("Apri nel Finder").clicked() {
                                if let Some(parent) = file_info.path.parent() {
                                    side_open = Some(parent.to_path_buf());
                                }
                                ui.close_menu();
                            }
                        });
                        ui.add_space(2.0);
                    }

                    // --- Cartelle saltate ---
                    if self.skipped_dirs > 0 && !scanning {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.heading(format!("⚠ {} cartelle saltate", self.skipped_dirs));
                        ui.label("Queste cartelle non erano leggibili (permesso negato).");
                        for msg in self.skipped_messages.iter().take(20) {
                            ui.small(msg);
                        }
                        if self.skipped_messages.len() > 20 {
                            ui.small(format!(
                                "...e altre {} non mostrate.",
                                self.skipped_messages.len() - 20
                            ));
                        }
                    }

                    if needs_refresh {
                        // Flag per rinfrescare dopo aver applicato i filtri.
                        // Gestito dopo il blocco UI.
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
