//! Calcolo delle dimensioni cumulative per cartella.
//!
//! Mantenuto per compatibilita'; la struttura ad albero (`DirTree`) ora gestisce
//! il rollup in modo incrementale. Questa funzione puo' essere utile per test
//! o conversioni da formati piatti.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dalle dimensioni "dirette" (solo i file contenuti in ogni cartella) calcola
/// le dimensioni cumulative, propagando ogni valore a tutte le cartelle antenate fino a `root`.
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollup_basic() {
        let mut direct = HashMap::new();
        direct.insert(PathBuf::from("/root/a"), 100);
        direct.insert(PathBuf::from("/root/b"), 200);
        direct.insert(PathBuf::from("/root/a/c"), 50);

        let result = rollup(&direct, Path::new("/root"));
        assert_eq!(*result.get(Path::new("/root")).unwrap(), 350);
        assert_eq!(*result.get(Path::new("/root/a")).unwrap(), 150);
        assert_eq!(*result.get(Path::new("/root/b")).unwrap(), 200);
        assert_eq!(*result.get(Path::new("/root/a/c")).unwrap(), 50);
    }

    #[test]
    fn test_rollup_empty() {
        let direct = HashMap::new();
        let result = rollup(&direct, Path::new("/root"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_rollup_single() {
        let mut direct = HashMap::new();
        direct.insert(PathBuf::from("/root"), 42);
        let result = rollup(&direct, Path::new("/root"));
        assert_eq!(*result.get(Path::new("/root")).unwrap(), 42);
    }
}
