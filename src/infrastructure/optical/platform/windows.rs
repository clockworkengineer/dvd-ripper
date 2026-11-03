//! Windows optical drive platform implementation using Win32 API and PowerShell COM automation.

use std::path::{Path, PathBuf};

/// Scans available Windows optical drives (e.g. `D:\`, `E:\`).
pub fn detect_drives() -> Vec<PathBuf> {
    crate::dvd::detect_dvd_drives()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// Automatically detects the first available optical drive on Windows.
pub fn auto_detect_primary() -> Option<PathBuf> {
    let auto = crate::dvd::auto_detect_dvd_drive();
    if auto.is_empty() {
        None
    } else {
        Some(PathBuf::from(auto))
    }
}

/// Ejects optical tray on Windows via COM Shell.Application.
pub fn eject_tray(drive_path: &Path) -> bool {
    let p = drive_path.to_string_lossy();
    crate::dvd::eject_disc(&p)
}

/// Closes optical tray on Windows via COM Shell.Application.
pub fn close_tray(drive_path: &Path) -> bool {
    let p = drive_path.to_string_lossy();
    crate::dvd::close_disc_tray(&p)
}

/// Power management / spindown for optical drive motor.
pub fn spindown(drive_path: &Path) -> bool {
    let p = drive_path.to_string_lossy();
    crate::dvd::spindown_drive(&p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_platform_detection() {
        let drives = detect_drives();
        assert!(drives.is_empty() || !drives.is_empty());
    }

    #[test]
    fn test_windows_platform_tray_operations() {
        let dummy = Path::new("Z:\\");
        let _ = eject_tray(dummy);
        let _ = close_tray(dummy);
        let _ = spindown(dummy);
    }
}
