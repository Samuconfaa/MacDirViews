//! Azioni sul file manager (Finder su macOS, Esplora Risorse su Windows): mostra nel file manager e sposta nel Cestino.

use std::path::Path;
#[cfg(any(windows, not(any(target_os = "macos", windows))))]
use std::process::Command;

#[cfg(target_os = "macos")]
use super::process::user_session_command;

/// Restituisce il nome dell'applicazione di gestione file predefinita del sistema.
#[allow(dead_code)]
pub fn file_manager_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Finder"
    } else if cfg!(windows) {
        "Esplora File"
    } else {
        "File Manager"
    }
}

/// Etichetta completa per aprire l'elemento nel file manager ("Apri nel Finder" su macOS, "Apri in Esplora File" su Windows).
pub fn open_in_file_manager_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Apri nel Finder"
    } else if cfg!(windows) {
        "Apri in Esplora File"
    } else {
        "Apri nel File Manager"
    }
}

/// Etichetta compatta per i pulsanti ("Finder" su macOS, "Esplora File" su Windows).
pub fn file_manager_button_label() -> &'static str {
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
        use std::os::windows::process::CommandExt;
        let _ = is_admin;
        let escaped_path = path.to_string_lossy().replace('\'', "''");
        let script = if path.is_dir() {
            format!(r#"
$path = '{escaped_path}'
Add-Type -AssemblyName Microsoft.VisualBasic
try {{
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory(
        $path,
        [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs,
        [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin,
        [Microsoft.VisualBasic.FileIO.UICancelOption]::ThrowException)
}} catch {{
    $visualBasicError = $_.Exception.Message
    try {{
        $shell = New-Object -ComObject Shell.Application
        $recycleBin = $shell.Namespace(10)
        if ($null -eq $recycleBin) {{ throw 'Il Cestino di Windows non è disponibile.' }}
        $recycleBin.MoveHere($path, 0x14)
        $deadline = [DateTime]::UtcNow.AddSeconds(120)
        while ((Test-Path -LiteralPath $path) -and [DateTime]::UtcNow -lt $deadline) {{
            Start-Sleep -Milliseconds 100
        }}
        if (Test-Path -LiteralPath $path) {{ throw 'L''elemento è ancora presente dopo la richiesta al Cestino.' }}
    }} catch {{
        throw "Invio al Cestino fallito. Visual Basic: $visualBasicError. Shell Windows: $($_.Exception.Message)"
    }}
}}
"#)
        } else {
            format!(r#"
$path = '{escaped_path}'
Add-Type -AssemblyName Microsoft.VisualBasic
try {{
    [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile(
        $path,
        [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs,
        [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin,
        [Microsoft.VisualBasic.FileIO.UICancelOption]::ThrowException)
}} catch {{
    $visualBasicError = $_.Exception.Message
    try {{
        $shell = New-Object -ComObject Shell.Application
        $recycleBin = $shell.Namespace(10)
        if ($null -eq $recycleBin) {{ throw 'Il Cestino di Windows non è disponibile.' }}
        $recycleBin.MoveHere($path, 0x14)
        $deadline = [DateTime]::UtcNow.AddSeconds(120)
        while ((Test-Path -LiteralPath $path) -and [DateTime]::UtcNow -lt $deadline) {{
            Start-Sleep -Milliseconds 100
        }}
        if (Test-Path -LiteralPath $path) {{ throw 'L''elemento è ancora presente dopo la richiesta al Cestino.' }}
    }} catch {{
        throw "Invio al Cestino fallito. Visual Basic: $visualBasicError. Shell Windows: $($_.Exception.Message)"
    }}
}}
"#)
        };
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: evita flash della console
        match cmd.output() {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                if err.is_empty() {
                    Err("Eliminazione nel Cestino di Windows non riuscita".into())
                } else {
                    Err(err)
                }
            }
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
        let mut cmd = Command::new("explorer.exe");
        if path.parent().is_none() {
            cmd.arg(p.as_ref());
        } else {
            cmd.arg(format!("/select,{}", p));
        }
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
