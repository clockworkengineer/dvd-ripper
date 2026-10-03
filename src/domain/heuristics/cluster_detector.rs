use crate::domain::entities::disc::DvdTitleInfo;
use super::speedup::calculate_runtime_diff;

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

    let feature_titles: Vec<&DvdTitleInfo> = if titles.iter().any(|t| t.duration_secs >= MIN_FEATURE_DURATION_SECS) {
        titles.iter().filter(|t| t.duration_secs >= MIN_FEATURE_DURATION_SECS).collect()
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
            if diff < best_diff - 15.0 || ((diff - best_diff).abs() <= 15.0 && t.title_num < best_title) {
                best_diff = diff;
                best_title = t.title_num;
                best_duration = Some(t.duration_secs);
            }
        }
        (best_title, best_duration)
    } else {
        let max_duration = feature_titles.iter().map(|t| t.duration_secs).fold(0.0f64, f64::max);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_ranking_bypasses_decoy_trap() {
        let titles = vec![
            DvdTitleInfo { title_num: 77, duration_secs: 7664.0 },
            DvdTitleInfo { title_num: 2, duration_secs: 7524.0 },
            DvdTitleInfo { title_num: 22, duration_secs: 7524.0 },
        ];
        let (best_title, duration) = rank_best_title_from_slice(&titles, None);
        assert_eq!(best_title, 2);
        assert_eq!(duration, Some(7524.0));
    }

    #[test]
    fn test_pal_speedup_metadata_matching() {
        let titles = vec![
            DvdTitleInfo { title_num: 77, duration_secs: 7664.0 },
            DvdTitleInfo { title_num: 2, duration_secs: 7524.0 },
            DvdTitleInfo { title_num: 22, duration_secs: 7524.0 },
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
}
