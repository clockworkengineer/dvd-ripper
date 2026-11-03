//! High-performance, zero-allocation parser for FFmpeg progress logs.

pub use crate::utils::FfmpegProgressMetrics;

/// Zero-allocation view of parsed FFmpeg progress metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressSlice<'a> {
    pub fps: Option<f64>,
    pub speed: Option<&'a str>,
    pub bitrate: Option<&'a str>,
    pub time_secs: Option<f64>,
}

/// Zero-allocation helper that extracts a key-value field directly from a byte slice.
pub fn extract_kv_field_bytes<'a>(bytes: &'a [u8], key: &[u8]) -> Option<&'a str> {
    if let Some(pos) = bytes.windows(key.len()).position(|w| w == key) {
        let after_key = &bytes[pos + key.len()..];
        let val_bytes = after_key
            .split(|&b| b == b' ' || b == b'\r' || b == b'\n')
            .find(|part| !part.is_empty())?;
        std::str::from_utf8(val_bytes).ok()
    } else {
        None
    }
}

/// Parses an FFmpeg progress output byte slice (e.g., `frame= 100 fps=25.5 ...`) with zero heap allocations.
///
/// # Examples
///
/// ```
/// use dvd_ripper::infrastructure::ffmpeg::progress_parser::parse_ffmpeg_progress_bytes;
///
/// let line = b"frame= 100 fps= 50.0 q=28.0 size= 1024kB time=00:01:30.50 speed= 2.0x";
/// let metrics = parse_ffmpeg_progress_bytes(line).unwrap();
/// assert_eq!(metrics.fps, Some(50.0));
/// assert_eq!(metrics.speed, Some("2.0x"));
/// assert_eq!(metrics.time_secs, Some(90.5));
/// ```
pub fn parse_ffmpeg_progress_bytes(line_bytes: &[u8]) -> Option<ProgressSlice<'_>> {
    let has_time = line_bytes.windows(5).any(|w| w == b"time=");
    let has_fps = line_bytes.windows(4).any(|w| w == b"fps=");
    if !has_time && !has_fps {
        return None;
    }

    let fps = extract_kv_field_bytes(line_bytes, b"fps=").and_then(|v| v.parse::<f64>().ok());
    let speed = extract_kv_field_bytes(line_bytes, b"speed=");
    let bitrate = extract_kv_field_bytes(line_bytes, b"bitrate=");
    let time_secs =
        extract_kv_field_bytes(line_bytes, b"time=").and_then(crate::utils::parse_duration);

    Some(ProgressSlice {
        fps,
        speed,
        bitrate,
        time_secs,
    })
}

/// Parses an FFmpeg progress stderr line into structured metrics.
pub fn parse_ffmpeg_progress_line(line: &str) -> Option<FfmpegProgressMetrics> {
    parse_ffmpeg_progress_bytes(line.as_bytes()).map(|slice| FfmpegProgressMetrics {
        fps: slice.fps,
        speed: slice.speed.map(|s| s.to_string()),
        bitrate: slice.bitrate.map(|s| s.to_string()),
        time_secs: slice.time_secs,
    })
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

    #[test]
    fn test_parse_progress_bytes_zero_alloc() {
        let line = b"frame=  120 fps= 45 q=28.0 size=    1024kB time=00:01:30.50 bitrate=  92.7kbits/s speed=1.85x";
        let res = parse_ffmpeg_progress_bytes(line);
        assert!(res.is_some());
        let metrics = res.unwrap();
        assert_eq!(metrics.fps, Some(45.0));
        assert_eq!(metrics.speed, Some("1.85x"));
        assert_eq!(metrics.bitrate, Some("92.7kbits/s"));
        assert_eq!(metrics.time_secs, Some(90.5));
    }
}
