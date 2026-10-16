//! Stato dell'applicazione. Azioni, ciclo di aggiornamento e interfaccia
//! sono implementati nei sottomoduli (`actions`, `update`, `ui`).

mod actions;
mod block;
pub mod squarify;
mod ui;
mod update;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::scanner::{DirTree, DuplicateGroup, FileInfo, JunkItem};
use crate::system::disk::DiskUsage;
use crate::system::process::is_root;
use block::Block;

/// Modalità di visualizzazione della vista centrale.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Treemap,
    Tree,
    Split,
}

/// Filtro per data di ultima modifica (Aging Filter).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AgingFilter {
    #[default]
    All,
    OlderThan1Month,
    OlderThan6Months,
    OlderThan1Year,
    OlderThan2Years,
}

impl AgingFilter {
    pub fn label(&self) -> &'static str {
        match self {
            AgingFilter::All => "Tutti i file",
            AgingFilter::OlderThan1Month => "> 1 Mese fa",
            AgingFilter::OlderThan6Months => "> 6 Mesi fa",
            AgingFilter::OlderThan1Year => "> 1 Anno fa",
            AgingFilter::OlderThan2Years => "> 2 Anni fa",
        }
    }
}

/// Strumenti secondari aperti a schermo o in sovrimpressione.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ActiveTool {
    #[default]
    None,
    JunkCleaner,
    DuplicateFinder,
}

pub struct DirTreeViewApp {
    pub path_input: String,
    pub scanning: Arc<Mutex<bool>>,
    pub scan_completed: Arc<std::sync::atomic::AtomicBool>,
    pub tree: Arc<Mutex<DirTree>>,
    pub files_counter: Arc<AtomicU64>,
    pub cancel_flag: Arc<AtomicBool>,

    pub scan_root: PathBuf,
    pub view_dir: PathBuf,

    // Dati derivati, copiati fuori dal mutex da `refresh()`.
    pub top10: Vec<(PathBuf, u64)>,
    pub blocks: Vec<Block>,
    pub total_size: u64,

    // Top file piu' grandi.
    pub top_files: Vec<FileInfo>,

    // Cartelle saltate.
    pub skipped_dirs: u64,
    pub skipped_messages: Vec<String>,

    pub selected: Option<PathBuf>,
    pub status: Option<String>,
    pub was_scanning: bool,
    pub last_refresh: Instant,

    // Cronologia di navigazione (Indietro / Avanti).
    pub back_stack: Vec<PathBuf>,
    pub fwd_stack: Vec<PathBuf>,

    // Permessi amministratore.
    pub is_admin: bool,
    pub admin_msg: Arc<Mutex<Option<String>>>,

    // Eliminazione: bersaglio del menu contestuale, richiesta in attesa di conferma
    // (percorso, definitiva?) ed esito del thread di eliminazione.
    pub ctx_target: Option<PathBuf>,
    pub pending_delete: Option<(PathBuf, bool)>,
    pub delete_result: Arc<Mutex<Option<(PathBuf, Result<(), String>)>>>,

    // Ricerca e filtri.
    pub search_query: String,
    pub filter_min_size_mb: f32,

    // --- Nuove Funzionalità ---
    // Vista centrale (Treemap / Albero / Split)
    pub view_mode: ViewMode,
    pub left_panel_open: bool,
    // Nodi espansi nella Tree View
    pub expanded_tree_nodes: HashSet<PathBuf>,

    // Filtro temporale / file vecchi
    pub aging_filter: AgingFilter,

    // Stato della memoria disco (Disk Usage Bar)
    pub disk_usage: Option<DiskUsage>,
    pub disk_usage_result: Arc<Mutex<Option<DiskUsage>>>,
    pub last_disk_refresh: Instant,

    // Strumenti attivi (Junk Cleaner & Duplicate Finder)
    pub active_tool: ActiveTool,

    // Developer & System Junk Cleaner
    pub junk_items: Vec<JunkItem>,
    pub junk_scanning: Arc<Mutex<bool>>,
    pub junk_result: Arc<Mutex<Option<Vec<JunkItem>>>>,

    // Duplicate Finder
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub duplicates_scanning: Arc<Mutex<bool>>,
    pub duplicates_cancel: Arc<AtomicBool>,
    pub duplicates_result: Arc<Mutex<Option<Vec<DuplicateGroup>>>>,

    // Sincronizzazione in tempo reale con il filesystem
    pub last_live_sync: Instant,
}

impl Default for DirTreeViewApp {
    fn default() -> Self {
        Self {
            path_input: String::new(),
            scanning: Arc::new(Mutex::new(false)),
            scan_completed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

            // Nuove opzioni
            view_mode: ViewMode::Treemap,
            left_panel_open: true,
            expanded_tree_nodes: HashSet::new(),
            aging_filter: AgingFilter::All,
            disk_usage: None,
            disk_usage_result: Arc::new(Mutex::new(None)),
            last_disk_refresh: Instant::now() - std::time::Duration::from_secs(60),
            active_tool: ActiveTool::None,
            junk_items: Vec::new(),
            junk_scanning: Arc::new(Mutex::new(false)),
            junk_result: Arc::new(Mutex::new(None)),
            duplicate_groups: Vec::new(),
            duplicates_scanning: Arc::new(Mutex::new(false)),
            duplicates_cancel: Arc::new(AtomicBool::new(false)),
            duplicates_result: Arc::new(Mutex::new(None)),
            last_live_sync: Instant::now(),
        }
    }
}
