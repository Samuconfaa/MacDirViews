//! Finestra dedicata per il tool "Developer & System Junk" (Pulizia Rapida).

use std::path::PathBuf;

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{ActiveTool, DirTreeViewApp};
use crate::system::finder::{file_manager_button_label, open_in_file_manager_label};
use crate::utils::format::fmt_size;

impl DirTreeViewApp {
    pub(in crate::app) fn junk_cleaner_ui(&mut self, ctx: &egui::Context) {
        let is_scanning = *self.junk_scanning.lock().unwrap();
        let mut close_window = false;
        let mut open_path: Option<PathBuf> = None;
        let mut clean_requested = false;
        let mut rescan_requested = false;

        let title = format!(
            "🧹 Pulizia Rapida ({} & Dev Junk)",
            if cfg!(windows) { "Windows" } else { "macOS" }
        );

        egui::Window::new(title)
            .min_width(650.0)
            .min_height(450.0)
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Pulisci in sicurezza cache temporanee, build di sviluppo e file orfani.");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("❌ Chiudi").clicked() {
                            close_window = true;
                        }
                        if ui.add_enabled(!is_scanning, egui::Button::new("🔄 Aggiorna scansione")).clicked() {
                            rescan_requested = true;
                        }
                    });
                });

                ui.separator();

                if is_scanning {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label("Scansione cartelle di sviluppo e cache in corso...");
                    });
                    return;
                }

                if self.junk_items.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(RichText::new("✨ Ottimo! Nessun dato spazzatura o cache ingombrante trovata.").color(Color32::LIGHT_GREEN));
                        ui.add_space(10.0);
                        if ui.button("Esegui scansione adesso").clicked() {
                            rescan_requested = true;
                        }
                    });
                    return;
                }

                // Calcolo totale recuperabile
                let selected_total: u64 = self
                    .junk_items
                    .iter()
                    .filter(|item| item.is_selected)
                    .map(|item| item.size)
                    .sum();

                let all_total: u64 = self.junk_items.iter().map(|item| item.size).sum();

                // Elenco elementi
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut current_category = String::new();

                        for item in &mut self.junk_items {
                            if item.category != current_category {
                                current_category = item.category.clone();
                                ui.add_space(6.0);
                                ui.label(RichText::new(&current_category).strong().color(Color32::from_rgb(150, 200, 255)));
                                ui.separator();
                            }

                            ui.horizontal(|ui| {
                                ui.checkbox(&mut item.is_selected, "");
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&item.name).strong());
                                    ui.label(RichText::new(item.path.display().to_string()).weak().small());
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .small_button(file_manager_button_label())
                                        .on_hover_text(open_in_file_manager_label())
                                        .clicked()
                                    {
                                        open_path = Some(item.path.clone());
                                    }
                                    ui.label(RichText::new(fmt_size(item.size)).strong().color(Color32::YELLOW));
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });

                ui.separator();

                // Barra inferiore con azioni
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Recuperabile selezionato: {} (Totale trovato: {})",
                        fmt_size(selected_total),
                        fmt_size(all_total)
                    ));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_clean = selected_total > 0;
                        let btn = egui::Button::new(
                            RichText::new(format!("🗑 Pulisci Selezionati ({})", fmt_size(selected_total)))
                                .strong(),
                        )
                        .fill(Color32::from_rgb(180, 40, 40));

                        if ui.add_enabled(can_clean, btn).clicked() {
                            clean_requested = true;
                        }

                        if ui.button("Seleziona tutti").clicked() {
                            for item in &mut self.junk_items {
                                item.is_selected = true;
                            }
                        }
                        if ui.button("Deseleziona").clicked() {
                            for item in &mut self.junk_items {
                                item.is_selected = false;
                            }
                        }
                    });
                });
            });

        if close_window {
            self.active_tool = ActiveTool::None;
        }
        if rescan_requested {
            self.start_junk_scan(ctx);
        }
        if let Some(p) = open_path {
            self.open_in_finder(&p);
        }
        if clean_requested {
            let to_clean: Vec<PathBuf> = self
                .junk_items
                .iter()
                .filter(|item| item.is_selected)
                .map(|item| item.path.clone())
                .collect();

            for path in to_clean {
                self.start_delete(ctx, path, false);
            }
        }
    }
}
