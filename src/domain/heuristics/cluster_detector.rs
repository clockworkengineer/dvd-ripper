use super::speedup::calculate_runtime_diff;
use crate::domain::entities::disc::DvdTitleInfo;

/// Cluster tolerance threshold (3%) to group titles sharing the authentic feature length.
pub const FEATURE_CLUSTER_TOLERANCE: f64 = 0.97;

/// Minimum duration (in seconds) to be considered a feature film candidate.
pub const MIN_FEATURE_DURATION_SECS: f64 = 300.0;

/// Ranks the best title track from a slice of probed DVD titles.
///
/// Heuristic Rules:
/// 1. Only considers titles >= 5 minutes (300 seconds) if any exist.
/// 2. If target metadata duration is given, checks both 24fps NTSC and 25fps PAL runtimes.
///    Prefers lower title number when difference is within 15 seconds.
/// 3. If no target metadata is given, groups the longest titles within 3% into a cluster
///    and selects the lowest title number (bypassing high-numbered decoy loops on ScreenPass discs).
pub fn rank_best_title_from_slice(
    titles: &[DvdTitleInfo],
    expected_runtime_secs: Option<f64>,
) -> (u32, Option<f64>) {
    if titles.is_empty() {
        return (1, None);
    }

    let feature_titles: Vec<&DvdTitleInfo> = if titles
        .iter()
        .any(|t| t.duration_secs >= MIN_FEATURE_DURATION_SECS)
    {
        titles
            .iter()
            .filter(|t| t.duration_secs >= MIN_FEATURE_DURATION_SECS)
            .collect()
    } else {
        titles.iter().collect()
    };

    if let Some(target) = expected_runtime_secs {
        let mut best_title = feature_titles[0].title_num;
        let mut best_duration = Some(feature_titles[0].duration_secs);
        let mut best_diff = f64::MAX;

        for t in &feature_titles {
            let diff = calculate_runtime_diff(t.duration_secs, target);

            // On copy-protected discs, the genuine primary authoring title is the lowest numbered title:
            if diff < best_diff - 15.0
                || ((diff - best_diff).abs() <= 15.0 && t.title_num < best_title)
            {
                best_diff = diff;
                best_title = t.title_num;
                best_duration = Some(t.duration_secs);
            }
        }
        (best_title, best_duration)
    } else {
        let max_duration = feature_titles
            .iter()
            .map(|t| t.duration_secs)
            .fold(0.0f64, f64::max);
        let threshold = max_duration * FEATURE_CLUSTER_TOLERANCE;
        let mut cluster: Vec<&DvdTitleInfo> = feature_titles
            .into_iter()
            .filter(|t| t.duration_secs >= threshold)
            .collect();
        cluster.sort_by_key(|t| t.title_num);

        let best = cluster[0];
        (best.title_num, Some(best.duration_secs))
    }
}

/// Binary IFO cell sector topology for a single DVD title, used to defeat 99-title structural obfuscation
/// (Lionsgate ScreenPass, Sony ARccOS, Disney X-Project).
#[derive(Debug, Clone, PartialEq)]
pub struct TitleCellTopology {
    pub title_num: u32,
    pub duration_secs: f64,
    /// Ordered list of `(first_sector, last_sector)` extents from the PGC Cell Playback Table (`C_PBIT`).
    pub cell_sectors: Vec<(u32, u32)>,
    pub angle_count: u8,
}

