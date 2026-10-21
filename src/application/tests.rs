#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use anyhow::{Result, anyhow};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    use crate::application::probe_disc::ProbeDiscUseCase;
    use crate::application::rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};
    use crate::application::rip_tv_batch::{RipTvBatchRequest, RipTvBatchUseCase};
    use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo, TvEpisodeInfo};
    use crate::domain::entities::job::RipRecord;
    use crate::domain::entities::media::{FilmMetadata, SearchCandidate};
    use crate::domain::ports::metadata::MetadataProvider;
    use crate::domain::ports::notification::{NotificationEvent, Notifier};
    use crate::domain::ports::optical_drive::OpticalDrive;
    use crate::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
    use crate::domain::ports::transcoder::{ProgressObserver, TranscodeJob, Transcoder};

    // --- MOCKS ---

    #[derive(Default)]
    struct MockTranscoder {
        pub calls: Arc<Mutex<Vec<String>>>,
        pub should_fail: bool,
    }

    impl Transcoder for MockTranscoder {
        fn transcode(
            &self,
            job: &TranscodeJob,
            observer: Option<&dyn ProgressObserver>,
            _cancel_flag: Option<Arc<AtomicBool>>,
        ) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(job.display_title.to_string());
            if let Some(obs) = observer {
                obs.on_progress(50.0, "60", "2.0x");
                obs.on_log("Mock transcode step");
                obs.on_progress(100.0, "60", "2.0x");
            }
            if self.should_fail {
                Err(anyhow!("Mock transcode simulated failure"))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct MockNotifier {
        pub events: Arc<Mutex<Vec<String>>>,
    }

    impl Notifier for MockNotifier {
        fn send(&self, event: &NotificationEvent) -> Result<()> {
            let desc = match event {
                NotificationEvent::RipStarted { title, .. } => format!("Started: {}", title),
                NotificationEvent::RipProgress { title, percent } => {
                    format!("Progress: {} {:.0}%", title, percent)
                }
                NotificationEvent::RipSuccess { title, .. } => format!("Success: {}", title),
                NotificationEvent::RipFailed {
                    title,
                    error_message,
                } => format!("Failed: {} ({})", title, error_message),
                NotificationEvent::RipCancelled { title } => format!("Cancelled: {}", title),
            };
            self.events.lock().unwrap().push(desc);
            Ok(())
        }
    }

    struct MockDiskSpaceGuard {
        pub allow: bool,
    }

    impl DiskSpaceGuard for MockDiskSpaceGuard {
        fn check_free_space(&self, _path: &Path, _min_free_gb: u64) -> Result<()> {
            if self.allow {
                Ok(())
            } else {
                Err(anyhow!("Mock insufficient free space"))
            }
        }
    }

    #[derive(Default)]
    struct MockHistoryRepository {
        pub records: Arc<Mutex<Vec<RipRecord>>>,
    }

    impl HistoryRepository for MockHistoryRepository {
        fn load_history(&self, _custom_path: Option<&Path>) -> Vec<RipRecord> {
            self.records.lock().unwrap().clone()
        }

        fn append_record(&self, record: &RipRecord, _custom_path: Option<&Path>) -> Result<()> {
            self.records.lock().unwrap().push(record.clone());
            Ok(())
        }

        fn clear_history(&self, _custom_path: Option<&Path>) -> Result<()> {
            self.records.lock().unwrap().clear();
            Ok(())
        }
    }

    struct MockOpticalDrive {
        pub path: PathBuf,
        pub label: Option<String>,
        pub titles: Vec<DvdTitleInfo>,
    }

    impl OpticalDrive for MockOpticalDrive {
        fn get_path(&self) -> &Path {
            &self.path
        }
        fn read_volume_label(&self) -> Option<String> {
            self.label.clone()
        }
        fn probe_titles(&self) -> Vec<DvdTitleInfo> {
            self.titles.clone()
        }
        fn inspect_protection(&self) -> DiscCopyProtectionReport {
            DiscCopyProtectionReport::default()
        }
        fn eject(&self) -> Result<()> {
            Ok(())
        }
    }

    struct MockMetadataProvider {
        pub film: Option<FilmMetadata>,
    }

    impl MetadataProvider for MockMetadataProvider {
        fn search(&self, _query: &str) -> Result<Vec<SearchCandidate>> {
            Ok(vec![])
        }
        fn get_details_by_id(&self, _id: &str) -> Result<Option<FilmMetadata>> {
            Ok(self.film.clone())
        }
        fn get_details_by_title_and_year(
            &self,
            _title: &str,
            _year: Option<u32>,
        ) -> Result<Option<FilmMetadata>> {
            Ok(self.film.clone())
        }
    }

    struct TestObserver {
        pub progresses: Arc<Mutex<Vec<f64>>>,
        pub logs: Arc<Mutex<Vec<String>>>,
    }

    impl ProgressObserver for TestObserver {
        fn on_progress(&self, percent: f64, _fps: &str, _speed: &str) {
            self.progresses.lock().unwrap().push(percent);
        }
        fn on_log(&self, line: &str) {
            self.logs.lock().unwrap().push(line.to_string());
        }
    }

    // --- TESTS ---

    #[test]
    fn test_rip_single_title_happy_path() {
        let transcoder = Arc::new(MockTranscoder::default());
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case = RipSingleTitleUseCase::new(
            transcoder.clone(),
            notifier.clone(),
            disk_guard,
            history.clone(),
        );

        let observer = TestObserver {
            progresses: Arc::new(Mutex::new(Vec::new())),
            logs: Arc::new(Mutex::new(Vec::new())),
        };

        let req = RipSingleTitleRequest {
            source_dvd: PathBuf::from("D:\\"),
            title_number: 1,
            output_path: PathBuf::from("Films/Test/test.mp4"),
            display_title: "Test Movie (2024)".to_string(),
            expected_duration_secs: Some(7200.0),
            metadata: Some(FilmMetadata {
                title: "Test Movie".to_string(),
                year: Some(2024),
                ..Default::default()
            }),
            min_free_gb: 10,
        };

        let res = use_case.execute(&req, Some(&observer), None);
        assert!(res.is_ok());

        // Verifications
        assert_eq!(
            transcoder.calls.lock().unwrap().as_slice(),
            &["Test Movie (2024)"]
        );
        assert_eq!(
            observer.progresses.lock().unwrap().as_slice(),
            &[50.0, 100.0]
        );
        assert_eq!(
            observer.logs.lock().unwrap().as_slice(),
            &["Mock transcode step"]
        );

        let events = notifier.events.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.starts_with("Started: Test Movie (2024)"))
        );
        assert!(
            events
                .iter()
                .any(|e| e.starts_with("Success: Test Movie (2024)"))
        );

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].title, "Test Movie (2024)");
        assert_eq!(recs[0].status, "Completed");
    }

    #[test]
    fn test_rip_single_title_insufficient_disk_space() {
        let transcoder = Arc::new(MockTranscoder::default());
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: false });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case = RipSingleTitleUseCase::new(
            transcoder.clone(),
            notifier.clone(),
            disk_guard,
            history.clone(),
        );

        let req = RipSingleTitleRequest {
            source_dvd: PathBuf::from("D:\\"),
            title_number: 1,
            output_path: PathBuf::from("Films/Test/test.mp4"),
            display_title: "Test Movie (2024)".to_string(),
            expected_duration_secs: None,
            metadata: None,
            min_free_gb: 50,
        };

        let res = use_case.execute(&req, None, None);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("insufficient free space")
        );

        // Transcoder must never have been called
        assert!(transcoder.calls.lock().unwrap().is_empty());
        assert!(history.records.lock().unwrap().is_empty());
    }

    #[test]
    fn test_rip_single_title_transcode_failure_handling() {
        let transcoder = Arc::new(MockTranscoder {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        });
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case =
            RipSingleTitleUseCase::new(transcoder, notifier.clone(), disk_guard, history.clone());

        let req = RipSingleTitleRequest {
            source_dvd: PathBuf::from("D:\\"),
            title_number: 1,
            output_path: PathBuf::from("Films/Test/fail.mp4"),
            display_title: "Failing Movie".to_string(),
            expected_duration_secs: None,
            metadata: None,
            min_free_gb: 10,
        };

        let res = use_case.execute(&req, None, None);
        assert!(res.is_err());

        let events = notifier.events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("Failed: Failing Movie")));

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].status, "Failed");
    }

    #[test]
    fn test_rip_tv_batch_sequential_episodes() {
        let transcoder = Arc::new(MockTranscoder::default());
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let batch_use_case =
            RipTvBatchUseCase::new(transcoder.clone(), notifier, disk_guard, history.clone());

        let episodes = vec![
            TvEpisodeInfo {
                title_num: 1,
                episode_num: 1,
                duration_secs: 1500.0,
                formatted_name: "Episode 1".to_string(),
            },
            TvEpisodeInfo {
                title_num: 2,
                episode_num: 2,
                duration_secs: 1520.0,
                formatted_name: "Episode 2".to_string(),
            },
        ];

        let req = RipTvBatchRequest {
            source_dvd: PathBuf::from("E:\\"),
            series_title: "Firefly".to_string(),
            season_number: 1,
            start_episode: 1,
            output_dir: std::env::temp_dir().join("tv_batch_test"),
            episodes,
            metadata: Some(FilmMetadata {
                title: "Firefly".to_string(),
                is_series: true,
                ..Default::default()
            }),
            min_free_gb: 5,
        };

        let res = batch_use_case.execute(&req, None, None);
        assert_eq!(res.unwrap(), 2);

        let calls = transcoder.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], "Firefly - S01E01");
        assert_eq!(calls[1], "Firefly - S01E02");

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 2);
    }

    #[test]
    fn test_rip_tv_batch_cooperative_cancellation() {
        let transcoder = Arc::new(MockTranscoder::default());
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let batch_use_case =
            RipTvBatchUseCase::new(transcoder, notifier.clone(), disk_guard, history);

        let episodes = vec![TvEpisodeInfo {
            title_num: 1,
            episode_num: 1,
            duration_secs: 1500.0,
            formatted_name: "Episode 1".to_string(),
        }];

        let cancel_flag = Arc::new(AtomicBool::new(true)); // Pre-cancelled

        let req = RipTvBatchRequest {
            source_dvd: PathBuf::from("E:\\"),
            series_title: "Firefly".to_string(),
            season_number: 1,
            start_episode: 1,
            output_dir: PathBuf::from("TV/Firefly"),
            episodes,
            metadata: None,
            min_free_gb: 5,
        };

        let res = batch_use_case.execute(&req, None, Some(cancel_flag));
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cancelled by user"));

        let events = notifier.events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("Cancelled: Firefly")));
    }

    #[test]
    fn test_probe_disc_use_case() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 99,
                duration_secs: 7500.0,
            }, // Decoy
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 7520.0,
            }, // Genuine feature
            DvdTitleInfo {
                title_num: 5,
                duration_secs: 120.0,
            }, // Trailer
        ];

        let mock_drive = MockOpticalDrive {
            path: PathBuf::from("D:\\"),
            label: Some("JOHN_WICK_3".to_string()),
            titles,
        };

        let mock_meta = MockMetadataProvider {
            film: Some(FilmMetadata {
                title: "John Wick: Chapter 3 - Parabellum".to_string(),
                year: Some(2019),
                runtime_secs: Some(7860.0), // Theatrical 24fps
                ..Default::default()
            }),
        };

        let use_case = ProbeDiscUseCase::new(Some(Arc::new(mock_meta)));
        let result = use_case.execute(&mock_drive).unwrap();

        assert_eq!(result.primary_title_num, 1);
        assert!(result.metadata.is_some());
        assert_eq!(
            result.metadata.unwrap().title,
            "John Wick: Chapter 3 - Parabellum"
        );
    }
}
