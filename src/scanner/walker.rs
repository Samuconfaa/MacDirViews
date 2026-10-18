//! Scansione parallela multi-thread ad altissime prestazioni per macOS e Windows.
//!
//! Architettura a prova di bomba:
//! - Lettura parallela distribuita su tutti i thread di calcolo della CPU.
//! - Utilizzo di std::fs::read_dir standard: zero bug C-FFI o loop infiniti di librerie esterne.
//! - Su Unix / macOS: usa d_type da dirent (ft.is_dir(), ft.is_file(), ft.is_symlink() a zero costo syscall).
//! - Filtro immediato per nome (is_protected_name_str) per evitare allocazioni PathBuf su cartelle protette.
//! - Batching dei risultati (Vec<DirBatch>) per abbattere il passaggio di messaggi tra thread.
//! - Terminazione e cancellazione immediata (nessun deadlock né blocco infinito).
//! - Inserimento ottimizzato nell'albero delle directory con zero allocazioni superflue per nodi esistenti.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crossbeam_channel::{unbounded, RecvTimeoutError};
use eframe::egui;

use super::tree::{DirTree, FileInfo};
use crate::system::protection::{is_protected_directory, is_protected_name};

/// Numero massimo di file nella classifica "Top N file".
const MAX_TOP_FILES: usize = 20;

/// Risultato della scansione di una cartella.
struct DirBatch {
    path: PathBuf,
    direct_size: u64,
}