/// Scores the structural authenticity of a DVD title's VOBU sector layout (`0.0..=100.0`).
///
/// Genuine theatrical titles exhibit:
/// - Duration >= 5 minutes (or matching `expected_runtime_secs`).
/// - Strictly monotonic forward sector progression (`next.first_sector >= prev.last_sector`).
/// - Minimal tiny/zero-length dummy cells (`<= 16` sectors) which ARccOS/Lionsgate insert as bad-sector traps.
/// - Non-repeating cell sector ranges (decoys often loop or shuffle the same cell sectors out of order).
pub fn score_title_structural_authenticity(
    topology: &TitleCellTopology,
    expected_runtime_secs: Option<f64>,
) -> f64 {
    if topology.duration_secs < MIN_FEATURE_DURATION_SECS || topology.cell_sectors.is_empty() {
        return 0.0;
    }

    let mut score = 100.0_f64;
    let mut backward_jumps = 0usize;
    let mut dummy_cells = 0usize;
    let mut duplicate_cells = 0usize;
    let mut seen_ranges: Vec<(u32, u32)> = Vec::with_capacity(topology.cell_sectors.len());

    let mut prev_last_sector: Option<u32> = None;
    for &(first_sec, last_sec) in &topology.cell_sectors {
        let span = last_sec.saturating_sub(first_sec);
        if span <= 16 {
            dummy_cells += 1;
        }
        if seen_ranges.contains(&(first_sec, last_sec)) {
            duplicate_cells += 1;
        } else {
            seen_ranges.push((first_sec, last_sec));
        }

        if let Some(prev_end) = prev_last_sector
            && first_sec < prev_end
        {
            backward_jumps += 1;
        }
        prev_last_sector = Some(last_sec);
    }

    // Penalize scrambled out-of-order cell jumps heavily (Lionsgate playlist obfuscation)
    score -= (backward_jumps as f64) * 18.0;
    // Penalize ARccOS dummy bad-sector cells at the start/middle of a PGC
    score -= (dummy_cells as f64) * 8.0;
    // Penalize repeated loop cells
    score -= (duplicate_cells as f64) * 15.0;

    if let Some(target_secs) = expected_runtime_secs
        && target_secs >= MIN_FEATURE_DURATION_SECS
    {
        let diff = calculate_runtime_diff(topology.duration_secs, target_secs);
        // Up to 25 points penalty if runtime deviates significantly from expected theatrical/PAL runtime
        let runtime_penalty = (diff / 60.0).min(25.0);
        score -= runtime_penalty;
    }

    score.clamp(0.0, 100.0)
}

/// Selects the single authentic theatrical main title from a list of probed `TitleCellTopology` candidates,
/// defeating 99-title structural fake-playlist obfuscation.
pub fn select_authentic_main_title(
    topologies: &[TitleCellTopology],
    expected_runtime_secs: Option<f64>,
) -> Option<u32> {
    if topologies.is_empty() {
        return None;
    }

    let max_dur = topologies
        .iter()
        .map(|t| t.duration_secs)
        .fold(0.0f64, f64::max);

    let mut best_title: Option<u32> = None;
    let mut best_score = -1.0_f64;

    for topo in topologies {
        if max_dur >= MIN_FEATURE_DURATION_SECS && topo.duration_secs < max_dur * 0.60 {
            continue;
        }
        let score = score_title_structural_authenticity(topo, expected_runtime_secs);
        if score > best_score + 0.5
            || ((score - best_score).abs() <= 0.5 && best_title.is_none_or(|b| topo.title_num < b))
        {
            best_score = score;
            best_title = Some(topo.title_num);
        }
    }

    best_title
}

/// Candidate subtitle stream metrics for "Foreign Audio Search" (detecting sparse alien/foreign dialogue tracks).
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleCandidateMetrics {
    pub stream_index: u32,
    pub language: String,
    pub forced_flag: bool,
    pub cue_count: usize,
    pub total_active_secs: f64,
    pub title_duration_secs: f64,
}

/// Automatically detects the forced or sparse foreign-dialogue subtitle stream (HandBrake "Foreign Audio Search" parity).
///
/// Priority:
/// 1. Explicitly flagged `forced_flag == true` matching `preferred_lang`.
/// 2. Sparse subtitle stream in `preferred_lang` whose active cue density is between `0.2%` and `15%`
///    of the main dialogue track (or `3..=90` cues in a full-length film).
pub fn detect_foreign_audio_forced_subtitle(
    candidates: &[SubtitleCandidateMetrics],
    preferred_lang: &str,
) -> Option<u32> {
    let pref = preferred_lang.trim().to_lowercase();
    let lang_matches = |lang: &str| -> bool {
        let l = lang.trim().to_lowercase();
        pref.is_empty() || l == pref || l.starts_with(&pref) || pref.starts_with(&l)
    };

    // 1. Explicit forced flag in preferred language
    if let Some(explicit) = candidates
        .iter()
        .find(|c| c.forced_flag && lang_matches(&c.language))
    {
        return Some(explicit.stream_index);
    }

    // 2. Sparse cue density comparison against full dialogue tracks in the same language
    let max_cues_in_lang = candidates
        .iter()
        .filter(|c| lang_matches(&c.language))
        .map(|c| c.cue_count)
        .max()
        .unwrap_or(0);

    for cand in candidates.iter().filter(|c| lang_matches(&c.language)) {
        if cand.cue_count == 0 {
            continue;
        }
        let ratio_vs_full = if max_cues_in_lang >= 100 {
            (cand.cue_count as f64) / (max_cues_in_lang as f64)
        } else if cand.title_duration_secs > 600.0 {
            cand.total_active_secs / cand.title_duration_secs
        } else {
            1.0
        };

        if (0.002..=0.15).contains(&ratio_vs_full) && cand.cue_count <= 120 {
            return Some(cand.stream_index);
        }
    }

    None
}

