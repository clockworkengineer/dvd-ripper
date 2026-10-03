/// Factor representing standard PAL 25fps speedup compared to 24fps film (24/25 = 0.96)
pub const PAL_SPEEDUP_FACTOR: f64 = 24.0 / 25.0;

/// Converts theatrical 24fps duration to expected PAL 25fps DVD duration.
pub fn theatrical_to_pal_duration(theatrical_secs: f64) -> f64 {
    theatrical_secs * PAL_SPEEDUP_FACTOR
}

/// Computes the minimal difference between probed duration and expected duration,
/// accounting for both standard 24fps NTSC and 25fps PAL 4% speedup.
pub fn calculate_runtime_diff(probed_secs: f64, expected_target_secs: f64) -> f64 {
    let diff_ntsc = (probed_secs - expected_target_secs).abs();
    let diff_pal = (probed_secs - theatrical_to_pal_duration(expected_target_secs)).abs();
    diff_ntsc.min(diff_pal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theatrical_to_pal_duration() {
        // 131 minutes = 7860s
        let theatrical = 7860.0;
        let pal = theatrical_to_pal_duration(theatrical);
        assert!((pal - 7545.6).abs() < 1e-4);
    }

    #[test]
    fn test_calculate_runtime_diff() {
        // John Wick 3 on PAL DVD is 7524s
        let diff = calculate_runtime_diff(7524.0, 7860.0);
        assert!((diff - 21.6).abs() < 1e-4);
    }
}
