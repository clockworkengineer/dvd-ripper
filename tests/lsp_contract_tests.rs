//! Liskov Substitution Principle (LSP / SOLID) Integration & Contract Test Suite.
//!
//! Validates that all ports, use cases, and infrastructure adapters satisfy behavioral subtyping:
//! interchangeable components must satisfy identical contracts without side effects or unexpected exceptions.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use dvd_ripper::AppContainer;
use dvd_ripper::api::{
    HttpEndpoint, HttpRouter, MetricsExporter, PrometheusMetricsExporter, SseBroadcastEngine,
    render_prometheus_metrics,
};
use dvd_ripper::application::dump_iso::{DumpIsoRequest, DumpIsoUseCase};
use dvd_ripper::application::probe_disc::ProbeDiscUseCase;
use dvd_ripper::application::rip_tv_batch::{RipTvBatchRequest, RipTvBatchUseCase};
use dvd_ripper::domain::entities::disc::{
    Chapter, DiscCopyProtectionReport, DvdTitleInfo, TitleStreamDetails, TvEpisodeInfo,
};
use dvd_ripper::domain::entities::error::DvdError;
use dvd_ripper::domain::entities::job::RipRecord;
use dvd_ripper::domain::entities::media::{FilmMetadata, SearchCandidate, TvEpisodeMetadata};
use dvd_ripper::domain::ports::iso_archiver::IsoArchiver;
use dvd_ripper::domain::ports::media_probe::StreamProber;
use dvd_ripper::domain::ports::metadata::{
    DiscFingerprintRepository, MetadataProvider, TvSeriesMetadataProvider,
};
use dvd_ripper::domain::ports::notification::{NotificationEvent, Notifier};
use dvd_ripper::domain::ports::optical_drive::{
    DriveIdentity, OpticalDrive, ProtectionInspector, TitleProber, TrayController, TrayOperator,
};
use dvd_ripper::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use dvd_ripper::domain::ports::queue::BoxSetRepository;
use dvd_ripper::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use dvd_ripper::domain::ports::transcoder::{ProgressObserver, TranscodeJob, Transcoder};
use dvd_ripper::infrastructure::optical::SystemTrayController;
use dvd_ripper::infrastructure::post_processors::{
    ArtworkPostProcessor, ChecksumPostProcessor, NfoPostProcessor, PostProcessingPipeline,
    TrayEjectPostProcessor,
};
use dvd_ripper::infrastructure::storage::{JsonHistoryRepository, SystemDiskSpaceGuard};
use dvd_ripper::queue::BoxSetRecord;
use std::collections::HashMap;

// -----------------------------------------------------------------------------
// In-Memory Test Doubles (Mocks) for LSP Substitution Testing
// -----------------------------------------------------------------------------

#[derive(Default)]
struct InMemoryHistoryRepository {
    records: Mutex<Vec<RipRecord>>,
}

impl HistoryRepository for InMemoryHistoryRepository {
    fn load_history(&self, _custom_path: Option<&Path>) -> Vec<RipRecord> {
        self.records.lock().unwrap().clone()
    }

    fn append_record(&self, record: &RipRecord, _custom_path: Option<&Path>) -> Result<()> {
        self.records.lock().unwrap().insert(0, record.clone());
        Ok(())
    }

    fn clear_history(&self, _custom_path: Option<&Path>) -> Result<()> {
        self.records.lock().unwrap().clear();
        Ok(())
    }
}

struct StaticDiskSpaceGuard {
    free_gb: u64,
}

