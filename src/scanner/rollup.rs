//! Calcolo delle dimensioni cumulative per cartella.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dalle dimensioni "dirette" (solo i file contenuti in ogni cartella) calcola
/// le dimensioni cumulative, propagando ogni valore a tutte le cartelle antenate fino a `root`.
pub fn rollup(direct: &HashMap<PathBuf, u64>, root: &Path) -> HashMap<PathBuf, u64> {
    let mut out: HashMap<PathBuf, u64> = HashMap::with_capacity(direct.len());
    for (dir, &size) in direct {
        for ancestor in dir.ancestors() {
            *out.entry(ancestor.to_path_buf()).or_insert(0) += size;
            if ancestor == root {
                break;
            }
        }
    }
    out
}
