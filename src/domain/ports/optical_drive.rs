use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};

/// Hardware abstraction for an optical drive device.
pub trait OpticalDrive: Send + Sync {
    fn get_path(&self) -> &Path;
    fn read_volume_label(&self) -> Option<String>;
    fn probe_titles(&self) -> Vec<DvdTitleInfo>;
    fn inspect_protection(&self) -> DiscCopyProtectionReport;
    fn eject(&self) -> Result<()>;
}

/// Hardware abstraction for optical drive discovery across operating systems.
pub trait DriveDetector: Send + Sync {
    fn detect_available_drives(&self) -> Vec<PathBuf>;
    fn auto_detect_primary(&self) -> Option<PathBuf>;
}
