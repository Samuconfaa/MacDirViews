//! Finestra dedicata per il tool "Ricerca File Duplicati (Fast Duplicate Finder)".

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{ActiveTool, MacDirViewsApp};
use crate::system::finder::file_manager_button_label;
use crate::utils::format::fmt_size;

impl MacDirViewsApp {
    pub(in crate::app) fn duplicate_finder_ui(&mut self, ctx: &egui::Context) {
        let is_scanning = *self.duplicates_scanning.lock().unwrap();
        let mut close_window = false;
        let mut open_path: Option<PathBuf> = None;
        let mut delete_file: Option<PathBuf> = None;
        let mut delete_batch: Vec<PathBuf> = Vec::new();
        let mut start_requested = false;
        let mut stop_requested = false;

        egui::Window::new("👯 Ricerca File Duplicati")
            .min_width(700.0)
            .min_height(480.0)
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Ricerca in: {}", self.view_dir.display()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("❌ Chiudi").clicked() {
                            close_window = true;
                        }
                        if is_scanning {
                            if ui.button("⏹ Interrompi").clicked() {
                                stop_requested = true;
                            }
                        } else {
                            if ui.button("🔍 Avvia Ricerca Duplicati").clicked() {
                                start_requested = true;
                            }
                        }
                    });
                });

                ui.separator();

                if is_scanning {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label("Confronto firme digitali dei file in corso...");
                        ui.small("Vengono analizzate dimensioni identiche e calcolati hash parziali.");
                    });
                    return;
                }

                if self.duplicate_groups.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.label(
                            RichText::new("Nessun file duplicato trovato nella visualizzazione corrente.")
                                .color(Color32::LIGHT_GREEN),
                        );
                        ui.add_space(10.0);
                        if ui.button("Cerca duplicati adesso").clicked() {
                            start_requested = true;
                        }
                    });
                    return;
                }

                let total_wasted: u64 = self.duplicate_groups.iter().map(|g| g.wasted_bytes).sum();

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Trovati {} gruppi di duplicati — Spazio recuperabile: {}",
                            self.duplicate_groups.len(),
                            fmt_size(total_wasted)
                        ))
                        .strong()
                        .color(Color32::from_rgb(255, 180, 100)),
                    );
                });
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (idx, group) in self.duplicate_groups.iter().enumerate() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "Gruppo #{}: {} file identici ({})",
                                            idx + 1,
                                            group.files.len(),
                                            fmt_size(group.size)
                                        ))
                                        .strong(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.colored_label(
                                                Color32::YELLOW,
                                                format!("Spreco: {}", fmt_size(group.wasted_bytes)),
                                            );

                                            if ui
                                                .button("Tieni 1° ed elimina copie")
                                                .on_hover_text("Conserva il primo file e sposta gli altri nel Cestino")
                                                .clicked()
                                            {
                                                for file in group.files.iter().skip(1) {
                                                    delete_batch.push(file.clone());
                                                }
                                            }
                                        },
                                    );
                                });

                                ui.separator();

                                for (f_idx, file) in group.files.iter().enumerate() {
                                    ui.horizontal(|ui| {
                                        let tag = if f_idx == 0 { " (Originale)" } else { " (Copia)" };
                                        ui.label(format!("{}{}", file.display(), tag));

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("🗑 Elimina")
                                                    .clicked()
                                                {
                                                    delete_file = Some(file.clone());
                                                }
                                                if ui
                                                    .small_button(file_manager_button_label())
                                                    .clicked()
                                                {
                                                    open_path = Some(file.clone());
                                                }
                                            },
                                        );
                                    });
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
            });

        if close_window {
            self.active_tool = ActiveTool::None;
        }
        if start_requested {
            self.start_duplicate_scan(ctx);
        }
        if stop_requested {
            self.duplicates_cancel.store(true, Ordering::Relaxed);
        }
        if let Some(p) = open_path {
            self.open_in_finder(&p);
        }
        if let Some(f) = delete_file {
            self.start_delete(ctx, f, false);
        }
        for f in delete_batch {
            self.start_delete(ctx, f, false);
        }
    }
}
