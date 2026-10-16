//! Struttura ad albero per i dati di scansione.
//!
//! `DirTree` tiene un albero di directory dove ogni nodo conosce la propria
//! dimensione diretta (file propri) e la dimensione cumulativa (inclusi figli).
//! Questo evita di ricalcolare tutto con `rollup` a ogni aggiornamento.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Informazioni su un singolo file (per la classifica Top N file e filtri).
#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

pub struct ShallowDirectory {
    pub path: PathBuf,
    pub direct_size: u64,
    pub child_names: Vec<String>,
    pub top_files: Vec<FileInfo>,
}

/// Nodo dell'albero: una directory con la sua dimensione.
#[derive(Clone, Debug)]
pub struct DirNode {
    /// Dimensione dei soli file direttamente in questa cartella.
    pub direct_size: u64,
    /// Dimensione cumulativa (direct_size + somma cumulativa di tutti i figli).
    pub cumulative_size: u64,
    pub scanned: bool,
    /// Figli diretti (nome componente -> nodo).
    pub children: HashMap<String, DirNode>,
}

impl DirNode {
    fn new() -> Self {
        Self {
            direct_size: 0,
            cumulative_size: 0,
            scanned: false,
            children: HashMap::new(),
        }
    }
}

/// Albero delle directory con accesso thread-safe (protetto esternamente da un Mutex).
#[derive(Clone, Debug)]
pub struct DirTree {
    pub root_path: PathBuf,
    pub root: DirNode,
    /// I file più grandi trovati durante la scansione.
    pub top_files: Vec<FileInfo>,
    /// Numero di cartelle saltate (permesso negato).
    pub skipped_dirs: u64,
    /// Messaggi sulle cartelle saltate (max 50 per non esplodere).
    pub skipped_messages: Vec<String>,
}

