//! Unix / Linux optical drive platform implementation using POSIX block devices and `eject`.

use std::path::{Path, PathBuf};
#[cfg(not(target_os = "windows"))]
use std::process::Command;

/// Scans standard Linux optical drive block device paths (`/dev/sr*`, `/dev/cdrom`, `/dev/dvd`).
pub fn detect_drives() -> Vec<PathBuf> {
    let candidates = ["/dev/sr0", "/dev/sr1", "/dev/sr2", "/dev/cdrom", "/dev/dvd"];
    let mut detected = Vec::new();
    for candidate in candidates {
        let p = Path::new(candidate);
        if p.exists() && !detected.contains(&p.to_path_buf()) {
            detected.push(p.to_path_buf());
        }
    }
    detected
}

/// Automatically detects the primary Linux optical drive.
pub fn auto_detect_primary() -> Option<PathBuf> {
    let candidates = ["/dev/cdrom", "/dev/dvd", "/dev/sr0", "/dev/sr1"];
    candidates
        .iter()
        .map(Path::new)
        .find(|p| p.exists())
        .map(Path::to_path_buf)
}

/// Ejects optical tray on Linux using `eject <device>`.
pub fn eject_tray(drive_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = drive_path;
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("eject")
            .arg(drive_path)
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Closes optical tray on Linux using `eject -t <device>`.
pub fn close_tray(drive_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = drive_path;
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("eject")
            .arg("-t")
            .arg(drive_path)
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Issues motor spindown on Linux using `eject -i 1 <device>`.
pub fn spindown(drive_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = drive_path;
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("eject")
            .args(["-i", "1"])
            .arg(drive_path)
            .status()
            .is_ok_and(|s| s.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_platform_stubs() {
        let dummy = Path::new("/dev/sr0");
        let _ = detect_drives();
        let _ = auto_detect_primary();
        let _ = eject_tray(dummy);
        let _ = close_tray(dummy);
        let _ = spindown(dummy);
    }
}
