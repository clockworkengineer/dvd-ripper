use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Represents runtime status of an individual optical drive in a multi-drive appliance pool.
///
/// # Examples
///
/// ```
/// use dvd_ripper::domain::entities::DriveStatusInfo;
///
/// let status = DriveStatusInfo::new("D:\\");
/// assert_eq!(status.drive, "D:\\");
/// assert_eq!(status.status, "Idle");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriveStatusInfo {
    pub drive: String,
    pub disc: String,
    pub status: String,
    pub current_title: String,
    pub progress: f64,
    pub fps: String,
    pub speed: String,
    pub has_selected_movie: bool,
    pub is_series: bool,
    pub year: Option<u32>,
}

impl DriveStatusInfo {
    pub fn new(drive: &str) -> Self {
        Self {
            drive: drive.to_string(),
            disc: String::new(),
            status: "Idle".to_string(),
            current_title: String::new(),
            progress: 0.0,
            fps: "0".to_string(),
            speed: "0x".to_string(),
            has_selected_movie: false,
            is_series: false,
            year: None,
        }
    }
}

/// Typed, focused configuration options for disc handling and optical drive behavior.
///
/// # Examples
///
/// ```
/// use dvd_ripper::domain::entities::DiscOptions;
///
/// let options = DiscOptions {
///     min_free_gb: 25,
///     no_eject: true,
///     ..Default::default()
/// };
/// assert_eq!(options.min_free_gb, 25);
/// assert!(options.no_eject);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscOptions {
    pub min_free_gb: u64,
    pub no_eject: bool,
    pub eject_autoclose: Option<u64>,
    pub spindown: bool,
}

impl Default for DiscOptions {
    fn default() -> Self {
        Self {
            min_free_gb: 10,
            no_eject: false,
            eject_autoclose: None,
            spindown: false,
        }
    }
}