impl DirTree {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            root: DirNode::new(),
            top_files: Vec::new(),
            skipped_dirs: 0,
            skipped_messages: Vec::new(),
        }
    }

    /// Aggiunge la dimensione aggregata di una directory.
    /// Aggiorna la dimensione diretta e propaga la dimensione cumulativa a tutti gli antenati
    /// senza allocazioni di stringhe superflue quando i nodi sono già presenti.
    pub fn add_dir_size(&mut self, dir_path: &Path, dir_size: u64) {
        if dir_size == 0 && dir_path != self.root_path {
            return;
        }
        let rel = match dir_path.strip_prefix(&self.root_path) {
            Ok(r) => r,
            Err(_) => return,
        };

        let mut node = &mut self.root;
        node.cumulative_size += dir_size;

        for component in rel.components() {
            if let std::path::Component::Normal(c) = component {
                let name = c.to_string_lossy();
                if !node.children.contains_key(name.as_ref()) {
                    node.children.insert(name.to_string(), DirNode::new());
                }
                node = node.children.get_mut(name.as_ref()).unwrap();
                node.cumulative_size += dir_size;
            }
        }
        node.direct_size += dir_size;
    }

    /// Records direct directory sizes; call `rebuild_cumulative_sizes` once afterward.
    #[allow(dead_code)]
    pub fn set_dir_direct_size(&mut self, dir_path: &Path, dir_size: u64) {
        let Ok(rel) = dir_path.strip_prefix(&self.root_path) else {
            return;
        };
        let mut node = &mut self.root;
        for component in rel.components() {
            if let std::path::Component::Normal(c) = component {
                let name = c.to_string_lossy().into_owned();
                node = node.children.entry(name).or_insert_with(DirNode::new);
            }
        }
        node.direct_size += dir_size;
    }

    /// Rebuilds cumulative sizes in one post-order pass.
    pub fn rebuild_cumulative_sizes(&mut self) {
        fn sum(node: &mut DirNode) -> u64 {
            node.cumulative_size = node.direct_size;
            for child in node.children.values_mut() {
                node.cumulative_size += sum(child);
            }
            node.cumulative_size
        }
        sum(&mut self.root);
    }

    pub fn update_shallow_batch(&mut self, directories: Vec<ShallowDirectory>) {
        for directory in directories {
            let Ok(rel) = directory.path.strip_prefix(&self.root_path) else { continue };
            let mut node = &mut self.root;
            for component in rel.components() {
                if let std::path::Component::Normal(c) = component {
                    let name = c.to_string_lossy().into_owned();
                    node = node.children.entry(name).or_insert_with(DirNode::new);
                }
            }
            node.direct_size = directory.direct_size;
            node.scanned = true;
            let names: HashSet<&str> = directory.child_names.iter().map(String::as_str).collect();
            node.children.retain(|name, _| names.contains(name.as_str()));
            for name in directory.child_names {
                node.children.entry(name).or_insert_with(DirNode::new);
            }
            self.top_files.extend(directory.top_files);
        }
        self.top_files.sort_by(|a, b| b.size.cmp(&a.size));
        self.top_files.dedup_by(|a, b| a.path == b.path);
        self.top_files.truncate(20);
        self.rebuild_cumulative_sizes();
    }

    pub fn is_scanned(&self, abs_path: &Path) -> bool {
        self.get_node(abs_path).map(|node| node.scanned).unwrap_or(false)
    }

    pub fn needs_shallow_scan(&self, abs_path: &Path) -> bool {
        self.get_node(abs_path)
            .map(|node| !node.scanned || node.children.values().any(|child| !child.scanned))
            .unwrap_or(true)
    }

    pub fn is_shallow_complete(&self, abs_path: &Path) -> bool {
        self.get_node(abs_path)
            .map(|node| node.scanned && node.children.values().all(|child| child.scanned))
            .unwrap_or(false)
    }

    /// Aggiunge la dimensione di un file alla directory `parent_dir`.
    #[allow(dead_code)]
    pub fn add_file(&mut self, parent_dir: &Path, file_size: u64) {
        self.add_dir_size(parent_dir, file_size);
    }

    /// Verifica rapida in O(1) se la dimensione del file è abbastanza grande per entrare nella Top N.
    #[allow(dead_code)]
    pub fn is_top_file_candidate(&self, size: u64, max_top: usize) -> bool {
        if self.top_files.len() < max_top {
            true
        } else if let Some(min) = self.top_files.last() {
            size > min.size
        } else {
            true
        }
    }

    /// Aggiunge un file alla classifica top files (mantiene solo i top N).
    #[allow(dead_code)]
    pub fn add_top_file(
        &mut self,
        path: PathBuf,
        size: u64,
        modified: Option<SystemTime>,
        max_top: usize,
    ) {
        if self.top_files.len() < max_top {
            self.top_files.push(FileInfo {
                path,
                size,
                modified,
            });
            if self.top_files.len() == max_top {
                self.top_files.sort_by(|a, b| b.size.cmp(&a.size));
            }
        } else if let Some(min) = self.top_files.last() {
            if size > min.size {
                self.top_files.pop();
                let pos = self
                    .top_files
                    .binary_search_by(|f| f.size.cmp(&size).reverse())
                    .unwrap_or_else(|p| p);
                self.top_files.insert(
                    pos,
                    FileInfo {
                        path,
                        size,
                        modified,
                    },
                );
            }
        }
    }

    /// Registra una cartella saltata.
    #[allow(dead_code)]
    pub fn record_skipped(&mut self, path: &Path, reason: &str) {
        self.skipped_dirs += 1;
        if self.skipped_messages.len() < 50 {
            self.skipped_messages
                .push(format!("{}: {}", path.display(), reason));
        }
    }

    /// Dimensione cumulativa della root.
    pub fn total_size(&self) -> u64 {
        self.root.cumulative_size
    }

    /// Dimensione cumulativa di un percorso assoluto.
    pub fn size_of(&self, abs_path: &Path) -> u64 {
        match self.get_node(abs_path) {
            Some(n) => n.cumulative_size,
            None => 0,
        }
    }

    /// Ottiene il nodo corrispondente a un percorso assoluto.
    pub fn get_node(&self, abs_path: &Path) -> Option<&DirNode> {
        let rel = abs_path.strip_prefix(&self.root_path).ok()?;
        let mut node = &self.root;
        for component in rel.components() {
            let name = component.as_os_str().to_string_lossy();
            node = node.children.get(name.as_ref())?;
        }
        Some(node)
    }

    /// Restituisce i figli diretti di un percorso assoluto come (nome, path, cumulative_size).
    pub fn children_of(&self, abs_path: &Path) -> Vec<(String, PathBuf, u64)> {
        let node = match self.get_node(abs_path) {
            Some(n) => n,
            None => return Vec::new(),
        };
        let mut result: Vec<(String, PathBuf, u64)> = node
            .children
            .iter()
            .map(|(name, child)| (name.clone(), abs_path.join(name), child.cumulative_size))
            .collect();
        result.sort_by(|a, b| b.2.cmp(&a.2));
        result
    }

    /// Raccoglie tutte le cartelle discendenti con le relative dimensioni,
    /// per la Top 10 nella vista corrente.
    #[allow(dead_code)]
    pub fn all_descendants(&self, abs_path: &Path) -> Vec<(PathBuf, u64)> {
        let node = match self.get_node(abs_path) {
            Some(n) => n,
            None => return Vec::new(),
        };
        let mut result = Vec::new();
        Self::collect_descendants(node, abs_path, &mut result);
        result
    }

    #[allow(dead_code)]
    fn collect_descendants(node: &DirNode, path: &Path, out: &mut Vec<(PathBuf, u64)>) {
        for (name, child) in &node.children {
            let child_path = path.join(name);
            out.push((child_path.clone(), child.cumulative_size));
            Self::collect_descendants(child, &child_path, out);
        }
    }

    /// Rimuove un percorso (cartella O file singolo) dall'albero e aggiorna
    /// le dimensioni degli antenati e i top_files.
    pub fn remove(&mut self, abs_path: &Path) {
        // Rimuovi da top_files (sia file esatto che se abs_path era una cartella contenitrice)
        let mut file_size_hint = None;
        if let Some(pos) = self.top_files.iter().position(|f| f.path == abs_path) {
            file_size_hint = Some(self.top_files[pos].size);
            self.top_files.remove(pos);
        } else {
            self.top_files.retain(|f| !f.path.starts_with(abs_path));
        }

        let rel = match abs_path.strip_prefix(&self.root_path) {
            Ok(r) => r.to_path_buf(),
            Err(_) => return,
        };

        let components: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();

        if components.is_empty() {
            return;
        }

        let is_dir_node = self.get_node(abs_path).is_some();
        if is_dir_node {
            let removed_size = self.size_of(abs_path);
            let mut node = &mut self.root;
            node.cumulative_size = node.cumulative_size.saturating_sub(removed_size);

            for (i, name) in components.iter().enumerate() {
                if i + 1 == components.len() {
                    node.children.remove(name);
                    return;
                }
                if let Some(child) = node.children.get_mut(name) {
                    child.cumulative_size =
                        child.cumulative_size.saturating_sub(removed_size);
                    node = child;
                } else {
                    return;
                }
            }
        } else {
            // È un file singolo: sottrai la sua dimensione dagli antenati
            let file_size = file_size_hint.unwrap_or_else(|| {
                std::fs::metadata(abs_path).map(|m| m.len()).unwrap_or(0)
            });
            if file_size > 0 {
                let mut node = &mut self.root;
                node.cumulative_size = node.cumulative_size.saturating_sub(file_size);

                for (i, name) in components.iter().enumerate() {
                    if i + 1 == components.len() {
                        node.direct_size = node.direct_size.saturating_sub(file_size);
                        return;
                    }
                    if let Some(child) = node.children.get_mut(name) {
                        child.cumulative_size =
                            child.cumulative_size.saturating_sub(file_size);
                        node = child;
                    } else {
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_file_and_total() {
        let mut tree = DirTree::new(PathBuf::from("/test"));
        tree.add_file(Path::new("/test"), 100);
        tree.add_file(Path::new("/test/sub"), 200);
        tree.add_file(Path::new("/test/sub/deep"), 50);
        assert_eq!(tree.total_size(), 350);
        assert_eq!(tree.size_of(Path::new("/test/sub")), 250);
        assert_eq!(tree.size_of(Path::new("/test/sub/deep")), 50);
    }

    #[test]
    fn test_children_of() {
        let mut tree = DirTree::new(PathBuf::from("/root"));
        tree.add_file(Path::new("/root/a"), 100);
        tree.add_file(Path::new("/root/b"), 300);
        tree.add_file(Path::new("/root/a/c"), 50);
        let children = tree.children_of(Path::new("/root"));
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].0, "b");
        assert_eq!(children[0].2, 300);
        assert_eq!(children[1].0, "a");
        assert_eq!(children[1].2, 150);
    }

    #[test]
    fn test_remove() {
        let mut tree = DirTree::new(PathBuf::from("/root"));
        tree.add_file(Path::new("/root/a"), 100);
        tree.add_file(Path::new("/root/b"), 200);
        tree.add_file(Path::new("/root/a/c"), 50);
        assert_eq!(tree.total_size(), 350);

        tree.remove(Path::new("/root/a"));
        assert_eq!(tree.total_size(), 200);
        assert!(tree.get_node(Path::new("/root/a")).is_none());
        assert_eq!(tree.size_of(Path::new("/root/b")), 200);
    }

    #[test]
    fn test_top_files() {
        let mut tree = DirTree::new(PathBuf::from("/root"));
        for i in 0..20 {
            tree.add_top_file(
                PathBuf::from(format!("/root/file{}", i)),
                i * 10,
                None,
                10,
            );
        }
        assert_eq!(tree.top_files.len(), 10);
        assert_eq!(tree.top_files[0].size, 190);
        assert_eq!(tree.top_files[9].size, 100);
    }

    #[test]
    fn test_skipped() {
        let mut tree = DirTree::new(PathBuf::from("/root"));
        tree.record_skipped(Path::new("/root/secret"), "Permission denied");
        assert_eq!(tree.skipped_dirs, 1);
        assert_eq!(tree.skipped_messages.len(), 1);
    }
}
