//! Azioni dell'applicazione: scansione, navigazione, eliminazione, sincronizzazione live,
//! strumenti per file duplicati, junk cleaner e spazio su disco.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use super::block::Block;
use super::MacDirViewsApp;
use crate::constants::MAX_BLOCKS;
use crate::scanner::{find_duplicates, scan_developer_and_system_junk, scan_directory, DirTree};
use crate::system::disk::get_disk_usage;
use crate::system::finder::{move_to_trash, reveal_in_finder};
use crate::system::protection::is_deletion_forbidden;

impl MacDirViewsApp {
    /// Avvia la scansione in un thread separato.
    pub(super) fn start_scan(&mut self, ctx: &egui::Context) {
        let input = self.path_input.trim().to_string();

        let root = if input.is_empty() {
            match dirs_next::home_dir() {
                Some(home) => home,
                None => {
                    self.status = Some("Impossibile determinare la cartella Home.".into());
                    return;
                }
            }
        } else if input == "~" || input.starts_with("~/") || input.starts_with(r"~\") {
            match dirs_next::home_dir() {
                Some(home) => {
                    let sub = input.trim_start_matches('~').trim_start_matches('/').trim_start_matches('\\');
                    home.join(sub)
                }
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
        self.top_files.clear();
        self.skipped_dirs = 0;
        self.skipped_messages.clear();
        self.scan_root = root.clone();
        self.view_dir = root.clone();
        self.back_stack.clear();
        self.fwd_stack.clear();
        self.files_counter.store(0, Ordering::Relaxed);
        self.cancel_flag.store(false, Ordering::Relaxed);
        *self.tree.lock().unwrap() = DirTree::new(root.clone());
        *self.scanning.lock().unwrap() = true;
        self.last_refresh = Instant::now();

        // Aggiorna lo spazio su disco
        self.refresh_disk_usage(ctx);

        let scanning = Arc::clone(&self.scanning);
        let tree = Arc::clone(&self.tree);
        let counter = Arc::clone(&self.files_counter);
        let cancel = Arc::clone(&self.cancel_flag);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            scan_directory(root, scanning, tree, counter, cancel, ctx);
        });
    }

