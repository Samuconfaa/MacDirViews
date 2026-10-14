//! Azioni sul Finder di macOS: mostra nel Finder e sposta nel Cestino.

use std::path::Path;

use super::process::user_session_command;

/// Sposta una cartella nel Cestino tramite il Finder.
pub fn move_to_trash(path: &Path, is_admin: bool) -> Result<(), String> {
    let p = path.to_string_lossy();
    if p.contains('"') || p.contains('\\') {
        return Err("il percorso contiene caratteri non supportati".into());
    }
    let mut cmd = user_session_command(is_admin, "osascript");
    cmd.arg("-e")
        .arg(format!(r#"tell application "Finder" to delete POSIX file "{}""#, p));
    match cmd.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Mostra il percorso nel Finder. In caso di errore restituisce il messaggio per la UI.
pub fn reveal_in_finder(path: &Path, is_admin: bool) -> Result<(), String> {
    // Come root, `open` da solo non raggiunge la sessione grafica dell'utente.
    let mut cmd = user_session_command(is_admin, "open");
    cmd.arg("-R").arg(path);

    // `output()` attende la fine (e' immediato) e permette di mostrare l'errore reale.
    match cmd.output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(format!("Apertura nel Finder fallita: {err}"))
        }
        Err(e) => Err(format!("Impossibile aprire il Finder: {e}")),
    }
}
