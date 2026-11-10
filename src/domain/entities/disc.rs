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

impl Chapter {
    /// Duration of this chapter in seconds.
    pub fn duration_secs(&self) -> f64 {
        (self.end_secs - self.start_secs).max(0.0)
    }
}

/// Detailed audio track metadata probed from IFO or FFmpeg stream analysis.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AudioStreamInfo {
    pub stream_index: u32,
    pub codec: String,
    pub language: String,
    pub channels: u32,
    pub channel_layout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl AudioStreamInfo {
    /// Human-readable summary badge for UI display (e.g., "Track #1: eng (AC3 5.1)").
    pub fn formatted_label(&self) -> String {
        let lang = if self.language.is_empty() {
            "und"
        } else {
            &self.language
        };
        let layout = if !self.channel_layout.is_empty() {
            self.channel_layout.clone()
        } else if self.channels == 6 {
            "5.1".to_string()
        } else if self.channels == 2 {
            "2.0 Stereo".to_string()
        } else if self.channels == 1 {
            "1.0 Mono".to_string()
        } else {
            format!("{}ch", self.channels)
        };
        let extra = self
            .title
            .as_deref()
            .map(|t| format!(" - {}", t))
            .unwrap_or_default();
        format!(
            "Track #{}: {} ({} {}){}",
            self.stream_index + 1,
            lang.to_uppercase(),
            self.codec.to_uppercase(),
            layout,
            extra
        )
    }
}

/// Detailed subtitle stream metadata probed from IFO or FFmpeg stream analysis.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SubtitleStreamInfo {
    pub stream_index: u32,
    pub language: String,
    pub format: String,
    pub forced: bool,
}

impl SubtitleStreamInfo {
    /// Human-readable summary badge for UI display (e.g., "Sub #1: ENG (VobSub) [Forced]").
    pub fn formatted_label(&self) -> String {
        let lang = if self.language.is_empty() {
            "und"
        } else {
            &self.language
        };
        let forced_tag = if self.forced { " [Forced]" } else { "" };
        format!(
            "Sub #{}: {} ({}){}",
            self.stream_index + 1,
            lang.to_uppercase(),
            self.format,
            forced_tag
        )
    }
}

