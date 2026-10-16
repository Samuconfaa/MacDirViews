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
                || upper == "$WINDOWS.~WS"
                || upper == "$WINREAGENT"
                || upper == "CONFIG.MSI"
            {
                return true;
            }
        }
        // Evita loop di junction points / reparse points storici di Windows
        let path_str = path.to_string_lossy().to_ascii_uppercase();
        if path_str.ends_with(r"\APPLICATION DATA\APPLICATION DATA")
            || (path_str.ends_with(r"\APPLICATION DATA") && path_str.contains(r"\APPDATA\LOCAL\APPLICATION DATA"))
        {
            return true;
        }
    }

    false
}

/// Verifica rapida sul solo nome della cartella per evitare allocazioni PathBuf inutili.
pub fn is_protected_name(name: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        if let Some(s) = name.to_str() {
            let upper = s.to_ascii_uppercase();
            if upper == "$RECYCLE.BIN"
                || upper == "SYSTEM VOLUME INFORMATION"
                || upper == "RECOVERY"
                || upper == "$WINDOWS.~BT"
                || upper == "$WINDOWS.~WS"
                || upper == "$WINREAGENT"
                || upper == "CONFIG.MSI"
            {
                return true;
            }
        }
    }

    #[cfg(unix)]
    {
        if let Some(s) = name.to_str() {
            if s == "dev" || s == "Network" || s == "net" || s == "cores" || s == ".vol" {
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
        if path_str.len() <= 3 && (path_str.ends_with(':') || path_str.ends_with(r":\")) {
            return true;
        }
        if let Some(win_dir) = std::env::var_os("WINDIR") {
            let win_str = win_dir.to_string_lossy().to_ascii_uppercase();
            if path_str == win_str || path_str.starts_with(&format!(r"{win_str}\")) {
                return true;
            }
        } else if path_str == r"C:\WINDOWS" || path_str.starts_with(r"C:\WINDOWS\") {
            return true;
        }
        if let Some(prog_files) = std::env::var_os("ProgramFiles") {
            let pf_str = prog_files.to_string_lossy().to_ascii_uppercase();
            if path_str == pf_str || path_str.starts_with(&format!(r"{pf_str}\")) {
                return true;
            }
        }
        if let Some(prog_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            let pf_str = prog_files_x86.to_string_lossy().to_ascii_uppercase();
            if path_str == pf_str || path_str.starts_with(&format!(r"{pf_str}\")) {
                return true;
            }
        }
        if let Some(prog_data) = std::env::var_os("ProgramData") {
            let pd_str = prog_data.to_string_lossy().to_ascii_uppercase();
            if path_str == pd_str || path_str.starts_with(&format!(r"{pd_str}\")) {
                return true;
            }
        }
        if let Some(home) = dirs_next::home_dir() {
            if let Some(users_dir) = home.parent() {
                let u_str = users_dir.to_string_lossy().to_ascii_uppercase();
                if path_str == u_str {
                    return true;
                }
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
                let display_name = format!("Unità ({letter})");
                volumes.push((display_name, drive_path));
            }
        }
        volumes
    }

    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

/// Restituisce la cartella radice predefinita per scansionare l'intero sistema.
pub fn system_root_path() -> PathBuf {
    #[cfg(windows)]
    {
        let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        PathBuf::from(format!("{sys_drive}\\"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/")
    }
}

/// Etichetta per il pulsante di scansione dell'intero sistema.
pub fn scan_all_label() -> &'static str {
    if cfg!(windows) {
        "Scansiona tutto (C:\\)"
    } else {
        "Scansiona tutto (/)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_protected_system_volumes() {
        assert!(is_protected_directory(Path::new("/System/Volumes")));
        assert!(is_protected_directory(Path::new("/dev")));
        assert!(is_protected_directory(Path::new("/.vol")));
        assert!(is_protected_directory(Path::new("/Network")));
    }

    #[test]
    #[cfg(unix)]
    fn test_not_protected() {
        assert!(!is_protected_directory(Path::new("/Users")));
        assert!(!is_protected_directory(Path::new("/tmp")));
        assert!(!is_protected_directory(Path::new("/Applications")));
        assert!(!is_protected_directory(Path::new("/Volumes")));
    }

    #[test]
    #[cfg(unix)]
    fn test_deletion_forbidden_root() {
        assert!(is_deletion_forbidden(Path::new("/")));
    }

    #[test]
    #[cfg(unix)]
    fn test_deletion_forbidden_top_level() {
        assert!(is_deletion_forbidden(Path::new("/Applications")));
        assert!(is_deletion_forbidden(Path::new("/Users")));
    }

    #[test]
    #[cfg(unix)]
    fn test_deletion_forbidden_users() {
        assert!(is_deletion_forbidden(Path::new("/Users/confa")));
    }

    #[test]
    #[cfg(unix)]
    fn test_deletion_forbidden_system() {
        assert!(is_deletion_forbidden(Path::new("/System")));
        assert!(is_deletion_forbidden(Path::new("/System/Library")));
        assert!(is_deletion_forbidden(Path::new("/usr/local")));
        assert!(is_deletion_forbidden(Path::new("/Volumes/Macintosh HD")));
    }

    #[test]
    #[cfg(windows)]
    fn test_protected_windows_directories() {
        assert!(is_protected_directory(Path::new(r"C:\$Recycle.Bin")));
        assert!(is_protected_directory(Path::new(r"C:\System Volume Information")));
        assert!(is_protected_directory(Path::new(r"C:\Recovery")));
        assert!(is_protected_directory(Path::new(r"C:\$WINDOWS.~BT")));
    }

    #[test]
    #[cfg(windows)]
    fn test_not_protected_windows() {
        assert!(!is_protected_directory(Path::new(r"C:\Users")));
        assert!(!is_protected_directory(Path::new(r"C:\Temp")));
        assert!(!is_protected_directory(Path::new(r"C:\Program Files")));
    }

    #[test]
    #[cfg(windows)]
    fn test_deletion_forbidden_windows() {
        assert!(is_deletion_forbidden(Path::new(r"C:\")));
        assert!(is_deletion_forbidden(Path::new(r"C:")));
        assert!(is_deletion_forbidden(Path::new(r"C:\Windows")));
        assert!(is_deletion_forbidden(Path::new(r"C:\Windows\System32")));
        assert!(is_deletion_forbidden(Path::new(r"C:\Program Files")));
    }

    #[test]
    fn test_deletion_allowed() {
        // Una cartella dentro la home dell'utente dovrebbe essere eliminabile
        // (a patto che non sia la home stessa o un suo antenato).
    }
}