    /// Ferma la scansione in corso.
    pub(super) fn stop_scan(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    /// Apre la finestra di dialogo nativa per scegliere una cartella.
    pub(super) fn browse_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.path_input = path.display().to_string();
        }
    }

    /// Ricarica in background le metriche di capienza del disco per il volume corrente.
    pub(super) fn refresh_disk_usage(&mut self, ctx: &egui::Context) {
        let path = if self.view_dir.as_os_str().is_empty() {
            self.scan_root.clone()
        } else {
            self.view_dir.clone()
        };
        let res_arc = Arc::clone(&self.disk_usage_result);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let usage = get_disk_usage(&path);
            *res_arc.lock().unwrap() = usage;
            ctx.request_repaint();
        });
        self.last_disk_refresh = Instant::now();
    }

    /// Avvia lo scanner per junk e cache in background.
    pub(super) fn start_junk_scan(&mut self, ctx: &egui::Context) {
        if *self.junk_scanning.lock().unwrap() {
            return;
        }
        *self.junk_scanning.lock().unwrap() = true;
        let root = if self.scan_root.as_os_str().is_empty() {
            None
        } else {
            Some(self.scan_root.clone())
        };
        let result_arc = Arc::clone(&self.junk_result);
        let scanning_arc = Arc::clone(&self.junk_scanning);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let items = scan_developer_and_system_junk(root.as_deref());
            *result_arc.lock().unwrap() = Some(items);
            *scanning_arc.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

    /// Avvia la ricerca di file duplicati in background.
    pub(super) fn start_duplicate_scan(&mut self, ctx: &egui::Context) {
        if *self.duplicates_scanning.lock().unwrap() {
            return;
        }
        *self.duplicates_scanning.lock().unwrap() = true;
        self.duplicates_cancel.store(false, Ordering::Relaxed);

        let root = if self.view_dir.as_os_str().is_empty() {
            dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        } else {
            self.view_dir.clone()
        };
        let result_arc = Arc::clone(&self.duplicates_result);
        let scanning_arc = Arc::clone(&self.duplicates_scanning);
        let cancel = Arc::clone(&self.duplicates_cancel);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            // Cerca duplicati con dimensione >= 250 KB
            let groups = find_duplicates(&root, 250 * 1024, &cancel);
            *result_arc.lock().unwrap() = Some(groups);
            *scanning_arc.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

    /// Ricalcola top-10 e blocchi della treemap dall'albero condiviso.
    pub(super) fn refresh(&mut self) {
        let view = self.view_dir.clone();

        let guard = self.tree.lock().unwrap();
        let root_size = guard.total_size();

        let mut all_descendants = guard.all_descendants(&view);
        if all_descendants.len() > 10 {
            all_descendants.select_nth_unstable_by(10, |a, b| b.1.cmp(&a.1));
            all_descendants.truncate(10);
        }
        all_descendants.sort_by(|a, b| b.1.cmp(&a.1));
        let top10 = all_descendants;

        let children_data = guard.children_of(&view);
        let view_size = guard.size_of(&view);

        let top_files = guard.top_files.clone();
        let skipped_dirs = guard.skipped_dirs;
        let skipped_messages = guard.skipped_messages.clone();

        drop(guard);

        let children_sum: u64 = children_data.iter().map(|c| c.2).sum();
        let own_files = view_size.saturating_sub(children_sum);

        let mut blocks: Vec<Block> = Vec::new();
        let mut rest_size: u64 = 0;
        let mut rest_count: usize = 0;
        for (i, (_name, path, size)) in children_data.into_iter().enumerate() {
            if size == 0 {
                continue;
            }
            let min_bytes = (self.filter_min_size_mb as u64) * 1024 * 1024;
            if min_bytes > 0 && size < min_bytes {
                rest_size += size;
                rest_count += 1;
                continue;
            }
            if !self.search_query.is_empty() {
                let label = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !label.to_lowercase().contains(&self.search_query.to_lowercase()) {
                    rest_size += size;
                    rest_count += 1;
                    continue;
                }
            }
            if i < MAX_BLOCKS {
                let label = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                blocks.push(Block {
                    path,
                    label,
                    size,
                    is_dir: true,
                });
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
        self.top_files = top_files;
        self.skipped_dirs = skipped_dirs;
        self.skipped_messages = skipped_messages;
        self.last_refresh = Instant::now();
    }

    /// Controlla in tempo reale se file o cartelle mostrati sono stati eliminati
    /// (anche dall'esterno dell'app nel Finder o Terminale).
    pub(super) fn sync_filesystem_changes(&mut self, ctx: &egui::Context) {
        if *self.scanning.lock().unwrap() || self.scan_root.as_os_str().is_empty() {
            return;
        }

        let mut deleted_paths = Vec::new();

        // 1. Controlla se la cartella attualmente visualizzata esiste ancora
        if !self.view_dir.as_os_str().is_empty() && !self.view_dir.exists() {
            let mut parent = self.view_dir.parent();
            while let Some(p) = parent {
                if p.exists() {
                    self.view_dir = p.to_path_buf();
                    self.refresh();
                    return;
                }
                parent = p.parent();
            }
            self.view_dir = self.scan_root.clone();
            self.refresh();
            return;
        }

        // 2. Controlla i blocchi attualmente visibili
        for block in &self.blocks {
            if block.is_dir && !block.path.exists() {
                deleted_paths.push(block.path.clone());
            }
        }

        // 3. Controlla l'elemento selezionato
        if let Some(ref sel) = self.selected {
            if !sel.exists() {
                deleted_paths.push(sel.clone());
            }
        }

        // 4. Controlla i top file
        for file in &self.top_files {
            if !file.path.exists() {
                deleted_paths.push(file.path.clone());
            }
        }

        // 5. Controlla se ci sono elementi eliminati
        if !deleted_paths.is_empty() {
            for p in deleted_paths {
                self.apply_deletion(&p, ctx);
            }
        }
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

    /// Rilancia l'app con permessi di amministratore (richiede password su macOS, UAC su Windows).
    pub(super) fn relaunch_as_admin(&mut self, ctx: &egui::Context) {
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                self.status = Some(format!("Percorso dell'eseguibile non disponibile: {e}"));
                return;
            }
        };

        #[cfg(target_os = "macos")]
        {
            let exe_str = exe.to_string_lossy().to_string();
            if exe_str.contains('"') || exe_str.contains('\\') || exe_str.contains('\'') {
                self.status = Some(
                    "Il percorso dell'app contiene caratteri non supportati: usa `sudo` da Terminale."
                        .into(),
                );
                return;
            }
            let script = format!(
                r#"do shell script "\"{}\" >/dev/null 2>&1 &" with administrator privileges"#,
                exe_str
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

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let exe_str = exe.to_string_lossy().to_string();
            let msg = Arc::clone(&self.admin_msg);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let script = format!(
                    "Start-Process -FilePath '{}' -Verb RunAs",
                    exe_str.replace('\'', "''")
                );
                let mut cmd = Command::new("powershell");
                cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                match cmd.output() {
                    Ok(out) if out.status.success() => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Ok(out) => {
                        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        if !err.is_empty() {
                            *msg.lock().unwrap() =
                                Some(format!("Avvio come amministratore annullato o fallito: {err}"));
                        }
                        ctx.request_repaint();
                    }
                    Err(e) => {
                        *msg.lock().unwrap() =
                            Some(format!("Impossibile avviare come amministratore: {e}"));
                        ctx.request_repaint();
                    }
                }
            });
        }

        #[cfg(not(any(target_os = "macos", windows)))]
        {
            self.status =
                Some("Riavvio come admin non supportato su questo sistema operativo.".into());
        }
    }

    /// Avvia l'eliminazione in un thread (Cestino oppure definitiva per cartelle e singoli file).
    pub(super) fn start_delete(&mut self, ctx: &egui::Context, path: PathBuf, permanent: bool) {
        if is_deletion_forbidden(&path) {
            self.status = Some(format!("Percorso protetto, non eliminabile: {}", path.display()));
            return;
        }
        self.status = Some(format!("Eliminazione in corso: {}", path.display()));
        let result = Arc::clone(&self.delete_result);
        let is_admin = self.is_admin;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = if permanent {
                if path.is_file() {
                    std::fs::remove_file(&path).map_err(|e| e.to_string())
                } else {
                    std::fs::remove_dir_all(&path).map_err(|e| e.to_string())
                }
            } else {
                move_to_trash(&path, is_admin)
            };
            *result.lock().unwrap() = Some((path, res));
            ctx.request_repaint();
        });
    }

    /// Dopo un'eliminazione riuscita aggiorna dimensioni, navigazione, liste e treemap
    /// in tempo reale senza dover rieseguire la scansione.
    pub(super) fn apply_deletion(&mut self, path: &Path, ctx: &egui::Context) {
        {
            let mut tree = self.tree.lock().unwrap();
            tree.remove(path);
        }
        self.back_stack.retain(|p| !p.starts_with(path));
        self.fwd_stack.retain(|p| !p.starts_with(path));
        if self
            .selected
            .as_ref()
            .map_or(false, |s| s == path || s.starts_with(path))
        {
            self.selected = None;
        }
        if self.view_dir.starts_with(path) {
            if let Some(parent) = path.parent() {
                self.view_dir = parent.to_path_buf();
            }
        }

        // Rimuovi anche da Junk Cleaner se presente
        self.junk_items.retain(|item| !item.path.starts_with(path));

        // Rimuovi anche da File Duplicati se presente
        for group in &mut self.duplicate_groups {
            group.files.retain(|f| f != path && !f.starts_with(path));
            let count = group.files.len() as u64;
            group.wasted_bytes = count.saturating_sub(1) * group.size;
        }
        self.duplicate_groups.retain(|g| g.files.len() >= 2);

        self.refresh();
        self.refresh_disk_usage(ctx);
    }

    /// Apre il percorso selezionato nel Finder di macOS.
    pub(super) fn open_in_finder(&mut self, path: &Path) {
        match reveal_in_finder(path, self.is_admin) {
            Ok(()) => self.status = None,
            Err(msg) => self.status = Some(msg),
        }
    }
}
