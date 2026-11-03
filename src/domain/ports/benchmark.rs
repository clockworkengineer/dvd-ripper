//! Domain ports for optical drive throughput performance benchmarking.

use crate::dvd::DriveBenchmarkReport;
use anyhow::Result;
use std::path::Path;

/// Abstraction for optical drive throughput benchmarking.
pub trait DriveBenchmarkService: Send + Sync {
    /// Runs an optical throughput benchmark measuring read speed and demux frame rates.
    ///
    /// # Errors
    /// Returns an error if the optical drive is not accessible or FFmpeg benchmark fails.
    fn benchmark(&self, drive_path: &Path, duration_secs: u64) -> Result<DriveBenchmarkReport>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBenchmarkService {
        report: DriveBenchmarkReport,
    }

    impl DriveBenchmarkService for MockBenchmarkService {
        fn benchmark(
            &self,
            _drive_path: &Path,
            _duration_secs: u64,
        ) -> Result<DriveBenchmarkReport> {
            Ok(self.report.clone())
        }
    }

    #[test]
    fn test_mock_benchmark_service() {
        let expected = DriveBenchmarkReport {
            drive_path: "D:\\".to_string(),
            test_duration_secs: 10,
            read_bytes: 52_428_800,
            read_speed_mbps: 5.24,
            demux_speed_mbps: 5.50,
            fps: "250.0".to_string(),
            rating_summary: "High Speed (4x-8x)".to_string(),
        };

        let service = MockBenchmarkService {
            report: expected.clone(),
        };

        let actual = service.benchmark(Path::new("D:\\"), 10).unwrap();
        assert_eq!(actual.drive_path, "D:\\");
        assert_eq!(actual.read_speed_mbps, 5.24);
        assert_eq!(actual.rating_summary, "High Speed (4x-8x)");
    }
}
