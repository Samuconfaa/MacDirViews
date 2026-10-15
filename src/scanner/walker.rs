//! Il thread di scansione vero e proprio.
//!
//! Novita' rispetto alla versione precedente:
//! - `AtomicBool` per fermare la scansione in qualsiasi momento.
//! - Deduplicazione hard link / cloni APFS tramite `HashSet<(u64, u64)>` di (dev, ino).
//! - Conteggio delle cartelle saltate (permesso negato).
//! - Classifica dei file piu' grandi.
//! - Struttura ad albero incrementale (niente piu' rollup completo a ogni secondo).

#[cfg(unix)]
use std::collections::HashSet;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt; // per blocks(), dev(), ino(), nlink()
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use jwalk::WalkDir;

use super::tree::DirTree;
use crate::system::protection::is_protected_directory;

/// Numero massimo di file nella classifica "Top N file".
const MAX_TOP_FILES: usize = 20;

/// Cammina l'albero a partire da `root` e riempie `tree`.
/// Gira nel thread di scansione; aggiorna periodicamente l'albero condiviso.
pub fn scan_directory(
    root: PathBuf,
    scanning: Arc<Mutex<bool>>,
    tree: Arc<Mutex<DirTree>>,
    files_counter: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    // Albero locale per evitare di contendere il mutex a ogni file.
    let mut local_tree = DirTree::new(root.clone());

    // Set di (dev, ino) per la deduplicazione hard link / cloni APFS.
    #[cfg(unix)]
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();

    let walker = WalkDir::new(&root)
        .skip_hidden(false)
        .follow_links(false)
        .process_read_dir(|_depth, _path, _state, children| {
            // Salta le cartelle protette prima ancora di entrarci.
            children.retain(|entry| match entry {
                Ok(e) => !(e.file_type().is_dir() && is_protected_directory(&e.path())),
                Err(_) => true, // gli errori (permessi negati) vengono gestiti dopo
            });
        });

    let mut last_publish = Instant::now();
    let mut seen: u64 = 0;

    for entry in walker {
        // Controlla il flag di cancellazione.
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                // Permesso negato o altro errore su una directory.
                let path = e
                    .path()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("<sconosciuto>"));
                local_tree.record_skipped(&path, &e.to_string());
                continue;
            }
        };

        if entry.file_type().is_dir() {
            // Verifica che la directory sia leggibile.
            let path = entry.path();
            if std::fs::read_dir(&path).is_err() {
                local_tree.record_skipped(&path, "Permesso negato");
            }
            continue;
        }

        if !entry.file_type().is_file() {
            continue; // niente symlink: contiamo solo i file regolari
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Deduplicazione hard link / cloni APFS:
        // Se il file ha piu' di un nome (nlink > 1), controlliamo se l'abbiamo gia' visto.
        #[cfg(unix)]
        if meta.nlink() > 1 {
            let key = (meta.dev(), meta.ino());
            if !seen_inodes.insert(key) {
                continue; // gia' contato con un altro nome
            }
        }

        // Spazio realmente allocato su disco (blocchi da 512 byte su Unix/macOS, altrimenti len()).
        #[cfg(unix)]
        let size = meta.blocks() * 512;
        #[cfg(not(unix))]
        let size = meta.len();

        let path = entry.path();
        if let Some(parent) = path.parent() {
            local_tree.add_file(parent, size);
        }

        // Classifica file piu' grandi.
        if size > 0 {
            let modified = meta.modified().ok();
            local_tree.add_top_file(path.clone(), size, modified, MAX_TOP_FILES);
        }

        seen += 1;
        if seen % 1000 == 0 {
            files_counter.store(seen, Ordering::Relaxed);
            // Pubblica un'istantanea circa ogni secondo, cosi' la UI mostra dati parziali.
            if last_publish.elapsed() >= Duration::from_secs(1) {
                *tree.lock().unwrap() = local_tree.clone();
                last_publish = Instant::now();
                ctx.request_repaint();
            }
        }
    }

    // Risultato finale.
    files_counter.store(seen, Ordering::Relaxed);
    *tree.lock().unwrap() = local_tree;
    *scanning.lock().unwrap() = false;
    ctx.request_repaint();
}