impl DiskSpaceGuard for StaticDiskSpaceGuard {
    fn check_free_space(&self, path: &Path, min_free_gb: u64) -> Result<()> {
        if self.free_gb < min_free_gb {
            Err(DvdError::InsufficientDiskSpace {
                available_gb: self.free_gb as f64,
                required_gb: min_free_gb,
                path: path.to_string_lossy().to_string(),
            }
            .into())
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct MockTrayController {
    ejected_drives: Mutex<Vec<PathBuf>>,
    closed_drives: Mutex<Vec<PathBuf>>,
    spindown_drives: Mutex<Vec<PathBuf>>,
}

impl TrayController for MockTrayController {
    fn eject_tray(&self, drive_path: &Path) -> Result<()> {
        self.ejected_drives
            .lock()
            .unwrap()
            .push(drive_path.to_path_buf());
        Ok(())
    }

    fn close_tray(&self, drive_path: &Path) -> Result<()> {
        self.closed_drives
            .lock()
            .unwrap()
            .push(drive_path.to_path_buf());
        Ok(())
    }

    fn spindown(&self, drive_path: &Path) -> Result<()> {
        self.spindown_drives
            .lock()
            .unwrap()
            .push(drive_path.to_path_buf());
        Ok(())
    }
}

struct MockCancellableTranscoder;

impl Transcoder for MockCancellableTranscoder {
    fn transcode(
        &self,
        _job: &TranscodeJob,
        _observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        if let Some(flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }
        Ok(())
    }
}

struct MockMetadataProvider {
    candidates: Vec<SearchCandidate>,
    details: Option<FilmMetadata>,
}

impl MetadataProvider for MockMetadataProvider {
    fn search(&self, _query: &str) -> Result<Vec<SearchCandidate>> {
        Ok(self.candidates.clone())
    }

    fn get_details_by_id(&self, _id: &str) -> Result<Option<FilmMetadata>> {
        Ok(self.details.clone())
    }

    fn get_details_by_title_and_year(
        &self,
        _title: &str,
        _year: Option<u32>,
    ) -> Result<Option<FilmMetadata>> {
        Ok(self.details.clone())
    }
}

struct MockTvSeriesMetadataProvider {
    episodes: Vec<TvEpisodeMetadata>,
}

impl TvSeriesMetadataProvider for MockTvSeriesMetadataProvider {
    fn fetch_episodes(&self, _series_name: &str) -> Result<Vec<TvEpisodeMetadata>> {
        Ok(self.episodes.clone())
    }
}

// -----------------------------------------------------------------------------
// LSP Contract Verification Harnesses
// -----------------------------------------------------------------------------

fn verify_history_repository_contract<R: HistoryRepository>(repo: &R, custom_path: Option<&Path>) {
    // 1. Initial clearing contract
    assert!(repo.clear_history(custom_path).is_ok());
    assert!(repo.load_history(custom_path).is_empty());

    // 2. Sequential appending contract
    let r1 = RipRecord::new("Inception", "Movie", "C:\\Rips\\Inception.mp4", "Success");
    let r2 = RipRecord::new(
        "Doctor Who - S01E01",
        "TV Series",
        "C:\\Rips\\Doctor Who.mp4",
        "Success",
    );

    assert!(repo.append_record(&r1, custom_path).is_ok());
    assert!(repo.append_record(&r2, custom_path).is_ok());

    // 3. Roundtrip query contract (newest records prepended first)
    let loaded = repo.load_history(custom_path);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].title, "Doctor Who - S01E01");
    assert_eq!(loaded[1].title, "Inception");

    // 4. Secondary clearing contract
    assert!(repo.clear_history(custom_path).is_ok());
    assert!(repo.load_history(custom_path).is_empty());
}

fn verify_disk_space_guard_contract<G: DiskSpaceGuard>(
    guard: &G,
    target_path: &Path,
    sufficient_limit: u64,
    insufficient_limit: u64,
) {
    // Contract: When limit <= available, returns Ok(())
    let ok_res = guard.check_free_space(target_path, sufficient_limit);
    assert!(ok_res.is_ok(), "Guard failed sufficient space check");

    // Contract: When limit > available, returns error
    let err_res = guard.check_free_space(target_path, insufficient_limit);
    assert!(err_res.is_err(), "Guard allowed insufficient space");
}

// -----------------------------------------------------------------------------
// Integration Tests: Behavioral Subtyping Proofs
// -----------------------------------------------------------------------------

#[test]
fn test_lsp_history_repository_substitutability() {
    // A) Mock Implementation Subtyping Check
    let in_memory_repo = InMemoryHistoryRepository::default();
    verify_history_repository_contract(&in_memory_repo, None);

    // B) Real JSON Storage Adapter Subtyping Check
    let temp_dir = std::env::temp_dir().join(format!("lsp_history_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let test_json_path = temp_dir.join("test_history.json");

    let json_repo = JsonHistoryRepository::new();
    verify_history_repository_contract(&json_repo, Some(&test_json_path));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_lsp_disk_space_guard_substitutability() {
    // A) Mock Implementation Subtyping Check (Static 50 GB)
    let static_guard = StaticDiskSpaceGuard { free_gb: 50 };
    verify_disk_space_guard_contract(&static_guard, Path::new("C:\\"), 10, 100);

    // B) Real System OS Adapter Subtyping Check
    let system_guard = SystemDiskSpaceGuard::new();
    verify_disk_space_guard_contract(&system_guard, Path::new("."), 0, 10_000_000);
}

#[test]
fn test_lsp_tray_controller_substitutability() {
    let mock_tray = MockTrayController::default();
    let drive_path = Path::new("D:\\");
    assert!(mock_tray.eject_tray(drive_path).is_ok());
    assert!(mock_tray.close_tray(drive_path).is_ok());
    assert!(mock_tray.spindown(drive_path).is_ok());
    assert_eq!(
        *mock_tray.ejected_drives.lock().unwrap(),
        vec![drive_path.to_path_buf()]
    );
    assert_eq!(
        *mock_tray.closed_drives.lock().unwrap(),
        vec![drive_path.to_path_buf()]
    );

    // SystemTrayController gracefully handles invalid/test drive identifiers without crashing
    let system_tray = SystemTrayController::new();
    let test_drive = Path::new("NON_EXISTENT_DRIVE_X:\\");
    let _ = system_tray.eject_tray(test_drive);
    let _ = system_tray.close_tray(test_drive);
    let _ = system_tray.spindown(test_drive);
}

#[test]
fn test_lsp_transcoder_cooperative_cancellation_contract() {
    let transcoder = MockCancellableTranscoder;
    let cancel_flag = Arc::new(AtomicBool::new(true));

    let src = PathBuf::from("D:\\");
    let dst = PathBuf::from("C:\\Out\\test.mp4");
    let job = TranscodeJob {
        source_dvd: &src,
        title_number: 1,
        output_path: &dst,
        display_title: "Test Movie",
        expected_duration_secs: None,
    };

    let result = transcoder.transcode(&job, None, Some(cancel_flag));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}

#[test]
fn test_lsp_post_processing_pipeline_composition() {
    let mock_tray = Arc::new(MockTrayController::default());
    let pipeline = PostProcessingPipeline::build_production(Some(mock_tray.clone()));

    let temp_out = std::env::temp_dir().join(format!("lsp_post_test_{}.mp4", std::process::id()));
    let _ = std::fs::write(&temp_out, b"mock video stream content");

    let mut context = PostProcessContext::new(
        Some(temp_out.clone()),
        "LSP Test Title",
        PathBuf::from("D:\\"),
    );
    context.no_eject = true;

    let process_res = pipeline.process(&context);
    assert!(process_res.is_ok());

    let _ = std::fs::remove_file(&temp_out);
}

#[test]
fn test_lsp_app_container_use_case_inversion() {
    // Verify container dependency injection and composition root
    let container = AppContainer::build_production("ffmpeg");

    let single_title_uc = container.rip_single_title_use_case();
    let tv_batch_uc = container.rip_tv_batch_use_case();
    let dump_iso_uc = container.dump_iso_use_case();
    let probe_disc_uc = container.probe_disc_use_case();

    // Verify use cases instantiate correctly with standard port adapters
    let _ = (single_title_uc, tv_batch_uc, dump_iso_uc, probe_disc_uc);
}

#[test]
fn test_lsp_metadata_provider_substitutability() {
    let mock_provider = MockMetadataProvider {
        candidates: vec![SearchCandidate {
            title: "Test Movie".to_string(),
            year: Some(2024),
            imdb_id: "tt1234567".to_string(),
            type_field: "movie".to_string(),
            poster_url: None,
        }],
        details: Some(FilmMetadata {
            title: "Test Movie".to_string(),
            year: Some(2024),
            runtime_secs: Some(7200.0),
            poster_url: None,
            plot: Some("A test movie".to_string()),
            genre: Some("Sci-Fi".to_string()),
            director: Some("Director".to_string()),
            actors: Some("Actor".to_string()),
            rating: Some("PG-13".to_string()),
            is_series: false,
            total_seasons: None,
            poster_bytes: None,
        }),
    };

    let search_res = mock_provider.search("Test");
    assert!(search_res.is_ok());
    assert_eq!(search_res.unwrap().len(), 1);

    let details_res = mock_provider.get_details_by_id("tt1234567");
    assert!(details_res.is_ok());
    assert_eq!(details_res.unwrap().unwrap().title, "Test Movie");
}

#[test]
fn test_lsp_tv_series_metadata_provider_substitutability() {
    let mock_tv = MockTvSeriesMetadataProvider {
        episodes: vec![
            TvEpisodeMetadata {
                season: 1,
                episode_num: 1,
                title: "Pilot".to_string(),
                summary: None,
                runtime_minutes: None,
            },
            TvEpisodeMetadata {
                season: 1,
                episode_num: 2,
                title: "Second Episode".to_string(),
                summary: None,
                runtime_minutes: None,
            },
        ],
    };

    let title = mock_tv.find_episode_title("Show", 1, 1).unwrap();
    assert_eq!(title, Some("Pilot".to_string()));

    let not_found = mock_tv.find_episode_title("Show", 1, 99).unwrap();
    assert_eq!(not_found, None);
}

#[test]
fn test_lsp_individual_post_processors_contract() {
    let temp_out = std::env::temp_dir().join(format!("lsp_indiv_test_{}.mp4", std::process::id()));
    let _ = std::fs::write(&temp_out, b"mock video stream content for hashing and nfo");

    let context = PostProcessContext::new(
        Some(temp_out.clone()),
        "Individual Processor Test",
        PathBuf::from("D:\\"),
    );

    // 1. Artwork post processor
    let artwork = ArtworkPostProcessor;
    assert!(artwork.process(&context).is_ok());

    // 2. NFO post processor
    let nfo = NfoPostProcessor;
    assert!(nfo.process(&context).is_ok());

    // 3. Checksum post processor
    let checksum = ChecksumPostProcessor;
    assert!(checksum.process(&context).is_ok());

    // 4. Tray eject post processor
    let mock_tray = Arc::new(MockTrayController::default());
    let tray_eject = TrayEjectPostProcessor::new(Some(mock_tray.clone()));
    assert!(tray_eject.process(&context).is_ok());

    let _ = std::fs::remove_file(&temp_out);
}

// -----------------------------------------------------------------------------
// Additional SOLID / LSP / OCP / DIP Contract Tests
// -----------------------------------------------------------------------------

struct PureMockOpticalDrive {
    path: PathBuf,
    titles: Vec<DvdTitleInfo>,
    volume_label: Option<String>,
    fingerprint: String,
}

impl DriveIdentity for PureMockOpticalDrive {
    fn get_path(&self) -> &Path {
        &self.path
    }

    fn read_volume_label(&self) -> Option<String> {
        self.volume_label.clone()
    }

    fn compute_fingerprint(&self) -> String {
        self.fingerprint.clone()
    }
}

impl TitleProber for PureMockOpticalDrive {
    fn probe_titles(&self) -> Vec<DvdTitleInfo> {
        self.titles.clone()
    }
}

impl ProtectionInspector for PureMockOpticalDrive {
    fn inspect_protection(&self) -> DiscCopyProtectionReport {
        DiscCopyProtectionReport::default()
    }
}

impl TrayOperator for PureMockOpticalDrive {
    fn eject(&self) -> Result<()> {
        Ok(())
    }
}

impl OpticalDrive for PureMockOpticalDrive {}

#[derive(Default)]
struct InMemoryFingerprintRepo {
    entries: Mutex<HashMap<String, FilmMetadata>>,
}

impl DiscFingerprintRepository for InMemoryFingerprintRepo {
    fn lookup_fingerprint(&self, hash: &str) -> Option<FilmMetadata> {
        self.entries.lock().unwrap().get(hash).cloned()
    }

    fn save_fingerprint(&self, hash: &str, metadata: &FilmMetadata) -> Result<()> {
        self.entries
            .lock()
            .unwrap()
            .insert(hash.to_string(), metadata.clone());
        Ok(())
    }
}

#[derive(Default)]
struct NoopNotifier;

impl Notifier for NoopNotifier {
    fn send(&self, _event: &NotificationEvent) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct MockIsoArchiver {
    calls: Mutex<Vec<(PathBuf, PathBuf, Option<String>)>>,
}

impl IsoArchiver for MockIsoArchiver {
    fn dump_disc_to_iso(
        &self,
        source_dvd: &Path,
        output_path: &Path,
        volume_label: Option<&str>,
        _observer: Option<&dyn ProgressObserver>,
        _cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<u64> {
        self.calls.lock().unwrap().push((
            source_dvd.to_path_buf(),
            output_path.to_path_buf(),
            volume_label.map(str::to_string),
        ));
        Ok(4_700_000_000)
    }
}

struct MockStreamProber {
    chapters: Vec<Chapter>,
}

impl StreamProber for MockStreamProber {
    fn probe_titles(&self, _dvd_path: &Path) -> Vec<DvdTitleInfo> {
        vec![]
    }

    fn probe_stream_details(&self, _dvd_path: &Path, title_num: u32) -> TitleStreamDetails {
        TitleStreamDetails {
            title_num,
            duration_secs: self.chapters.iter().map(|c| c.duration_secs()).sum(),
            chapters: self.chapters.clone(),
            ..Default::default()
        }
    }
}

#[derive(Default)]
struct InMemoryBoxSetRepo {
    episodes: Mutex<HashMap<(String, u32), u32>>,
}

impl BoxSetRepository for InMemoryBoxSetRepo {
    fn get_next_episode(&self, show_name: &str, season: u32) -> u32 {
        *self
            .episodes
            .lock()
            .unwrap()
            .get(&(show_name.to_lowercase(), season))
            .unwrap_or(&0)
            + 1
    }

    fn record_episodes_ripped(&self, show_name: &str, season: u32, count: u32) -> u32 {
        let mut guard = self.episodes.lock().unwrap();
        let entry = guard.entry((show_name.to_lowercase(), season)).or_insert(0);
        *entry += count;
        *entry
    }

    fn reset_tracker(&self, show_name: &str, season: u32) -> bool {
        self.episodes
            .lock()
            .unwrap()
            .remove(&(show_name.to_lowercase(), season))
            .is_some()
    }

    fn list_boxsets(&self) -> Vec<BoxSetRecord> {
        vec![]
    }
}

#[test]
fn test_lsp_probe_disc_use_case_pure_mock_optical_drive() {
    // Proves ProbeDiscUseCase works purely through OpticalDrive and DiscFingerprintRepository
    // without touching the real host filesystem (fixes the previous LSP/DIP violation).
    let drive = PureMockOpticalDrive {
        path: PathBuf::from("MOCK:\\"),
        titles: vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 5400.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 120.0,
            },
        ],
        volume_label: Some("BLADE_RUNNER_1982".to_string()),
        fingerprint: "deadbeef12345678".to_string(),
    };

    let fp_repo = Arc::new(InMemoryFingerprintRepo::default());
    let expected_meta = FilmMetadata {
        title: "Blade Runner".to_string(),
        year: Some(1982),
        runtime_secs: Some(5400.0),
        poster_url: None,
        plot: None,
        genre: Some("Sci-Fi".to_string()),
        director: Some("Ridley Scott".to_string()),
        actors: None,
        rating: None,
        is_series: false,
        total_seasons: None,
        poster_bytes: None,
    };
    fp_repo
        .save_fingerprint("deadbeef12345678", &expected_meta)
        .unwrap();

    let use_case = ProbeDiscUseCase::new(None).with_fingerprint_repo(fp_repo);
    let result = use_case.execute(&drive).unwrap();

    assert_eq!(result.disc_info.titles.len(), 2);
    assert_eq!(result.primary_title_num, 1);
    assert_eq!(result.disc_info.fingerprint, "deadbeef12345678");
    assert_eq!(result.metadata.unwrap().title, "Blade Runner");
}

#[test]
fn test_lsp_dump_iso_use_case_archiver_substitutability() {
    let guard = Arc::new(StaticDiskSpaceGuard { free_gb: 100 });
    let history = Arc::new(InMemoryHistoryRepository::default());
    let archiver = Arc::new(MockIsoArchiver::default());
    let notifier = Arc::new(NoopNotifier);

    let use_case =
        DumpIsoUseCase::with_archiver(archiver.clone(), notifier, guard, history.clone());
    let req = DumpIsoRequest::builder("MOCK:\\", "C:\\Out\\TestDisc.iso")
        .volume_label("TEST_DISC")
        .display_title("Test Disc")
        .min_free_gb(10)
        .build()
        .unwrap();

    let bytes = use_case.execute(&req, None, None).unwrap();
    assert_eq!(bytes, 4_700_000_000);

    let calls = archiver.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, PathBuf::from("MOCK:\\"));
    assert_eq!(calls[0].1, PathBuf::from("C:\\Out\\TestDisc.iso"));
    assert_eq!(calls[0].2.as_deref(), Some("TEST_DISC"));
    assert_eq!(history.load_history(None).len(), 1);
}

#[test]
fn test_lsp_rip_tv_batch_use_case_port_inversion() {
    let transcoder = Arc::new(MockCancellableTranscoder);
    let notifier = Arc::new(NoopNotifier);
    let guard = Arc::new(StaticDiskSpaceGuard { free_gb: 100 });
    let history = Arc::new(InMemoryHistoryRepository::default());
    let tv_provider = Arc::new(MockTvSeriesMetadataProvider {
        episodes: vec![
            TvEpisodeMetadata {
                season: 2,
                episode_num: 1,
                title: "The North Remembers".to_string(),
                summary: None,
                runtime_minutes: Some(53),
            },
            TvEpisodeMetadata {
                season: 2,
                episode_num: 2,
                title: "The Night Lands".to_string(),
                summary: None,
                runtime_minutes: Some(54),
            },
        ],
    });
    let stream_prober = Arc::new(MockStreamProber { chapters: vec![] });
    let boxset_repo = Arc::new(InMemoryBoxSetRepo::default());

    let use_case = RipTvBatchUseCase::new(transcoder, notifier, guard, history.clone()).with_ports(
        tv_provider,
        stream_prober,
        boxset_repo.clone(),
    );

    let temp_dir = std::env::temp_dir().join(format!("lsp_tv_batch_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    let episodes = vec![
        TvEpisodeInfo {
            title_num: 1,
            episode_num: 1,
            duration_secs: 3180.0,
            formatted_name: "Game of Thrones S02E01".to_string(),
        },
        TvEpisodeInfo {
            title_num: 2,
            episode_num: 2,
            duration_secs: 3240.0,
            formatted_name: "Game of Thrones S02E02".to_string(),
        },
    ];

    let req = RipTvBatchRequest::builder("MOCK:\\", "Game of Thrones", &temp_dir)
        .season(2)
        .start_episode(1)
        .episodes(episodes)
        .file_extension("mkv")
        .lookup_episode_titles(true)
        .track_boxset(true)
        .min_free_gb(5)
        .build()
        .unwrap();

    let completed = use_case.execute(&req, None, None).unwrap();
    assert_eq!(completed, 2);
    // BoxSetRepository should have recorded 2 episodes so next episode is 3
    assert_eq!(boxset_repo.get_next_episode("Game of Thrones", 2), 3);
    assert_eq!(history.load_history(None).len(), 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

struct CustomPingEndpoint;

impl HttpEndpoint for CustomPingEndpoint {
    fn matches(&self, method: &str, path: &str) -> bool {
        method == "GET" && path == "/api/custom-ping"
    }

    fn handle(&self, _method: &str, _path: &str, _body: &str) -> (String, String, String) {
        (
            "200 OK".to_string(),
            "application/json".to_string(),
            r#"{"pong":true}"#.to_string(),
        )
    }
}

#[test]
fn test_ocp_http_router_and_sse_metrics_contracts() {
    let mut router = HttpRouter::new();
    router.register(Box::new(CustomPingEndpoint));

    // Custom endpoint works without modifying router internals
    let (status, content_type, body) = router
        .dispatch("GET", "/api/custom-ping", "")
        .expect("custom endpoint should match");
    assert_eq!(status, "200 OK");
    assert_eq!(content_type, "application/json");
    assert_eq!(body, r#"{"pong":true}"#);

    // Built-in Prometheus exporter and render_prometheus_metrics produce consistent metrics
    let exporter = PrometheusMetricsExporter;
    assert_eq!(exporter.render_metrics(), render_prometheus_metrics());

    // SSE broadcast engine delivers real payloads to subscribers
    let sse = SseBroadcastEngine::new();
    let rx = sse.subscribe();
    sse.broadcast_json_event(r#"{"status":"Halfway","progress":50.0}"#);
    let msg = rx.try_recv().unwrap();
    assert!(msg.starts_with("data: "));
    assert!(msg.contains("Halfway"));
}

