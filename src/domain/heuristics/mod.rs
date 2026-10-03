pub mod cluster_detector;
pub mod speedup;

pub use cluster_detector::rank_best_title_from_slice;
pub use speedup::{calculate_runtime_diff, theatrical_to_pal_duration, PAL_SPEEDUP_FACTOR};
