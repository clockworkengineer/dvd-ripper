use std::path::Path;
use anyhow::{anyhow, Result};
use crate::domain::ports::storage::DiskSpaceGuard;

/// Implementation of DiskSpaceGuard querying filesystem free capacity.
#[derive(Debug, Default, Clone)]
pub struct SystemDiskSpaceGuard;

impl SystemDiskSpaceGuard {
    pub fn new() -> Self {
        Self
    }
}

impl DiskSpaceGuard for SystemDiskSpaceGuard {
    fn check_free_space(&self, path: &Path, min_free_gb: u64) -> Result<()> {
        if min_free_gb == 0 {
            return Ok(());
        }
        let free_bytes = crate::utils::get_free_disk_space_bytes(path)?;
        let required_bytes = min_free_gb * 1024 * 1024 * 1024;
        if free_bytes < required_bytes {
            let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            return Err(anyhow!(
                "Disk Space Guard Safeguard Triggered: Only {:.2} GB free space available in '{}', but minimum threshold is {} GB.",
                free_gb, path.display(), min_free_gb
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_disk_space_guard_zero_gb() {
        let guard = SystemDiskSpaceGuard::new();
        assert!(guard.check_free_space(Path::new("."), 0).is_ok());
    }
}
