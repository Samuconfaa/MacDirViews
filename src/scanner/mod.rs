//! Scansione del disco: cammina l'albero in un thread separato e calcola le dimensioni.

mod rollup;
mod walker;

pub use walker::scan_directory;
