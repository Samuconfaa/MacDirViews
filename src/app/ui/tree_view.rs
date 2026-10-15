//! Vista alternativa ad Albero (Tree View gerarchica) con nodi espandibili,
//! dimensioni, percentuali e menu contestuali.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui::{Color32, RichText};

use super::folder_menu::folder_menu;
use crate::app::MacDirViewsApp;
use crate::utils::format::fmt_size;

impl MacDirViewsApp {
    /// Disegna la vista ad albero gerarchica a partire da `self.view_dir`.
    pub(in crate::app) fn tree_view_ui(&mut self, ui: &mut egui::Ui, scanning: bool) {
        if self.view_dir.as_os_str().is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label("Nessuna cartella scansionata. Seleziona una cartella per iniziare.");
            });
            return;
        }

        let mut nav_target: Option<PathBuf> = None;
        let mut open_finder: Option<PathBuf> = None;
        let mut del_request: Option<(PathBuf, bool)> = None;
        let mut toggle_node: Option<PathBuf> = None;

        let total_view_size = self.total_size.max(1);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Intestazione tabella
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📁 Nome").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("% Disco").strong());
                        ui.add_space(20.0);
                        ui.label(RichText::new("Dimensione").strong());
                        ui.add_space(20.0);
                    });
                });
                ui.separator();

                // Nodi radice della vista corrente
                let view_dir = self.view_dir.clone();
                self.render_tree_node(
                    ui,
                    &view_dir,
                    0,
                    total_view_size,
                    scanning,
                    &mut nav_target,
                    &mut open_finder,
                    &mut del_request,
                    &mut toggle_node,
                );
            });

        // Applica le azioni richieste durante il rendering
        if let Some(target) = toggle_node {
            if self.expanded_tree_nodes.contains(&target) {
                self.expanded_tree_nodes.remove(&target);
            } else {
                self.expanded_tree_nodes.insert(target);
            }
        }
        if let Some(target) = nav_target {
            self.navigate_to(target);
        }
        if let Some(path) = open_finder {
            self.open_in_finder(&path);
        }
        if let Some(d) = del_request {
            self.pending_delete = Some(d);
        }
    }

    /// Rende ricorsivamente un nodo e i suoi figli se espanso.
    fn render_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        depth: usize,
        root_size: u64,
        scanning: bool,
        nav_target: &mut Option<PathBuf>,
        open_finder: &mut Option<PathBuf>,
        del_request: &mut Option<(PathBuf, bool)>,
        toggle_node: &mut Option<PathBuf>,
    ) {
        let (children, direct_size) = {
            let guard = self.tree.lock().unwrap();
            let ch = guard.children_of(path);
            let dir_size = guard.get_node(path).map(|n| n.direct_size).unwrap_or(0);
            (ch, dir_size)
        };

        if children.is_empty() && direct_size == 0 {
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 18.0);
                ui.label(RichText::new("(cartella vuota)").weak());
            });
            return;
        }

        for (name, child_path, size) in children {
            let is_expanded = self.expanded_tree_nodes.contains(&child_path);
            let is_sel = self.selected.as_ref() == Some(&child_path);
            let pct = (size as f64 / root_size as f64 * 100.0) as f32;

            ui.horizontal(|ui| {
                // Indentazione
                ui.add_space(depth as f32 * 18.0);

                // Icona espandi / collassa
                let arrow = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    *toggle_node = Some(child_path.clone());
                }

                // Nome ed icona
                let icon = "📁 ";
                let label_text = format!("{}{}", icon, name);
                let resp = ui.selectable_label(
                    is_sel,
                    RichText::new(&label_text).color(if is_sel {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(200, 220, 255)
                    }),
                );

                if resp.clicked() {
                    self.selected = Some(child_path.clone());
                }
                if resp.double_clicked() {
                    *nav_target = Some(child_path.clone());
                }

                // Menu contestuale
                resp.context_menu(|ui| {
                    folder_menu(ui, &child_path, !scanning, open_finder, del_request);
                });

                // Dimensione e percentuale a destra
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.1}%", pct)).weak());
                    ui.add_space(10.0);
                    ui.label(RichText::new(fmt_size(size)).strong());
                });
            });

            // Se il nodo è espanso, renderizza i sotto-figli
            if is_expanded {
                self.render_tree_node(
                    ui,
                    &child_path,
                    depth + 1,
                    root_size,
                    scanning,
                    nav_target,
                    open_finder,
                    del_request,
                    toggle_node,
                );
            }
        }

        // File propri direttamente in questa cartella
        if direct_size > 0 {
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 18.0 + 18.0);
                ui.label(RichText::new("📄 (file in questa cartella)").weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pct = (direct_size as f64 / root_size as f64 * 100.0) as f32;
                    ui.label(RichText::new(format!("{:.1}%", pct)).weak());
                    ui.add_space(10.0);
                    ui.label(RichText::new(fmt_size(direct_size)).weak());
                });
            });
        }
    }
}
