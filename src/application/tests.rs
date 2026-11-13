#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use anyhow::{Result, anyhow};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::application::probe_disc::ProbeDiscUseCase;
    use crate::application::rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};
    use crate::application::rip_tv_batch::{RipTvBatchRequest, RipTvBatchUseCase};
    use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo, TvEpisodeInfo};
    use crate::domain::entities::job::RipRecord;
    use crate::domain::entities::media::{FilmMetadata, SearchCandidate};
    use crate::domain::ports::metadata::MetadataProvider;
    use crate::domain::ports::notification::{NotificationEvent, Notifier};
    use crate::domain::ports::optical_drive::{
        DriveIdentity, OpticalDrive, ProtectionInspector, TitleProber, TrayOperator,
    };
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
        pub probe_count: Arc<AtomicUsize>,
    }

    impl DriveIdentity for MockOpticalDrive {
        fn get_path(&self) -> &Path {
            &self.path
        }
        fn read_volume_label(&self) -> Option<String> {
            self.label.clone()
        }
    }

    impl TitleProber for MockOpticalDrive {
        fn probe_titles(&self) -> Vec<DvdTitleInfo> {
            self.probe_count.fetch_add(1, Ordering::SeqCst);
            self.titles.clone()
        }
    }

    impl ProtectionInspector for MockOpticalDrive {
        fn inspect_protection(&self) -> DiscCopyProtectionReport {
            DiscCopyProtectionReport::default()
        }
    }

    impl TrayOperator for MockOpticalDrive {
        fn eject(&self) -> Result<()> {
            Ok(())
        }
    }

    impl OpticalDrive for MockOpticalDrive {}

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
    fn test_rip_single_title_cancellation_handling() {
        let transcoder = Arc::new(MockTranscoder {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true, // will simulate error or cancellation
        });
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case =
            RipSingleTitleUseCase::new(transcoder, notifier.clone(), disk_guard, history.clone());

        let cancel_flag = Arc::new(AtomicBool::new(true)); // Pre-cancelled

        let req = RipSingleTitleRequest {
            source_dvd: PathBuf::from("D:\\"),
            title_number: 1,
            output_path: PathBuf::from("Films/Test/cancel.mp4"),
            display_title: "Cancelled Movie".to_string(),
            expected_duration_secs: None,
            metadata: None,
            min_free_gb: 10,
        };

        let res = use_case.execute(&req, None, Some(cancel_flag));
        assert!(res.is_err());

        let events = notifier.events.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.contains("Cancelled: Cancelled Movie"))
        );

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].status, "Cancelled");
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
            end_episode: None,
            output_dir: std::env::temp_dir().join("tv_batch_test"),
            episodes,
            metadata: Some(FilmMetadata {
                title: "Firefly".to_string(),
                is_series: true,
                ..Default::default()
            }),
            min_free_gb: 5,
            file_extension: None,
            lookup_episode_titles: false,
            track_boxset: false,
            tv_dir: None,
        };

        let res = batch_use_case.execute(&req, None, None);
        assert_eq!(res.unwrap(), 2);

        let calls = transcoder.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], "Firefly - S01E01");
        assert_eq!(calls[1], "Firefly - S01E02");

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 2);
        assert!(
            recs[0].output_path.contains("TV")
                && recs[0].output_path.contains("Firefly")
                && recs[0].output_path.contains("Season 01")
        );
        assert!(
            recs[1].output_path.contains("TV")
                && recs[1].output_path.contains("Firefly")
                && recs[1].output_path.contains("Season 01")
        );
    }

    #[test]
    fn test_rip_tv_batch_end_episode_filtering() {
        let transcoder = Arc::new(MockTranscoder::default());
        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let batch_use_case =
            RipTvBatchUseCase::new(transcoder.clone(), notifier.clone(), disk_guard, history);

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
            TvEpisodeInfo {
                title_num: 3,
                episode_num: 3,
                duration_secs: 1510.0,
                formatted_name: "Episode 3".to_string(),
            },
        ];

        // Restrict to end_episode = 2
        let req = RipTvBatchRequest::builder("E:\\", "Firefly", "TV/Firefly")
            .season(1)
            .start_episode(1)
            .end_episode(2)
            .episodes(episodes)
            .build()
            .unwrap();

        let res = batch_use_case.execute(&req, None, None);
        assert_eq!(res.unwrap(), 2);

        let calls = transcoder.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], "Firefly - S01E01");
        assert_eq!(calls[1], "Firefly - S01E02");
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
            end_episode: None,
            output_dir: PathBuf::from("TV/Firefly"),
            episodes,
            metadata: None,
            min_free_gb: 5,
            file_extension: None,
            lookup_episode_titles: false,
            track_boxset: false,
            tv_dir: None,
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

        let probe_count = Arc::new(AtomicUsize::new(0));
        let mock_drive = MockOpticalDrive {
            path: PathBuf::from(format!(
                "Z:\\Unique_Probe_Drive_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )),
            label: Some("JOHN_WICK_3".to_string()),
            titles,
            probe_count: probe_count.clone(),
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
        assert_eq!(probe_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_probe_disc_session_cache() {
        use crate::application::probe_disc::SessionDiscCache;

        let titles = vec![DvdTitleInfo {
            title_num: 1,
            duration_secs: 7200.0,
        }];

        let probe_count = Arc::new(AtomicUsize::new(0));
        let mock_drive = MockOpticalDrive {
            path: PathBuf::from(format!(
                "Z:\\Unique_Session_Cache_Drive_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )),
            label: Some("CACHED_DISC".to_string()),
            titles,
            probe_count: probe_count.clone(),
        };

        let cache = SessionDiscCache::new();
        let use_case = ProbeDiscUseCase::with_cache(None, cache.clone());

        // First probe executes hardware interrogation
        let res1 = use_case.execute(&mock_drive).unwrap();
        assert_eq!(res1.disc_info.volume_label.as_deref(), Some("CACHED_DISC"));
        assert_eq!(probe_count.load(Ordering::SeqCst), 1);

        // Second probe utilizes session cache, bypassing hardware read
        let res2 = use_case.execute(&mock_drive).unwrap();
        assert_eq!(res2.disc_info.fingerprint, res1.disc_info.fingerprint);
        assert_eq!(probe_count.load(Ordering::SeqCst), 1);

        // Explicit cache clear causes subsequent probe to re-read hardware
        cache.clear();
        let res3 = use_case.execute(&mock_drive).unwrap();
        assert_eq!(res3.disc_info.fingerprint, res1.disc_info.fingerprint);
        assert_eq!(probe_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_dump_iso_use_case_insufficient_space() {
        use crate::application::dump_iso::{DumpIsoRequest, DumpIsoUseCase};

        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: false });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case = DumpIsoUseCase::new(notifier, disk_guard, history);

        let req = DumpIsoRequest {
            source_dvd: PathBuf::from("D:\\"),
            output_path: PathBuf::from("Films/Test/image.iso"),
            volume_label: Some("TEST_DISC".to_string()),
            display_title: "Test Disc ISO".to_string(),
            min_free_gb: 100,
        };

        let res = use_case.execute(&req, None, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_dump_iso_use_case_cancellation() {
        use crate::application::dump_iso::{DumpIsoRequest, DumpIsoUseCase};

        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case = DumpIsoUseCase::new(notifier.clone(), disk_guard, history.clone());
        let cancel_flag = Arc::new(AtomicBool::new(true));

        let temp_dir = std::env::temp_dir().join(format!("iso_cancel_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let out_file = temp_dir.join("test_cancelled.iso");

        let req = DumpIsoRequest {
            source_dvd: PathBuf::from("Z:\\DefinitelyNonExistent"),
            output_path: out_file,
            volume_label: Some("CANCELLED_DISC".to_string()),
            display_title: "Cancelled Disc".to_string(),
            min_free_gb: 1,
        };

        let res = use_case.execute(&req, None, Some(cancel_flag));
        assert!(res.is_err());

        let events = notifier.events.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.contains("Cancelled: Cancelled Disc"))
        );

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].status, "Cancelled");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rip_single_title_builder() {
        let req = RipSingleTitleRequest::builder("D:\\", 1, "C:\\Out\\Movie.mp4")
            .display_title("The Matrix")
            .expected_duration(8160.0)
            .min_free_gb(15)
            .build()
            .unwrap();

        assert_eq!(req.source_dvd, PathBuf::from("D:\\"));
        assert_eq!(req.title_number, 1);
        assert_eq!(req.output_path, PathBuf::from("C:\\Out\\Movie.mp4"));
        assert_eq!(req.display_title, "The Matrix");
        assert_eq!(req.expected_duration_secs, Some(8160.0));
        assert_eq!(req.min_free_gb, 15);
    }

    #[test]
    fn test_dump_iso_builder() {
        use crate::application::dump_iso::DumpIsoRequest;

        let req = DumpIsoRequest::builder("D:\\", "C:\\Out\\disc.iso")
            .volume_label("MATRIX_DISC1")
            .min_free_gb(20)
            .build()
            .unwrap();

        assert_eq!(req.source_dvd, PathBuf::from("D:\\"));
        assert_eq!(req.output_path, PathBuf::from("C:\\Out\\disc.iso"));
        assert_eq!(req.volume_label.as_deref(), Some("MATRIX_DISC1"));
        assert_eq!(req.display_title, "MATRIX_DISC1");
        assert_eq!(req.min_free_gb, 20);
    }

    #[test]
    fn test_rip_tv_batch_builder() {
        let ep = TvEpisodeInfo {
            title_num: 2,
            episode_num: 1,
            duration_secs: 2700.0,
            formatted_name: "S01E01 - Pilot".to_string(),
        };

        let req = RipTvBatchRequest::builder("D:\\", "Doctor Who", "C:\\Out\\TV")
            .season(2)
            .start_episode(1)
            .episodes(vec![ep])
            .lookup_episode_titles(true)
            .track_boxset(true)
            .file_extension("mkv")
            .min_free_gb(30)
            .build()
            .unwrap();

        assert_eq!(req.source_dvd, PathBuf::from("D:\\"));
        assert_eq!(req.series_title, "Doctor Who");
        assert_eq!(req.output_dir, PathBuf::from("C:\\Out\\TV"));
        assert_eq!(req.season_number, 2);
        assert_eq!(req.start_episode, 1);
        assert_eq!(req.episodes.len(), 1);
        assert!(req.lookup_episode_titles);
        assert!(req.track_boxset);
        assert_eq!(req.file_extension.as_deref(), Some("mkv"));
        assert_eq!(req.min_free_gb, 30);
    }

    #[test]
    fn test_dump_iso_builder_fallback_and_options() {
        use crate::application::dump_iso::DumpIsoRequest;

        // Fallback when neither display_title nor volume_label is provided
        let req1 = DumpIsoRequest::builder("D:\\", "C:\\Out\\disc.iso")
            .build()
            .unwrap();
        assert_eq!(req1.display_title, "Archival ISO Dump");

        // Explicit display_title overrides volume_label
        let req2 = DumpIsoRequest::builder("D:\\", "C:\\Out\\disc.iso")
            .volume_label("DISC_LABEL")
            .display_title("Custom ISO Title")
            .build()
            .unwrap();
        assert_eq!(req2.display_title, "Custom ISO Title");
    }

    #[test]
    fn test_dump_iso_use_case_failure_handling() {
        use crate::application::dump_iso::{DumpIsoRequest, DumpIsoUseCase};

        let notifier = Arc::new(MockNotifier::default());
        let disk_guard = Arc::new(MockDiskSpaceGuard { allow: true });
        let history = Arc::new(MockHistoryRepository::default());

        let use_case = DumpIsoUseCase::new(notifier.clone(), disk_guard, history.clone());

        let req = DumpIsoRequest {
            source_dvd: PathBuf::from("Z:\\DefinitelyNonExistent123"),
            output_path: PathBuf::from("Z:\\DefinitelyNonExistentOut\\image.iso"),
            volume_label: Some("FAIL_DISC".to_string()),
            display_title: "Failing ISO".to_string(),
            min_free_gb: 1,
        };

        let res = use_case.execute(&req, None, None);
        assert!(res.is_err());

        let events = notifier.events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("Failed: Failing ISO")));

        let recs = history.records.lock().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].status, "Failed");
    }

    #[test]
    fn test_rip_tv_batch_use_case_transcode_failure() {
        let transcoder = Arc::new(MockTranscoder {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        });
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

        let req = RipTvBatchRequest::builder("D:\\", "Failing Show", "C:\\Out\\TV")
            .episodes(episodes)
            .build()
            .unwrap();

        let res = batch_use_case.execute(&req, None, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_rip_single_title_builder_defaults() {
        let out_path = std::path::Path::new("Out").join("Movie.mp4");
        let req = RipSingleTitleRequest::builder("D:\\", 1, &out_path)
            .build()
            .unwrap();

        assert_eq!(req.display_title, "Movie");
        assert_eq!(req.min_free_gb, 10);
    }

    #[test]
    fn test_resolve_tv_batch_destination_dir_standards() {
        use crate::application::rip_tv_batch::resolve_tv_batch_destination_dir;

        // 1. Standard root directory
        let out = resolve_tv_batch_destination_dir(Path::new("."), "Firefly", 1, None);
        assert_eq!(out, Path::new("TV").join("Firefly").join("Season 01"));

        // 2. With year metadata
        let out_year =
            resolve_tv_batch_destination_dir(Path::new("C:/Videos"), "The Office", 2, Some(2005));
        assert_eq!(
            out_year,
            Path::new("C:/Videos")
                .join("TV")
                .join("The Office (2005)")
                .join("Season 02")
        );

        // 3. Sibling root: out_dir ends in Films
        let out_films =
            resolve_tv_batch_destination_dir(Path::new("C:/Videos/Films"), "Battlestar", 1, None);
        assert_eq!(
            out_films,
            Path::new("C:/Videos")
                .join("TV")
                .join("Battlestar")
                .join("Season 01")
        );

        // 4. Existing TV root: out_dir ends in TV
        let out_tv =
            resolve_tv_batch_destination_dir(Path::new("D:/Media/TV"), "Star Trek", 3, None);
        assert_eq!(
            out_tv,
            Path::new("D:/Media/TV").join("Star Trek").join("Season 03")
        );

        // 5. Existing show folder: out_dir already points to TV/Firefly
        let out_show =
            resolve_tv_batch_destination_dir(Path::new("D:/Media/TV/Firefly"), "Firefly", 1, None);
        assert_eq!(out_show, Path::new("D:/Media/TV/Firefly").join("Season 01"));

        // 6. Existing season folder: out_dir already points to Season 01
        let out_season = resolve_tv_batch_destination_dir(
            Path::new("D:/Media/TV/Firefly/Season 01"),
            "Firefly",
            1,
            None,
        );
        assert_eq!(out_season, Path::new("D:/Media/TV/Firefly/Season 01"));
    }
}
