use std::path::Path;
use anyhow::Result;

/// Abstraction for persistent application history storage.
pub trait HistoryRepository: Send + Sync {
    fn load_history(&self, custom_path: Option<&Path>) -> Vec<crate::domain::entities::job::RipRecord>;
    fn append_record(&self, record: &crate::domain::entities::job::RipRecord, custom_path: Option<&Path>) -> Result<()>;
    fn clear_history(&self, custom_path: Option<&Path>) -> Result<()>;
}

/// Abstraction for target storage capacity guards.
pub trait DiskSpaceGuard: Send + Sync {
    fn check_free_space(&self, path: &Path, min_free_gb: u64) -> Result<()>;
}
