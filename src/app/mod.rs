//! Stato dell'applicazione. Azioni, ciclo di aggiornamento e interfaccia
//! sono implementati nei sottomoduli (`actions`, `update`, `ui`).

mod actions;
mod block;
pub mod squarify;
mod ui;
mod update;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::scanner::{DirTree, FileInfo};
use crate::system::process::is_root;
use block::Block;

pub struct MacDirViewsApp {
    path_input: String,
    scanning: Arc<Mutex<bool>>,
    tree: Arc<Mutex<DirTree>>,
    files_counter: Arc<AtomicU64>,
    cancel_flag: Arc<AtomicBool>,

    scan_root: PathBuf,
    view_dir: PathBuf,

    // Dati derivati, copiati fuori dal mutex da `refresh()`.
    top10: Vec<(PathBuf, u64)>,
    blocks: Vec<Block>,
    total_size: u64,

    // Top file piu' grandi.
    top_files: Vec<FileInfo>,

    // Cartelle saltate.
    skipped_dirs: u64,
    skipped_messages: Vec<String>,

    selected: Option<PathBuf>,
    status: Option<String>,
    was_scanning: bool,
    last_refresh: Instant,

    // Cronologia di navigazione (Indietro / Avanti).
    back_stack: Vec<PathBuf>,
    fwd_stack: Vec<PathBuf>,

    // Permessi amministratore.
    is_admin: bool,
    admin_msg: Arc<Mutex<Option<String>>>,

    // Eliminazione: bersaglio del menu contestuale, richiesta in attesa di conferma
    // (percorso, definitiva?) ed esito del thread di eliminazione.
    ctx_target: Option<PathBuf>,
    pending_delete: Option<(PathBuf, bool)>,
    delete_result: Arc<Mutex<Option<(PathBuf, Result<(), String>)>>>,

    // Ricerca e filtri.
    search_query: String,
    filter_min_size_mb: f32,
}

impl Default for MacDirViewsApp {
    fn default() -> Self {
        Self {
            path_input: String::new(),
            scanning: Arc::new(Mutex::new(false)),
            tree: Arc::new(Mutex::new(DirTree::new(PathBuf::new()))),
            files_counter: Arc::new(AtomicU64::new(0)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            scan_root: PathBuf::new(),
            view_dir: PathBuf::new(),
            top10: Vec::new(),
            blocks: Vec::new(),
            total_size: 0,
            top_files: Vec::new(),
            skipped_dirs: 0,
            skipped_messages: Vec::new(),
            selected: None,
            status: None,
            was_scanning: false,
            last_refresh: Instant::now(),
            back_stack: Vec::new(),
            fwd_stack: Vec::new(),
            is_admin: is_root(),
            admin_msg: Arc::new(Mutex::new(None)),
            ctx_target: None,
            pending_delete: None,
            delete_result: Arc::new(Mutex::new(None)),
            search_query: String::new(),
            filter_min_size_mb: 0.0,
        }
    }
}
