use anyhow::Result;
/**
 * @file history.rs
 * @brief Persistent history log for completed DVD ripping jobs.
 */
use std::path::{Path, PathBuf};

/// Re-export canonical domain storage port, entities, and infrastructure JSON adapter.
pub use crate::domain::entities::job::RipRecord;
pub use crate::domain::ports::storage::HistoryRepository;
pub use crate::infrastructure::storage::JsonHistoryRepository;

/// Formats a clean user-facing media display title (e.g. "Aliens (1986)" or "The Office - S01E05").
#[allow(dead_code)]
pub fn format_media_display_title(
    title: &str,
    year: Option<u32>,
    tv_info: Option<(u32, u32)>,
) -> String {
    if let Some((season, episode)) = tv_info {
        format!("{} - S{:02}E{:02}", title, season, episode)
    } else if let Some(yr) = year {
        format!("{} ({})", title, yr)
    } else {
        title.to_string()
    }
}

pub fn resolve_history_path(path: Option<&Path>) -> PathBuf {
    JsonHistoryRepository::resolve_history_path(path)
}

/// Loads history records from disk (default: `ripping_history.json`).
pub fn load_history(path: Option<&Path>) -> Vec<RipRecord> {
    JsonHistoryRepository::new().load_history(path)
}

/// Saves history records to disk using atomic file writing.
pub fn save_history(records: &[RipRecord], path: Option<&Path>) -> Result<()> {
    JsonHistoryRepository::new().save_history(records, path)
}

/// Clears history file on disk.
#[allow(dead_code)]
pub fn clear_history(path: Option<&Path>) -> Result<()> {
    JsonHistoryRepository::new().clear_history(path)
}

/// Convenience facade to insert a new rip record at the top of history and persist to disk.
pub fn record_rip_event(
    title: &str,
    media_type: &str,
    output_path: &str,
    status: &str,
) -> Result<()> {
    let repo = JsonHistoryRepository::new();
    repo.append_record(&RipRecord::new(title, media_type, output_path, status), None)
}

/// Purges history records older than max_days retention limit.
pub fn purge_expired_rip_records(max_days: u64, path: Option<&Path>) -> Result<usize> {
    let mut history = load_history(path);
    let original_len = history.len();
    if max_days == 0 {
        return Ok(0);
    }
    let cutoff_sec = max_days * 86400;
    let now = chrono::Utc::now().timestamp() as u64;

    history.retain(|r| {
        if let Ok(ts) = chrono::NaiveDateTime::parse_from_str(&r.timestamp, "%Y-%m-%d %H:%M:%S") {
            let record_time = ts.and_utc().timestamp() as u64;
            (now.saturating_sub(record_time)) <= cutoff_sec
        } else {
            true
        }
    });

    let removed = original_len.saturating_sub(history.len());
    if removed > 0 {
        save_history(&history, path)?;
        println!(
            "[Auto-Cleanup] Purged {} expired rip history records older than {} days.",
            removed, max_days
        );
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dvd_ripper_history_test_{}_{}",
                name,
                std::process::id()
            ));
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

        let rec1 = RipRecord::new(
            "Aliens (1986)",
            "Movie",
            "Films/Aliens (1986).mpg",
            "Success",
        );
        let rec2 = RipRecord::new(
            "Doctor Who - S01E01",
            "TV Series",
            "TV/Doctor Who/Season 01/S01E01.mpg",
            "Success",
        );

        let records = vec![rec1.clone(), rec2.clone()];
        save_history(&records, Some(&file_path)).unwrap();

        let loaded = load_history(Some(&file_path));
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].title, "Aliens (1986)");

        clear_history(Some(&file_path)).unwrap();
        assert!(load_history(Some(&file_path)).is_empty());
    }

    #[test]
    fn test_resolve_history_path_defaults() {
        let default_path = resolve_history_path(None);
        assert_eq!(
            default_path.file_name().and_then(|f| f.to_str()),
            Some("ripping_history.json")
        );
        let custom = PathBuf::from("/tmp/custom_history.json");
        assert_eq!(resolve_history_path(Some(&custom)), custom);
    }

    #[test]
    fn test_rip_record_construction() {
        let rec = RipRecord::new("The Matrix", "Movie", "Films/The Matrix.mp4", "Success");
        assert_eq!(rec.title, "The Matrix");
        assert_eq!(rec.media_type, "Movie");
        assert_eq!(rec.output_path, "Films/The Matrix.mp4");
        assert_eq!(rec.status, "Success");
        assert!(!rec.timestamp.is_empty());
    }

    #[test]
    fn test_format_media_display_title() {
        assert_eq!(
            format_media_display_title("Aliens", Some(1986), None),
            "Aliens (1986)"
        );
        assert_eq!(
            format_media_display_title("The Office", None, Some((1, 5))),
            "The Office - S01E05"
        );
        assert_eq!(
            format_media_display_title("Unknown Movie", None, None),
            "Unknown Movie"
        );
    }
}
