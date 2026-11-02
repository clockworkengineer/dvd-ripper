//! Domain heuristic for detecting and filtering multi-episode TV series titles on optical discs.

use crate::domain::entities::disc::{DvdTitleInfo, TvEpisodeInfo};

/// Default minimum runtime (in seconds) for a valid TV episode track (10 minutes).
pub const DEFAULT_MIN_TV_EPISODE_SECS: f64 = 600.0;

/// Default composite "Play All" multiplier threshold relative to median episode runtime.
pub const PLAY_ALL_THRESHOLD_MULTIPLIER: f64 = 1.6;

/// Calculates the median duration from a slice of durations in seconds.
pub fn calculate_median_duration(durations: &[f64]) -> Option<f64> {
    if durations.is_empty() {
        return None;
    }
    let mut sorted = durations.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(sorted[sorted.len() / 2])
}

/// Pure domain heuristic filtering out studio warnings, intros, menus, and composite "Play All" tracks.
///
/// Returns only `DvdTitleInfo` candidates that qualify as individual television episodes.
pub fn filter_tv_episode_titles(
    titles: &[DvdTitleInfo],
    min_duration_secs: f64,
) -> Vec<DvdTitleInfo> {
    let eligible: Vec<&DvdTitleInfo> = titles
        .iter()
        .filter(|t| t.duration_secs >= min_duration_secs)
        .collect();

    if eligible.is_empty() {
        return Vec::new();
    }

    let durations: Vec<f64> = eligible.iter().map(|t| t.duration_secs).collect();
    let median_duration = calculate_median_duration(&durations).unwrap_or(0.0);
    let total_eligible = eligible.len();

    eligible
        .into_iter()
        .filter(|t| {
            !(total_eligible > 1
                && t.duration_secs > median_duration * PLAY_ALL_THRESHOLD_MULTIPLIER)
        })
        .cloned()
        .collect()
}

/// Pure domain heuristic that assigns sequential episode numbers and names to filtered TV titles.
pub fn detect_tv_episodes_from_titles(
    titles: &[DvdTitleInfo],
    show_name: &str,
    season: u32,
    start_ep: u32,
    min_duration_secs: Option<f64>,
) -> Vec<TvEpisodeInfo> {
    let min_secs = min_duration_secs.unwrap_or(DEFAULT_MIN_TV_EPISODE_SECS);
    let filtered_titles = filter_tv_episode_titles(titles, min_secs);

    let mut episodes = Vec::with_capacity(filtered_titles.len());

    for (current_ep, title) in (start_ep..).zip(filtered_titles) {
        let formatted_name = format!("{} - S{:02}E{:02}", show_name, season, current_ep);
        episodes.push(TvEpisodeInfo {
            title_num: title.title_num,
            episode_num: current_ep,
            duration_secs: title.duration_secs,
            formatted_name,
        });
    }

    episodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_tv_episodes_removes_short_titles_and_menus() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 120.0,
            }, // Studio logo: 2m
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 300.0,
            }, // Menu loop: 5m
            DvdTitleInfo {
                title_num: 3,
                duration_secs: 1400.0,
            }, // Ep 1: 23m20s
            DvdTitleInfo {
                title_num: 4,
                duration_secs: 1380.0,
            }, // Ep 2: 23m00s
        ];

        let filtered = filter_tv_episode_titles(&titles, DEFAULT_MIN_TV_EPISODE_SECS);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].title_num, 3);
        assert_eq!(filtered[1].title_num, 4);
    }

    #[test]
    fn test_filter_tv_episodes_rejects_play_all_composite_track() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 5600.0,
            }, // Play All track (4 eps * 23m = 93m)
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 1400.0,
            }, // Ep 1
            DvdTitleInfo {
                title_num: 3,
                duration_secs: 1420.0,
            }, // Ep 2
            DvdTitleInfo {
                title_num: 4,
                duration_secs: 1390.0,
            }, // Ep 3
            DvdTitleInfo {
                title_num: 5,
                duration_secs: 1410.0,
            }, // Ep 4
        ];

        let filtered = filter_tv_episode_titles(&titles, DEFAULT_MIN_TV_EPISODE_SECS);
        assert_eq!(filtered.len(), 4);
        assert!(filtered.iter().all(|t| t.title_num != 1));
        assert_eq!(filtered[0].title_num, 2);
        assert_eq!(filtered[3].title_num, 5);
    }

    #[test]
    fn test_detect_tv_episodes_from_titles_assigns_sequential_names() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 1350.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 1380.0,
            },
            DvdTitleInfo {
                title_num: 3,
                duration_secs: 1410.0,
            },
        ];

        let episodes = detect_tv_episodes_from_titles(&titles, "The Office", 2, 5, None);
        assert_eq!(episodes.len(), 3);

        assert_eq!(episodes[0].episode_num, 5);
        assert_eq!(episodes[0].formatted_name, "The Office - S02E05");

        assert_eq!(episodes[1].episode_num, 6);
        assert_eq!(episodes[1].formatted_name, "The Office - S02E06");

        assert_eq!(episodes[2].episode_num, 7);
        assert_eq!(episodes[2].formatted_name, "The Office - S02E07");
    }

    #[test]
    fn test_single_episode_disc_not_discarded() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 3600.0,
            }, // 1-hour special
        ];

        let filtered = filter_tv_episode_titles(&titles, DEFAULT_MIN_TV_EPISODE_SECS);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title_num, 1);
    }

    #[test]
    fn test_calculate_median_duration() {
        assert_eq!(calculate_median_duration(&[]), None);
        assert_eq!(calculate_median_duration(&[100.0]), Some(100.0));
        assert_eq!(calculate_median_duration(&[30.0, 10.0, 20.0]), Some(20.0));
        assert_eq!(
            calculate_median_duration(&[10.0, 20.0, 30.0, 40.0]),
            Some(30.0)
        );
    }
}
