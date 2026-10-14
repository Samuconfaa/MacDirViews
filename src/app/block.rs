//! Blocco della treemap.

use std::path::PathBuf;

/// Un blocco della treemap.
pub struct Block {
    pub path: PathBuf,
    pub label: String,
    pub size: u64,
    /// `true` se e' una sottocartella navigabile (non i blocchi speciali "file" / "altri").
    pub is_dir: bool,
}
