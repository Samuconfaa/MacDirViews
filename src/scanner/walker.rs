//! Scansione parallela multi-thread ad altissime prestazioni per macOS e Windows.
//!
//! Ottimizzazioni chiave rispetto all'implementazione precedente:
//! - Lettura parallela distribuita su tutti i thread di calcolo della CPU.
//! - Su Windows: sfrutta i metadati pre-caricati da WIN32_FIND_DATA in std::fs::DirEntry (0 chiamate stat di sistema).
//! - Filtro immediato per nome (is_protected_name) per evitare allocazioni PathBuf su cartelle protette.
//! - Aggregazione per cartella: una sola scrittura cumulativa nell'albero per cartella invece di 1 per file.
//! - Classifica Top N file gestita con soglia minima atomica in O(1) per non allocare percorsi su file normali.
//! - Nessuna clonazione dell'intero albero durante la scansione: contatore atomico in tempo reale.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::collections::HashSet;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crossbeam_channel::{unbounded, RecvTimeoutError};
use eframe::egui;

use super::tree::{DirTree, FileInfo, ShallowDirectory};
use crate::system::protection::{is_protected_directory, is_protected_name};

/// Numero massimo di file nella classifica "Top N file".
const MAX_TOP_FILES: usize = 20;

pub fn scan_directory_shallow(
    dir: PathBuf,
    tree: Arc<Mutex<DirTree>>,
    files_counter: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,
) {
    let (root_result, mut count) = read_directory_level(&dir);
    let children: Vec<PathBuf> = root_result.child_names.iter().map(|name| dir.join(name)).collect();
    let mut results = vec![root_result];
    for child in children {
        if cancel_flag.load(Ordering::Relaxed) { break; }
        let (result, files) = read_directory_level(&child);
        count += files;
        results.push(result);
    }
    files_counter.fetch_add(count, Ordering::Relaxed);
    if !cancel_flag.load(Ordering::Relaxed) {
        tree.lock().unwrap().update_shallow_batch(results);
    }
}

fn read_directory_level(dir: &std::path::Path) -> (ShallowDirectory, u64) {
    let mut direct_size = 0u64;
    let mut child_names = Vec::new();
    let mut top_files = Vec::new();
    let mut count = 0u64;
    #[cfg(unix)]
    let mut seen_inodes = HashSet::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else { continue };
            if file_type.is_symlink() { continue; }
            if file_type.is_dir() {
                let name = entry.file_name();
                if !is_protected_name(&name) && !is_protected_directory(&entry.path()) {
                    child_names.push(name.to_string_lossy().into_owned());
                }
            } else if file_type.is_file() {
                let Ok(meta) = entry.metadata() else { continue };
                #[cfg(unix)]
                if meta.nlink() > 1 && !seen_inodes.insert((meta.dev(), meta.ino())) { continue; }
                #[cfg(unix)]
                let size = meta.blocks() * 512;
                #[cfg(not(unix))]
                let size = meta.len();
                direct_size += size;
                count += 1;
                if size > 0 && (top_files.len() < MAX_TOP_FILES
                    || top_files.last().is_some_and(|file: &FileInfo| size > file.size))
                {
                    top_files.push(FileInfo { path: entry.path(), size, modified: meta.modified().ok() });
                    top_files.sort_by(|a, b| b.size.cmp(&a.size));
                    top_files.truncate(MAX_TOP_FILES);
                }
            }
        }
    }
    (ShallowDirectory { path: dir.to_path_buf(), direct_size, child_names, top_files }, count)
}

/// Risultato della scansione di una cartella.
#[allow(dead_code)]
struct DirBatch {
    path: PathBuf,
    direct_size: u64,
}

