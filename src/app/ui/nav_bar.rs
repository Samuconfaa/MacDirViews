//! Barra di navigazione (Indietro / Avanti / Su) con breadcrumbs interattivi a segmenti cliccabili.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui::Color32;

use crate::app::DirTreeViewApp;

impl DirTreeViewApp {
    pub(in crate::app) fn nav_bar_ui(&mut self, ui: &mut egui::Ui) {
        let mut go_up = false;
        let mut go_back = false;
        let mut go_fwd = false;
        let mut navigate_target: Option<PathBuf> = None;

        ui.horizontal(|ui| {
            // Pulsanti di navigazione cronologica
            if ui
                .add_enabled(!self.back_stack.is_empty(), egui::Button::new("◀ Indietro"))
                .on_hover_text("Torna alla cartella precedente")
                .clicked()
            {
                go_back = true;
            }
            if ui
                .add_enabled(!self.fwd_stack.is_empty(), egui::Button::new("Avanti ▶"))
                .on_hover_text("Avanza nella cronologia")
                .clicked()
            {
                go_fwd = true;
            }
            let can_up = self.view_dir != self.scan_root
                && self.view_dir.parent().is_some()
                && !self.scan_root.as_os_str().is_empty();
            if ui
                .add_enabled(can_up, egui::Button::new("▲ Su"))
                .on_hover_text("Sali alla cartella superiore")
                .clicked()
            {
                go_up = true;
            }

            ui.separator();

            // Breadcrumbs interattivi a segmenti cliccabili
            if self.view_dir.as_os_str().is_empty() {
                ui.label("Nessuna cartella selezionata");
            } else {
                let home = dirs_next::home_dir();
                let mut segments: Vec<PathBuf> = Vec::new();
                let mut curr = Some(self.view_dir.as_path());
                while let Some(p) = curr {
                    segments.push(p.to_path_buf());
                    curr = p.parent();
                }
                segments.reverse();

                let total_segments = segments.len();

                egui::ScrollArea::horizontal()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (i, seg_path) in segments.into_iter().enumerate() {
                                if i > 0 {
                                    ui.colored_label(Color32::from_gray(120), "›");
                                }

                                let is_current = i + 1 == total_segments;
                                let name = if seg_path == Path::new("/") || seg_path.parent().is_none() {
                                    if cfg!(target_os = "macos") {
                                        "Macintosh HD".to_string()
                                    } else if cfg!(windows) {
                                        let s = seg_path.to_string_lossy();
                                        if s.is_empty() {
                                            "Questo PC".to_string()
                                        } else {
                                            s.to_string()
                                        }
                                    } else {
                                        "/".to_string()
                                    }
                                } else if home.as_ref() == Some(&seg_path) {
                                    "🏠 Home".to_string()
                                } else {
                                    seg_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_else(|| seg_path.display().to_string())
                                };

                                if is_current {
                                    ui.label(
                                        egui::RichText::new(&name)
                                            .strong()
                                            .color(Color32::from_rgb(255, 255, 255)),
                                    );
                                } else {
                                    let btn = egui::Button::new(
                                        egui::RichText::new(&name)
                                            .color(Color32::from_rgb(120, 180, 255)),
                                    )
                                    .frame(false);

                                    if ui
                                        .add(btn)
                                        .on_hover_text(format!("Vai a {}", seg_path.display()))
                                        .clicked()
                                    {
                                        navigate_target = Some(seg_path);
                                    }
                                }
                            }
                        });
                    });
            }
        });

        // Gestione tasto mouse "Indietro"
        if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Extra1)) {
            go_back = true;
        }

        if let Some(target) = navigate_target {
            self.navigate_to(target);
        } else if go_back {
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
