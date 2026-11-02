pub mod cluster_detector;
pub mod speedup;
pub mod tv_detector;

pub use cluster_detector::rank_best_title_from_slice;
pub use speedup::{PAL_SPEEDUP_FACTOR, calculate_runtime_diff, theatrical_to_pal_duration};
pub use tv_detector::{
    DEFAULT_MIN_TV_EPISODE_SECS, PLAY_ALL_THRESHOLD_MULTIPLIER, calculate_median_duration,
    detect_tv_episodes_from_titles, filter_tv_episode_titles,
};
