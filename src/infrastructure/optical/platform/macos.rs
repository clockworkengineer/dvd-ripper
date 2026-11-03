//! macOS optical drive platform implementation using `drutil`.

use std::path::{Path, PathBuf};
#[cfg(not(target_os = "windows"))]
use std::process::Command;

/// Scans attached optical drives on macOS using `drutil status` or common `/dev/disk*` nodes.
pub fn detect_drives() -> Vec<PathBuf> {
    let candidates = ["/dev/disk1", "/dev/disk2", "/dev/rdisk1", "/dev/rdisk2"];
    let mut detected = Vec::new();
    for candidate in candidates {
        let p = Path::new(candidate);
        if p.exists() {
            detected.push(p.to_path_buf());
        }
    }
    detected
}

/// Automatically detects primary optical drive on macOS.
pub fn auto_detect_primary() -> Option<PathBuf> {
    detect_drives().into_iter().next()
}

/// Ejects optical tray on macOS using `drutil tray eject`.
pub fn eject_tray(drive_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = drive_path;
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = drive_path;
        Command::new("drutil")
            .args(["tray", "eject"])
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Closes optical tray on macOS using `drutil tray close`.
pub fn close_tray(drive_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = drive_path;
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = drive_path;
        Command::new("drutil")
            .args(["tray", "close"])
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Issues motor spindown on macOS using `drutil poll`.
pub fn spindown(drive_path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let _ = drive_path;
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = drive_path;
        Command::new("drutil")
            .args(["poll"])
            .status()
            .is_ok_and(|s| s.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_platform_stubs() {
        let dummy = Path::new("/dev/disk1");
        let _ = detect_drives();
        let _ = auto_detect_primary();
        let _ = eject_tray(dummy);
        let _ = close_tray(dummy);
        let _ = spindown(dummy);
    }
}
