//! Regole sulle cartelle da saltare in scansione e da non eliminare mai.

use std::path::Path;

/// Restituisce `true` per le cartelle di sistema macOS che vanno saltate
/// (protette da SIP/TCC, virtuali, oppure che causerebbero loop o permessi negati).
pub fn is_protected_directory(path: &Path) -> bool {
    // Con i permessi admin si scandisce tutto, tranne le cartelle che falserebbero
    // il conteggio o sono virtuali. Il confronto e' esatto: se la cartella viene
    // esclusa, non si scende nemmeno nei suoi figli.
    const SKIPPED: &[&str] = &[
        "/System/Volumes", // firmlink: il volume Data e' gia' raggiungibile da "/" (doppio conteggio)
        "/Volumes",        // altri dischi montati + link al disco di avvio (loop)
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
