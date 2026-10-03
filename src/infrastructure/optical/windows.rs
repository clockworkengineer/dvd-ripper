use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};
use crate::domain::ports::optical_drive::{DriveDetector, OpticalDrive};

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
        let p = self.drive_path.to_string_lossy();
        crate::dvd::get_volume_label(&p)
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
        let rep = crate::dvd::inspect_disc_copy_protection(&self.drive_path);
        DiscCopyProtectionReport {
            has_css_indicators: rep.has_css_indicators,
            vob_count: rep.vob_count,
            ifo_count: rep.ifo_count,
            total_bytes: rep.total_bytes,
            diagnostic_notes: rep.diagnostic_notes,
        }
    }

    fn eject(&self) -> Result<()> {
        let p = self.drive_path.to_string_lossy();
        if crate::dvd::eject_disc(&p) {
            Ok(())
        } else {
            Err(anyhow!("Failed to eject disc drive at {}", self.drive_path.display()))
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
