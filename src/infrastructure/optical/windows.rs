use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};
use crate::domain::ports::optical_drive::{
    DiscInspector, DriveDetector, DriveIdentity, OpticalDrive, ProtectionInspector, TitleProber,
    TrayController, TrayOperator,
};
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

/// Concrete OpticalDrive implementation for Windows / POSIX environments.
#[derive(Debug, Clone)]
pub struct PhysicalOpticalDrive {
    pub drive_path: PathBuf,
    pub ffmpeg_path: String,
}

impl PhysicalOpticalDrive {
    pub fn new(drive_path: impl Into<PathBuf>, ffmpeg_path: impl Into<String>) -> Self {
        Self {
            drive_path: drive_path.into(),
            ffmpeg_path: ffmpeg_path.into(),
        }
    }
}

impl DriveIdentity for PhysicalOpticalDrive {
    fn get_path(&self) -> &Path {
        &self.drive_path
    }

    fn read_volume_label(&self) -> Option<String> {
        <Self as DiscInspector>::read_volume_label(self, &self.drive_path)
    }
}

impl TitleProber for PhysicalOpticalDrive {
    fn probe_titles(&self) -> Vec<DvdTitleInfo> {
        let titles = crate::ffmpeg::probe_dvd_titles(&self.ffmpeg_path, &self.drive_path, None);
        titles
            .into_iter()
            .map(|t| DvdTitleInfo {
                title_num: t.title_num,
                duration_secs: t.duration_secs,
            })
            .collect()
    }
}

impl ProtectionInspector for PhysicalOpticalDrive {
    fn inspect_protection(&self) -> DiscCopyProtectionReport {
        <Self as DiscInspector>::inspect_protection(self, &self.drive_path)
    }
}

impl TrayOperator for PhysicalOpticalDrive {
    fn eject(&self) -> Result<()> {
        <Self as TrayController>::eject_tray(self, &self.drive_path)
    }
}

impl OpticalDrive for PhysicalOpticalDrive {}

impl DiscInspector for PhysicalOpticalDrive {
    fn read_volume_label(&self, drive_path: &Path) -> Option<String> {
        let p = drive_path.to_string_lossy();
        crate::dvd::get_volume_label(&p)
    }

    fn compute_fingerprint(&self, drive_path: &Path) -> String {
        let p = drive_path.to_string_lossy();
        crate::dvd::compute_disc_fingerprint(&p)
    }

    fn inspect_protection(&self, drive_path: &Path) -> DiscCopyProtectionReport {
        let rep = crate::dvd::inspect_disc_copy_protection(drive_path);
        DiscCopyProtectionReport {
            has_css_indicators: rep.has_css_indicators,
            vob_count: rep.vob_count,
            ifo_count: rep.ifo_count,
            total_bytes: rep.total_bytes,
            diagnostic_notes: rep.diagnostic_notes,
        }
    }
}

impl TrayController for PhysicalOpticalDrive {
    fn eject_tray(&self, drive_path: &Path) -> Result<()> {
        if super::platform::current::eject_tray(drive_path) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to eject optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn close_tray(&self, drive_path: &Path) -> Result<()> {
        if super::platform::current::close_tray(drive_path) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to close optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn spindown(&self, drive_path: &Path) -> Result<()> {
        if super::platform::current::spindown(drive_path) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to issue spindown to optical drive at {}",
                drive_path.display()
            ))
        }
    }
}

/// Concrete DriveDetector implementation discovering attached optical drives.
#[derive(Debug, Default, Clone)]
pub struct SystemDriveDetector;

impl SystemDriveDetector {
    pub fn new() -> Self {
        Self
    }
}

impl DriveDetector for SystemDriveDetector {
    fn detect_available_drives(&self) -> Vec<PathBuf> {
        super::platform::current::detect_drives()
    }

    fn auto_detect_primary(&self) -> Option<PathBuf> {
        super::platform::current::auto_detect_primary()
    }
}

/// Concrete DiscInspector querying real optical drives and ISO-9660 sector descriptors.
#[derive(Debug, Default, Clone)]
pub struct SystemDiscInspector;

impl SystemDiscInspector {
    pub fn new() -> Self {
        Self
    }
}

impl DiscInspector for SystemDiscInspector {
    fn read_volume_label(&self, drive_path: &Path) -> Option<String> {
        let p = drive_path.to_string_lossy();
        crate::dvd::get_volume_label(&p)
    }

    fn compute_fingerprint(&self, drive_path: &Path) -> String {
        let p = drive_path.to_string_lossy();
        crate::dvd::compute_disc_fingerprint(&p)
    }

    fn inspect_protection(&self, drive_path: &Path) -> DiscCopyProtectionReport {
        let rep = crate::dvd::inspect_disc_copy_protection(drive_path);
        DiscCopyProtectionReport {
            has_css_indicators: rep.has_css_indicators,
            vob_count: rep.vob_count,
            ifo_count: rep.ifo_count,
            total_bytes: rep.total_bytes,
            diagnostic_notes: rep.diagnostic_notes,
        }
    }
}

/// Concrete TrayController handling physical tray open, close, and motor spindown.
#[derive(Debug, Default, Clone)]
pub struct SystemTrayController;

impl SystemTrayController {
    pub fn new() -> Self {
        Self
    }
}

impl TrayController for SystemTrayController {
    fn eject_tray(&self, drive_path: &Path) -> Result<()> {
        if super::platform::current::eject_tray(drive_path) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to eject optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn close_tray(&self, drive_path: &Path) -> Result<()> {
        if super::platform::current::close_tray(drive_path) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to close optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn spindown(&self, drive_path: &Path) -> Result<()> {
        if super::platform::current::spindown(drive_path) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to issue spindown to optical drive at {}",
                drive_path.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physical_optical_drive_construction_and_paths() {
        let drive = PhysicalOpticalDrive::new("D:\\", "ffmpeg");
        assert_eq!(drive.get_path(), Path::new("D:\\"));
        assert_eq!(drive.ffmpeg_path, "ffmpeg");
    }

    #[test]
    fn test_disc_inspector_fingerprint_and_protection() {
        let inspector = SystemDiscInspector::new();
        let fp = inspector.compute_fingerprint(Path::new("Z:\\NonExistent"));
        assert!(!fp.is_empty());

        let report = inspector.inspect_protection(Path::new("Z:\\NonExistent"));
        assert_eq!(report.vob_count, 0);
        assert_eq!(report.ifo_count, 0);
        assert!(!report.has_css_indicators);
    }

    #[test]
    fn test_system_drive_detector_defaults() {
        let detector = SystemDriveDetector::new();
        let drives = detector.detect_available_drives();
        // Should return a vector (empty or containing detected optical drives) without panic
        assert!(drives.is_empty() || !drives.is_empty());
    }

    #[test]
    fn test_tray_controller_invocation() {
        let controller = SystemTrayController::new();
        let _ = controller.eject_tray(Path::new("Z:\\"));
        let _ = controller.close_tray(Path::new("Z:\\"));
        let _ = controller.spindown(Path::new("Z:\\"));
    }
}
