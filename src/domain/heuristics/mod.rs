pub mod cluster_detector;
pub mod speedup;
pub mod tv_detector;

pub use cluster_detector::rank_best_title_from_slice;
pub use speedup::{PAL_SPEEDUP_FACTOR, calculate_runtime_diff, theatrical_to_pal_duration};
pub use tv_detector::{
    DEFAULT_MIN_TV_EPISODE_SECS, PLAY_ALL_THRESHOLD_MULTIPLIER, calculate_median_duration,
    detect_tv_episodes_from_titles, filter_tv_episode_titles,
};

use crate::domain::entities::disc::{DvdTitleInfo, TvEpisodeInfo};

/// Open/Closed Principle (OCP/SOLID): Strategy trait for feature-film title selection algorithms.
pub trait TitleRankingStrategy: Send + Sync {
    /// Ranks titles and returns the primary feature film title index (1-based) and its duration.
    fn rank_title(
        &self,
        titles: &[DvdTitleInfo],
        expected_duration_secs: Option<f64>,
    ) -> (u32, Option<f64>);
}

/// Default duration clustering heuristic ranking strategy (bypasses decoy traps and short extras).
#[derive(Debug, Default, Clone, Copy)]
pub struct ClusterTitleRankingStrategy;

impl TitleRankingStrategy for ClusterTitleRankingStrategy {
    fn rank_title(
        &self,
        titles: &[DvdTitleInfo],
        expected_duration_secs: Option<f64>,
    ) -> (u32, Option<f64>) {
        rank_best_title_from_slice(titles, expected_duration_secs)
    }
}

/// Fallback strategy that simply selects the single longest title on disc.
#[derive(Debug, Default, Clone, Copy)]
pub struct LongestTrackStrategy;

impl TitleRankingStrategy for LongestTrackStrategy {
    fn rank_title(
        &self,
        titles: &[DvdTitleInfo],
        _expected_duration_secs: Option<f64>,
    ) -> (u32, Option<f64>) {
        titles
            .iter()
            .max_by(|a, b| a.duration_secs.total_cmp(&b.duration_secs))
            .map(|t| (t.title_num, Some(t.duration_secs)))
            .unwrap_or((1, None))
    }
}

/// Configuration for TV series episodic title filtering and naming.
#[derive(Debug, Clone)]
pub struct TvDetectionConfig {
    pub show_name: String,
    pub season: u32,
    pub start_episode: u32,
    pub min_duration_secs: Option<f64>,
}

/// Open/Closed Principle (OCP/SOLID): Strategy trait for discovering episodic TV titles on disc.
pub trait EpisodeDetectionStrategy: Send + Sync {
    fn detect_episodes(
        &self,
        titles: &[DvdTitleInfo],
        config: &TvDetectionConfig,
    ) -> Vec<TvEpisodeInfo>;
}

/// Default statistical duration clustering TV episode detector.
#[derive(Debug, Default, Clone, Copy)]
pub struct DurationClusteringTvStrategy;

impl EpisodeDetectionStrategy for DurationClusteringTvStrategy {
    fn detect_episodes(
        &self,
        titles: &[DvdTitleInfo],
        config: &TvDetectionConfig,
    ) -> Vec<TvEpisodeInfo> {
        detect_tv_episodes_from_titles(
            titles,
            &config.show_name,
            config.season,
            config.start_episode,
            config.min_duration_secs,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_ranking_strategies() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 5400.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 7200.0,
            },
            DvdTitleInfo {
                title_num: 3,
                duration_secs: 600.0,
            },
        ];

        let cluster_strat = ClusterTitleRankingStrategy;
        let longest_strat = LongestTrackStrategy;

        let best = cluster_strat.rank_title(&titles, None);
        assert_eq!(best.0, 2);

        let longest = longest_strat.rank_title(&titles, None);
        assert_eq!(longest.0, 2);
    }

    #[test]
    fn test_episode_detection_strategy() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 1320.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 1350.0,
            },
        ];

        let strat = DurationClusteringTvStrategy;
        let config = TvDetectionConfig {
            show_name: "The Office".to_string(),
            season: 1,
            start_episode: 1,
            min_duration_secs: None,
        };

        let episodes = strat.detect_episodes(&titles, &config);
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].episode_num, 1);
        assert_eq!(episodes[1].episode_num, 2);
    }
}
