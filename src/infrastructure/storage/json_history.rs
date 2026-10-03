use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::domain::entities::job::RipRecord;
use crate::domain::ports::storage::HistoryRepository;

const HISTORY_FILE: &str = "ripping_history.json";

/// JSON file-backed history repository implementing the domain HistoryRepository port.
#[derive(Debug, Default, Clone)]
pub struct JsonHistoryRepository;

impl JsonHistoryRepository {
    pub fn new() -> Self {
        Self
    }

    fn resolve_path(&self, custom: Option<&Path>) -> PathBuf {
        if let Some(p) = custom {
            p.to_path_buf()
        } else {
            let local = PathBuf::from(HISTORY_FILE);
            if local.exists() {
                local
            } else {
                crate::utils::get_app_file_path(HISTORY_FILE)
            }
        }
    }
}

impl HistoryRepository for JsonHistoryRepository {
    fn load_history(&self, custom_path: Option<&Path>) -> Vec<RipRecord> {
        let path = self.resolve_path(custom_path);
        crate::utils::load_json_file(&path).unwrap_or_default()
    }

    fn append_record(&self, record: &RipRecord, custom_path: Option<&Path>) -> Result<()> {
        let mut history = self.load_history(custom_path);
        history.insert(0, record.clone());
        let path = self.resolve_path(custom_path);
        crate::utils::save_json_file(&path, &history)
    }

    fn clear_history(&self, custom_path: Option<&Path>) -> Result<()> {
        let path = self.resolve_path(custom_path);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_history_repo_roundtrip() {
        let temp = std::env::temp_dir().join("test_infra_history.json");
        let repo = JsonHistoryRepository::new();
        let _ = repo.clear_history(Some(&temp));

        let rec = RipRecord::new("Test Infra", "movie", "out.mp4", "Completed");
        repo.append_record(&rec, Some(&temp)).unwrap();

        let list = repo.load_history(Some(&temp));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Test Infra");

        repo.clear_history(Some(&temp)).unwrap();
        assert!(repo.load_history(Some(&temp)).is_empty());
    }
}
