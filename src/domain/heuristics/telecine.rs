//! Cadence-Aware 3:2 Pulldown (IVTC) and Interlacing Detection Domain Heuristics.
//!
//! Provides algorithmic classification of DVD video streams into progressive, soft telecine,
//! hard 3:2 telecine pulldown, or true interlacing, recommending the optimal lossless
//! frame reconstruction filter graph.

use serde::{Deserialize, Serialize};

/// Dominant temporal field ordering for interlaced or telecined video frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FieldOrder {
    /// Top Field First (standard for most DVD MPEG-2 video streams).
    #[default]
    TopFieldFirst,
    /// Bottom Field First (common in DV and certain NTSC DVD encodings).
    BottomFieldFirst,
    /// Progressive frame structure without field interlacing.
    Progressive,
    /// Undetermined or variable field parity.
    Unknown,
}

impl FieldOrder {
    /// Returns the FFmpeg `fieldmatch` filter parity order token (`tff` or `bff`).
    #[must_use]
    pub fn as_fieldmatch_order(&self) -> &'static str {
        match self {
            Self::TopFieldFirst | Self::Unknown | Self::Progressive => "tff",
            Self::BottomFieldFirst => "bff",
        }
    }
}

/// Structural cadence classification of a DVD video stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VideoCadence {
    /// True progressive frame sequence (e.g. 24p, 25p PAL, 23.976p without flags).
    Progressive,
    /// Soft 3:2 pulldown using MPEG-2 Repeat First Field (`RFF`) flags.
    /// Original 23.976 fps frames are intact and can be extracted without deinterlace degradation.
    SoftTelecine {
        /// Ratio of frames flagged with `repeat_pict` / `RFF` (0.0 - 1.0).
        rff_ratio: f64,
    },
    /// Hard 3:2 pulldown where 24 fps film frames were permanently telecined into 59.94i video fields.
    /// Requires inverse telecine field matching and 5-frame decimation.
    HardTelecine {
        /// Detected field parity.
        field_order: FieldOrder,
        /// Confidence score of periodic 3:2 telecine comb patterns (0.0 - 1.0).
        confidence: f64,
    },
    /// True interlaced video (e.g. 50i PAL, 59.94i NTSC concert/sports/video camera footage).
    /// Requires motion-adaptive deinterlacing (such as BWDIF or YADIF).
    Interlaced {
        /// Dominant field order.
        field_order: FieldOrder,
    },
    /// Hybrid or mixed cadence (e.g. progressive film with 60i video credits or mixed anime).
    MixedCadence {
        /// Ratio of interlaced/telecined frames vs total frames.
        interlaced_ratio: f64,
    },
}

/// Quantitative probe metrics collected from MPEG-2 packet headers or probe samples.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TelecineMetrics {
    /// Total frames analyzed in sample window.
    pub total_frames: u64,
    /// Count of frames with Repeat First Field (`RFF`) flag set.
    pub rff_frames: u64,
    /// Count of frames detected with interlacing combing artifacts.
    pub combed_frames: u64,
    /// Dominant field order reported by container or packet headers.
    pub reported_field_order: FieldOrder,
    /// Container frame rate (e.g. 29.97, 25.0, 23.976).
    pub frame_rate: f64,
    /// Progressive frame flag from MPEG-2 sequence header.
    pub sequence_progressive: bool,
}

impl TelecineMetrics {
    /// Calculates the ratio of frames utilizing MPEG-2 Repeat First Field flags.
    #[must_use]
    pub fn rff_ratio(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.rff_frames as f64 / self.total_frames as f64
        }
    }

    /// Calculates the ratio of analyzed frames exhibiting interlacing comb artifacts.
    #[must_use]
    pub fn combed_ratio(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.combed_frames as f64 / self.total_frames as f64
        }
    }
}

/// Recommendation for optimal FFmpeg video filter chain and target framerate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelecineFilterRecommendation {
    /// FFmpeg video filter expression (`-vf <filter_chain>`), or None if untouched progressive.
    pub filter_chain: Option<String>,
    /// Recommended target frame rate override (e.g. `"24000/1001"`, `"25"`), if applicable.
    pub target_fps: Option<String>,
    /// Human-readable explanation of why this filter chain was selected.
    pub explanation: String,
}

/// Cadence analyzer implementing heuristic DVD cadence classification.
pub struct CadenceAnalyzer;

