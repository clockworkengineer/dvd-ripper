use crate::domain::ports::benchmark::DriveBenchmarkService;
use crate::dvd::{DriveBenchmarkReport, run_drive_benchmark};
use anyhow::Result;
use std::path::Path;

/// Concrete DriveBenchmarkService implementation using FFmpeg optical reading diagnostics.
#[derive(Debug, Clone)]
pub struct FfmpegDriveBenchmarkService {
    ffmpeg_path: String,
}

impl FfmpegDriveBenchmarkService {
    pub fn new(ffmpeg_path: impl Into<String>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.into(),
        }
    }
}

impl DriveBenchmarkService for FfmpegDriveBenchmarkService {
    fn benchmark(&self, drive_path: &Path, duration_secs: u64) -> Result<DriveBenchmarkReport> {
        run_drive_benchmark(&self.ffmpeg_path, drive_path, duration_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_service_construction() {
        let service = FfmpegDriveBenchmarkService::new("ffmpeg");
        assert_eq!(service.ffmpeg_path, "ffmpeg");
    }
}
