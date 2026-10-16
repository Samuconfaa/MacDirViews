//! Gestione dei privilegi e della sessione grafica dell'utente.

use std::process::Command;

/// `true` se il processo gira con privilegi elevati (root su Unix / amministratore su Windows).
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // 'net session' restituisce codice 0 solo se il processo è elevato come amministratore
        let mut cmd = Command::new("net");
        cmd.arg("session");
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: evita flash della console
        cmd.output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

/// Crea un comando che gira nella sessione grafica dell'utente loggato.
/// Su macOS come root si passa da `launchctl asuser <uid>`, su Windows/altri il comando e' diretto.
#[cfg(target_os = "macos")]
pub fn user_session_command(is_admin: bool, program: &str) -> Command {
    if is_admin {
        let uid = std::env::var("SUDO_UID")
            .ok()
            .filter(|u| !u.is_empty())
            .or_else(|| {
                Command::new("stat")
                    .args(["-f", "%u", "/dev/console"])
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            })
            .unwrap_or_else(|| "501".to_string());
        let mut c = Command::new("launchctl");
        c.arg("asuser").arg(uid).arg(program);
        c
    } else {
        Command::new(program)
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn user_session_command(_is_admin: bool, program: &str) -> Command {
    Command::new(program)
}
