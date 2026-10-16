//! Regole sulle cartelle da saltare in scansione e da non eliminare mai.

use std::path::{Path, PathBuf};

/// Restituisce `true` per le cartelle di sistema (macOS o Windows) che vanno saltate
/// (protette da permessi/SIP, virtuali, o che causerebbero loop o errori).
pub fn is_protected_directory(path: &Path) -> bool {
    #[cfg(unix)]
    {
        const SKIPPED: &[&str] = &[
            "/System/Volumes", // firmlink: il volume Data e' gia' raggiungibile da "/"
            "/dev",            // file di dispositivo
            "/Network",        // montaggi di rete
            "/net",            // autofs: puo' bloccarsi
            "/home",           // autofs: puo' bloccarsi
            "/.vol",           // accesso per ID, duplica tutto
            "/cores",
        ];
        if SKIPPED.iter().any(|p| path == Path::new(p)) {
            return true;
        }
    }

    #[cfg(windows)]
    {
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let upper = file_name.to_ascii_uppercase();
            if upper == "$RECYCLE.BIN"
                || upper == "SYSTEM VOLUME INFORMATION"
                || upper == "RECOVERY"
                || upper == "$WINDOWS.~BT"
                || upper == "$WINREAGENT"
            {
                return true;
            }
        }
    }

    false
}

/// Cartelle che non si devono mai poter eliminare dall'app: la radice, le cartelle
/// di primo livello, le home degli utenti, gli antenati della propria Home e le
/// cartelle di sistema.
pub fn is_deletion_forbidden(path: &Path) -> bool {
    // Non eliminare mai se non c'è una cartella genitore (es. "/" o radici)
    if path.parent().is_none() {
        return true;
    }

    #[cfg(unix)]
    {
        if let Some(p) = path.parent() {
            if p == Path::new("/") || p == Path::new("/Users") {
                return true;
            }
        }
        const SYSTEM: &[&str] = &[
            "/System", "/usr", "/bin", "/sbin", "/etc", "/dev", "/cores", "/Volumes", "/private",
        ];
        if SYSTEM.iter().any(|p| path.starts_with(p)) {
            return true;
        }
    }

    #[cfg(windows)]
    {
        let path_str = path.to_string_lossy().to_ascii_uppercase();
        // Evita eliminazione di interi drive es. "C:\" o "C:"
        if path_str.len() <= 3 && (path_str.ends_with(':') || path_str.ends_with(":\\")) {
            return true;
        }
        if let Some(win_dir) = std::env::var_os("WINDIR") {
            let win_path = Path::new(&win_dir);
            if path.starts_with(win_path) || win_path.starts_with(path) {
                return true;
            }
        }
        if let Some(prog_files) = std::env::var_os("ProgramFiles") {
            let pf_path = Path::new(&prog_files);
            if path == pf_path {
                return true;
            }
        }
    }

    if let Some(home) = dirs_next::home_dir() {
        if home.starts_with(path) {
            return true; // la Home stessa o una cartella che la contiene
        }
    }

    false
}

/// Restituisce l'elenco dei volumi montati (in `/Volumes` su macOS o lettere di unità da A: a Z: su Windows).
pub fn list_volumes() -> Vec<(String, PathBuf)> {
    #[cfg(unix)]
    {
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

    #[cfg(windows)]
    {
        let mut volumes = Vec::new();
        for c in b'A'..=b'Z' {
            let letter = format!("{}:", c as char);
            let drive_path = PathBuf::from(format!("{}:\\", c as char));
            if drive_path.exists() {
                volumes.push((letter, drive_path));
            }
        }
        volumes
    }

    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
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