/// Cammina l'albero a partire da `root` in parallelo su tutti i core CPU e riempie `tree`.
#[allow(dead_code)]
pub fn scan_directory(
    root: PathBuf,
    scanning: Arc<Mutex<bool>>,
    tree: Arc<Mutex<DirTree>>,
    files_counter: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    let mut local_tree = DirTree::new(root.clone());

    #[cfg(unix)]
    let seen_inodes: Arc<Mutex<HashSet<(u64, u64)>>> = Arc::new(Mutex::new(HashSet::new()));

    // Soglia minima per considerare un file tra i Top 20 (in byte).
    let min_top_threshold = Arc::new(AtomicU64::new(0));
    let top_files_shared: Arc<Mutex<Vec<FileInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let skipped_dirs_shared: Arc<Mutex<Vec<(PathBuf, String)>>> = Arc::new(Mutex::new(Vec::new()));

    let (tx_dirs, rx_dirs) = unbounded::<PathBuf>();
    let (tx_results, rx_results) = unbounded::<DirBatch>();

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
        let min_top_w = Arc::clone(&min_top_threshold);
        let top_files_w = Arc::clone(&top_files_shared);
        let skipped_w = Arc::clone(&skipped_dirs_shared);

        #[cfg(unix)]
        let seen_inodes_w = Arc::clone(&seen_inodes);

        let handle = std::thread::spawn(move || {
            let mut local_file_acc = 0u64;

            while !cancel_w.load(Ordering::Relaxed) {
                let dir = match rx_dirs_w.recv_timeout(Duration::from_millis(20)) {
                    Ok(d) => d,
                    Err(RecvTimeoutError::Timeout) => {
                        if pending_w.load(Ordering::SeqCst) == 0 {
                            break;
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                };

                // Leggi la directory in modo nativo ad alte prestazioni
                let read_res = std::fs::read_dir(&dir);
                let entries = match read_res {
                    Ok(e) => e,
                    Err(err) => {
                        let mut sk = skipped_w.lock().unwrap();
                        if sk.len() < 50 {
                            sk.push((dir, err.to_string()));
                        }
                        let rem = pending_w.fetch_sub(1, Ordering::SeqCst);
                        if rem == 1 {
                            break;
                        }
                        continue;
                    }
                };

                let mut dir_size = 0u64;
                let mut new_subdirs = Vec::new();

                for entry in entries.flatten() {
                    if cancel_w.load(Ordering::Relaxed) {
                        break;
                    }

                    let ft = match entry.file_type() {
                        Ok(t) => t,
                        Err(_) => continue,
                    };

                    // Salta tassativamente qualsiasi symlink, junction point o reparse point.
                    // Su Windows i junction point (come "Dati applicazioni", "Documents and Settings", "All Users")
                    // hanno is_symlink == true, causando altrimenti loop infiniti di ricorsione.
                    if ft.is_symlink() {
                        continue;
                    }

                    if ft.is_dir() {
                        let fname = entry.file_name();
                        if is_protected_name(&fname) {
                            continue;
                        }
                        let p = entry.path();
                        if !is_protected_directory(&p) {
                            new_subdirs.push(p);
                        }
                    } else if ft.is_file() {
                        let meta = match entry.metadata() {
                            Ok(m) => m,
                            Err(_) => continue,
                        };

                        #[cfg(unix)]
                        if meta.nlink() > 1 {
                            let key = (meta.dev(), meta.ino());
                            let mut inodes = seen_inodes_w.lock().unwrap();
                            if !inodes.insert(key) {
                                continue;
                            }
                        }

                        #[cfg(unix)]
                        let size = meta.blocks() * 512;
                        #[cfg(not(unix))]
                        let size = meta.len();

                        dir_size += size;
                        local_file_acc += 1;

                        if size > 0 && size > min_top_w.load(Ordering::Relaxed) {
                            let path = entry.path();
                            let modified = meta.modified().ok();
                            let mut top = top_files_w.lock().unwrap();
                            if top.len() < MAX_TOP_FILES {
                                top.push(FileInfo { path, size, modified });
                                if top.len() == MAX_TOP_FILES {
                                    top.sort_by(|a, b| b.size.cmp(&a.size));
                                    min_top_w.store(top.last().map(|f| f.size).unwrap_or(0), Ordering::Relaxed);
                                }
                            } else if let Some(last) = top.last() {
                                if size > last.size {
                                    top.pop();
                                    let pos = top.binary_search_by(|f| f.size.cmp(&size).reverse()).unwrap_or_else(|p| p);
                                    top.insert(pos, FileInfo { path, size, modified });
                                    min_top_w.store(top.last().map(|f| f.size).unwrap_or(0), Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }

                // Invia i risultati aggregati della cartella solo se ha file diretti
                if dir_size > 0 {
                    let _ = tx_results_w.send(DirBatch {
                        path: dir,
                        direct_size: dir_size,
                    });
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
                if local_file_acc >= 1000 {
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
        });
        worker_handles.push(handle);
    }

    // Chiudi i trasmettitori nel thread principale
    drop(tx_dirs);
    drop(tx_results);

    // Thread principale: riceve i batch e costruisce l'albero
    let mut last_repaint = Instant::now();
    while let Ok(batch) = rx_results.recv() {
        local_tree.set_dir_direct_size(&batch.path, batch.direct_size);

        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        if last_repaint.elapsed() >= Duration::from_millis(500) {
            ctx.request_repaint();
            last_repaint = Instant::now();
        }
    }

    // Attendi la conclusione dei worker
    for handle in worker_handles {
        let _ = handle.join();
    }

    // Aggregate each directory's size into its ancestors in one tree pass, instead
    // of revisiting every ancestor for every directory returned by the workers.
    local_tree.rebuild_cumulative_sizes();

    // Sincronizza i top file e le cartelle saltate
    {
        let top = top_files_shared.lock().unwrap();
        local_tree.top_files = top.clone();
    }
    {
        let sk = skipped_dirs_shared.lock().unwrap();
        local_tree.skipped_dirs = sk.len() as u64;
        local_tree.skipped_messages = sk.iter().map(|(p, m)| format!("{}: {}", p.display(), m)).collect();
    }

    // Risultato finale
    *tree.lock().unwrap() = local_tree;
    *scanning.lock().unwrap() = false;
    ctx.request_repaint();
}
