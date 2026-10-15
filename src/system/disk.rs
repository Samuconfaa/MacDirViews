//! Rilevamento dello spazio su disco per il volume corrente.

use std::path::Path;
use std::process::Command;

/// Informazioni sulla capienza e uso del disco/volume.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiskUsage {
    pub volume_name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

impl DiskUsage {
    /// Percentuale di spazio usato (0.0 - 100.0).
    pub fn used_percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes as f64 / self.total_bytes as f64 * 100.0) as f32
        }
    }
}

/// Legge lo spazio su disco del volume che contiene `path`.
/// Esegue `df -k -P <path>` (standard POSIX supportato nativamente su macOS e Linux).
pub fn get_disk_usage(path: &Path) -> Option<DiskUsage> {
    let target = if path.as_os_str().is_empty() {
        Path::new("/")
    } else {
        path
    };

    // Su Unix / macOS: usa `df -k -P`
    #[cfg(unix)]
    {
        let output = Command::new("df")
            .arg("-k")
            .arg("-P")
            .arg(target)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        parse_df_output(&text)
    }

    // Su Windows (fallback di sviluppo per test locali)
    #[cfg(windows)]
    {
        // Se siamo su Windows, tentiamo di estrarre la lettera del drive (es. "C:" o "E:")
        let path_str = target.to_string_lossy();
        let drive = if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
            &path_str[0..2]
        } else {
            "C:"
        };

        // Prova a leggere via wmic / powershell in modo non bloccante
        let cmd = format!(
            "(Get-PSDrive -Name '{}' | Select-Object Used,Free | ConvertTo-Csv -NoTypeInformation)[1]",
            drive.trim_end_matches(':')
        );
        if let Ok(output) = Command::new("powershell").args(["-NoProfile", "-Command", &cmd]).output() {
            if output.status.success() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = out_str.trim().split(',').map(|s| s.trim_matches('"')).collect();
                if parts.len() >= 2 {
                    let used: u64 = parts[0].parse().unwrap_or(0);
                    let free: u64 = parts[1].parse().unwrap_or(0);
                    let total = used + free;
                    return Some(DiskUsage {
                        volume_name: format!("Disco ({})", drive),
                        mount_point: format!("{}\\", drive),
                        total_bytes: total,
                        used_bytes: used,
                        free_bytes: free,
                    });
                }
            }
        }
        None
    }
}

/// Esegue il parsing dell'output standard di `df -k -P`.
/// Formato:
/// Filesystem 1024-blocks Used Available Capacity Mounted on
/// /dev/disk3s1s1 482884968 11054368 44837584 20% /
pub fn parse_df_output(output: &str) -> Option<DiskUsage> {
    let mut lines = output.lines();
    let _header = lines.next()?;
    let data_line = lines.next()?;

    let parts: Vec<&str> = data_line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }

    let fs_name = parts[0];
    let total_k: u64 = parts[1].parse().ok()?;
    let used_k: u64 = parts[2].parse().ok()?;
    let avail_k: u64 = parts[3].parse().ok()?;
    // Il mount point può contenere spazi, quindi unisci gli elementi rimanenti
    let mount_point = parts[5..].join(" ");

    let volume_name = if mount_point == "/" {
        "Macintosh HD".to_string()
    } else if let Some(name) = Path::new(&mount_point).file_name() {
        name.to_string_lossy().to_string()
    } else {
        fs_name.to_string()
    };

    Some(DiskUsage {
        volume_name,
        mount_point,
        total_bytes: total_k.saturating_mul(1024),
        used_bytes: used_k.saturating_mul(1024),
        free_bytes: avail_k.saturating_mul(1024),
    })
}
