pub use crate::utils::FfmpegProgressMetrics;

/// Parses an FFmpeg progress stderr line into structured metrics.
pub fn parse_ffmpeg_progress_line(line: &str) -> Option<FfmpegProgressMetrics> {
    crate::utils::parse_ffmpeg_progress_line(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress() {
        let line = "frame=  120 fps= 45 q=28.0 size=    1024kB time=00:01:30.50 bitrate=  92.7kbits/s speed=1.85x";
        let res = parse_ffmpeg_progress_line(line);
        assert!(res.is_some());
        let metrics = res.unwrap();
        assert_eq!(metrics.fps, Some(45.0));
        assert_eq!(metrics.speed.as_deref(), Some("1.85x"));
        assert_eq!(metrics.time_secs, Some(90.5));
    }
}
