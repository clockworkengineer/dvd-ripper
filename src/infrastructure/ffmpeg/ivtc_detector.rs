//! FFmpeg Cadence and Interlace Detection (idet) Analyzer Adapter.
//!
//! Executes or parses FFmpeg `idet` filter analysis runs on DVD titles to determine
//! whether content is progressive, soft telecine (RFF), hard telecine (3:2 pulldown),
//! or pure interlaced, feeding into [`CadenceAnalyzer`].

use crate::domain::heuristics::telecine::{
    CadenceAnalyzer, FieldOrder, TelecineFilterRecommendation, TelecineMetrics, VideoCadence,
};
use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Parses FFmpeg `idet` filter output from stderr log text.
///
/// FFmpeg `idet` outputs lines such as:
/// ```text
/// [Parsed_idet_0 @ 000001a] Repeated fields: Neither:   600 Top:   400 Bottom:     0
/// [Parsed_idet_0 @ 000001a] Single frame detection: TFF:   800 BFF:     0 Progressive:   200 Undetermined:     0
/// [Parsed_idet_0 @ 000001a] Multi frame detection: TFF:   850 BFF:     0 Progressive:   150 Undetermined:     0
/// ```
#[must_use]
pub fn parse_idet_output(stderr_text: &str, frame_rate: f64) -> TelecineMetrics {
    let mut metrics = TelecineMetrics {
        frame_rate,
        ..Default::default()
    };

    let mut tff_count: u64 = 0;
    let mut bff_count: u64 = 0;
    let mut prog_count: u64 = 0;
    let mut _neither_count: u64 = 0;
    let mut rff_top: u64 = 0;
    let mut rff_bottom: u64 = 0;

    for line in stderr_text.lines() {
        let trimmed = line.trim();

        // 1. Parse Repeated fields
        if trimmed.contains("Repeated fields:") {
            if let Some(val) = extract_idet_metric(trimmed, "Neither:") {
                _neither_count = val;
            }
            if let Some(val) = extract_idet_metric(trimmed, "Top:") {
                rff_top = val;
            }
            if let Some(val) = extract_idet_metric(trimmed, "Bottom:") {
                rff_bottom = val;
            }
        }

        // 2. Parse Multi frame detection (more reliable than single frame)
        if trimmed.contains("Multi frame detection:") {
            if let Some(val) = extract_idet_metric(trimmed, "TFF:") {
                tff_count = val;
            }
            if let Some(val) = extract_idet_metric(trimmed, "BFF:") {
                bff_count = val;
            }
            if let Some(val) = extract_idet_metric(trimmed, "Progressive:") {
                prog_count = val;
            }
        }
    }

    let total_frames = tff_count + bff_count + prog_count;
    metrics.total_frames = total_frames;
    metrics.rff_frames = rff_top + rff_bottom;
    metrics.combed_frames = tff_count + bff_count;

    if tff_count >= bff_count && (tff_count > 0 || total_frames == 0) {
        metrics.reported_field_order = FieldOrder::TopFieldFirst;
    } else if bff_count > tff_count {
        metrics.reported_field_order = FieldOrder::BottomFieldFirst;
    } else if prog_count > 0 {
        metrics.reported_field_order = FieldOrder::Progressive;
    }

    metrics.sequence_progressive = prog_count > (tff_count + bff_count) * 2;

    metrics
}

fn extract_idet_metric(line: &str, label: &str) -> Option<u64> {
    if let Some(pos) = line.find(label) {
        let remainder = &line[pos + label.len()..];
        let token = remainder
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| !c.is_ascii_digit());
        token.parse::<u64>().ok()
    } else {
        None
    }
}

/// Service that executes a fast 60-second probe with FFmpeg `idet` to detect cadence.
pub struct IvtcDetector {
    ffmpeg_bin: String,
}

impl Default for IvtcDetector {
    fn default() -> Self {
        Self {
            ffmpeg_bin: "ffmpeg".to_string(),
        }
    }
}

impl IvtcDetector {
    /// Creates a new detector specifying custom FFmpeg binary.
    #[must_use]
    pub fn new(ffmpeg_bin: impl Into<String>) -> Self {
        Self {
            ffmpeg_bin: ffmpeg_bin.into(),
        }
    }