impl CadenceAnalyzer {
    /// Analyzes stream metrics and classifies the video cadence.
    ///
    /// # Heuristics
    /// - High RFF ratio (>= 0.35) -> Soft Telecine (original 23.976p film).
    /// - Sequence marked progressive with low combed ratio (< 0.05) -> Progressive.
    /// - Low combed ratio (< 0.05) -> Progressive.
    /// - High combed ratio (>= 0.70) with 29.97/25 fps -> True Interlaced.
    /// - Combed ratio between 0.20 and 0.70 with ~29.97 fps -> Hard 3:2 Telecine.
    /// - Otherwise -> Mixed Cadence.
    #[must_use]
    pub fn classify(metrics: &TelecineMetrics) -> VideoCadence {
        let rff_ratio = metrics.rff_ratio();
        let combed_ratio = metrics.combed_ratio();

        // 1. Soft telecine check (MPEG-2 3:2 pulldown flags)
        if rff_ratio >= 0.30 {
            return VideoCadence::SoftTelecine { rff_ratio };
        }

        // 2. Pure progressive check
        if (metrics.sequence_progressive && combed_ratio < 0.08) || combed_ratio < 0.04 {
            return VideoCadence::Progressive;
        }

        // 3. Hard 3:2 telecine check (typically ~40% combed frames in 3:2 cadence: 2 out of 5 frames combed)
        let is_ntsc_rate = (metrics.frame_rate - 29.97).abs() < 0.1 || (metrics.frame_rate - 30.0).abs() < 0.1;
        if is_ntsc_rate && (0.25..0.65).contains(&combed_ratio) {
            let confidence = 1.0 - ((combed_ratio - 0.40).abs() / 0.40).clamp(0.0, 1.0);
            return VideoCadence::HardTelecine {
                field_order: metrics.reported_field_order,
                confidence,
            };
        }

        // 4. True interlaced check (majority of frames combed, e.g. 50i/60i video source)
        if combed_ratio >= 0.65 {
            return VideoCadence::Interlaced {
                field_order: metrics.reported_field_order,
            };
        }

        // 5. Mixed / hybrid cadence
        VideoCadence::MixedCadence {
            interlaced_ratio: combed_ratio,
        }
    }

