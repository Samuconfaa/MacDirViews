//! Formattazione dei valori per la UI.

/// Converte byte in una stringa leggibile (B, KB, MB, GB, TB).
pub fn fmt_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(1023), "1023 B");
    }

    #[test]
    fn test_kilobytes() {
        assert_eq!(fmt_size(1024), "1.0 KB");
        assert_eq!(fmt_size(3 * 1024), "3.0 KB");
        assert_eq!(fmt_size(1536), "1.5 KB");
    }

    #[test]
    fn test_megabytes() {
        assert_eq!(fmt_size(1024 * 1024), "1.00 MB");
        assert_eq!(fmt_size(50 * 1024 * 1024), "50.00 MB");
    }

    #[test]
    fn test_gigabytes() {
        assert_eq!(fmt_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(fmt_size(20 * 1024 * 1024 * 1024), "20.00 GB");
    }

    #[test]
    fn test_terabytes() {
        assert_eq!(fmt_size(1024_u64 * 1024 * 1024 * 1024), "1.00 TB");
        assert_eq!(fmt_size(3 * 1024_u64 * 1024 * 1024 * 1024), "3.00 TB");
    }
}
