use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Hardware abstraction dedicated to optical drive discovery across operating systems.
pub trait DriveDetector: Send + Sync {
    fn detect_available_drives(&self) -> Vec<PathBuf>;
    fn auto_detect_primary(&self) -> Option<PathBuf>;
}

/// Hardware abstraction dedicated to querying optical volume labels, disc fingerprints, and copy protection.
pub trait DiscInspector: Send + Sync {
    fn read_volume_label(&self, drive_path: &Path) -> Option<String>;
    fn compute_fingerprint(&self, drive_path: &Path) -> String;
    fn inspect_protection(&self, drive_path: &Path) -> DiscCopyProtectionReport;
}

/// Hardware abstraction dedicated solely to optical tray ejection, tray closing, and drive spindown.
pub trait TrayController: Send + Sync {
    fn eject_tray(&self, drive_path: &Path) -> Result<()>;
    fn close_tray(&self, drive_path: &Path) -> Result<()>;
    fn spindown(&self, drive_path: &Path) -> Result<()>;
}

/// Fine-grained hardware abstraction for querying an optical drive's path identity, volume label, and disc fingerprint.
pub trait DriveIdentity: Send + Sync {
    fn get_path(&self) -> &Path;
    fn read_volume_label(&self) -> Option<String>;
    fn compute_fingerprint(&self) -> String {
        let p = self.get_path().to_string_lossy();
        crate::dvd::compute_disc_fingerprint(&p)
    }
}

/// Fine-grained hardware abstraction for discovering available title tracks on an optical disc.
pub trait TitleProber: Send + Sync {
    fn probe_titles(&self) -> Vec<DvdTitleInfo>;
}

/// Fine-grained hardware abstraction for diagnosing copy protection flags on an optical disc.
pub trait ProtectionInspector: Send + Sync {
    fn inspect_protection(&self) -> DiscCopyProtectionReport;
}

/// Fine-grained hardware abstraction for physical optical tray motor ejection.
pub trait TrayOperator: Send + Sync {
    fn eject(&self) -> Result<()>;
}

/// Unified hardware abstraction for an optical drive device, composed from fine-grained traits.
pub trait OpticalDrive:
    DriveIdentity + TitleProber + ProtectionInspector + TrayOperator + Send + Sync
{
}
