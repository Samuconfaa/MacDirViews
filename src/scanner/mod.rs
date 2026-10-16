//! Scansione del disco: cammina l'albero in un thread separato e calcola le dimensioni.
//!
//! La struttura ad albero (`DirTree`) sostituisce la mappa piatta per il rollup:
//! ogni nodo tiene la sua dimensione diretta (file propri) e la somma cumulativa,
//! evitando di ricostruire tutto a ogni aggiornamento.

pub mod duplicates;
pub mod junk;
mod rollup;
mod tree;
mod walker;

pub use duplicates::{find_duplicates, DuplicateGroup};
pub use junk::{scan_developer_and_system_junk, JunkItem};
pub use tree::{DirTree, FileInfo};
pub use walker::scan_directory;