    /// Analyzes an optical or file source and returns the classified cadence and recommended filter.
    pub fn probe_cadence(
        &self,
        source: &Path,
        sample_duration_secs: u32,
    ) -> Result<(VideoCadence, TelecineFilterRecommendation)> {
        let mut cmd = Command::new(&self.ffmpeg_bin);
        cmd.arg("-hide_banner")
            .arg("-nostats")
            .arg("-ss")
            .arg("30") // Skip intro / FBI warnings
            .arg("-t")
            .arg(sample_duration_secs.to_string())
            .arg("-i")
            .arg(source)
            .arg("-vf")
            .arg("idet")
            .arg("-an")
            .arg("-sn")
            .arg("-f")
            .arg("null")
            .arg("-");

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = cmd.output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let metrics = parse_idet_output(&stderr, 29.97);
        let cadence = CadenceAnalyzer::classify(&metrics);
        let recommendation = CadenceAnalyzer::recommend_filter(&cadence, None);

        Ok((cadence, recommendation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_idet_output_soft_telecine() {
        let stderr_sample = r#"
[Parsed_idet_0 @ 000001a1b2c3] Repeated fields: Neither:   600 Top:   380 Bottom:    20
[Parsed_idet_0 @ 000001a1b2c3] Single frame detection: TFF:   750 BFF:     0 Progressive:   250 Undetermined:     0
[Parsed_idet_0 @ 000001a1b2c3] Multi frame detection: TFF:   780 BFF:     0 Progressive:   220 Undetermined:     0
        "#;

        let metrics = parse_idet_output(stderr_sample, 29.97);
        assert_eq!(metrics.total_frames, 1000);
        assert_eq!(metrics.rff_frames, 400); // 380 + 20
        assert_eq!(metrics.combed_frames, 780);
        assert_eq!(metrics.reported_field_order, FieldOrder::TopFieldFirst);

        let cadence = CadenceAnalyzer::classify(&metrics);
        match cadence {
            VideoCadence::SoftTelecine { rff_ratio } => {
                assert!((rff_ratio - 0.40).abs() < 0.01);
            }
            other => panic!("Expected SoftTelecine, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_idet_output_hard_telecine() {
        let stderr_sample = r#"
[Parsed_idet_0 @ 000001a1b2c3] Repeated fields: Neither:  1000 Top:     0 Bottom:     0
[Parsed_idet_0 @ 000001a1b2c3] Single frame detection: TFF:   390 BFF:     0 Progressive:   610 Undetermined:     0
[Parsed_idet_0 @ 000001a1b2c3] Multi frame detection: TFF:   400 BFF:     0 Progressive:   600 Undetermined:     0
        "#;

        let metrics = parse_idet_output(stderr_sample, 29.97);
        assert_eq!(metrics.total_frames, 1000);
        assert_eq!(metrics.rff_frames, 0);
        assert_eq!(metrics.combed_frames, 400); // 40% combed = 3:2 pulldown
        assert_eq!(metrics.reported_field_order, FieldOrder::TopFieldFirst);

        let cadence = CadenceAnalyzer::classify(&metrics);
        match cadence {
            VideoCadence::HardTelecine { field_order, .. } => {
                assert_eq!(field_order, FieldOrder::TopFieldFirst);
            }
            other => panic!("Expected HardTelecine, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_idet_output_pure_progressive() {
        let stderr_sample = r#"
[Parsed_idet_0 @ 000001a1b2c3] Repeated fields: Neither:  1000 Top:     0 Bottom:     0
[Parsed_idet_0 @ 000001a1b2c3] Single frame detection: TFF:    10 BFF:     0 Progressive:   990 Undetermined:     0
[Parsed_idet_0 @ 000001a1b2c3] Multi frame detection: TFF:    12 BFF:     0 Progressive:   988 Undetermined:     0
        "#;

        let metrics = parse_idet_output(stderr_sample, 23.976);
        assert_eq!(metrics.total_frames, 1000);
        assert_eq!(metrics.combed_frames, 12);
        assert!(metrics.sequence_progressive);

        let cadence = CadenceAnalyzer::classify(&metrics);
        assert_eq!(cadence, VideoCadence::Progressive);
    }
}
