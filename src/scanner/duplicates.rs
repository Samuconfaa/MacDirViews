//! Ricerca rapida di file duplicati con hashing gerarchico a più livelli.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hasher;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::system::protection::{is_protected_directory, is_protected_name};

/// Gruppo di file con contenuto identico.
#[derive(Clone, Debug, PartialEq)]
pub struct DuplicateGroup {
    pub size: u64,
    pub wasted_bytes: u64,
    pub files: Vec<PathBuf>,
}

/// Calcola l'hash parziale (primi 8 KB) di un file.
fn hash_partial(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0u8; 8192];
    let n = file.read(&mut buffer).ok()?;
    let mut hasher = DefaultHasher::new();
    hasher.write(&buffer[..n]);
    Some(hasher.finish())
}

/// Calcola l'hash completo di un file in blocchi da 64 KB.
fn hash_full(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0u8; 65536];
    let mut hasher = DefaultHasher::new();
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.write(&buffer[..n]);
    }
    Some(hasher.finish())
}

/// Cerca file duplicati nella cartella specificata.
///
/// Algoritmo in 3 fasi (ultra-veloce):
/// 1. Raggruppa per dimensione esatta. Scarta i file unici.
/// 2. Per i file con la stessa dimensione, calcola l'hash dei primi 8 KB.
/// 3. Per i file con stesso hash parziale, calcola l'hash completo.
pub fn find_duplicates(
    root: &Path,
    min_size_bytes: u64,
    cancel_flag: &Arc<AtomicBool>,
) -> Vec<DuplicateGroup> {
    // Fase 1: Raggruppamento per dimensione
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut dir_stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = dir_stack.pop() {
        if cancel_flag.load(Ordering::Relaxed) {
            return Vec::new();
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };

            if ft.is_dir() {
                let fname = entry.file_name();
                if fname.to_string_lossy().starts_with('.') || is_protected_name(&fname) {
                    continue;
                }
                let p = entry.path();
                if !is_protected_directory(&p) {
                    dir_stack.push(p);
                }
            } else if ft.is_file() {
                let fname = entry.file_name();
                if fname.to_string_lossy().starts_with('.') {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    let size = meta.len();
                    if size >= min_size_bytes {
                        by_size.entry(size).or_default().push(entry.path());
                    }
                }
            }
        }
    }

    // Filtra dimensioni con almeno 2 file
    let candidate_sizes: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_iter()
        .filter(|(_, files)| files.len() >= 2)
        .collect();

    // Fase 2 e 3: Hash parziale e completo
    let mut duplicate_groups: Vec<DuplicateGroup> = Vec::new();

    for (size, files) in candidate_sizes {
        if cancel_flag.load(Ordering::Relaxed) {
            return Vec::new();
        }

        // Se la dimensione è <= 8 KB, il sample hash è già l'hash completo
        let mut by_hash: HashMap<u64, Vec<PathBuf>> = HashMap::new();
        for file in files {
            if let Some(h) = hash_partial(&file) {
                by_hash.entry(h).or_default().push(file);
            }
        }

        for (_, partial_matches) in by_hash {
            if partial_matches.len() < 2 {
                continue;
            }

            if size <= 8192 {
                // Dimensione <= 8KB: già verificati identici
                let count = partial_matches.len() as u64;
                duplicate_groups.push(DuplicateGroup {
                    size,
                    wasted_bytes: (count.saturating_sub(1)) * size,
                    files: partial_matches,
                });
            } else {
                // Fase 3: Verifica con hash completo
                let mut full_hash_map: HashMap<u64, Vec<PathBuf>> = HashMap::new();
                for file in partial_matches {
                    if let Some(fh) = hash_full(&file) {
                        full_hash_map.entry(fh).or_default().push(file);
                    }
                }
                for (_, confirmed) in full_hash_map {
                    if confirmed.len() >= 2 {
                        let count = confirmed.len() as u64;
                        duplicate_groups.push(DuplicateGroup {
                            size,
                            wasted_bytes: (count.saturating_sub(1)) * size,
                            files: confirmed,
                        });
                    }
                }
            }
        }
    }

    // Ordina i gruppi per spazio sprecato (dal più pesante al più leggero)
    duplicate_groups.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));
    duplicate_groups
}
