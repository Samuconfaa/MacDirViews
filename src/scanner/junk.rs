//! Rilevamento di cartelle cache e file temporanei di sistema e per sviluppatori.

use std::path::{Path, PathBuf};

/// Elemento spazzatura rilevato sul Mac.
#[derive(Clone, Debug, PartialEq)]
pub struct JunkItem {
    pub id: String,
    pub category: String,
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub safe_to_delete: bool,
    pub is_selected: bool,
}

/// Calcola rapidamente la dimensione di una cartella (senza scendere nei symlink).
pub fn quick_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    let mut visited = 0;

    while let Some(current) = stack.pop() {
        visited += 1;
        if visited > 50_000 {
            // Limite di sicurezza per evitare blocchi prolungati
            break;
        }
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        stack.push(entry.path());
                    } else if ft.is_file() {
                        if let Ok(m) = entry.metadata() {
                            total = total.saturating_add(m.len());
                        }
                    }
                }
            }
        }
    }
    total
}

/// Scansiona i percorsi noti di file temporanei e cartelle per sviluppatori.
pub fn scan_developer_and_system_junk(extra_search_root: Option<&Path>) -> Vec<JunkItem> {
    let mut items = Vec::new();
    let home = dirs_next::home_dir();

    if let Some(ref h) = home {
        // 1. Cestino macOS (~/.Trash)
        let trash_path = h.join(".Trash");
        if trash_path.is_dir() {
            let size = quick_dir_size(&trash_path);
            if size > 0 {
                items.push(JunkItem {
                    id: "trash".into(),
                    category: "Cestino macOS".into(),
                    name: "File nel Cestino".into(),
                    path: trash_path,
                    size,
                    safe_to_delete: true,
                    is_selected: true,
                });
            }
        }

        // 2. Xcode DerivedData
        let derived_data = h.join("Library/Developer/Xcode/DerivedData");
        if derived_data.is_dir() {
            let size = quick_dir_size(&derived_data);
            if size > 0 {
                items.push(JunkItem {
                    id: "xcode_derived".into(),
                    category: "Sviluppo (Xcode)".into(),
                    name: "Xcode DerivedData (Build temporanee)".into(),
                    path: derived_data,
                    size,
                    safe_to_delete: true,
                    is_selected: true,
                });
            }
        }

        // 3. Xcode iOS DeviceSupport
        let device_support = h.join("Library/Developer/Xcode/iOS DeviceSupport");
        if device_support.is_dir() {
            let size = quick_dir_size(&device_support);
            if size > 0 {
                items.push(JunkItem {
                    id: "xcode_devices".into(),
                    category: "Sviluppo (Xcode)".into(),
                    name: "iOS DeviceSupport (Simboli vecchi)".into(),
                    path: device_support,
                    size,
                    safe_to_delete: true,
                    is_selected: false,
                });
            }
        }

        // 4. Cache Homebrew
        let brew_cache = h.join("Library/Caches/Homebrew");
        if brew_cache.is_dir() {
            let size = quick_dir_size(&brew_cache);
            if size > 0 {
                items.push(JunkItem {
                    id: "brew_cache".into(),
                    category: "Package Manager".into(),
                    name: "Homebrew Cache (Download pacchetti)".into(),
                    path: brew_cache,
                    size,
                    safe_to_delete: true,
                    is_selected: true,
                });
            }
        }

        // 5. Cache Cargo (Rust)
        let cargo_cache = h.join(".cargo/registry/cache");
        if cargo_cache.is_dir() {
            let size = quick_dir_size(&cargo_cache);
            if size > 10 * 1024 * 1024 {
                // > 10 MB
                items.push(JunkItem {
                    id: "cargo_cache".into(),
                    category: "Package Manager (Rust)".into(),
                    name: "Cargo Registry Cache (.crate compressi)".into(),
                    path: cargo_cache,
                    size,
                    safe_to_delete: true,
                    is_selected: false,
                });
            }
        }

        // 6. Cache npm
        let npm_cache = h.join(".npm/_cacache");
        if npm_cache.is_dir() {
            let size = quick_dir_size(&npm_cache);
            if size > 10 * 1024 * 1024 {
                items.push(JunkItem {
                    id: "npm_cache".into(),
                    category: "Package Manager (Node.js)".into(),
                    name: "npm Cache temporanea".into(),
                    path: npm_cache,
                    size,
                    safe_to_delete: true,
                    is_selected: true,
                });
            }
        }

        // 7. Sottocartelle pesanti in ~/Library/Caches
        let caches_dir = h.join("Library/Caches");
        if let Ok(entries) = std::fs::read_dir(&caches_dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        let path = entry.path();
                        let folder_name = entry.file_name().to_string_lossy().to_string();
                        // Salta cartelle già conteggiate sopra
                        if folder_name == "Homebrew" || folder_name == "com.apple.dt.Xcode" {
                            continue;
                        }
                        let size = quick_dir_size(&path);
                        if size > 100 * 1024 * 1024 {
                            // > 100 MB
                            items.push(JunkItem {
                                id: format!("cache_{}", folder_name),
                                category: "Cache Applicazioni".into(),
                                name: format!("Cache: {}", folder_name),
                                path,
                                size,
                                safe_to_delete: true,
                                is_selected: false,
                            });
                        }
                    }
                }
            }
        }
    }

    // 8. Se è specificata una cartella di scansione attiva, cerca cartelle di build come target/ o node_modules
    if let Some(root) = extra_search_root {
        if root.is_dir() {
            find_build_folders(root, 0, 3, &mut items);
        }
    }

    items.sort_by(|a, b| b.size.cmp(&a.size));
    items
}

/// Cerca ricorsivamente cartelle note di build (max_depth per non rallentare)
fn find_build_folders(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<JunkItem>) {
    if depth > max_depth {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name == "node_modules" {
                        let size = quick_dir_size(&path);
                        out.push(JunkItem {
                            id: format!("node_{}", path.display()),
                            category: "Dipendenze Progetto (Node.js)".into(),
                            name: format!("node_modules in {}", dir.display()),
                            path,
                            size,
                            safe_to_delete: false, // richiedi esplicita selezione
                            is_selected: false,
                        });
                    } else if name == "target" && (dir.join("Cargo.toml").exists() || path.join("CACHEDIR.TAG").exists()) {
                        let size = quick_dir_size(&path);
                        out.push(JunkItem {
                            id: format!("target_{}", path.display()),
                            category: "Build Progetto (Rust)".into(),
                            name: format!("target/ in {}", dir.display()),
                            path,
                            size,
                            safe_to_delete: true,
                            is_selected: false,
                        });
                    } else if !name.starts_with('.') && name != "Library" && name != "System" {
                        find_build_folders(&path, depth + 1, max_depth, out);
                    }
                }
            }
        }
    }
}
