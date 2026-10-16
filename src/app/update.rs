//! Ciclo di aggiornamento di eframe: legge lo stato dai thread,
//! sincronizza il filesystem in tempo reale e compone i pannelli.

use std::time::Duration;

use eframe::egui;

use super::{ActiveTool, DirTreeViewApp};

impl eframe::App for DirTreeViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let scanning = *self.scanning.lock().unwrap();

        // Eventuale errore dal thread di avvio come amministratore.
        let admin_err = self.admin_msg.lock().unwrap().take();
        if let Some(m) = admin_err {
            self.status = Some(m);
        }

        // Esito di un'eliminazione terminata nel thread.
        let delete_done = self.delete_result.lock().unwrap().take();
        if let Some((path, res)) = delete_done {
            match res {
                Ok(()) => {
                    self.apply_deletion(&path, ctx);
                    self.status = None;
                }
                Err(e) => self.status = Some(format!("Eliminazione fallita: {e}")),
            }
        }

        // Aggiorna spazio su disco se arrivato dal thread in background
        if let Some(usage) = self.disk_usage_result.lock().unwrap().take() {
            self.disk_usage = Some(usage);
        }

        // Aggiorna risultati Junk Cleaner se arrivati
        if let Some(items) = self.junk_result.lock().unwrap().take() {
            self.junk_items = items;
        }

        // Aggiorna risultati Duplicati se arrivati
        if let Some(groups) = self.duplicates_result.lock().unwrap().take() {
            self.duplicate_groups = groups;
        }

        // Drag & drop: accetta cartelle trascinate sulla finestra.
        let dropped_files: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped_files.first() {
            if let Some(path) = &file.path {
                if path.is_dir() {
                    self.path_input = path.display().to_string();
                    self.start_scan(ctx);
                }
            }
        }

        // Sincronizzazione in tempo reale all'eliminazione di file o cartelle
        // Viene controllata periodicamente ogni 500ms quando non stiamo scansionando
        if !scanning && self.last_live_sync.elapsed() >= Duration::from_millis(500) {
            self.sync_filesystem_changes(ctx);
            self.last_live_sync = std::time::Instant::now();
        }

        let scan_completed = self.scan_completed.swap(false, std::sync::atomic::Ordering::Acquire);
        if scanning {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.was_scanning || scan_completed {
            self.refresh();
            self.refresh_disk_usage(ctx);
            ctx.request_repaint();
        }
        self.was_scanning = scanning;

        if !scanning && !self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed)
            && !self.scan_root.as_os_str().is_empty()
        {
            let next_scan = {
                let tree = self.tree.lock().unwrap();
                if tree.needs_shallow_scan(&self.view_dir) {
                    Some(self.view_dir.clone())
                } else {
                    self.expanded_tree_nodes.iter().find(|path| !tree.is_scanned(path)).cloned()
                }
            };
            if let Some(path) = next_scan { self.start_shallow_scan(path, ctx); }
        }

        // Pannelli dell'interfaccia
        self.top_bar_ui(ctx, scanning);
        if self.left_panel_open { self.side_panel_ui(ctx, scanning); }
        egui::CentralPanel::default().show(ctx, |ui| {
            self.central_ui(ui, scanning);
        });

        // Finestre di dialogo e strumenti dedicati
        self.delete_dialog_ui(ctx);

        match self.active_tool {
            ActiveTool::None => {}
            ActiveTool::JunkCleaner => {
                self.junk_cleaner_ui(ctx);
            }
            ActiveTool::DuplicateFinder => {
                self.duplicate_finder_ui(ctx);
            }
        }
    }
}
