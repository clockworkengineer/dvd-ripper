use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};
use crate::domain::ports::optical_drive::{
    DiscInspector, DriveDetector, OpticalDrive, TrayController,
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

impl OpticalDrive for PhysicalOpticalDrive {
    fn get_path(&self) -> &Path {
        &self.drive_path
    }

    fn read_volume_label(&self) -> Option<String> {
        <Self as DiscInspector>::read_volume_label(self, &self.drive_path)
    }

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

    fn inspect_protection(&self) -> DiscCopyProtectionReport {
        <Self as DiscInspector>::inspect_protection(self, &self.drive_path)
    }

    fn eject(&self) -> Result<()> {
        <Self as TrayController>::eject_tray(self, &self.drive_path)
    }
}

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
        let p = drive_path.to_string_lossy();
        if crate::dvd::eject_disc(&p) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to eject optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn close_tray(&self, drive_path: &Path) -> Result<()> {
        let p = drive_path.to_string_lossy();
        if crate::dvd::close_disc_tray(&p) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to close optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn spindown(&self, drive_path: &Path) -> Result<()> {
        let p = drive_path.to_string_lossy();
        if crate::dvd::spindown_drive(&p) {
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
        crate::dvd::detect_dvd_drives()
            .into_iter()
            .map(PathBuf::from)
            .collect()
    }

    fn auto_detect_primary(&self) -> Option<PathBuf> {
        let auto = crate::dvd::auto_detect_dvd_drive();
        if auto.is_empty() {
            None
        } else {
            Some(PathBuf::from(auto))
        }
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
        let p = drive_path.to_string_lossy();
        if crate::dvd::eject_disc(&p) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to eject optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn close_tray(&self, drive_path: &Path) -> Result<()> {
        let p = drive_path.to_string_lossy();
        if crate::dvd::close_disc_tray(&p) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to close optical drive tray at {}",
                drive_path.display()
            ))
        }
    }

    fn spindown(&self, drive_path: &Path) -> Result<()> {
        let p = drive_path.to_string_lossy();
        if crate::dvd::spindown_drive(&p) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to issue spindown to optical drive at {}",
                drive_path.display()
            ))
        }
    }
}
