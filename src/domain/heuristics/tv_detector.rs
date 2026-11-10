//! Domain heuristic for detecting and filtering multi-episode TV series titles on optical discs.

use crate::domain::entities::disc::{Chapter, DvdTitleInfo, TvEpisodeInfo};

/// Default minimum runtime (in seconds) for a valid TV episode track (10 minutes).
pub const DEFAULT_MIN_TV_EPISODE_SECS: f64 = 600.0;

/// Default target runtime (in seconds) when clustering chapters into episodes (~23 minutes).
pub const DEFAULT_CHAPTER_EPISODE_TARGET_SECS: f64 = 1380.0;

/// Default composite "Play All" multiplier threshold relative to median episode runtime.
pub const PLAY_ALL_THRESHOLD_MULTIPLIER: f64 = 1.6;

/// Represents a virtual TV episode segmented from a chapter range within a monolithic DVD title.
#[derive(Debug, Clone, PartialEq)]
pub struct ChapterEpisodeSpan {
    pub episode: TvEpisodeInfo,
    pub start_chapter: u32,
    pub end_chapter: u32,
}

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

/// Pure domain heuristic that segments a single monolithic "Play All" title's chapters into individual TV episodes.
///
/// Handles both:
/// 1. 1-chapter-per-episode authoring (where each chapter is already >= 10 minutes).
/// 2. Multi-chapter-per-episode authoring (e.g. 5-6 chapters per 23-minute episode inside a 2-hour title).
pub fn detect_tv_episodes_from_chapters(
    title_num: u32,
    chapters: &[Chapter],
    show_name: &str,
    season: u32,
    start_ep: u32,
    target_ep_duration_secs: Option<f64>,
) -> Vec<ChapterEpisodeSpan> {
    if chapters.is_empty() {
        return Vec::new();
    }

    let target_secs = target_ep_duration_secs.unwrap_or(DEFAULT_CHAPTER_EPISODE_TARGET_SECS);
    let min_ep_secs = (target_secs * 0.65).max(DEFAULT_MIN_TV_EPISODE_SECS);

    let mut groups: Vec<(u32, u32, f64)> = Vec::new();
    let mut cur_start = chapters[0].index.max(1);
    let mut cur_end = cur_start;
    let mut cur_dur = 0.0;

    for ch in chapters {
        let ch_idx = ch.index.max(1);
        let ch_dur = ch.duration_secs();
        if ch_dur <= 0.0 {
            continue;
        }

        if cur_dur == 0.0 {
            cur_start = ch_idx;
            cur_end = ch_idx;
            cur_dur = ch_dur;
            continue;
        }

        let diff_without = (cur_dur - target_secs).abs();
        let diff_with = ((cur_dur + ch_dur) - target_secs).abs();

        if cur_dur >= min_ep_secs && diff_without <= diff_with {
            groups.push((cur_start, cur_end, cur_dur));
            cur_start = ch_idx;
            cur_end = ch_idx;
            cur_dur = ch_dur;
        } else {
            cur_end = ch_idx;
            cur_dur += ch_dur;
        }
    }

    if cur_dur > 0.0 {
        if cur_dur < min_ep_secs * 0.5 && !groups.is_empty() {
            // Merge trailing bumper/credits chapter into the last episode
            if let Some(last) = groups.last_mut() {
                last.1 = cur_end;
                last.2 += cur_dur;
            }
        } else if cur_dur >= DEFAULT_MIN_TV_EPISODE_SECS * 0.5 {
            groups.push((cur_start, cur_end, cur_dur));
        }
    }

    groups
        .into_iter()
        .enumerate()
        .map(|(idx, (s_ch, e_ch, dur))| {
            let ep_num = start_ep + idx as u32;
            let formatted_name = format!("{} - S{:02}E{:02}", show_name, season, ep_num);
            ChapterEpisodeSpan {
                episode: TvEpisodeInfo {
                    title_num,
                    episode_num: ep_num,
                    duration_secs: dur,
                    formatted_name,
                },
                start_chapter: s_ch,
                end_chapter: e_ch,
            }
        })
        .collect()
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

    #[test]
    fn test_detect_tv_episodes_from_chapters_groups_subchapters() {
        // Simulate a 3-episode Play-All title where each 24-min (1440s) episode has 4 chapters of 360s each,
        // plus a trailing 45s studio logo chapter at the end (13 chapters total).
        let mut chapters = Vec::new();
        let mut t = 0.0;
        for i in 1..=12 {
            chapters.push(Chapter {
                index: i,
                start_secs: t,
                end_secs: t + 360.0,
                title: format!("Chapter {:02}", i),
            });
            t += 360.0;
        }
        chapters.push(Chapter {
            index: 13,
            start_secs: t,
            end_secs: t + 45.0,
            title: "End Bumper".to_string(),
        });

        let spans = detect_tv_episodes_from_chapters(1, &chapters, "Fawlty Towers", 1, 1, None);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].start_chapter, 1);
        assert_eq!(spans[0].end_chapter, 4);
        assert_eq!(spans[0].episode.formatted_name, "Fawlty Towers - S01E01");

        assert_eq!(spans[1].start_chapter, 5);
        assert_eq!(spans[1].end_chapter, 8);
        assert_eq!(spans[1].episode.formatted_name, "Fawlty Towers - S01E02");

        assert_eq!(spans[2].start_chapter, 9);
        assert_eq!(spans[2].end_chapter, 13);
        assert_eq!(spans[2].episode.formatted_name, "Fawlty Towers - S01E03");
    }
}