    /// Recommends the optimal FFmpeg filter chain for a classified video cadence.
    #[must_use]
    pub fn recommend_filter(
        cadence: &VideoCadence,
        preferred_deinterlace_algo: Option<&str>,
    ) -> TelecineFilterRecommendation {
        let algo = preferred_deinterlace_algo.unwrap_or("bwdif");

        match cadence {
            VideoCadence::Progressive => TelecineFilterRecommendation {
                filter_chain: None,
                target_fps: None,
                explanation: "Native progressive frames detected; bypassing deinterlace filters to preserve full spatial resolution".to_string(),
            },
            VideoCadence::SoftTelecine { rff_ratio } => TelecineFilterRecommendation {
                filter_chain: Some("fps=24000/1001".to_string()),
                target_fps: Some("24000/1001".to_string()),
                explanation: format!(
                    "Soft 3:2 pulldown detected (RFF ratio {:.1}%); discarding repeated fields to reconstruct native 23.976p film",
                    rff_ratio * 100.0
                ),
            },
            VideoCadence::HardTelecine { field_order, confidence } => {
                let order = field_order.as_fieldmatch_order();
                let filter = format!("fieldmatch=order={order}:mode=pc:combmatch=full,decimate=cycle=5");
                TelecineFilterRecommendation {
                    filter_chain: Some(filter),
                    target_fps: Some("24000/1001".to_string()),
                    explanation: format!(
                        "Hard 3:2 pulldown detected (confidence {:.1}%); applying inverse telecine (fieldmatch + decimate) for lossless 23.976p recovery",
                        confidence * 100.0
                    ),
                }
            }
            VideoCadence::Interlaced { field_order } => {
                let parity_flag = match field_order {
                    FieldOrder::TopFieldFirst => "parity=tff",
                    FieldOrder::BottomFieldFirst => "parity=bff",
                    _ => "parity=auto",
                };

                let filter = match algo.to_lowercase().as_str() {
                    "yadif" => format!("yadif=mode=send_frame:{parity_flag}:deint=all"),
                    "w3fdif" => "w3fdif=filter=complex".to_string(),
                    _ => format!("bwdif=mode=send_frame:{parity_flag}:deint=all"),
                };

                TelecineFilterRecommendation {
                    filter_chain: Some(filter),
                    target_fps: None,
                    explanation: format!(
                        "True interlaced video detected; applying motion-adaptive {algo} deinterlacing ({parity_flag})"
                    ),
                }
            }
            VideoCadence::MixedCadence { interlaced_ratio } => {
                // Adaptive deinterlace via fieldmatch with post-processing comb filter
                TelecineFilterRecommendation {
                    filter_chain: Some("fieldmatch=mode=pc:combmatch=full:combpel=80,decimate=cycle=5".to_string()),
                    target_fps: Some("24000/1001".to_string()),
                    explanation: format!(
                        "Mixed hybrid cadence detected ({:.1}% combed); applying adaptive fieldmatch with combed-pel recovery",
                        interlaced_ratio * 100.0
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_progressive_stream() {
        let metrics = TelecineMetrics {
            total_frames: 1000,
            rff_frames: 0,
            combed_frames: 10, // 1% combed (noise/slight motion)
            reported_field_order: FieldOrder::Progressive,
            frame_rate: 23.976,
            sequence_progressive: true,
        };

        let cadence = CadenceAnalyzer::classify(&metrics);
        assert_eq!(cadence, VideoCadence::Progressive);

        let rec = CadenceAnalyzer::recommend_filter(&cadence, None);
        assert!(rec.filter_chain.is_none());
        assert!(rec.explanation.contains("bypassing"));
    }

    #[test]
    fn test_classify_soft_telecine() {
        let metrics = TelecineMetrics {
            total_frames: 1000,
            rff_frames: 400, // 40% RFF flags (typical for 3:2 pulldown)
            combed_frames: 50,
            reported_field_order: FieldOrder::TopFieldFirst,
            frame_rate: 29.97,
            sequence_progressive: false,
        };

        let cadence = CadenceAnalyzer::classify(&metrics);
        match cadence {
            VideoCadence::SoftTelecine { rff_ratio } => {
                assert!((rff_ratio - 0.40).abs() < 0.001);
            }
            other => panic!("Expected SoftTelecine, got {other:?}"),
        }

        let rec = CadenceAnalyzer::recommend_filter(&cadence, None);
        assert_eq!(rec.filter_chain.as_deref(), Some("fps=24000/1001"));
        assert_eq!(rec.target_fps.as_deref(), Some("24000/1001"));
    }

    #[test]
    fn test_classify_hard_telecine() {
        let metrics = TelecineMetrics {
            total_frames: 1000,
            rff_frames: 0,
            combed_frames: 395, // ~40% combed frames (2 out of 5 frames in 3:2 cadence)
            reported_field_order: FieldOrder::TopFieldFirst,
            frame_rate: 29.97,
            sequence_progressive: false,
        };

        let cadence = CadenceAnalyzer::classify(&metrics);
        match cadence {
            VideoCadence::HardTelecine { field_order, confidence } => {
                assert_eq!(field_order, FieldOrder::TopFieldFirst);
                assert!(confidence > 0.90);
            }
            other => panic!("Expected HardTelecine, got {other:?}"),
        }

        let rec = CadenceAnalyzer::recommend_filter(&cadence, None);
        assert_eq!(
            rec.filter_chain.as_deref(),
            Some("fieldmatch=order=tff:mode=pc:combmatch=full,decimate=cycle=5")
        );
        assert_eq!(rec.target_fps.as_deref(), Some("24000/1001"));
    }

    #[test]
    fn test_classify_true_interlaced_with_yadif() {
        let metrics = TelecineMetrics {
            total_frames: 1000,
            rff_frames: 0,
            combed_frames: 850, // 85% combed
            reported_field_order: FieldOrder::BottomFieldFirst,
            frame_rate: 29.97,
            sequence_progressive: false,
        };

        let cadence = CadenceAnalyzer::classify(&metrics);
        assert_eq!(
            cadence,
            VideoCadence::Interlaced {
                field_order: FieldOrder::BottomFieldFirst
            }
        );

        let rec = CadenceAnalyzer::recommend_filter(&cadence, Some("yadif"));
        assert_eq!(
            rec.filter_chain.as_deref(),
            Some("yadif=mode=send_frame:parity=bff:deint=all")
        );
        assert!(rec.target_fps.is_none());
    }

    #[test]
    fn test_field_order_token() {
        assert_eq!(FieldOrder::TopFieldFirst.as_fieldmatch_order(), "tff");
        assert_eq!(FieldOrder::BottomFieldFirst.as_fieldmatch_order(), "bff");
        assert_eq!(FieldOrder::Unknown.as_fieldmatch_order(), "tff");
    }
}

