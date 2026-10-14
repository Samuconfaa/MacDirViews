//! Regole sulle cartelle da saltare in scansione e da non eliminare mai.

use std::path::Path;

/// Restituisce `true` per le cartelle di sistema macOS che vanno saltate
/// (protette da SIP/TCC, virtuali, oppure che causerebbero loop o permessi negati).
///
/// Nota: `/Volumes` non e' piu' escluso dalla scansione, cosi' si possono analizzare
/// i dischi esterni. I sotto-volumi vengono pero' scansionati uno alla volta
/// (il selettore volume permette di sceglierne uno).
pub fn is_protected_directory(path: &Path) -> bool {
    // Con i permessi admin si scandisce tutto, tranne le cartelle che falserebbero
    // il conteggio o sono virtuali. Il confronto e' esatto: se la cartella viene
    // esclusa, non si scende nemmeno nei suoi figli.
    const SKIPPED: &[&str] = &[
        "/System/Volumes", // firmlink: il volume Data e' gia' raggiungibile da "/" (doppio conteggio)
        "/dev",            // file di dispositivo
        "/Network",        // montaggi di rete
        "/net",            // autofs: puo' bloccarsi
        "/home",           // autofs: puo' bloccarsi
        "/.vol",           // accesso per ID, duplica tutto
        "/cores",
    ];
    SKIPPED.iter().any(|p| path == Path::new(p))
}

/// Cartelle che non si devono mai poter eliminare dall'app: la radice, le cartelle
/// di primo livello, le home degli utenti, gli antenati della propria Home e le
/// cartelle di sistema.
pub fn is_deletion_forbidden(path: &Path) -> bool {
    match path.parent() {
        None => return true,
        Some(p) if p == Path::new("/") || p == Path::new("/Users") => return true,
        _ => {}
    }
    if let Some(home) = dirs_next::home_dir() {
        if home.starts_with(path) {
            return true; // la Home stessa o una cartella che la contiene
        }
    }
    const SYSTEM: &[&str] = &[
        "/System", "/usr", "/bin", "/sbin", "/etc", "/dev", "/cores", "/Volumes", "/private",
    ];
    SYSTEM.iter().any(|p| path.starts_with(p))
}

/// Restituisce l'elenco dei volumi montati in `/Volumes` (dischi esterni e immagini disco).
pub fn list_volumes() -> Vec<(String, std::path::PathBuf)> {
    let volumes_dir = Path::new("/Volumes");
    let mut volumes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(volumes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                volumes.push((name, path));
            }
        }
    }
    volumes.sort_by(|a, b| a.0.cmp(&b.0));
    volumes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protected_system_volumes() {
        assert!(is_protected_directory(Path::new("/System/Volumes")));
        assert!(is_protected_directory(Path::new("/dev")));
        assert!(is_protected_directory(Path::new("/.vol")));
        assert!(is_protected_directory(Path::new("/Network")));
    }

    #[test]
    fn test_not_protected() {
        assert!(!is_protected_directory(Path::new("/Users")));
        assert!(!is_protected_directory(Path::new("/tmp")));
        assert!(!is_protected_directory(Path::new("/Applications")));
        // /Volumes non e' piu' protetto dalla scansione.
        assert!(!is_protected_directory(Path::new("/Volumes")));
    }

    #[test]
    fn test_deletion_forbidden_root() {
        assert!(is_deletion_forbidden(Path::new("/")));
    }

    #[test]
    fn test_deletion_forbidden_top_level() {
        assert!(is_deletion_forbidden(Path::new("/Applications")));
        assert!(is_deletion_forbidden(Path::new("/Users")));
    }

    #[test]
    fn test_deletion_forbidden_users() {
        assert!(is_deletion_forbidden(Path::new("/Users/confa")));
    }

    #[test]
    fn test_deletion_forbidden_system() {
        assert!(is_deletion_forbidden(Path::new("/System")));
        assert!(is_deletion_forbidden(Path::new("/System/Library")));
        assert!(is_deletion_forbidden(Path::new("/usr/local")));
        assert!(is_deletion_forbidden(Path::new("/Volumes/Macintosh HD")));
    }

    #[test]
    fn test_deletion_allowed() {
        // Una cartella dentro la home dell'utente dovrebbe essere eliminabile
        // (a patto che non sia la home stessa o un suo antenato).
        // Nota: non possiamo testare questo in modo deterministico perche'
        // dirs_next::home_dir() dipende dall'ambiente.
    }
}
