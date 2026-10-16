//! Azioni sul file manager (Finder su macOS, Esplora Risorse su Windows): mostra nel file manager e sposta nel Cestino.

use std::path::Path;
#[cfg(windows)]
use std::process::Command;

use super::process::user_session_command;

/// Restituisce il nome dell'applicazione di gestione file predefinita del sistema.
pub fn file_manager_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Finder"
    } else if cfg!(windows) {
        "Esplora File"
    } else {
        "File Manager"
    }
}

/// Sposta una cartella o file nel Cestino di sistema (Finder su macOS, Cestino su Windows).
pub fn move_to_trash(path: &Path, is_admin: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(windows)]
    {
        let _ = is_admin;
        let script = if path.is_dir() {
            format!(
                "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory('{}', 'OnlyErrorDialogs', 'SendToRecycleBin')",
                path.to_string_lossy().replace('\'', "''")
            )
        } else {
            format!(
                "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{}', 'OnlyErrorDialogs', 'SendToRecycleBin')",
                path.to_string_lossy().replace('\'', "''")
            )
        };
        match Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
        {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = is_admin;
        if path.is_file() {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        } else {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())
        }
    }
}

/// Mostra il percorso nel file manager (Finder su macOS, Esplora Risorse su Windows).
pub fn reveal_in_finder(path: &Path, is_admin: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(windows)]
    {
        let _ = is_admin;
        let p = path.to_string_lossy();
        // explorer.exe /select,"path" evidenzia l'elemento
        let mut cmd = Command::new("explorer.exe");
        cmd.arg(format!("/select,{}", p));
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Impossibile aprire Esplora File: {e}")),
        }
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = is_admin;
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(path);
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Impossibile aprire file manager: {e}")),
        }
    }
}
