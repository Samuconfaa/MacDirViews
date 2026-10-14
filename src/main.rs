//! MacDirViews - clone moderno di WinDirStat per macOS.
//!
//! Architettura:
//! - La scansione (`scanner`) gira in un thread separato (`std::thread::spawn`), cosi' la UI non si blocca.
//! - Stato condiviso: `Arc<Mutex<bool>>` (scansione in corso) e
//!   `Arc<Mutex<HashMap<PathBuf, u64>>>` (dimensione cumulativa di ogni cartella).
//! - La UI NON tiene mai il lock durante il disegno: `refresh()` copia i dati che servono
//!   in strutture locali e rilascia subito il mutex.
//! - La treemap e' disegnata solo con il painter nativo di egui (rettangoli e testo).
//!
//! Struttura dei moduli:
//! - `app`: stato dell'applicazione, azioni e interfaccia (`app/ui`)
//! - `scanner`: camminata dell'albero e calcolo delle dimensioni cumulative
//! - `system`: processi, Finder e regole sulle cartelle protette
//! - `utils`: formattazione e colori
//! - `constants`, `icon`: costanti globali e icona della finestra

mod app;
mod constants;
mod icon;
mod scanner;
mod system;
mod utils;

use eframe::egui;

use app::MacDirViewsApp;
use constants::APP_NAME;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_app_id(APP_NAME)
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([700.0, 450.0]);
    if let Some(icon) = icon::load() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|_cc| Ok(Box::new(MacDirViewsApp::default()))),
    )
}