/// Cammina l'albero a partire da `root` in parallelo su tutti i core CPU e riempie `tree`.
pub fn scan_directory(
    root: PathBuf,
    scanning: Arc<Mutex<bool>>,
    tree: Arc<Mutex<DirTree>>,
    files_counter: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    let mut local_tree = DirTree::new(root.clone());
    let is_root = root == Path::new("/");

    let top_files_shared: Arc<Mutex<Vec<FileInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let skipped_dirs_shared: Arc<Mutex<Vec<(PathBuf, String)>>> = Arc::new(Mutex::new(Vec::new()));

    let (tx_dirs, rx_dirs) = unbounded::<PathBuf>();
    let (tx_results, rx_results) = unbounded::<Vec<DirBatch>>();

    let pending_dirs = Arc::new(AtomicUsize::new(1));
    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .clamp(4, 32);

    // Inserisci la cartella radice iniziale
    let _ = tx_dirs.send(root);

    // Avvia i worker thread paralleli
    let mut worker_handles = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        let rx_dirs_w = rx_dirs.clone();
        let tx_dirs_w = tx_dirs.clone();
        let tx_results_w = tx_results.clone();
        let pending_w = Arc::clone(&pending_dirs);
        let cancel_w = Arc::clone(&cancel_flag);
        let files_counter_w = Arc::clone(&files_counter);
        let top_files_w = Arc::clone(&top_files_shared);
        let skipped_w = Arc::clone(&skipped_dirs_shared);

        let handle = std::thread::spawn(move || {
            let mut local_file_acc = 0u64;
            let mut local_top_files: Vec<FileInfo> = Vec::with_capacity(MAX_TOP_FILES);
            let mut local_results: Vec<DirBatch> = Vec::with_capacity(64);

            while !cancel_w.load(Ordering::Relaxed) {
                if pending_w.load(Ordering::SeqCst) == 0 {
                    break;
                }

                let dir = match rx_dirs_w.recv_timeout(Duration::from_millis(5)) {
                    Ok(d) => d,
                    Err(RecvTimeoutError::Timeout) => {
                        if !local_results.is_empty() {
                            let _ = tx_results_w.send(std::mem::take(&mut local_results));
                        }
                        if pending_w.load(Ordering::SeqCst) == 0 {
                            break;
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                };

                let mut dir_size = 0u64;
                let mut new_subdirs = Vec::new();

                match std::fs::read_dir(&dir) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            if cancel_w.load(Ordering::Relaxed) {
                                break;
                            }
                            let ft = match entry.file_type() {
                                Ok(t) => t,
                                Err(_) => continue,
                            };

                            // Salta tassativamente qualsiasi symlink, junction point o link simbolico
                            if ft.is_symlink() {
                                continue;
                            }

                            if ft.is_dir() {
                                let fname = entry.file_name();
                                if is_protected_name(&fname) {
                                    continue;
                                }
                                let p = entry.path();
                                if is_protected_directory(&p) || (is_root && p.as_os_str() == "/Volumes") {
                                    continue;
                                }
                                new_subdirs.push(p);
                            } else if ft.is_file() {
                                let meta = match entry.metadata() {
                                    Ok(m) => m,
                                    Err(_) => continue,
                                };

                                #[cfg(unix)]
                                let size = meta.blocks() * 512;
                                #[cfg(not(unix))]
                                let size = meta.len();

                                dir_size += size;
                                local_file_acc += 1;

                                if size > 0
                                    && (local_top_files.len() < MAX_TOP_FILES
                                        || local_top_files.last().is_some_and(|f| size > f.size))
                                {
                                    let path = entry.path();
                                    let modified = meta.modified().ok();
                                    local_top_files.push(FileInfo { path, size, modified });
                                    local_top_files.sort_by(|a, b| b.size.cmp(&a.size));
                                    local_top_files.truncate(MAX_TOP_FILES);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        let mut sk = skipped_w.lock().unwrap();
                        if sk.len() < 50 {
                            sk.push((dir.clone(), err.to_string()));
                        }
                    }
                }

                local_results.push(DirBatch { path: dir, direct_size: dir_size });
                if local_results.len() >= 64 {
                    let _ = tx_results_w.send(std::mem::take(&mut local_results));
                }

                // Aggiungi le sottocartelle trovate
                let num_new = new_subdirs.len();
                if num_new > 0 {
                    pending_w.fetch_add(num_new, Ordering::SeqCst);
                    for sub in new_subdirs {
                        let _ = tx_dirs_w.send(sub);
                    }
                }

                // Aggiorna periodicamente il contatore globale dei file
                if local_file_acc >= 500 {
                    files_counter_w.fetch_add(local_file_acc, Ordering::Relaxed);
                    local_file_acc = 0;
                }

                let remaining = pending_w.fetch_sub(1, Ordering::SeqCst);
                if remaining == 1 {
                    break;
                }
            }

            if local_file_acc > 0 {
                files_counter_w.fetch_add(local_file_acc, Ordering::Relaxed);
            }
            if !local_results.is_empty() {
                let _ = tx_results_w.send(std::mem::take(&mut local_results));
            }
            top_files_w.lock().unwrap().extend(local_top_files);
        });
        worker_handles.push(handle);
    }

    // Chiudi i trasmettitori nel thread principale
    drop(tx_dirs);
    drop(tx_results);

    // Ricevi i batch in tempo reale: ciclo non bloccante con verifica costante di cancel_flag
    let mut last_repaint = Instant::now();
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        match rx_results.recv_timeout(Duration::from_millis(15)) {
            Ok(batches) => {
                for batch in batches {
                    local_tree.set_dir_direct_size(&batch.path, batch.direct_size);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if pending_dirs.load(Ordering::SeqCst) == 0 {
                    while let Ok(batches) = rx_results.try_recv() {
                        for batch in batches {
                            local_tree.set_dir_direct_size(&batch.path, batch.direct_size);
                        }
                    }
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if last_repaint.elapsed() >= Duration::from_millis(150) {
            ctx.request_repaint();
            last_repaint = Instant::now();
        }
    }

    // Segnala l'uscita a tutti i worker
    cancel_flag.store(true, Ordering::Relaxed);

    // Attendi la conclusione dei worker (ora si sbloccano subito entro pochi millisecondi)
    for handle in worker_handles {
        let _ = handle.join();
    }

    // Ricostruisci le dimensioni cumulative in una sola passata O(N)
    local_tree.rebuild_cumulative_sizes();

    // Sincronizza i top file e le cartelle saltate
    {
        let top = top_files_shared.lock().unwrap();
        local_tree.top_files = top.clone();
    }
    local_tree.top_files.sort_by(|a, b| b.size.cmp(&a.size));
    local_tree.top_files.truncate(MAX_TOP_FILES);
    {
        let sk = skipped_dirs_shared.lock().unwrap();
        local_tree.skipped_dirs = sk.len() as u64;
        local_tree.skipped_messages = sk.iter().map(|(p, m)| format!("{}: {}", p.display(), m)).collect();
    }

    // Risultato finale pubblicato
    *tree.lock().unwrap() = local_tree;
    *scanning.lock().unwrap() = false;
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_and_stop() {
        let root = PathBuf::from("target");
        let scanning = Arc::new(Mutex::new(true));
        let tree = Arc::new(Mutex::new(DirTree::new(root.clone())));
        let files_counter = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let ctx = egui::Context::default();

        scan_directory(root, scanning.clone(), tree, files_counter, cancel, ctx);
        assert!(!*scanning.lock().unwrap());
    }

    #[test]
    fn test_cancel_immediately() {
        let root = PathBuf::from("target");
        let scanning = Arc::new(Mutex::new(true));
        let tree = Arc::new(Mutex::new(DirTree::new(root.clone())));
        let files_counter = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(true));
        let ctx = egui::Context::default();

        scan_directory(root, scanning.clone(), tree, files_counter, cancel, ctx);
        assert!(!*scanning.lock().unwrap());
    }
}

