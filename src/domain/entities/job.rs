use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Execution status of an optical rip job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// Dynamic progress metrics emitted during transcoding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RipProgress {
    pub percent: f64,
    pub fps: String,
    pub speed: String,
    pub eta_secs: Option<u64>,
}

/// Historical record of a completed or cancelled rip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RipRecord {
    pub title: String,
    pub media_type: String,
    pub output_path: String,
    pub timestamp: String,
    pub status: String,
}

impl RipRecord {
    pub fn new(title: &str, media_type: &str, output_path: &str, status: &str) -> Self {
        let now = chrono::Local::now();
        Self {
            title: title.to_string(),
            media_type: media_type.to_string(),
            output_path: output_path.to_string(),
            timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            status: status.to_string(),
        }
    }
}
