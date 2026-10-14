//! Icona della finestra.
//!
//! L'immagine viene incorporata nell'eseguibile a compilazione: per cambiare icona
//! basta sostituire `assets/icon.png` (consigliato PNG quadrato 256x256 o 512x512).
//!
//! Nota macOS: questa icona vale per la finestra durante `cargo run`. Per l'icona
//! del Dock/Finder serve un bundle `.app` con un file `.icns` (vedi README/Cargo.toml).

use eframe::egui;

const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// Carica l'icona incorporata. Se il PNG non e' valido l'app parte comunque senza icona.
pub fn load() -> Option<egui::IconData> {
    match eframe::icon_data::from_png_bytes(ICON_PNG) {
        Ok(icon) => Some(icon),
        Err(e) => {
            eprintln!("Icona non caricata (assets/icon.png non valido): {e}");
            None
        }
    }
}
