//! Ciclo di aggiornamento di eframe: legge lo stato dai thread e compone i pannelli.

use std::time::Duration;

use eframe::egui;

use super::MacDirViewsApp;

impl eframe::App for MacDirViewsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Lettura dello stato: il guard e' un temporaneo e viene rilasciato subito.
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
                    self.apply_deletion(&path);
                    self.status = None;
                }
                Err(e) => self.status = Some(format!("Eliminazione fallita: {e}")),
            }
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

        if scanning {
            // Durante la scansione la UI resta fluida: ridisegno periodico + aggiornamento dati.
            ctx.request_repaint_after(Duration::from_millis(100));
            if self.last_refresh.elapsed() >= Duration::from_millis(500) {
                self.refresh();
            }
        } else if self.was_scanning {
            // Scansione appena terminata: ultimo aggiornamento con i dati finali.
            self.refresh();
            ctx.request_repaint();
        }
        self.was_scanning = scanning;

        self.top_bar_ui(ctx, scanning);
        self.side_panel_ui(ctx, scanning);
        egui::CentralPanel::default().show(ctx, |ui| {
            self.central_ui(ui, scanning);
        });
        self.delete_dialog_ui(ctx);
    }
}