/// Deep stream, chapter, and video signal topology for a single optical disc title.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TitleStreamDetails {
    pub title_num: u32,
    pub duration_secs: f64,
    pub chapters: Vec<Chapter>,
    pub audio_streams: Vec<AudioStreamInfo>,
    pub subtitle_streams: Vec<SubtitleStreamInfo>,
    pub angles: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<(u32, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dvd_title_info() {
        let title = DvdTitleInfo {
            title_num: 1,
            duration_secs: 7200.0,
        };
        let cloned = title.clone();
        assert_eq!(title, cloned);
        let json = serde_json::to_string(&title).unwrap();
        let de: DvdTitleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(title, de);
    }

    #[test]
    fn test_tv_episode_info() {
        let ep = TvEpisodeInfo {
            title_num: 2,
            episode_num: 1,
            duration_secs: 1800.0,
            formatted_name: "Show S01E01".to_string(),
        };
        let cloned = ep.clone();
        assert_eq!(ep, cloned);
        let json = serde_json::to_string(&ep).unwrap();
        let de: TvEpisodeInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(ep, de);
    }

    #[test]
    fn test_chapter() {
        let mut ch = Chapter::default();
        assert_eq!(ch.index, 0);
        ch.index = 1;
        ch.start_secs = 0.0;
        ch.end_secs = 300.0;
        ch.title = "Intro".to_string();

        let cloned = ch.clone();
        assert_eq!(ch, cloned);
        let json = serde_json::to_string(&ch).unwrap();
        let de: Chapter = serde_json::from_str(&json).unwrap();
        assert_eq!(ch, de);
    }

    #[test]
    fn test_disc_copy_protection_report() {
        let mut rep = DiscCopyProtectionReport::default();
        assert!(!rep.has_css_indicators);
        rep.has_css_indicators = true;
        rep.vob_count = 5;
        rep.ifo_count = 2;
        rep.total_bytes = 4_700_000_000;
        rep.diagnostic_notes
            .push("CSS encrypted sectors detected".to_string());

        let cloned = rep.clone();
        assert_eq!(rep, cloned);
        let json = serde_json::to_string(&rep).unwrap();
        let de: DiscCopyProtectionReport = serde_json::from_str(&json).unwrap();
        assert_eq!(rep, de);
    }

    #[test]
    fn test_disc_info() {
        let disc = DiscInfo {
            device_path: PathBuf::from("D:\\"),
            volume_label: Some("TEST_DISC".to_string()),
            fingerprint: "abcdef123456".to_string(),
            titles: vec![DvdTitleInfo {
                title_num: 1,
                duration_secs: 5400.0,
            }],
            protection: DiscCopyProtectionReport::default(),
        };
        let cloned = disc.clone();
        assert_eq!(disc, cloned);
        let json = serde_json::to_string(&disc).unwrap();
        let de: DiscInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(disc, de);
    }

    #[test]
    fn test_drive_status_info() {
        let status = DriveStatusInfo::new("E:\\");
        assert_eq!(status.drive, "E:\\");
        assert_eq!(status.status, "Idle");
        assert_eq!(status.progress, 0.0);
        assert_eq!(status.fps, "0");
        assert_eq!(status.speed, "0x");
        assert!(!status.has_selected_movie);
        assert!(!status.is_series);
        assert_eq!(status.year, None);

        let cloned = status.clone();
        assert_eq!(status, cloned);
        let json = serde_json::to_string(&status).unwrap();
        let de: DriveStatusInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(status, de);
    }

    #[test]
    fn test_disc_options() {
        let opts = DiscOptions::default();
        assert_eq!(opts.min_free_gb, 10);
        assert!(!opts.no_eject);
        assert_eq!(opts.eject_autoclose, None);
        assert!(!opts.spindown);

        let custom = DiscOptions {
            min_free_gb: 20,
            no_eject: true,
            eject_autoclose: Some(30),
            spindown: true,
        };
        let cloned = custom.clone();
        assert_eq!(custom, cloned);
        let json = serde_json::to_string(&custom).unwrap();
        let de: DiscOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(custom, de);
    }

    #[test]
    fn test_title_stream_details_and_stream_labels() {
        let ch = Chapter {
            index: 1,
            start_secs: 60.0,
            end_secs: 360.0,
            title: "Chapter 01".to_string(),
        };
        assert_eq!(ch.duration_secs(), 300.0);

        let audio = AudioStreamInfo {
            stream_index: 0,
            codec: "ac3".to_string(),
            language: "eng".to_string(),
            channels: 6,
            channel_layout: String::new(),
            title: Some("Surround".to_string()),
        };
        assert_eq!(
            audio.formatted_label(),
            "Track #1: ENG (AC3 5.1) - Surround"
        );

        let sub = SubtitleStreamInfo {
            stream_index: 1,
            language: "spa".to_string(),
            format: "dvdsub".to_string(),
            forced: true,
        };
        assert_eq!(sub.formatted_label(), "Sub #2: SPA (dvdsub) [Forced]");

        let details = TitleStreamDetails {
            title_num: 1,
            duration_secs: 5400.0,
            chapters: vec![ch],
            audio_streams: vec![audio],
            subtitle_streams: vec![sub],
            angles: 2,
            resolution: Some((720, 480)),
            aspect_ratio: Some("16:9".to_string()),
            frame_rate: Some("29.97".to_string()),
        };
        let json = serde_json::to_string(&details).unwrap();
        let de: TitleStreamDetails = serde_json::from_str(&json).unwrap();
        assert_eq!(details, de);
    }
}
