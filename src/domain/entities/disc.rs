use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Basic probed duration and title index for a DVD title track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DvdTitleInfo {
    pub title_num: u32,
    pub duration_secs: f64,
}

/// Probed TV episode title candidate on disc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TvEpisodeInfo {
    pub title_num: u32,
    pub episode_num: u32,
    pub duration_secs: f64,
    pub formatted_name: String,
}

/// Chapter marker metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Chapter {
    pub index: u32,
    pub start_secs: f64,
    pub end_secs: f64,
    pub title: String,
}

/// Comprehensive copy protection diagnostic report.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DiscCopyProtectionReport {
    pub has_css_indicators: bool,
    pub vob_count: usize,
    pub ifo_count: usize,
    pub total_bytes: u64,
    pub diagnostic_notes: Vec<String>,
}

/// Full disc inspection entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscInfo {
    pub device_path: PathBuf,
    pub volume_label: Option<String>,
    pub fingerprint: String,
    pub titles: Vec<DvdTitleInfo>,
    pub protection: DiscCopyProtectionReport,
}
