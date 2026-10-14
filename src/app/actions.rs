//! Azioni dell'applicazione: scansione, navigazione, eliminazione, avvio come admin.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use super::block::Block;
use super::MacDirViewsApp;
use crate::constants::MAX_BLOCKS;
use crate::scanner::scan_directory;
use crate::system::finder::{move_to_trash, reveal_in_finder};
use crate::system::protection::is_deletion_forbidden;

impl MacDirViewsApp {
    /// Avvia la scansione in un thread separato.
    pub(super) fn start_scan(&mut self, ctx: &egui::Context) {
        let input = self.path_input.trim().to_string();

        // Percorso vuoto -> cartella Home; supporto base per "~".
        let root = if input.is_empty() {
            match dirs_next::home_dir() {
                Some(home) => home,
                None => {
                    self.status = Some("Impossibile determinare la cartella Home.".into());
                    return;
                }
            }
        } else if input == "~" || input.starts_with("~/") {
            match dirs_next::home_dir() {
                Some(home) => home.join(input.trim_start_matches('~').trim_start_matches('/')),
                None => {
                    self.status = Some("Impossibile determinare la cartella Home.".into());
                    return;
                }
            }
        } else {
            PathBuf::from(&input)
        };

        if !root.is_dir() {
            self.status = Some(format!("Non e' una cartella valida: {}", root.display()));
            return;
        }

        // Reset dello stato.
        self.status = None;
        self.selected = None;
        self.top10.clear();
        self.blocks.clear();
        self.total_size = 0;
        self.scan_root = root.clone();
        self.view_dir = root.clone();
        self.back_stack.clear();
        self.fwd_stack.clear();
        self.files_counter.store(0, Ordering::Relaxed);
        self.sizes.lock().unwrap().clear();
        *self.scanning.lock().unwrap() = true;
        self.last_refresh = Instant::now();

        // Cloni per il thread.
        let scanning = Arc::clone(&self.scanning);
        let sizes = Arc::clone(&self.sizes);
        let counter = Arc::clone(&self.files_counter);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            scan_directory(root, scanning, sizes, counter, ctx);
        });
    }

    /// Ricalcola top-10 e blocchi della treemap dalla mappa condivisa.
    /// Il mutex viene rilasciato esplicitamente PRIMA di toccare qualsiasi altro campo.
    pub(super) fn refresh(&mut self) {
        let view = self.view_dir.clone();
        let root = self.scan_root.clone();

        let guard = self.sizes.lock().unwrap();

        // Top 10 delle cartelle contenute (a qualsiasi profondita') nella cartella
        // in cui si sta navigando, esclusa la cartella stessa.
        let mut all: Vec<(&PathBuf, u64)> = guard
            .iter()
            .filter(|(p, _)| **p != view && p.starts_with(&view))
            .map(|(p, s)| (p, *s))
            .collect();
        if all.len() > 10 {
            all.select_nth_unstable_by(10, |a, b| b.1.cmp(&a.1));
            all.truncate(10);
        }
        all.sort_by(|a, b| b.1.cmp(&a.1));
        let top10: Vec<(PathBuf, u64)> = all.into_iter().map(|(p, s)| (p.clone(), s)).collect();

        // Figli diretti della cartella visualizzata.
        let view_size = guard.get(&view).copied().unwrap_or(0);
        let root_size = guard.get(&root).copied().unwrap_or(0);
        let mut children: Vec<(PathBuf, u64)> = guard
            .iter()
            .filter(|(p, _)| **p != view && p.parent() == Some(view.as_path()))
            .map(|(p, s)| (p.clone(), *s))
            .collect();

        // Lock rilasciato qui: da ora si lavora solo su copie locali.
        drop(guard);

        children.sort_by(|a, b| b.1.cmp(&a.1));
        let children_sum: u64 = children.iter().map(|c| c.1).sum();
        let own_files = view_size.saturating_sub(children_sum);

        let mut blocks: Vec<Block> = Vec::new();
        let mut rest_size: u64 = 0;
        let mut rest_count: usize = 0;
        for (i, (path, size)) in children.into_iter().enumerate() {
            if size == 0 {
                continue;
            }
            if i < MAX_BLOCKS {
                let label = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                blocks.push(Block { path, label, size, is_dir: true });
            } else {
                rest_size += size;
                rest_count += 1;
            }
        }
        if rest_size > 0 {
            blocks.push(Block {
                path: view.clone(),
                label: format!("(altre {} cartelle)", rest_count),
                size: rest_size,
                is_dir: false,
            });
        }
        if own_files > 0 {
            blocks.push(Block {
                path: view.clone(),
                label: "(file in questa cartella)".to_string(),
                size: own_files,
                is_dir: false,
            });
        }
        blocks.sort_by(|a, b| b.size.cmp(&a.size));

        self.top10 = top10;
        self.blocks = blocks;
        self.total_size = root_size;
        self.last_refresh = Instant::now();
    }

    /// Naviga in una cartella registrando la posizione corrente nella cronologia.
    pub(super) fn navigate_to(&mut self, dir: PathBuf) {
        if dir == self.view_dir {
            return;
        }
        let current = self.view_dir.clone();
        self.back_stack.push(current);
        self.fwd_stack.clear();
        self.view_dir = dir;
        self.selected = None;
        self.refresh();
    }

    /// Torna alla cartella visitata prima.
    pub(super) fn go_back(&mut self) {
        if let Some(prev) = self.back_stack.pop() {
            let current = std::mem::replace(&mut self.view_dir, prev);
            self.fwd_stack.push(current);
            self.selected = None;
            self.refresh();
        }
    }

    /// Va avanti dopo un "Indietro".
    pub(super) fn go_forward(&mut self) {
        if let Some(next) = self.fwd_stack.pop() {
            let current = std::mem::replace(&mut self.view_dir, next);
            self.back_stack.push(current);
            self.selected = None;
            self.refresh();
        }
    }

    /// Rilancia l'app con permessi di amministratore (richiede la password di macOS)
    /// e chiude l'istanza corrente solo se l'avvio riesce.
    pub(super) fn relaunch_as_admin(&mut self, ctx: &egui::Context) {
        let exe = match std::env::current_exe() {
            Ok(e) => e.to_string_lossy().to_string(),
            Err(e) => {
                self.status = Some(format!("Percorso dell'eseguibile non disponibile: {e}"));
                return;
            }
        };
        if exe.contains('"') || exe.contains('\\') || exe.contains('\'') {
            self.status = Some(
                "Il percorso dell'app contiene caratteri non supportati: usa `sudo` da Terminale."
                    .into(),
            );
            return;
        }
        let script = format!(
            r#"do shell script "\"{}\" >/dev/null 2>&1 &" with administrator privileges"#,
            exe
        );
        let msg = Arc::clone(&self.admin_msg);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            match Command::new("osascript").arg("-e").arg(script).output() {
                Ok(out) if out.status.success() => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    *msg.lock().unwrap() =
                        Some(format!("Avvio come amministratore annullato o fallito: {err}"));
                    ctx.request_repaint();
                }
                Err(e) => {
                    *msg.lock().unwrap() = Some(format!("Impossibile eseguire osascript: {e}"));
                    ctx.request_repaint();
                }
            }
        });
    }

    /// Avvia l'eliminazione in un thread (Cestino oppure definitiva).
    pub(super) fn start_delete(&mut self, ctx: &egui::Context, path: PathBuf, permanent: bool) {
        if is_deletion_forbidden(&path) {
            self.status = Some(format!("Cartella protetta, non eliminabile: {}", path.display()));
            return;
        }
        self.status = Some(format!("Eliminazione in corso: {}", path.display()));
        let result = Arc::clone(&self.delete_result);
        let is_admin = self.is_admin;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = if permanent {
                std::fs::remove_dir_all(&path).map_err(|e| e.to_string())
            } else {
                move_to_trash(&path, is_admin)
            };
            *result.lock().unwrap() = Some((path, res));
            ctx.request_repaint();
        });
    }

    /// Dopo un'eliminazione riuscita aggiorna dimensioni, navigazione e treemap
    /// senza dover rifare la scansione.
    pub(super) fn apply_deletion(&mut self, path: &Path) {
        {
            let mut map = self.sizes.lock().unwrap();
            let removed = map.get(path).copied().unwrap_or(0);
            map.retain(|p, _| !p.starts_with(path));
            for anc in path.ancestors().skip(1) {
                if let Some(v) = map.get_mut(anc) {
                    *v = v.saturating_sub(removed);
                }
                if anc == self.scan_root.as_path() {
                    break;
                }
            }
        }
        self.back_stack.retain(|p| !p.starts_with(path));
        self.fwd_stack.retain(|p| !p.starts_with(path));
        if self.selected.as_ref().map_or(false, |s| s.starts_with(path)) {
            self.selected = None;
        }
        if self.view_dir.starts_with(path) {
            if let Some(parent) = path.parent() {
                self.view_dir = parent.to_path_buf();
            }
        }
        self.refresh();
    }

    /// Apre il percorso selezionato nel Finder di macOS.
    pub(super) fn open_in_finder(&mut self, path: &Path) {
        match reveal_in_finder(path, self.is_admin) {
            Ok(()) => self.status = None,
            Err(msg) => self.status = Some(msg),
        }
    }
}
