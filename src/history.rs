/**
 * @file history.rs
 * @brief Persistent history log for completed DVD ripping jobs.
 */

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};

const HISTORY_FILE: &str = "ripping_history.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RipRecord {
    pub timestamp: String,
    pub title: String,
    pub media_type: String,
    pub output_path: String,
    pub status: String,
}

impl RipRecord {
    pub fn new(title: &str, media_type: &str, output_path: &str, status: &str) -> Self {
        let now = chrono::Local::now();
        Self {
            timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            title: title.to_string(),
            media_type: media_type.to_string(),
            output_path: output_path.to_string(),
            status: status.to_string(),
        }
    }
}

/// Loads history records from disk (default: `ripping_history.json`).
pub fn load_history(path: Option<&Path>) -> Vec<RipRecord> {
    let default_path = PathBuf::from(HISTORY_FILE);
    let history_path = path.unwrap_or(&default_path);
    if !history_path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(history_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Saves history records to disk.
pub fn save_history(records: &[RipRecord], path: Option<&Path>) -> Result<()> {
    let default_path = PathBuf::from(HISTORY_FILE);
    let history_path = path.unwrap_or(&default_path);
    let json = serde_json::to_string_pretty(records)?;
    fs::write(history_path, json)?;
    Ok(())
}

/// Clears history file on disk.
pub fn clear_history(path: Option<&Path>) -> Result<()> {
    let default_path = PathBuf::from(HISTORY_FILE);
    let history_path = path.unwrap_or(&default_path);
    if history_path.exists() {
        fs::remove_file(history_path)?;
    }
    Ok(())
}

/// Convenience facade to insert a new rip record at the top of history and persist to disk.
pub fn record_rip_event(title: &str, media_type: &str, output_path: &str, status: &str) -> Result<()> {
    let mut history = load_history(None);
    history.insert(0, RipRecord::new(title, media_type, output_path, status));
    save_history(&history, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("dvd_ripper_history_test_{}_{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&path);
            let _ = fs::create_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_history_persistence() {
        let temp = TestTempDir::new("persist");
        let file_path = temp.0.join("test_history.json");

        assert!(load_history(Some(&file_path)).is_empty());

        let rec1 = RipRecord::new("Aliens (1986)", "Movie", "Films/Aliens (1986).mpg", "Success");
        let rec2 = RipRecord::new("Doctor Who - S01E01", "TV Series", "TV/Doctor Who/Season 01/S01E01.mpg", "Success");

        let records = vec![rec1.clone(), rec2.clone()];
        save_history(&records, Some(&file_path)).unwrap();

        let loaded = load_history(Some(&file_path));
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].title, "Aliens (1986)");

        clear_history(Some(&file_path)).unwrap();
        assert!(load_history(Some(&file_path)).is_empty());
    }
}