/// Plex / Jellyfin compliant local media extras category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExtraCategory {
    Trailer,
    DeletedScene,
    BehindTheScenes,
    Featurette,
    Short,
}

impl ExtraCategory {
    /// Returns the official Plex / Jellyfin subdirectory name for local bonus extras.
    pub fn plex_subfolder_name(&self) -> &'static str {
        match self {
            Self::Trailer => "Trailers",
            Self::DeletedScene => "Deleted Scenes",
            Self::BehindTheScenes => "Behind The Scenes",
            Self::Featurette => "Featurettes",
            Self::Short => "Shorts",
        }
    }
}

/// Classified bonus/extra title track discovered on a movie DVD/Blu-ray.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedExtraTitle {
    pub title_num: u32,
    pub duration_secs: f64,
    pub category: ExtraCategory,
    pub suggested_filename: String,
}

/// Classifies non-main disc titles (`45s..=3000s`) into Plex/Jellyfin-compliant bonus extras folders.
pub fn classify_bonus_extra_titles(
    titles: &[DvdTitleInfo],
    main_title_num: u32,
    film_name: &str,
) -> Vec<ClassifiedExtraTitle> {
    let clean_film = if film_name.trim().is_empty() {
        "Movie"
    } else {
        film_name.trim()
    };

    let mut extras = Vec::new();
    let mut trailer_idx = 1u32;
    let mut deleted_idx = 1u32;
    let mut bts_idx = 1u32;
    let mut feat_idx = 1u32;

    for t in titles {
        if t.title_num == main_title_num || !(45.0..=3000.0).contains(&t.duration_secs) {
            continue;
        }

        let (category, label) = if t.duration_secs <= 210.0 {
            let name = format!("{} - Trailer {:02}", clean_film, trailer_idx);
            trailer_idx += 1;
            (ExtraCategory::Trailer, name)
        } else if t.duration_secs <= 480.0 {
            let name = format!("{} - Deleted Scene {:02}", clean_film, deleted_idx);
            deleted_idx += 1;
            (ExtraCategory::DeletedScene, name)
        } else if t.duration_secs <= 960.0 {
            let name = format!("{} - Behind The Scenes {:02}", clean_film, bts_idx);
            bts_idx += 1;
            (ExtraCategory::BehindTheScenes, name)
        } else {
            let name = format!("{} - Featurette {:02}", clean_film, feat_idx);
            feat_idx += 1;
            (ExtraCategory::Featurette, name)
        };

        extras.push(ClassifiedExtraTitle {
            title_num: t.title_num,
            duration_secs: t.duration_secs,
            category,
            suggested_filename: label,
        });
    }

    extras
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_ranking_bypasses_decoy_trap() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 77,
                duration_secs: 7664.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 7524.0,
            },
            DvdTitleInfo {
                title_num: 22,
                duration_secs: 7524.0,
            },
        ];
        let (best_title, duration) = rank_best_title_from_slice(&titles, None);
        assert_eq!(best_title, 2);
        assert_eq!(duration, Some(7524.0));
    }

    #[test]
    fn test_pal_speedup_metadata_matching() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 77,
                duration_secs: 7664.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 7524.0,
            },
            DvdTitleInfo {
                title_num: 22,
                duration_secs: 7524.0,
            },
        ];
        let (best_title, duration) = rank_best_title_from_slice(&titles, Some(7860.0));
        assert_eq!(best_title, 2);
        assert_eq!(duration, Some(7524.0));
    }

    #[test]
    fn test_empty_titles_fallback() {
        let (best_title, duration) = rank_best_title_from_slice(&[], None);
        assert_eq!(best_title, 1);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_structural_fake_title_deobfuscation_lionsgate_arccos() {
        // Title #1: ARccOS decoy with tiny dummy bad-sector cells and scrambled backward jump
        let decoy_arccos = TitleCellTopology {
            title_num: 1,
            duration_secs: 6120.0,
            cell_sectors: vec![(0, 5), (5, 10), (50_000, 120_000), (10_000, 49_999)],
            angle_count: 1,
        };
        // Title #14: Genuine theatrical title with clean monotonic VOBU sector layout
        let genuine_feature = TitleCellTopology {
            title_num: 14,
            duration_secs: 6115.0,
            cell_sectors: vec![
                (10_000, 45_000),
                (45_001, 90_000),
                (90_001, 140_000),
                (140_001, 195_000),
            ],
            angle_count: 1,
        };
        // Title #42: Decoy with repeated loop cells
        let decoy_loop = TitleCellTopology {
            title_num: 42,
            duration_secs: 6125.0,
            cell_sectors: vec![
                (10_000, 45_000),
                (10_000, 45_000),
                (90_001, 140_000),
                (45_001, 90_000),
            ],
            angle_count: 1,
        };

        let s_decoy1 = score_title_structural_authenticity(&decoy_arccos, Some(6115.0));
        let s_genuine = score_title_structural_authenticity(&genuine_feature, Some(6115.0));
        let s_decoy2 = score_title_structural_authenticity(&decoy_loop, Some(6115.0));

        assert!(s_genuine > s_decoy1 + 25.0);
        assert!(s_genuine > s_decoy2 + 25.0);

        let selected =
            select_authentic_main_title(&[decoy_arccos, genuine_feature, decoy_loop], Some(6115.0));
        assert_eq!(selected, Some(14));
    }

    #[test]
    fn test_foreign_audio_forced_subtitle_detection() {
        let candidates = vec![
            SubtitleCandidateMetrics {
                stream_index: 0,
                language: "eng".to_string(),
                forced_flag: false,
                cue_count: 1350,
                total_active_secs: 3800.0,
                title_duration_secs: 7200.0,
            },
            SubtitleCandidateMetrics {
                stream_index: 1,
                language: "spa".to_string(),
                forced_flag: false,
                cue_count: 1310,
                total_active_secs: 3750.0,
                title_duration_secs: 7200.0,
            },
            // Track #2: Sparse alien/foreign dialogue track (42 cues = ~3.1% of main dialogue)
            SubtitleCandidateMetrics {
                stream_index: 2,
                language: "eng".to_string(),
                forced_flag: false,
                cue_count: 42,
                total_active_secs: 115.0,
                title_duration_secs: 7200.0,
            },
        ];

        assert_eq!(
            detect_foreign_audio_forced_subtitle(&candidates, "eng"),
            Some(2)
        );
    }

    #[test]
    fn test_classify_bonus_extra_titles_plex_categories() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 7200.0,
            }, // Main movie
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 135.0,
            }, // Trailer (2m 15s)
            DvdTitleInfo {
                title_num: 3,
                duration_secs: 320.0,
            }, // Deleted Scene (5m 20s)
            DvdTitleInfo {
                title_num: 4,
                duration_secs: 720.0,
            }, // Behind The Scenes (12m)
            DvdTitleInfo {
                title_num: 5,
                duration_secs: 1800.0,
            }, // Featurette (30m)
            DvdTitleInfo {
                title_num: 6,
                duration_secs: 12.0,
            }, // Tiny menu loop (ignored)
        ];

        let extras = classify_bonus_extra_titles(&titles, 1, "Gladiator");
        assert_eq!(extras.len(), 4);
        assert_eq!(extras[0].category, ExtraCategory::Trailer);
        assert_eq!(extras[0].category.plex_subfolder_name(), "Trailers");
        assert_eq!(extras[1].category, ExtraCategory::DeletedScene);
        assert_eq!(extras[1].category.plex_subfolder_name(), "Deleted Scenes");
        assert_eq!(extras[2].category, ExtraCategory::BehindTheScenes);
        assert_eq!(extras[3].category, ExtraCategory::Featurette);
    }
}
