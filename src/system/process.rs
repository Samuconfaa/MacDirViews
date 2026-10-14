//! Gestione dei privilegi e della sessione grafica dell'utente.

use std::process::Command;

/// `true` se il processo gira come root (uid 0).
pub fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

/// Crea un comando che gira nella sessione grafica dell'utente loggato.
/// Come root si passa da `launchctl asuser <uid>`, altrimenti il comando e' diretto.
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
