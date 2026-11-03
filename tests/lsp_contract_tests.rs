//! Liskov Substitution Principle (LSP / SOLID) Integration & Contract Test Suite.
//!
//! Validates that all ports, use cases, and infrastructure adapters satisfy behavioral subtyping:
//! interchangeable components must satisfy identical contracts without side effects or unexpected exceptions.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use dvd_ripper::AppContainer;
use dvd_ripper::domain::entities::error::DvdError;
use dvd_ripper::domain::entities::job::RipRecord;
use dvd_ripper::domain::ports::optical_drive::TrayController;
use dvd_ripper::domain::ports::post_processor::{PostProcessContext, PostProcessor};
use dvd_ripper::domain::ports::storage::{DiskSpaceGuard, HistoryRepository};
use dvd_ripper::domain::ports::transcoder::{ProgressObserver, TranscodeJob, Transcoder};
use dvd_ripper::infrastructure::optical::SystemTrayController;
use dvd_ripper::infrastructure::post_processors::PostProcessingPipeline;
use dvd_ripper::infrastructure::storage::{JsonHistoryRepository, SystemDiskSpaceGuard};

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
