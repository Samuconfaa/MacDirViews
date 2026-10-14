//! Il thread di scansione vero e proprio.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt; // per `blocks()` (spazio realmente occupato su disco)
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use jwalk::WalkDir;

use super::rollup::rollup;
use crate::system::protection::is_protected_directory;

/// Cammina l'albero a partire da `root` e riempie `sizes`.
/// Gira nel thread di scansione; aggiorna periodicamente la mappa condivisa.
pub fn scan_directory(
    root: PathBuf,
    scanning: Arc<Mutex<bool>>,
    sizes: Arc<Mutex<HashMap<PathBuf, u64>>>,
    files_counter: Arc<AtomicU64>,
    ctx: egui::Context,
) {
    // Dimensioni dirette per cartella, tenute in locale per non contendere il mutex.
    let mut direct: HashMap<PathBuf, u64> = HashMap::new();
    direct.insert(root.clone(), 0);

    let walker = WalkDir::new(&root)
        .skip_hidden(false)
        .follow_links(false)
        .process_read_dir(|_depth, _path, _state, children| {
            // Salta le cartelle protette prima ancora di entrarci.
            children.retain(|entry| match entry {
                Ok(e) => !(e.file_type().is_dir() && is_protected_directory(&e.path())),
                Err(_) => true, // gli errori (permessi negati) vengono ignorati dopo
            });
        });

    let mut last_publish = Instant::now();
    let mut seen: u64 = 0;

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // permesso negato o file sparito: si ignora
        };
        if !entry.file_type().is_file() {
            continue; // niente symlink/cartelle: contiamo solo i file
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        // Spazio realmente allocato su disco (blocchi da 512 byte).
        let size = meta.blocks() * 512;

        let path = entry.path();
        if let Some(parent) = path.parent() {
            if let Some(v) = direct.get_mut(parent) {
                *v += size;
            } else {
                direct.insert(parent.to_path_buf(), size);
            }
        }

        seen += 1;
        if seen % 1000 == 0 {
            files_counter.store(seen, Ordering::Relaxed);
            // Pubblica un'istantanea circa ogni secondo, cosi' la UI mostra dati parziali.
            if last_publish.elapsed() >= Duration::from_secs(1) {
                let snapshot = rollup(&direct, &root);
                *sizes.lock().unwrap() = snapshot;
                last_publish = Instant::now();
                ctx.request_repaint();
            }
        }
    }

    // Risultato finale.
    files_counter.store(seen, Ordering::Relaxed);
    let snapshot = rollup(&direct, &root);
    *sizes.lock().unwrap() = snapshot;
    *scanning.lock().unwrap() = false;
    ctx.request_repaint();
}
