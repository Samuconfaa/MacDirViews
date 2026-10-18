//! Pannello laterale: filtri (ricerca, soglia minima, aging filter),
//! classifica delle 10 cartelle e dei file più grandi / inutilizzati.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use eframe::egui;

use super::folder_menu::folder_menu;
use crate::app::{AgingFilter, DirTreeViewApp};
use crate::system::finder::open_in_file_manager_label;
use crate::system::protection::is_deletion_forbidden;
use crate::utils::format::fmt_size;

impl DirTreeViewApp {
    pub(in crate::app) fn side_panel_ui(&mut self, ctx: &egui::Context, scanning: bool) {
        let mut clicked_path: Option<PathBuf> = None;
        let mut side_open: Option<PathBuf> = None;
        let mut side_del: Option<(PathBuf, bool)> = None;
        let mut do_refresh = false;
        let can_modify = !scanning;

        egui::SidePanel::left("top10_panel")
            .resizable(true)
            .min_width(260.0)
            .default_width(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // --- Header con pulsante per nascondere il menu a scomparsa ---
                    ui.horizontal(|ui| {
                        if ui
                            .button("◀ Chiudi Menu")
                            .on_hover_text("Nascondi questo menu laterale a scomparsa (Cmd+B / Ctrl+B)")
                            .clicked()
                        {
                            self.left_panel_open = false;
                        }
                        ui.heading("🔍 Ricerca e Filtri");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Nome:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Cerca per nome...")
                                    .desired_width(180.0),
                            )
                            .changed()
                        {
                            do_refresh = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Soglia minima:");
                        if ui
                            .add(
                                egui::Slider::new(&mut self.filter_min_size_mb, 0.0..=1024.0)
                                    .suffix(" MB")
                                    .logarithmic(true)
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            do_refresh = true;
                        }
                    });

                    // Filtro Temporale (Aging Filter)
                    ui.horizontal(|ui| {
                        ui.label("⏳ Età file:");
                        egui::ComboBox::from_id_salt("aging_filter_combo")
                            .selected_text(self.aging_filter.label())
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut self.aging_filter,
                                        AgingFilter::All,
                                        AgingFilter::All.label(),
                                    )
                                    .clicked()
                                {
                                    do_refresh = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.aging_filter,
                                        AgingFilter::OlderThan1Month,
                                        AgingFilter::OlderThan1Month.label(),
                                    )
                                    .clicked()
                                {
                                    do_refresh = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.aging_filter,
                                        AgingFilter::OlderThan6Months,
                                        AgingFilter::OlderThan6Months.label(),
                                    )
                                    .clicked()
                                {
                                    do_refresh = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.aging_filter,
                                        AgingFilter::OlderThan1Year,
                                        AgingFilter::OlderThan1Year.label(),
                                    )
                                    .clicked()
                                {
                                    do_refresh = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut self.aging_filter,
                                        AgingFilter::OlderThan2Years,
                                        AgingFilter::OlderThan2Years.label(),
                                    )
                                    .clicked()
                                {
                                    do_refresh = true;
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Applica filtri").clicked() && !scanning {
                            do_refresh = true;
                        }
                        if (self.filter_min_size_mb > 0.0
                            || !self.search_query.is_empty()
                            || self.aging_filter != AgingFilter::All)
                            && ui.button("Resetta filtri").clicked()
                        {
                            self.filter_min_size_mb = 0.0;
                            self.search_query.clear();
                            self.aging_filter = AgingFilter::All;
                            do_refresh = true;
                        }
                    });

                    ui.separator();

                    // --- Top 10 cartelle ---
                    ui.heading("📁 Top 10 cartelle in questa vista");
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
                        resp.context_menu(|ui| {
                            folder_menu(ui, path, can_modify, &mut side_open, &mut side_del);
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(8.0);
                    ui.separator();

                    // --- Top file più grandi (con supporto Aging Filter) ---
                    let aging_heading = match self.aging_filter {
                        AgingFilter::All => "📄 Top file più grandi".to_string(),
                        other => format!("⏳ File Inutilizzati ({})", other.label()),
                    };
                    ui.heading(aging_heading);
                    ui.small("File già caricati nelle cartelle visitate");

                    let min_age = match self.aging_filter {
                        AgingFilter::All => None,
                        AgingFilter::OlderThan1Month => Some(Duration::from_secs(30 * 86400)),
                        AgingFilter::OlderThan6Months => Some(Duration::from_secs(180 * 86400)),
                        AgingFilter::OlderThan1Year => Some(Duration::from_secs(365 * 86400)),
                        AgingFilter::OlderThan2Years => Some(Duration::from_secs(730 * 86400)),
                    };

                    let now = SystemTime::now();
                    let filtered_files: Vec<_> = self
                        .top_files
                        .iter()
                        .filter(|f| {
                            if let Some(req_age) = min_age {
                                if let Some(m) = f.modified {
                                    if let Ok(elapsed) = now.duration_since(m) {
                                        return elapsed >= req_age;
                                    }
                                }
                                false
                            } else {
                                true
                            }
                        })
                        .collect();

                    if filtered_files.is_empty() {
                        ui.label("Nessun file corrispondente ai filtri.");
                    }

                    for (i, file_info) in filtered_files.into_iter().enumerate() {
                        let name = file_info
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| file_info.path.display().to_string());

                        let age_str = if let Some(m) = file_info.modified {
                            if let Ok(elapsed) = now.duration_since(m) {
                                let days = elapsed.as_secs() / 86400;
                                if days > 365 {
                                    format!(" (modificato {:.1} anni fa)", days as f32 / 365.0)
                                } else {
                                    format!(" (modificato {} giorni fa)", days)
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };

                        let text = format!(
                            "{}. {} — {}{}\n{}",
                            i + 1,
                            name,
                            fmt_size(file_info.size),
                            age_str,
                            file_info.path.display()
                        );
                        let is_sel = self.selected.as_ref() == Some(&file_info.path);
                        let resp = ui.selectable_label(is_sel, text);

                        if resp.clicked() {
                            clicked_path = Some(file_info.path.clone());
                        }

                        // Menu contestuale per aprire nel file manager o eliminare il singolo file
                        resp.context_menu(|ui| {
                            ui.label(file_info.path.display().to_string());
                            ui.separator();
                            if ui.button(open_in_file_manager_label()).clicked() {
                                side_open = Some(file_info.path.clone());
                                ui.close_menu();
                            }
                            let allowed = can_modify && !is_deletion_forbidden(&file_info.path);
                            if ui
                                .add_enabled(allowed, egui::Button::new("Sposta nel Cestino..."))
                                .clicked()
                            {
                                side_del = Some((file_info.path.clone(), false));
                                ui.close_menu();
                            }
                            if ui
                                .add_enabled(
                                    allowed,
                                    egui::Button::new("Elimina definitivamente..."),
                                )
                                .clicked()
                            {
                                side_del = Some((file_info.path.clone(), true));
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
                });
            });

        if do_refresh {
            self.refresh();
        }
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
