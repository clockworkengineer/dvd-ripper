use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Hardware abstraction for an optical drive device.
pub trait OpticalDrive: Send + Sync {
    fn get_path(&self) -> &Path;
    fn read_volume_label(&self) -> Option<String>;
    fn probe_titles(&self) -> Vec<DvdTitleInfo>;
    fn inspect_protection(&self) -> DiscCopyProtectionReport;
    fn eject(&self) -> Result<()>;
}

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
