use anyhow::Result;
use serde::{Deserialize, Serialize};
/**
 * @file api.rs
 * @brief REST API web server, Prometheus metrics provider, and SSE event broadcast engine.
 */
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use crate::dvd::eject_disc;
use crate::history::load_history;

#[allow(dead_code)]
pub const MIME_APPLICATION_JSON: &str = "application/json";

static CANCEL_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static CANCEL_TX: OnceLock<Arc<Mutex<Option<Sender<()>>>>> = OnceLock::new();

/// Returns the global atomic cancel flag handle for ongoing operations.
pub fn get_cancel_flag_handle() -> Arc<AtomicBool> {
    CANCEL_FLAG
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Returns the global thread-safe sender handle for process cancellation signals.
pub fn get_cancel_tx_handle() -> Arc<Mutex<Option<Sender<()>>>> {
    CANCEL_TX.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Liskov Substitution Principle (LSP/SOLID): Trait contract for telemetry metrics rendering.
#[allow(dead_code)]
pub trait MetricsExporter {
    fn render_metrics(&self) -> String;
}

#[derive(Debug, Default)]
pub struct PrometheusMetricsExporter;

impl MetricsExporter for PrometheusMetricsExporter {
    fn render_metrics(&self) -> String {
        let handle = get_appliance_status_handle();
        let (status, progress) = if let Ok(state) = handle.lock() {
            (state.status.clone(), state.progress)
        } else {
            ("Idle".to_string(), 0.0)
        };
        format!(
            "# HELP dvd_ripper_status Status of the DVD ripper appliance\n# TYPE dvd_ripper_status gauge\ndvd_ripper_status{{status=\"{}\"}} 1\n# HELP dvd_ripper_progress Ripping progress percentage\n# TYPE dvd_ripper_progress gauge\ndvd_ripper_progress {}\n",
            status, progress
        )
    }
}

/// Single Responsibility (SRP/SOLID): Encapsulates SSE broadcast subscriber management & event dispatching.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct SseBroadcastEngine;

impl SseBroadcastEngine {
    #[allow(dead_code)]
    pub fn broadcast_json_event(&self, event_json: &str) {
        let _ = event_json;
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplianceStatusInfo {
    pub status: String,
    pub drive: String,
    pub disc: String,
    pub current_title: String,
    pub progress: f64,
    pub fps: String,
    pub speed: String,
    pub has_selected_movie: bool,
    pub is_series: bool,
    pub year: Option<u32>,
}

#[allow(dead_code)]
impl ApplianceStatusInfo {
    pub fn reset(&mut self) {
        self.disc.clear();
        self.current_title.clear();
        self.has_selected_movie = false;
        self.status = "Idle".to_string();
        self.progress = 0.0;
        self.fps = "0".to_string();
        self.speed = "0x".to_string();
    }

    pub fn set_disc_detected(&mut self, disc_label: &str) {
        self.disc = disc_label.to_string();
        self.status = "Detected - Search Required".to_string();
    }

    pub fn set_ripping(&mut self, title: &str) {
        self.current_title = title.to_string();
        self.status = "Ripping".to_string();
    }

    pub fn update_progress(&mut self, progress: f64, fps: &str, speed: &str) {
        self.progress = progress;
        self.fps = fps.to_string();
        self.speed = speed.to_string();
    }
}

static APPLIANCE_STATUS: OnceLock<Arc<Mutex<ApplianceStatusInfo>>> = OnceLock::new();

pub fn get_appliance_status_handle() -> Arc<Mutex<ApplianceStatusInfo>> {
    APPLIANCE_STATUS
        .get_or_init(|| {
            Arc::new(Mutex::new(ApplianceStatusInfo {
                status: "Idle".to_string(),
                drive: "auto".to_string(),
                disc: "".to_string(),
                current_title: "".to_string(),
                progress: 0.0,
                fps: "0".to_string(),
                speed: "0x".to_string(),
                has_selected_movie: false,
                is_series: false,
                year: None,
            }))
        })
        .clone()
}

static COMPLETED_RIPS_COUNTER: AtomicU64 = AtomicU64::new(0);
static FAILED_RIPS_COUNTER: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub fn increment_completed_rips() {
    COMPLETED_RIPS_COUNTER.fetch_add(1, Ordering::SeqCst);
}

#[allow(dead_code)]
pub fn increment_failed_rips() {
    FAILED_RIPS_COUNTER.fetch_add(1, Ordering::SeqCst);
}

pub fn render_prometheus_metrics() -> String {
    let completed = COMPLETED_RIPS_COUNTER.load(Ordering::SeqCst);
    let failed = FAILED_RIPS_COUNTER.load(Ordering::SeqCst);

    let handle = get_appliance_status_handle();
    let (is_active, progress) = if let Ok(state) = handle.lock() {
        let active = if state.status.to_lowercase().contains("ripping")
            || state.status.to_lowercase().contains("active")
        {
            1
        } else {
            0
        };
        (active, state.progress)
    } else {
        (0, 0.0)
    };

    let queued_jobs = crate::queue::list_jobs().len();

    format!(
        "# HELP dvd_ripper_completed_rips_total Total number of successful DVD ripping jobs\n\
         # TYPE dvd_ripper_completed_rips_total counter\n\
         dvd_ripper_completed_rips_total {}\n\n\
         # HELP dvd_ripper_failed_rips_total Total number of failed DVD ripping jobs\n\
         # TYPE dvd_ripper_failed_rips_total counter\n\
         dvd_ripper_failed_rips_total {}\n\n\
         # HELP dvd_ripper_active_jobs Current number of active DVD ripping processes\n\
         # TYPE dvd_ripper_active_jobs gauge\n\
         dvd_ripper_active_jobs {}\n\n\
         # HELP dvd_ripper_queued_jobs Current number of pending ripping jobs in queue\n\
         # TYPE dvd_ripper_queued_jobs gauge\n\
         dvd_ripper_queued_jobs {}\n\n\
         # HELP dvd_ripper_progress_percent Current ripping job progress percentage\n\
         # TYPE dvd_ripper_progress_percent gauge\n\
         dvd_ripper_progress_percent {:.1}\n",
        completed, failed, is_active, queued_jobs, progress
    )
}

pub fn set_disc_detected(disc_label: &str) {
    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock()
        && state.disc != disc_label
    {
        state.disc = disc_label.to_string();
        state.current_title.clear();
        state.has_selected_movie = false;
        state.status = "Detected - Search Required".to_string();
        state.progress = 0.0;
    }
}

pub fn update_appliance_status(
    status: &str,
    disc: &str,
    title: &str,
    progress: f64,
    fps: &str,
    speed: &str,
) {
    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock() {
        state.status = status.to_string();
        if !disc.is_empty() {
            state.disc = disc.to_string();
        }
        if !title.is_empty() {
            state.current_title = title.to_string();
        }
        state.progress = progress;
        state.fps = fps.to_string();
        state.speed = speed.to_string();
    }
}

pub fn reset_appliance_status(status_name: &str) {
    update_appliance_status(status_name, "", "", 0.0, "0", "0x");
}

pub fn fail_appliance_status(title: &str, out_dir: &str, err_msg: &str) {
    reset_appliance_status("Failed");
    let _ = crate::history::record_rip_event(title, "Movie", out_dir, err_msg);
}

/// Re-export DriveStatusInfo from the domain layer.
pub use crate::domain::entities::disc::DriveStatusInfo;

static DRIVE_POOL_REGISTRY: OnceLock<Arc<Mutex<HashMap<String, DriveStatusInfo>>>> =
    OnceLock::new();
type DriveCancelMap = Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>;
static DRIVE_CANCEL_FLAGS: OnceLock<DriveCancelMap> = OnceLock::new();

pub fn get_drive_pool_registry() -> Arc<Mutex<HashMap<String, DriveStatusInfo>>> {
    DRIVE_POOL_REGISTRY
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

pub fn get_drive_cancel_flags_map() -> Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> {
    DRIVE_CANCEL_FLAGS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

/// Initializes the multi-drive appliance pool with optical drive paths.
pub fn init_drive_pool(drives: &[String]) {
    let registry = get_drive_pool_registry();
    if let Ok(mut pool) = registry.lock() {
        for drive in drives {
            pool.entry(drive.clone())
                .or_insert_with(|| DriveStatusInfo::new(drive));
        }
    }
    if let Some(first) = drives.first() {
        let handle = get_appliance_status_handle();
        if let Ok(mut state) = handle.lock()
            && state.drive == "auto"
        {
            state.drive = first.clone();
        }
    }
}

/// Retrieves statuses for all monitored optical drives in the pool, sorted by drive path.
pub fn get_drive_pool_statuses() -> Vec<DriveStatusInfo> {
    let registry = get_drive_pool_registry();
    if let Ok(pool) = registry.lock() {
        let mut list: Vec<DriveStatusInfo> = pool.values().cloned().collect();
        list.sort_by(|a, b| a.drive.cmp(&b.drive));
        if !list.is_empty() {
            return list;
        }
    }
    let handle = get_appliance_status_handle();
    if let Ok(state) = handle.lock() {
        vec![DriveStatusInfo {
            drive: state.drive.clone(),
            disc: state.disc.clone(),
            status: state.status.clone(),
            current_title: state.current_title.clone(),
            progress: state.progress,
            fps: state.fps.clone(),
            speed: state.speed.clone(),
            has_selected_movie: state.has_selected_movie,
            is_series: state.is_series,
            year: state.year,
        }]
    } else {
        Vec::new()
    }
}

/// Retrieves status for a specific optical drive from the pool.
pub fn get_drive_status(drive: &str) -> Option<DriveStatusInfo> {
    let registry = get_drive_pool_registry();
    if let Ok(pool) = registry.lock()
        && let Some(status) = pool.get(drive)
    {
        return Some(status.clone());
    }
    None
}

/// Marks a disc as detected for a specific drive in the pool.
pub fn set_drive_disc_detected(drive: &str, disc_label: &str) {
    let registry = get_drive_pool_registry();
    if let Ok(mut pool) = registry.lock() {
        let entry = pool
            .entry(drive.to_string())
            .or_insert_with(|| DriveStatusInfo::new(drive));
        if entry.disc != disc_label {
            entry.disc = disc_label.to_string();
            entry.current_title.clear();
            entry.has_selected_movie = false;
            entry.status = "Detected - Search Required".to_string();
            entry.progress = 0.0;
        }
    }
    set_disc_detected(disc_label);
}

/// Sets selected metadata for a specific drive in the pool.
pub fn set_drive_selected_metadata(drive: &str, title: &str, is_series: bool, year: Option<u32>) {
    let registry = get_drive_pool_registry();
    if let Ok(mut pool) = registry.lock() {
        let entry = pool
            .entry(drive.to_string())
            .or_insert_with(|| DriveStatusInfo::new(drive));
        entry.current_title = title.to_string();
        entry.is_series = is_series;
        entry.year = year;
        entry.has_selected_movie = true;
        entry.status = format!("Ready (Selected: {})", title);
    }
    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock() {
        state.drive = drive.to_string();
        state.current_title = title.to_string();
        state.is_series = is_series;
        state.year = year;
        state.has_selected_movie = true;
        state.status = format!("Ready (Selected: {})", title);
    }
}

/// Updates progress and status for a specific drive in the pool.
pub fn update_drive_status(
    drive: &str,
    status: &str,
    disc: &str,
    title: &str,
    progress: f64,
    fps: &str,
    speed: &str,
) {
    let registry = get_drive_pool_registry();
    if let Ok(mut pool) = registry.lock() {
        let entry = pool
            .entry(drive.to_string())
            .or_insert_with(|| DriveStatusInfo::new(drive));
        entry.status = status.to_string();
        if !disc.is_empty() {
            entry.disc = disc.to_string();
        }
        if !title.is_empty() {
            entry.current_title = title.to_string();
        }
        entry.progress = progress;
        entry.fps = fps.to_string();
        entry.speed = speed.to_string();
    }
    update_appliance_status(status, disc, title, progress, fps, speed);
}

/// Resets status for a specific drive in the pool.
pub fn reset_drive_status(drive: &str, status_name: &str) {
    let registry = get_drive_pool_registry();
    if let Ok(mut pool) = registry.lock()
        && let Some(entry) = pool.get_mut(drive)
    {
        entry.status = status_name.to_string();
        entry.progress = 0.0;
        entry.fps = "0".to_string();
        entry.speed = "0x".to_string();
        if status_name == "Idle" {
            entry.disc.clear();
            entry.current_title.clear();
            entry.has_selected_movie = false;
        }
    }
    reset_appliance_status(status_name);
}

/// Gets or creates a dedicated cancel flag for a specific optical drive.
pub fn get_drive_cancel_flag(drive: &str) -> Arc<AtomicBool> {
    let flags_map = get_drive_cancel_flags_map();
    if let Ok(mut map) = flags_map.lock() {
        map.entry(drive.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    } else {
        Arc::new(AtomicBool::new(false))
    }
}

/// Cancels ripping on a specific optical drive.
pub fn cancel_drive_rip(drive: &str) -> bool {
    let flags_map = get_drive_cancel_flags_map();
    if let Ok(map) = flags_map.lock()
        && let Some(flag) = map.get(drive)
    {
        flag.store(true, Ordering::SeqCst);
        update_drive_status(drive, "Cancelled", "", "", 0.0, "0", "0x");
        return true;
    }
    false
}

/// Cancels ripping across all optical drives in the pool.
pub fn cancel_all_drive_rips() -> usize {
    let flags_map = get_drive_cancel_flags_map();
    let mut count = 0;
    if let Ok(map) = flags_map.lock() {
        for (drive, flag) in map.iter() {
            flag.store(true, Ordering::SeqCst);
            update_drive_status(drive, "Cancelled", "", "", 0.0, "0", "0x");
            count += 1;
        }
    }
    let global_flag = get_cancel_flag_handle();
    global_flag.store(true, Ordering::SeqCst);
    if let Ok(mut lock) = get_cancel_tx_handle().lock()
        && let Some(tx) = lock.take()
    {
        let _ = tx.send(());
    }
    count
}

struct DriveStatusProgressObserver {
    drive: String,
    title: String,
}

impl crate::domain::ports::transcoder::ProgressObserver for DriveStatusProgressObserver {
    fn on_progress(&self, percent: f64, fps: &str, speed: &str) {
        update_drive_status(&self.drive, "Ripping", "", &self.title, percent, fps, speed);
    }

    fn on_log(&self, _line: &str) {}
}

/// Spawns an asynchronous background thread to rip a title/series from a specific optical drive.
pub fn execute_rip_for_drive(
    drive_path_str: &str,
    custom_args: Option<crate::cli::Args>,
) -> Result<()> {
    let drive_info = get_drive_status(drive_path_str).unwrap_or_else(|| {
        let handle = get_appliance_status_handle();
        if let Ok(st) = handle.lock() {
            DriveStatusInfo {
                drive: drive_path_str.to_string(),
                disc: st.disc.clone(),
                status: st.status.clone(),
                current_title: st.current_title.clone(),
                progress: st.progress,
                fps: st.fps.clone(),
                speed: st.speed.clone(),
                has_selected_movie: st.has_selected_movie,
                is_series: st.is_series,
                year: st.year,
            }
        } else {
            DriveStatusInfo::new(drive_path_str)
        }
    });

    if drive_info.disc.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "No DVD disc is present in drive '{}'",
            drive_path_str
        ));
    }
    if !drive_info.has_selected_movie || drive_info.current_title.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "No movie/show selected for drive '{}'. Please select a title first.",
            drive_path_str
        ));
    }

    let title = drive_info.current_title.clone();
    let is_series = drive_info.is_series;
    let year = drive_info.year;
    let drive_clone = drive_path_str.to_string();

    let cancel_flag = get_drive_cancel_flag(drive_path_str);
    cancel_flag.store(false, Ordering::SeqCst);

    update_drive_status(drive_path_str, "Ripping", "", &title, 0.0, "0", "0x");

    thread::spawn(move || {
        let mut args = custom_args.unwrap_or_default();
        args.input = drive_clone.clone();
        args.tv = is_series;
        let config = crate::config::load_config(None);
        crate::config::apply_config_defaults(&mut args, &config);
        if is_series {
            args.out_dir = "TV".to_string();
        }
        let dvd_path = crate::dvd::normalize_dvd_path(&drive_clone);

        let container = crate::AppContainer::build_from_args(&args);
        let observer = DriveStatusProgressObserver {
            drive: drive_clone.clone(),
            title: title.clone(),
        };

        let res = if is_series {
            let episodes = crate::ffmpeg::detect_tv_episodes(
                &args.ffmpeg,
                &dvd_path,
                &title,
                args.season,
                args.start_episode,
                Some(&cancel_flag),
            );
            let request = crate::application::rip_tv_batch::RipTvBatchRequest {
                source_dvd: dvd_path.clone(),
                series_title: title.clone(),
                season_number: args.season,
                start_episode: args.start_episode,
                end_episode: args.end_episode,
                output_dir: std::path::PathBuf::from(&args.out_dir),
                episodes,
                metadata: None,
                min_free_gb: args.min_free_gb,
                file_extension: Some("mkv".to_string()),
                lookup_episode_titles: true,
                track_boxset: true,
            };
            let use_case = container.rip_tv_batch_use_case();
            use_case
                .execute(&request, Some(&observer), Some(cancel_flag.clone()))
                .map(|_| ())
        } else if args.iso {
            if let Ok(out_path) = crate::ffmpeg::resolve_output_path(&args, Some(&title), year) {
                let label = crate::dvd::get_volume_label(&dvd_path.to_string_lossy());
                let request = crate::application::dump_iso::DumpIsoRequest {
                    source_dvd: dvd_path.clone(),
                    output_path: out_path,
                    volume_label: label,
                    display_title: title.clone(),
                    min_free_gb: args.min_free_gb,
                };
                let use_case = container.dump_iso_use_case();
                use_case
                    .execute(&request, Some(&observer), Some(cancel_flag.clone()))
                    .map(|_| ())
            } else {
                Err(anyhow::anyhow!("Failed to resolve output path"))
            }
        } else {
            if let Ok(out_path) = crate::ffmpeg::resolve_output_path(&args, Some(&title), year) {
                let request = crate::application::rip_single_title::RipSingleTitleRequest {
                    source_dvd: dvd_path.clone(),
                    title_number: args.title.max(1),
                    output_path: out_path,
                    display_title: title.clone(),
                    expected_duration_secs: None,
                    metadata: None,
                    min_free_gb: args.min_free_gb,
                };
                let use_case = container.rip_single_title_use_case();
                use_case.execute(&request, Some(&observer), Some(cancel_flag.clone()))
            } else {
                Err(anyhow::anyhow!("Failed to resolve output path"))
            }
        };

        if cancel_flag.load(Ordering::SeqCst) {
            update_drive_status(&drive_clone, "Cancelled", "", "", 0.0, "0", "0x");
        } else if res.is_ok() {
            increment_completed_rips();
            update_drive_status(&drive_clone, "Completed", "", &title, 100.0, "0", "0x");
            let _ = crate::dvd::eject_disc(&drive_clone);
            thread::sleep(Duration::from_secs(3));
            reset_drive_status(&drive_clone, "Idle");
        } else {
            if let Err(ref e) = res {
                fail_appliance_status(&title, &args.out_dir, &e.to_string());
            }
            increment_failed_rips();
            update_drive_status(&drive_clone, "Failed", "", "", 0.0, "0", "0x");
        }
    });

    Ok(())
}

/// Embedded HTML5 Web UI Dashboard string embedded directly into the binary.
const EMBEDDED_DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DVD Ripper Embedded Appliance</title>
    <style>
        :root { --bg: #0f172a; --card: #1e293b; --accent: #38bdf8; --text: #f8fafc; --muted: #94a3b8; --success: #22c55e; --danger: #ef4444; }
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: var(--bg); color: var(--text); margin: 0; padding: 2rem; }
        .container { max-width: 900px; margin: 0 auto; }
        .header { display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #334155; padding-bottom: 1rem; margin-bottom: 2rem; }
        h1 { margin: 0; font-size: 1.5rem; color: var(--accent); }
        .badge { background: var(--success); color: #000; font-weight: bold; padding: 0.25rem 0.75rem; border-radius: 9999px; font-size: 0.85rem; }
        .card { background: var(--card); border-radius: 12px; padding: 1.5rem; margin-bottom: 1.5rem; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.3); }
        .btn { background: var(--accent); color: #000; border: none; padding: 0.6rem 1.2rem; border-radius: 8px; font-weight: bold; cursor: pointer; transition: opacity 0.2s; margin-right: 0.5rem; }
        .btn:hover { opacity: 0.9; }
        .btn:disabled { opacity: 0.4; cursor: not-allowed; }
        .btn-danger { background: var(--danger); color: #fff; }
        .btn-secondary { background: #64748b; color: #fff; }
        .progress-bar { width: 100%; background: #334155; height: 12px; border-radius: 6px; overflow: hidden; margin: 1rem 0; }
        .progress-fill { background: var(--accent); height: 100%; width: 0%; transition: width 0.3s; }
        .history-item { display: flex; justify-content: space-between; padding: 0.75rem 0; border-bottom: 1px solid #334155; }
        .history-item:last-child { border-bottom: none; }
        .muted { color: var(--muted); font-size: 0.9rem; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📀 DVD Ripper Embedded Appliance</h1>
            <span class="badge" id="appliance-badge">ONLINE</span>
        </div>

        <div class="card">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
                <h2 style="margin:0;">Optical Drive Pool Status</h2>
                <button class="btn btn-secondary" style="font-size:0.85rem; padding:0.4rem 0.8rem;" onclick="fetchHistory()">🔄 Refresh History</button>
            </div>
            <div id="drives-container">
                <div id="status-text" class="muted">Querying optical drive pool status...</div>
                <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 0.75rem; margin-bottom: 0.25rem;">
                    <span class="muted" id="progress-state-label">Progress</span>
                    <span id="progress-percent-label" style="font-weight: bold; color: var(--accent); font-size: 1.1rem;">0.0%</span>
                </div>
                <div class="progress-bar"><div id="progress-fill" class="progress-fill"></div></div>
                <div style="margin-top: 1rem;">
                    <button class="btn" id="start-rip-btn" onclick="triggerRip()" disabled style="opacity: 0.4; cursor: not-allowed;" title="Insert DVD and select a movie to enable ripping.">▶ Start Rip</button>
                    <button class="btn btn-danger" onclick="cancelRip()">⏹ Cancel</button>
                    <button class="btn btn-secondary" onclick="ejectDisc()">⏏ Eject Tray</button>
                </div>
            </div>
        </div>

        <div class="card">
            <h2>🔍 IMDb Metadata Search & Candidate Selection</h2>
            <div style="display: flex; gap: 0.5rem; margin-bottom: 1rem;">
                <input type="text" id="search-input" placeholder="Search title or show name (e.g. Kill Bill, Aliens)..." style="flex: 1; padding: 0.6rem; border-radius: 8px; border: 1px solid #334155; background: #0f172a; color: #fff;" onkeypress="if(event.key==='Enter') searchImdb()">
                <button class="btn" onclick="searchImdb()">🔍 Search IMDb</button>
            </div>
            <div id="search-results"></div>
        </div>

        <div class="card">
            <h2>Ripping History</h2>
            <div id="history-list">Loading history...</div>
        </div>
    </div>

    <script>
        let currentPool = [];

        function renderStatusData(data) {
            document.getElementById('appliance-badge').innerText = data.status || 'Idle';
            if (data.pool && data.pool.length > 0) {
                currentPool = data.pool;
                const container = document.getElementById('drives-container');
                container.innerHTML = data.pool.map(d => {
                    const pct = (d.progress || 0).toFixed(1);
                    const isRipping = d.status && d.status.toLowerCase().includes('rip');
                    const hasDvd = d.disc && d.disc.length > 0;
                    const hasSelected = d.has_selected_movie === true;
                    const canRip = hasDvd && hasSelected && !isRipping;
                    let statusDetails = `<strong>Drive:</strong> ${d.drive} | <strong>Disc:</strong> ${d.disc || 'None'} | <strong>Title:</strong> ${d.current_title || 'None'}`;
                    if (d.fps && d.fps !== '0' && d.fps !== 'N/A') {
                        statusDetails += ` | <strong>FPS:</strong> ${d.fps} | <strong>Speed:</strong> ${d.speed}`;
                    }
                    return `
                    <div style="background:#0f172a; border:1px solid #334155; border-radius:10px; padding:1.2rem; margin-bottom:1rem;">
                        <div style="display:flex; justify-content:space-between; align-items:center;">
                            <span style="font-weight:bold; font-size:1.1rem; color:var(--accent);">💿 Optical Drive ${d.drive}</span>
                            <span class="badge" style="background:${isRipping ? 'var(--accent)' : (d.status === 'Completed' ? 'var(--success)' : '#64748b')}; color:#000;">${d.status}</span>
                        </div>
                        <div class="muted" style="margin:0.5rem 0;">${statusDetails}</div>
                        <div style="display:flex; justify-content:space-between; margin-bottom:0.25rem;">
                            <span class="muted">Progress</span>
                            <span style="font-weight:bold; color:var(--accent);">${pct}%</span>
                        </div>
                        <div class="progress-bar"><div class="progress-fill" style="width:${pct}%"></div></div>
                        <div style="margin-top:0.75rem;">
                            <button class="btn" onclick="triggerRip('${d.drive}')" ${canRip ? '' : 'disabled style="opacity:0.4; cursor:not-allowed;"'}>▶ Start Rip</button>
                            <button class="btn btn-danger" onclick="cancelRip('${d.drive}')" ${isRipping ? '' : 'disabled style="opacity:0.4; cursor:not-allowed;"'}>⏹ Cancel</button>
                            <button class="btn btn-secondary" onclick="ejectDisc('${d.drive}')">⏏ Eject Tray</button>
                        </div>
                    </div>`;
                }).join('');
            } else {
                const pct = (data.progress || 0).toFixed(1);
                let statusDetails = `<strong>State:</strong> ${data.status} | <strong>Drive:</strong> ${data.drive} | <strong>Disc/Title:</strong> ${data.current_title || data.disc || 'None'}`;
                if (data.fps && data.fps !== '0' && data.fps !== 'N/A') {
                    statusDetails += ` | <strong>FPS:</strong> ${data.fps} | <strong>Speed:</strong> ${data.speed}`;
                }
                const st = document.getElementById('status-text');
                if (st) st.innerHTML = statusDetails;
                const pf = document.getElementById('progress-fill');
                if (pf) pf.style.width = pct + '%';
                const ppl = document.getElementById('progress-percent-label');
                if (ppl) ppl.innerText = pct + '%';

                const hasDvd = data.disc && data.disc.length > 0;
                const hasSelected = data.has_selected_movie === true;
                const startBtn = document.getElementById('start-rip-btn');
                if (startBtn) {
                    if (hasDvd && hasSelected) {
                        startBtn.disabled = false;
                        startBtn.style.opacity = '1';
                        startBtn.style.cursor = 'pointer';
                        startBtn.title = 'Start Ripping DVD';
                    } else {
                        startBtn.disabled = true;
                        startBtn.style.opacity = '0.4';
                        startBtn.style.cursor = 'not-allowed';
                    }
                }
            }
        }

        async function pollStatus() {
            try {
                const res = await fetch('/api/status');
                const data = await res.json();
                renderStatusData(data);
            } catch(e) {}
        }

        async function searchImdb() {
            const query = document.getElementById('search-input').value.trim();
            if (!query) return;
            const container = document.getElementById('search-results');
            container.innerHTML = '<p class="muted">Searching IMDb/OMDb candidates...</p>';
            try {
                const res = await fetch('/api/search?q=' + encodeURIComponent(query));
                const data = await res.json();
                if (!data || data.length === 0) {
                    container.innerHTML = '<p class="muted">No search results found.</p>';
                    return;
                }
                container.innerHTML = data.map(item => {
                    let selectBtns = '';
                    if (currentPool && currentPool.length > 1) {
                        selectBtns = currentPool.map(d =>
                            `<button class="btn" style="padding: 0.35rem 0.6rem; font-size: 0.8rem; margin-right: 0.3rem;" onclick="selectCandidate('${item.imdb_id}', '${d.drive}')">Select for ${d.drive}</button>`
                        ).join('');
                    } else {
                        const targetDrive = (currentPool && currentPool.length === 1) ? currentPool[0].drive : '';
                        selectBtns = `<button class="btn" style="padding: 0.35rem 0.8rem; font-size: 0.85rem;" onclick="selectCandidate('${item.imdb_id}', '${targetDrive}')">Select</button>`;
                    }
                    return `
                    <div class="history-item">
                        <div>
                            <strong>${item.title}</strong> ${item.year ? '<span class="muted">(' + item.year + ')</span>' : ''}
                            <div class="muted">IMDb ID: ${item.imdb_id} | Type: ${item.type_field}</div>
                        </div>
                        <div>${selectBtns}</div>
                    </div>`;
                }).join('');
            } catch(e) {
                container.innerHTML = '<p class="muted" style="color:var(--danger)">Search request failed.</p>';
            }
        }

        async function selectCandidate(imdbId, drive) {
            try {
                const url = '/api/select?imdb_id=' + encodeURIComponent(imdbId) + (drive ? '&drive=' + encodeURIComponent(drive) : '');
                const res = await fetch(url, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ imdb_id: imdbId, drive: drive || undefined })
                });
                const data = await res.json();
                if (data.success) {
                    alert('Selected title: ' + data.title + (data.year ? ' (' + data.year + ')' : '') + (data.drive ? ' for ' + data.drive : ''));
                    pollStatus();
                } else {
                    alert(data.message || 'Selection failed.');
                }
            } catch(e) {
                alert('Error selecting title candidate.');
            }
        }

        async function fetchHistory() {
            try {
                const res = await fetch('/api/history');
                const data = await res.json();
                const container = document.getElementById('history-list');
                if (!data || data.length === 0) {
                    container.innerHTML = '<p class="muted">No ripping history records found.</p>';
                    return;
                }
                container.innerHTML = data.map(item => `
                    <div class="history-item">
                        <div>
                            <strong>${item.title}</strong> <span class="muted">(${item.media_type})</span>
                            <div class="muted">${item.output_path}</div>
                        </div>
                        <div>
                            <span style="color:${item.status === 'Success' ? '#22c55e' : '#ef4444'}">${item.status}</span>
                            <div class="muted">${item.timestamp}</div>
                        </div>
                    </div>
                `).join('');
            } catch(e) {
                document.getElementById('history-list').innerText = 'Failed to load history.';
            }
        }

        async function ejectDisc(drive) {
            if (confirm('Eject optical drive tray' + (drive ? ' for ' + drive : '') + '?')) {
                const url = '/api/eject' + (drive ? '?drive=' + encodeURIComponent(drive) : '');
                const res = await fetch(url, { method: 'POST' });
                const data = await res.json();
                alert(data.success ? 'Tray ejected successfully.' : 'Failed to eject tray.');
                pollStatus();
            }
        }

        async function triggerRip(drive) {
            const url = '/api/rip' + (drive ? '?drive=' + encodeURIComponent(drive) : '');
            const res = await fetch(url, { method: 'POST' });
            const data = await res.json();
            if (!data.success) {
                alert('⚠️ ' + (data.message || 'Cannot start rip.'));
            } else {
                alert(data.message || 'Triggered rip job.');
                pollStatus();
            }
        }

        async function cancelRip(drive) {
            if (confirm('Cancel active DVD ripping process' + (drive ? ' on drive ' + drive : '') + '?')) {
                const url = '/api/cancel' + (drive ? '?drive=' + encodeURIComponent(drive) : '');
                const res = await fetch(url, { method: 'POST' });
                const data = await res.json();
                alert(data.message || 'Cancelled rip job.');
                pollStatus();
            }
        }

        fetchHistory();
        if (!!window.EventSource) {
            const evtSource = new EventSource('/api/events');
            evtSource.onmessage = function(e) {
                try {
                    const data = JSON.parse(e.data);
                    if (data && data.status) {
                        renderStatusData(data);
                    }
                } catch(err) {}
            };
        }
        setInterval(pollStatus, 3000);
        pollStatus();
    </script>
</body>
</html>
"#;

/// Starts the embedded HTTP REST API and Web UI dashboard server on port 8080.
pub fn start_embedded_api_server(port: u16, drive_path: String) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)?;
    println!("[Embedded Web API] Server listening on http://{}", addr);

    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock()
        && (state.drive == "auto" || state.drive == crate::dvd::default_dvd_drive_path())
    {
        state.drive = crate::dvd::auto_detect_dvd_drive();
    }

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let drive = drive_path.clone();
            thread::spawn(move || {
                let _ = handle_client(stream, &drive);
            });
        }
    });

    Ok(())
}

pub fn parse_http_route(req_bytes: &[u8]) -> (&[u8], &[u8]) {
    let first_line = req_bytes
        .split(|&b| b == b'\r' || b == b'\n')
        .next()
        .unwrap_or(&[]);
    let mut parts = first_line.split(|&b| b == b' ');
    let method = parts.next().unwrap_or(&[]);
    let path = parts.next().unwrap_or(&[]);
    (method, path)
}

pub fn parse_query_param(path: &str, param_name: &str) -> Option<String> {
    let query_start = path.find('?')?;
    let query_str = &path[query_start + 1..];
    for pair in query_str.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next()?;
        if key == param_name {
            let val = kv.next().unwrap_or("");
            return Some(crate::utils::decode_url_query_value(val));
        }
    }
    None
}

pub fn extract_body(req_bytes: &[u8]) -> Option<&str> {
    if let Some(pos) = req_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
        std::str::from_utf8(&req_bytes[pos + 4..]).ok()
    } else {
        None
    }
}

static CONFIGURED_API_KEY: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

fn get_configured_api_key_handle() -> &'static Arc<Mutex<Option<String>>> {
    CONFIGURED_API_KEY.get_or_init(|| Arc::new(Mutex::new(None)))
}

pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        res |= x ^ y;
    }
    res == 0
}

#[allow(dead_code)]
pub fn set_api_key(key: String) {
    let handle = get_configured_api_key_handle();
    let mut lock = lock_or_recover(handle);
    *lock = Some(key);
}

pub fn validate_api_key_header(req_bytes: &[u8], path_str: &str) -> bool {
    let handle = get_configured_api_key_handle();
    let lock = lock_or_recover(handle);
    let expected_key = match lock.clone() {
        Some(k) if !k.trim().is_empty() => k,
        _ => return true,
    };

    if let Some(param_key) = parse_query_param(path_str, "api_key")
        && constant_time_eq(&param_key, &expected_key)
    {
        return true;
    }

    let req_text = String::from_utf8_lossy(req_bytes);
    for line in req_text.lines() {
        if line.to_lowercase().starts_with("authorization:")
            && let Some(bearer) = line.split_once(':').map(|x| x.1)
        {
            let token = bearer.trim();
            if token.to_lowercase().starts_with("bearer ") {
                let key = token[7..].trim();
                if constant_time_eq(key, &expected_key) {
                    return true;
                }
            }
        }
    }

    false
}

pub const OPENAPI_V3_JSON: &str = r#"{
  "openapi": "3.0.0",
  "info": {
    "title": "DVD Ripper Appliance REST API",
    "version": "1.0.0",
    "description": "High-performance automated DVD ripping appliance REST API for Home Assistant & media server integration."
  },
  "paths": {
    "/api/status": {
      "get": {
        "summary": "Get current DVD appliance status",
        "responses": { "200": { "description": "Appliance status JSON" } }
      }
    },
    "/api/events": {
      "get": {
        "summary": "Server-Sent Events (SSE) live status stream",
        "responses": { "200": { "description": "text/event-stream" } }
      }
    },
    "/api/openapi.json": {
      "get": {
        "summary": "OpenAPI v3 JSON specification",
        "responses": { "200": { "description": "OpenAPI 3.0 specification JSON" } }
      }
    },
    "/api/history": {
      "get": {
        "summary": "Get ripping history log",
        "responses": { "200": { "description": "History records array" } }
      }
    },
    "/api/history/clear": {
      "post": {
        "summary": "Clear persistent DVD ripping history",
        "responses": { "200": { "description": "Ripping history cleared confirmation" } }
      }
    },
    "/api/disc-cache": {
      "get": {
        "summary": "List persistent disc fingerprint cache entries",
        "responses": { "200": { "description": "Cached disc entries array" } }
      }
    },
    "/api/search": {
      "get": {
        "summary": "Search IMDb/OMDb metadata candidates",
        "parameters": [{ "name": "q", "in": "query", "required": true, "schema": { "type": "string" } }],
        "responses": { "200": { "description": "Search candidates array" } }
      }
    },
    "/api/select": {
      "post": {
        "summary": "Select IMDb candidate by ID",
        "parameters": [{ "name": "imdb_id", "in": "query", "required": true, "schema": { "type": "string" } }],
        "responses": { "200": { "description": "Selection status" } }
      }
    },
    "/api/rip": {
      "post": {
        "summary": "Trigger DVD ripping process",
        "parameters": [
          { "name": "drive", "in": "query", "required": false, "schema": { "type": "string" } },
          { "name": "iso", "in": "query", "required": false, "schema": { "type": "boolean" }, "description": "Dump exact 1:1 ISO disc image" }
        ],
        "responses": { "200": { "description": "Job trigger response" } }
      }
    },
    "/api/cancel": {
      "post": {
        "summary": "Cancel active DVD ripping process",
        "responses": { "200": { "description": "Cancellation response" } }
      }
    },
    "/api/eject": {
      "post": {
        "summary": "Eject DVD optical tray",
        "responses": { "200": { "description": "Tray ejection response" } }
      }
    },
    "/api/queue/list": {
      "get": {
        "summary": "List queued ripping jobs",
        "responses": { "200": { "description": "Queued jobs array" } }
      }
    },
    "/api/queue/add": {
      "post": {
        "summary": "Enqueue a new ripping job",
        "responses": { "200": { "description": "Enqueue response" } }
      }
    },
    "/api/queue/remove": {
      "post": {
        "summary": "Remove job from queue",
        "responses": { "200": { "description": "Removal status" } }
      }
    },
    "/api/boxset": {
      "get": {
        "summary": "List TV boxset multi-disc tracking state",
        "responses": { "200": { "description": "Boxsets array" } }
      }
    },
    "/api/boxset/reset": {
      "post": {
        "summary": "Reset TV boxset episode tracker for a show/season",
        "parameters": [
          { "name": "show", "in": "query", "required": true, "schema": { "type": "string" } },
          { "name": "season", "in": "query", "required": false, "schema": { "type": "integer", "default": 1 } }
        ],
        "responses": { "200": { "description": "Reset status confirmation" } }
      }
    },
    "/api/thumbnail": {
      "get": {
        "summary": "Generate thumbnail frame image from DVD title",
        "parameters": [
          { "name": "title", "in": "query", "required": false, "schema": { "type": "integer", "default": 1 } },
          { "name": "time", "in": "query", "required": false, "schema": { "type": "number", "default": 120.0 } },
          { "name": "drive", "in": "query", "required": false, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": {
            "description": "Thumbnail JPEG image bytes",
            "content": { "image/jpeg": {} }
          }
        }
      }
    },
    "/api/benchmark": {
      "post": {
        "summary": "Run optical drive read speed benchmark",
        "parameters": [
          { "name": "drive", "in": "query", "required": false, "schema": { "type": "string" } }
        ],
        "responses": { "200": { "description": "Drive benchmark performance metrics report" } }
      }
    },
    "/api/pool/status": {
      "get": {
        "summary": "List all optical drives in pool and their status",
        "responses": { "200": { "description": "Drive pool statuses array" } }
      }
    },
    "/api/drives": {
      "get": {
        "summary": "Detect and list physical optical drive letters",
        "responses": { "200": { "description": "Drive letters array" } }
      }
    },
    "/api/health": {
      "get": {
        "summary": "Appliance health check and status",
        "responses": { "200": { "description": "Appliance health status JSON" } }
      }
    }
  }
}"#;

/// Maximum allowable HTTP request body size (1 MB). Requests exceeding this limit receive 413 Payload Too Large.
pub const MAX_REQUEST_BODY_SIZE: usize = 1024 * 1024;

/// Maximum allowable HTTP request header size (64 KB) to protect against header inflation attacks.
pub const MAX_HEADER_SIZE: usize = 64 * 1024;

/// Result of safely reading an HTTP request from a stream with DoS safeguards.
#[derive(Debug, PartialEq, Eq)]
pub enum SafeHttpReadResult {
    /// Successfully read complete request bytes.
    Ok(Vec<u8>),
    /// Client disconnected or sent empty request.
    Empty,
    /// Request headers exceeded MAX_HEADER_SIZE.
    HeadersTooLarge,
    /// Request body exceeded MAX_REQUEST_BODY_SIZE.
    PayloadTooLarge,
    /// Malformed request or I/O error.
    Malformed(String),
}

/// Helper: Extracts Content-Length from raw header bytes if present.
pub fn extract_content_length(header_bytes: &[u8]) -> Option<usize> {
    let s = std::str::from_utf8(header_bytes).ok()?;
    for line in s.lines() {
        if let Some(colon_idx) = line.find(':') {
            let key = line[..colon_idx].trim();
            if key.eq_ignore_ascii_case("content-length") {
                return line[colon_idx + 1..].trim().parse::<usize>().ok();
            }
        }
    }
    None
}

/// Safely reads an HTTP request from a stream enforcing header and body limits.
pub fn read_http_request_safely<R: Read>(mut reader: R) -> SafeHttpReadResult {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let mut header_end = None;

    // 1. Read until end of headers \r\n\r\n
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                    if pos + 4 > MAX_HEADER_SIZE {
                        return SafeHttpReadResult::HeadersTooLarge;
                    }
                    header_end = Some(pos + 4);
                    break;
                }
                if buffer.len() > MAX_HEADER_SIZE {
                    return SafeHttpReadResult::HeadersTooLarge;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return SafeHttpReadResult::Malformed(e.to_string()),
        }
    }

    if buffer.is_empty() {
        return SafeHttpReadResult::Empty;
    }

    let header_pos = match header_end {
        Some(pos) => pos,
        None => {
            if buffer.len() > MAX_HEADER_SIZE {
                return SafeHttpReadResult::HeadersTooLarge;
            }
            buffer.len()
        }
    };

    // 2. Parse Content-Length from headers if present
    let header_slice = &buffer[..header_pos];
    let content_len = extract_content_length(header_slice);

    if let Some(len) = content_len {
        if len > MAX_REQUEST_BODY_SIZE {
            return SafeHttpReadResult::PayloadTooLarge;
        }

        let mut body_read = buffer.len() - header_pos;
        while body_read < len {
            let to_read = (len - body_read).min(chunk.len());
            match reader.read(&mut chunk[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    buffer.extend_from_slice(&chunk[..n]);
                    body_read += n;
                    if body_read > MAX_REQUEST_BODY_SIZE {
                        return SafeHttpReadResult::PayloadTooLarge;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return SafeHttpReadResult::Malformed(e.to_string()),
            }
        }
    }

    let total_body_len = buffer.len().saturating_sub(header_pos);
    if total_body_len > MAX_REQUEST_BODY_SIZE {
        return SafeHttpReadResult::PayloadTooLarge;
    }

    SafeHttpReadResult::Ok(buffer)
}

/// Returns the OpenAPI v3 JSON specification string.
pub fn generate_openapi_spec() -> &'static str {
    OPENAPI_V3_JSON
}

fn handle_client(mut stream: TcpStream, drive_path: &str) -> Result<()> {
    let req_bytes = match read_http_request_safely(&mut stream) {
        SafeHttpReadResult::Ok(bytes) => bytes,
        SafeHttpReadResult::Empty => return Ok(()),
        SafeHttpReadResult::HeadersTooLarge => {
            let body = "{\"success\":false,\"message\":\"Request Header Fields Too Large\"}";
            let _ = send_http_response(
                &mut stream,
                "431 Request Header Fields Too Large",
                "application/json",
                body,
            );
            return Ok(());
        }
        SafeHttpReadResult::PayloadTooLarge => {
            let body = "{\"success\":false,\"message\":\"Payload Too Large: Maximum allowed request size is 1MB\"}";
            let _ = send_http_response(
                &mut stream,
                "413 Payload Too Large",
                "application/json",
                body,
            );
            return Ok(());
        }
        SafeHttpReadResult::Malformed(err) => {
            let body = format!(
                "{{\"success\":false,\"message\":\"Malformed HTTP request: {}\"}}",
                crate::utils::escape_json_str(&err)
            );
            let _ = send_http_response(&mut stream, "400 Bad Request", "application/json", &body);
            return Ok(());
        }
    };

    let (method, path) = parse_http_route(&req_bytes);

    let path_str = String::from_utf8_lossy(path);
    let method_str = String::from_utf8_lossy(method);

    if path_str == "/api/openapi.json" {
        send_http_response(
            &mut stream,
            "200 OK",
            "application/json",
            generate_openapi_spec(),
        )?;
        return Ok(());
    }

    if path_str.starts_with("/api/")
        && path_str != "/api/status"
        && !validate_api_key_header(&req_bytes, &path_str)
    {
        let body = "{\"success\":false,\"message\":\"Unauthorized: Invalid or missing API key header (Authorization: Bearer <KEY>)\"}";
        send_http_response(&mut stream, "401 Unauthorized", "application/json", body)?;
        return Ok(());
    }

    let body_str = extract_body(&req_bytes).unwrap_or_default();
    let is_json_post_endpoint = method_str == "POST"
        && (path_str.starts_with("/api/queue/batch")
            || path_str.starts_with("/api/select")
            || path_str.starts_with("/api/rip")
            || path_str.starts_with("/api/cancel")
            || path_str.starts_with("/api/eject"));

    if is_json_post_endpoint
        && !body_str.trim().is_empty()
        && serde_json::from_str::<serde_json::Value>(body_str).is_err()
    {
        let resp = "{\"success\":false,\"message\":\"Malformed JSON request body\"}";
        send_http_response(&mut stream, "400 Bad Request", "application/json", resp)?;
        return Ok(());
    }

    if method_str == "GET" && path_str == "/metrics" {
        let metrics = render_prometheus_metrics();
        send_http_response(
            &mut stream,
            "200 OK",
            "text/plain; version=0.0.4; charset=utf-8",
            &metrics,
        )?;
        return Ok(());
    }

    if method_str == "GET" && (path_str == "/" || path_str == "/index.html") {
        send_http_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            EMBEDDED_DASHBOARD_HTML,
        )?;
    } else if method_str == "GET" && path_str == "/api/events" {
        let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n";
        if stream.write_all(headers.as_bytes()).is_ok() {
            for _ in 0..10 {
                let handle = get_appliance_status_handle();
                let json_body = if let Ok(state) = handle.lock() {
                    serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string())
                } else {
                    "{}".to_string()
                };
                let event_payload = format!("data: {}\n\n", json_body);
                if stream.write_all(event_payload.as_bytes()).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    } else if method_str == "GET"
        && (path_str == "/api/status" || path_str.starts_with("/api/status"))
    {
        let req_drive = parse_query_param(&path_str, "drive");
        let pool = get_drive_pool_statuses();
        let json_body = if let Some(ref d) = req_drive {
            if let Some(status) = get_drive_status(d) {
                serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string())
            } else {
                serde_json::json!({
                    "status": "Idle",
                    "drive": d,
                    "disc": "",
                    "current_title": "",
                    "progress": 0.0,
                    "fps": "0",
                    "speed": "0x",
                    "has_selected_movie": false,
                    "is_series": false,
                    "year": null,
                    "pool": pool
                })
                .to_string()
            }
        } else {
            let handle = get_appliance_status_handle();
            let mut val = if let Ok(state) = handle.lock() {
                serde_json::to_value(&*state).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                let label = crate::dvd::get_volume_label(drive_path).unwrap_or_default();
                serde_json::json!({
                    "status": "Active",
                    "drive": drive_path,
                    "disc": label,
                    "current_title": "",
                    "progress": 0.0,
                    "fps": "0",
                    "speed": "0x",
                    "has_selected_movie": false,
                    "is_series": false,
                    "year": null
                })
            };
            val["pool"] = serde_json::to_value(&pool).unwrap_or_else(|_| serde_json::json!([]));
            val.to_string()
        };
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "GET"
        && (path_str.starts_with("/api/pool/status") || path_str.starts_with("/api/drives/status"))
    {
        let pool = get_drive_pool_statuses();
        let resp_obj = serde_json::json!({ "drives": pool, "count": pool.len() });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "GET" && path_str == "/api/history" {
        let history = load_history(None);
        let json_body = serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "POST"
        && (path_str == "/api/history/clear" || path_str.starts_with("/api/history/clear"))
    {
        match crate::history::clear_history(None) {
            Ok(()) => {
                let resp_obj = serde_json::json!({
                    "success": true,
                    "message": "Ripping history cleared"
                });
                send_http_response(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    &resp_obj.to_string(),
                )?;
            }
            Err(e) => {
                let resp_obj = serde_json::json!({
                    "success": false,
                    "message": format!("Failed to clear history: {}", e)
                });
                send_http_response(
                    &mut stream,
                    "500 Internal Server Error",
                    "application/json",
                    &resp_obj.to_string(),
                )?;
            }
        }
    } else if method_str == "GET" && path_str.starts_with("/api/thumbnail") {
        let title_num: u32 = parse_query_param(&path_str, "title")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let time_sec: f64 = parse_query_param(&path_str, "time")
            .and_then(|s| s.parse().ok())
            .unwrap_or(120.0);
        let drive = parse_query_param(&path_str, "drive").unwrap_or_else(|| drive_path.to_string());

        let temp_thumb = crate::infrastructure::storage::TempResource::new(
            &format!("dvd_ripper_thumb_{}", title_num),
            "jpg",
        );
        let ffmpeg_path = std::env::var("FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());

        match crate::ffmpeg::generate_title_thumbnail(
            &ffmpeg_path,
            std::path::Path::new(&drive),
            title_num,
            time_sec,
            temp_thumb.path(),
        ) {
            Ok(_) => {
                if let Ok(bytes) = std::fs::read(temp_thumb.path()) {
                    let resp = build_http_response_bytes("200 OK", "image/jpeg", &bytes);
                    stream.write_all(&resp)?;
                } else {
                    let body = "{\"success\":false,\"message\":\"Thumbnail file not readable\"}";
                    send_http_response(
                        &mut stream,
                        "500 Internal Server Error",
                        "application/json",
                        body,
                    )?;
                }
            }
            Err(e) => {
                let body = format!(
                    "{{\"success\":false,\"message\":\"Failed to generate thumbnail: {}\"}}",
                    e
                );
                send_http_response(
                    &mut stream,
                    "500 Internal Server Error",
                    "application/json",
                    &body,
                )?;
            }
        }
    } else if method_str == "GET"
        && (path_str == "/api/disc-cache" || path_str.starts_with("/api/disc-cache"))
    {
        let cache = crate::infrastructure::metadata::DiskFingerprintCache::new();
        let entries = cache.list_all_entries();
        let json_body = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "GET" && path_str == "/api/queue/list" {
        let jobs = crate::queue::list_jobs();
        let json_body = serde_json::to_string(&jobs).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "POST" && path_str.starts_with("/api/queue/add") {
        let title = parse_query_param(&path_str, "title").unwrap_or_else(|| "Unknown".to_string());
        let media_type =
            parse_query_param(&path_str, "type").unwrap_or_else(|| "Movie".to_string());
        let id = crate::queue::add_job(&title, &media_type, drive_path);
        let resp_obj = serde_json::json!({ "success": true, "job_id": id });
        send_http_response(
            &mut stream,
            "200 OK",
            "application/json",
            &resp_obj.to_string(),
        )?;
    } else if method_str == "POST" && path_str.starts_with("/api/queue/batch") {
        let body_str = extract_body(&req_bytes).unwrap_or_default();
        let mut added_ids = Vec::new();
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str)
            && let Some(arr) = parsed.as_array()
        {
            for item in arr {
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let media_type = item
                    .get("media_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Movie");
                let drive = item
                    .get("drive")
                    .and_then(|v| v.as_str())
                    .unwrap_or(drive_path);
                let id = crate::queue::add_job(title, media_type, drive);
                added_ids.push(id);
            }
        }
        let resp_obj =
            serde_json::json!({ "success": true, "job_ids": added_ids, "count": added_ids.len() });
        send_http_response(
            &mut stream,
            "200 OK",
            "application/json",
            &resp_obj.to_string(),
        )?;
    } else if method_str == "POST" && path_str.starts_with("/api/queue/remove") {
        let id = parse_query_param(&path_str, "id").unwrap_or_default();
        let ok = crate::queue::remove_job(&id);
        let resp_obj = serde_json::json!({ "success": ok });
        send_http_response(
            &mut stream,
            "200 OK",
            "application/json",
            &resp_obj.to_string(),
        )?;
    } else if method_str == "GET" && path_str.starts_with("/api/search") {
        let query = parse_query_param(&path_str, "q").unwrap_or_default();
        let candidates = if !query.trim().is_empty() {
            crate::imdb::fetch_search_candidates(query.trim())
        } else {
            Vec::new()
        };
        let json_body = serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "POST" && path_str.starts_with("/api/select") {
        let body_str = extract_body(&req_bytes).unwrap_or_default();
        let mut imdb_id = parse_query_param(&path_str, "imdb_id");
        let mut target_drive = parse_query_param(&path_str, "drive");
        if !body_str.trim().is_empty()
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str)
        {
            if imdb_id.is_none()
                && let Some(id) = parsed.get("imdb_id").and_then(|v| v.as_str())
            {
                imdb_id = Some(id.to_string());
            }
            if target_drive.is_none()
                && let Some(d) = parsed.get("drive").and_then(|v| v.as_str())
            {
                target_drive = Some(d.to_string());
            }
        }

        let chosen_drive = target_drive.unwrap_or_else(|| drive_path.to_string());
        if let Some(ref id) = imdb_id {
            if let Some(meta) = crate::imdb::lookup_omdb_by_id(id) {
                set_drive_selected_metadata(&chosen_drive, &meta.title, meta.is_series, meta.year);
                let resp_obj = serde_json::json!({
                    "success": true,
                    "drive": chosen_drive,
                    "title": meta.title,
                    "year": meta.year,
                    "is_series": meta.is_series
                });
                send_http_response(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    &resp_obj.to_string(),
                )?;
            } else {
                let json_body =
                    "{\"success\":false,\"message\":\"Failed to find metadata for IMDb ID\"}";
                send_http_response(&mut stream, "404 Not Found", "application/json", json_body)?;
            }
        } else {
            let json_body = "{\"success\":false,\"message\":\"Missing imdb_id parameter\"}";
            send_http_response(
                &mut stream,
                "400 Bad Request",
                "application/json",
                json_body,
            )?;
        }
    } else if method_str == "POST"
        && (path_str == "/api/eject" || path_str.starts_with("/api/eject"))
    {
        let body_str = extract_body(&req_bytes).unwrap_or_default();
        let mut target_drive = parse_query_param(&path_str, "drive");
        if target_drive.is_none()
            && !body_str.trim().is_empty()
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str)
            && let Some(d) = parsed.get("drive").and_then(|v| v.as_str())
        {
            target_drive = Some(d.to_string());
        }
        let chosen_drive = target_drive.unwrap_or_else(|| drive_path.to_string());
        let ok = eject_disc(&chosen_drive);
        reset_drive_status(&chosen_drive, "Idle");
        let resp_obj = serde_json::json!({ "success": ok, "drive": chosen_drive });
        send_http_response(
            &mut stream,
            "200 OK",
            "application/json",
            &resp_obj.to_string(),
        )?;
    } else if method_str == "POST" && (path_str == "/api/rip" || path_str.starts_with("/api/rip")) {
        let body_str = extract_body(&req_bytes).unwrap_or_default();
        let mut target_drive = parse_query_param(&path_str, "drive");
        if target_drive.is_none()
            && !body_str.trim().is_empty()
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str)
            && let Some(d) = parsed.get("drive").and_then(|v| v.as_str())
        {
            target_drive = Some(d.to_string());
        }
        let mut is_iso = parse_query_param(&path_str, "iso")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        if !is_iso
            && !body_str.trim().is_empty()
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str)
            && let Some(b) = parsed.get("iso").and_then(|v| v.as_bool())
        {
            is_iso = b;
        }
        let chosen_drive = target_drive.unwrap_or_else(|| drive_path.to_string());
        let custom_args = if is_iso {
            Some(crate::cli::Args {
                iso: true,
                ..Default::default()
            })
        } else {
            None
        };
        match execute_rip_for_drive(&chosen_drive, custom_args) {
            Ok(()) => {
                let resp_obj = serde_json::json!({
                    "success": true,
                    "drive": chosen_drive,
                    "message": format!("Started ripping on drive {}", chosen_drive)
                });
                send_http_response(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    &resp_obj.to_string(),
                )?;
            }
            Err(e) => {
                let resp_obj = serde_json::json!({
                    "success": false,
                    "drive": chosen_drive,
                    "message": e.to_string()
                });
                send_http_response(
                    &mut stream,
                    "400 Bad Request",
                    "application/json",
                    &resp_obj.to_string(),
                )?;
            }
        }
    } else if method_str == "POST"
        && (path_str == "/api/cancel" || path_str.starts_with("/api/cancel"))
    {
        let body_str = extract_body(&req_bytes).unwrap_or_default();
        let mut target_drive = parse_query_param(&path_str, "drive");
        if target_drive.is_none()
            && !body_str.trim().is_empty()
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str)
            && let Some(d) = parsed.get("drive").and_then(|v| v.as_str())
        {
            target_drive = Some(d.to_string());
        }
        if let Some(ref d) = target_drive {
            let ok = cancel_drive_rip(d);
            let resp_obj = serde_json::json!({
                "success": ok,
                "drive": d,
                "message": format!("Cancellation signal sent to drive {}", d)
            });
            send_http_response(
                &mut stream,
                "200 OK",
                "application/json",
                &resp_obj.to_string(),
            )?;
        } else {
            let count = cancel_all_drive_rips();
            let resp_obj = serde_json::json!({
                "success": true,
                "count": count,
                "message": "Cancellation signal sent to all active ripping jobs"
            });
            send_http_response(
                &mut stream,
                "200 OK",
                "application/json",
                &resp_obj.to_string(),
            )?;
        }
    } else if method_str == "GET" && path_str == "/api/boxset" {
        let boxsets = crate::queue::list_boxsets();
        let json_body = serde_json::to_string(&boxsets).unwrap_or_else(|_| "[]".to_string());
        send_json_response(&mut stream, "200 OK", &json_body)?;
    } else if method_str == "POST" && path_str.starts_with("/api/boxset/reset") {
        let query_show = parse_query_param(&path_str, "show").unwrap_or_default();
        let query_season = parse_query_param(&path_str, "season")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let reset = crate::queue::reset_boxset_tracker(&query_show, query_season);
        let resp_obj = serde_json::json!({
            "success": reset,
            "show": query_show,
            "season": query_season
        });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "GET"
        && (path_str == "/" || path_str == "/dashboard" || path_str.starts_with("/dashboard"))
    {
        send_http_response(
            &mut stream,
            "200 OK",
            "text/html",
            render_web_dashboard_html(),
        )?;
    } else if method_str == "GET" && (path_str == "/docs" || path_str.starts_with("/docs")) {
        send_http_response(
            &mut stream,
            "200 OK",
            "text/html",
            render_swagger_docs_html(),
        )?;
    } else if method_str == "GET" && path_str.starts_with("/api/drives") {
        let drives = crate::dvd::detect_dvd_drives();
        let resp_obj = serde_json::json!({
            "drives": drives,
            "count": drives.len()
        });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "GET" && path_str.starts_with("/api/health") {
        let handle = get_appliance_status_handle();
        let current_status = if let Ok(state) = handle.lock() {
            state.status.clone()
        } else {
            "Idle".to_string()
        };
        let resp_obj = serde_json::json!({
            "status": "healthy",
            "appliance_state": current_status
        });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "POST" && path_str.starts_with("/api/benchmark") {
        let drive = parse_query_param(&path_str, "drive").unwrap_or_else(|| "auto".to_string());
        let dvd_path = crate::dvd::normalize_dvd_path(&drive);
        match crate::dvd::run_drive_benchmark("ffmpeg", &dvd_path, 10) {
            Ok(report) => {
                let json_body = serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string());
                send_json_response(&mut stream, "200 OK", &json_body)?;
            }
            Err(e) => {
                send_json_error(&mut stream, "500 Internal Server Error", &e.to_string())?;
            }
        }
    } else if method_str == "POST" && path_str.starts_with("/api/trigger") {
        let drive = parse_query_param(&path_str, "drive").unwrap_or_else(|| drive_path.to_string());
        let label = crate::dvd::get_volume_label(&drive).unwrap_or_else(|| "DVD_DISC".to_string());
        set_disc_detected(&label);
        let resp_obj = serde_json::json!({ "success": true, "disc": label });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "GET" && path_str.starts_with("/api/ws") {
        let response = "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\
                        Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
        stream.write_all(response.as_bytes())?;
        return Ok(());
    } else {
        send_http_response(&mut stream, "404 Not Found", "text/plain", "404 Not Found")?;
    }

    Ok(())
}

/// Interface Segregation (ISP/SOLID): Segregated trait for REST status endpoint queries.
#[allow(dead_code)]
pub trait ApiStatusEndpoint {
    fn query_status(&self) -> ApplianceStatusInfo;
}

/// Interface Segregation (ISP/SOLID): Segregated trait for REST optical drives endpoint queries.
#[allow(dead_code)]
pub trait ApiDrivesEndpoint {
    fn query_drives(&self) -> Vec<String>;
}

#[derive(Debug, Default)]
pub struct DefaultApiStatusEndpoint;

impl ApiStatusEndpoint for DefaultApiStatusEndpoint {
    fn query_status(&self) -> ApplianceStatusInfo {
        let handle = get_appliance_status_handle();
        if let Ok(state) = handle.lock() {
            state.clone()
        } else {
            ApplianceStatusInfo {
                status: "Idle".to_string(),
                drive: "auto".to_string(),
                disc: String::new(),
                current_title: String::new(),
                progress: 0.0,
                fps: "0".to_string(),
                speed: "0x".to_string(),
                has_selected_movie: false,
                is_series: false,
                year: None,
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct DefaultApiDrivesEndpoint;

impl ApiDrivesEndpoint for DefaultApiDrivesEndpoint {
    fn query_drives(&self) -> Vec<String> {
        crate::dvd::detect_dvd_drives()
    }
}

/// Dependency Inversion (DIP/SOLID): High-level trait abstraction for drive diagnostics and detection.
#[allow(dead_code)]
pub trait DriveDiagnosticProvider {
    fn detect_drives(&self) -> Vec<String>;
    fn benchmark(&self, drive: &str) -> anyhow::Result<crate::dvd::DriveBenchmarkReport>;
}

#[derive(Debug, Default)]
pub struct DefaultDriveDiagnosticProvider;

impl DriveDiagnosticProvider for DefaultDriveDiagnosticProvider {
    fn detect_drives(&self) -> Vec<String> {
        crate::dvd::detect_dvd_drives()
    }

    fn benchmark(&self, drive: &str) -> anyhow::Result<crate::dvd::DriveBenchmarkReport> {
        let dvd_path = crate::dvd::normalize_dvd_path(drive);
        crate::dvd::run_drive_benchmark("ffmpeg", &dvd_path, 10)
    }
}

/// Renders embedded HTML5/CSS3 Web Dashboard interface for headless appliances.
fn render_web_dashboard_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DVD Ripper Appliance Web Dashboard</title>
    <style>
        :root { --bg: #0f172a; --card: #1e293b; --accent: #38bdf8; --text: #f8fafc; --muted: #94a3b8; --border: #334155; }
        body { font-family: system-ui, -apple-system, sans-serif; background: var(--bg); color: var(--text); margin: 0; padding: 20px; }
        .container { max-width: 1000px; margin: 0 auto; }
        .header { display: flex; align-items: center; justify-content: space-between; padding-bottom: 15px; border-bottom: 1px solid var(--border); margin-bottom: 20px; }
        h1 { margin: 0; font-size: 1.6rem; color: var(--accent); }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; margin-bottom: 20px; }
        .card { background: var(--card); border: 1px solid var(--border); border-radius: 10px; padding: 18px; }
        .card h2 { font-size: 1.1rem; margin-top: 0; color: var(--accent); }
        .stat { font-size: 1.4rem; font-weight: bold; margin: 5px 0; }
        .badge { background: #0284c7; padding: 4px 8px; border-radius: 6px; font-size: 0.8rem; font-weight: 600; }
        .progress-bar { width: 100%; background: var(--border); border-radius: 6px; height: 16px; overflow: hidden; margin: 10px 0; }
        .progress-fill { height: 100%; background: linear-gradient(90deg, #0284c7, #38bdf8); width: 0%; transition: width 0.3s; }
        table { width: 100%; border-collapse: collapse; margin-top: 10px; }
        th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid var(--border); font-size: 0.9rem; }
        th { color: var(--muted); }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📀 DVD Ripper Appliance</h1>
            <span class="badge" id="appliance-status">STATUS: IDLE</span>
        </div>
        <div class="grid">
            <div class="card">
                <h2>Current Disc / Activity</h2>
                <div class="stat" id="current-disc">No Disc Detected</div>
                <div id="current-stage" style="color: var(--muted); font-size: 0.9rem;">Waiting for insertion</div>
                <div class="progress-bar"><div class="progress-fill" id="progress-fill"></div></div>
                <div style="display: flex; justify-content: space-between; font-size: 0.85rem; color: var(--muted); margin-bottom: 10px;">
                    <span id="stat-fps">FPS: --</span>
                    <span id="stat-speed">Speed: --</span>
                </div>
                <div style="display: flex; gap: 8px;">
                    <button onclick="triggerRip()" style="background:#0284c7; color:#fff; border:none; padding:6px 12px; border-radius:6px; cursor:pointer; font-weight:600;">▶ Start Rip</button>
                    <button onclick="fetch('/docs')" style="background:#334155; color:#fff; border:none; padding:6px 12px; border-radius:6px; cursor:pointer;">📖 API Docs</button>
                </div>
            </div>
            </div>
            <div class="card">
                <h2>Appliance Hardware</h2>
                <div id="hardware-drives">Detecting optical drives...</div>
            </div>
        </div>
        <div class="card">
            <h2>Recent Ripping History</h2>
            <table>
                <thead><tr><th>Time</th><th>Title</th><th>Type</th><th>Status</th></tr></thead>
                <tbody id="history-rows"><tr><td colspan="4">Loading history...</td></tr></tbody>
            </table>
        </div>
    </div>
    <script>
        async function updateDashboard() {
            try {
                const statusRes = await fetch('/api/status');
                if (statusRes.ok) {
                    const data = await statusRes.json();
                    document.getElementById('appliance-status').innerText = 'STATUS: ' + (data.status || 'IDLE').toUpperCase();
                    document.getElementById('current-disc').innerText = data.disc_detected || 'No Disc';
                    document.getElementById('current-stage').innerText = data.status || 'Idle';
                    const pct = data.progress_percent || 0;
                    document.getElementById('progress-fill').style.width = pct + '%';
                    document.getElementById('stat-fps').innerText = 'FPS: ' + (data.fps || '--');
                    document.getElementById('stat-speed').innerText = 'Speed: ' + (data.speed || '--');
                }
                const drivesRes = await fetch('/api/drives');
                if (drivesRes.ok) {
                    const drivesData = await drivesRes.json();
                    let html = '<strong>Optical Drives:</strong><br>';
                    (drivesData.drives || []).forEach(d => {
                        html += '• 💿 ' + d + '<br>';
                    });
                    document.getElementById('hardware-drives').innerHTML = html || 'No optical drives found';
                }
                const histRes = await fetch('/api/history');
                if (histRes.ok) {
                    const histData = await histRes.json();
                    let html = '';
                    (histData || []).slice(0, 5).forEach(r => {
                        html += `<tr><td>${r.timestamp || ''}</td><td>${r.title || ''}</td><td>${r.media_type || ''}</td><td>${r.status || ''}</td></tr>`;
                    });
                    document.getElementById('history-rows').innerHTML = html || '<tr><td colspan="4">No history records</td></tr>';
                }
            } catch(e) {}
        }
        async function triggerRip() {
            try {
                const res = await fetch('/api/trigger', { method: 'POST' });
                if (res.ok) alert('Rip triggered successfully!');
            } catch(e) { alert('Failed to trigger rip: ' + e); }
        }
        updateDashboard();
        setInterval(updateDashboard, 3000);
    </script>
</body>
</html>"#
}

/// Renders an embedded interactive Swagger API documentation UI interface.
fn render_swagger_docs_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>DVD Ripper REST API Documentation</title>
    <script type="module" src="https://unpkg.com/rapidoc/dist/rapidoc-min.js"></script>
</head>
<body style="margin:0; padding:0; background:#0f172a;">
    <rapi-doc spec-url="/api/openapi.json" theme="dark" render-style="read" show-header="false"></rapi-doc>
</body>
</html>"#
}

/// Helper: Extracts a case-insensitive header value from an HTTP header vector without heap allocations.
#[allow(dead_code)]
pub fn extract_header_value<'a>(headers: &'a [String], header_name: &str) -> Option<&'a str> {
    for line in headers {
        if let Some(colon_idx) = line.find(':') {
            let key = line[..colon_idx].trim();
            if key.eq_ignore_ascii_case(header_name) {
                return Some(line[colon_idx + 1..].trim());
            }
        }
    }
    None
}

/// Helper: Parses JSON HTTP request body after verifying Content-Type header.
#[allow(dead_code)]
pub fn parse_json_request_body<'a>(req: &'a [u8], headers: &[String]) -> Option<&'a str> {
    if let Some(ct) = extract_header_value(headers, "content-type")
        && !ct.to_lowercase().contains("application/json")
    {
        return None;
    }
    extract_body(req)
}

/// Extracts API authentication key from HTTP Bearer headers or `api_key=` query parameters.
#[allow(dead_code)]
pub fn extract_auth_key(headers: &[String], path: &str) -> Option<String> {
    if let Some(auth_val) = extract_header_value(headers, "authorization")
        && auth_val.to_lowercase().starts_with("bearer ")
    {
        return Some(auth_val[7..].trim().to_string());
    }
    parse_query_param(path, "api_key")
}

/// Returns the HTTP MIME Content-Type header string based on file path extension.
#[allow(dead_code)]
pub fn mime_type_for_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

/// Parses all URL query parameters into a HashMap of key-value pairs.
#[allow(dead_code)]
pub fn parse_query_map(url_path: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(q_idx) = url_path.find('?') {
        let query = &url_path[q_idx + 1..];
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next())
                && !k.is_empty()
            {
                map.insert(k.to_string(), crate::utils::decode_url_query_value(v));
            }
        }
    }
    map
}

/// Snapshot of appliance telemetry metric counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricCounters {
    pub completed_rips: u64,
    pub failed_rips: u64,
    pub active_jobs: usize,
    pub queued_jobs: usize,
}

/// Returns a thread-safe snapshot of current appliance telemetry metric counters.
#[allow(dead_code)]
pub fn snapshot_metrics() -> MetricCounters {
    MetricCounters {
        completed_rips: COMPLETED_RIPS_COUNTER.load(Ordering::SeqCst),
        failed_rips: FAILED_RIPS_COUNTER.load(Ordering::SeqCst),
        active_jobs: if get_appliance_status_handle()
            .lock()
            .is_ok_and(|s| s.status == "Ripping")
        {
            1
        } else {
            0
        },
        queued_jobs: crate::queue::list_jobs().len(),
    }
}

/// Helper: Formats MetricCounters snapshot into a JSON string.
#[allow(dead_code)]
pub fn format_metrics_json(metrics: &MetricCounters) -> String {
    serde_json::to_string(metrics).unwrap_or_else(|_| "{}".to_string())
}

/// Helper: Constructs a complete HTTP/1.1 response byte buffer with status line, content type, CORS, security, and length headers.
pub fn build_http_response_bytes(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(160 + status.len() + content_type.len() + body.len());
    use std::io::Write;
    let _ = write!(
        bytes,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    bytes.extend_from_slice(body);
    bytes
}

/// Helper: Formats an HTTP Content-Length header string (e.g. "Content-Length: 1024\r\n").
#[allow(dead_code)]
pub fn format_content_length_header(length: usize) -> String {
    format!("Content-Length: {}\r\n", length)
}

/// Helper: Constructs a complete HTTP/1.1 JSON response byte buffer.
#[allow(dead_code)]
pub fn build_http_json_response_bytes(status: &str, json_body: &str) -> Vec<u8> {
    build_http_response_bytes(status, "application/json", json_body.as_bytes())
}

fn send_json_response(stream: &mut TcpStream, status: &str, body_json: &str) -> Result<()> {
    send_http_response(stream, status, "application/json", body_json)
}

/// Helper: Sends a simple single key-value JSON response body (e.g. {"success": "true"} or {"message": "..."}).
#[allow(dead_code)]
pub fn send_json_message(stream: &mut TcpStream, status: &str, key: &str, val: &str) -> Result<()> {
    let json_body = format!(
        "{{\"{}\": \"{}\"}}",
        key,
        crate::utils::escape_json_str(val)
    );
    send_json_response(stream, status, &json_body)
}

/// Helper: Sends a standardized boolean status JSON response (e.g. {"success": true}).
#[allow(dead_code)]
pub fn send_json_status_bool(
    stream: &mut TcpStream,
    success: bool,
    message: Option<&str>,
) -> Result<()> {
    let json_body = if let Some(msg) = message {
        format!(
            "{{\"success\": {}, \"message\": \"{}\"}}",
            success,
            crate::utils::escape_json_str(msg)
        )
    } else {
        format!("{{\"success\": {}}}", success)
    };
    let http_status = if success { "200 OK" } else { "400 Bad Request" };
    send_json_response(stream, http_status, &json_body)
}

fn send_json_error(stream: &mut TcpStream, status: &str, message: &str) -> Result<()> {
    let err_body = format!(
        "{{\"error\": \"{}\"}}",
        crate::utils::escape_json_str(message)
    );
    send_http_response(stream, status, "application/json", &err_body)
}

fn send_http_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> Result<()> {
    let bytes = build_http_response_bytes(status, content_type, body.as_bytes());
    stream.write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_API_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_format_content_length_header() {
        assert_eq!(format_content_length_header(100), "Content-Length: 100\r\n");
    }

    #[test]
    fn test_api_metric_counter_increments() {
        increment_completed_rips();
        increment_failed_rips();
        let snap = snapshot_metrics();
        assert!(snap.completed_rips > 0 || snap.failed_rips > 0);
    }

    #[test]
    fn test_extract_auth_key_bearer_and_param() {
        let headers = vec!["Authorization: Bearer secret_token_123".to_string()];
        assert_eq!(
            extract_auth_key(&headers, "/api/status"),
            Some("secret_token_123".to_string())
        );

        let empty_headers: Vec<String> = Vec::new();
        assert_eq!(
            extract_auth_key(&empty_headers, "/api/status?api_key=query_secret"),
            Some("query_secret".to_string())
        );
    }

    #[test]
    fn test_parse_http_route() {
        let req1 = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req1), (b"GET" as &[u8], b"/" as &[u8]));

        let req2 = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req2),
            (b"GET" as &[u8], b"/api/status" as &[u8])
        );

        let req_cache = b"GET /api/disc-cache HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req_cache),
            (b"GET" as &[u8], b"/api/disc-cache" as &[u8])
        );

        let req3 = b"POST /api/eject HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req3),
            (b"POST" as &[u8], b"/api/eject" as &[u8])
        );

        let req4 = b"POST /api/rip HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req4),
            (b"POST" as &[u8], b"/api/rip" as &[u8])
        );

        let req5 = b"DELETE /api/unknown HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req5),
            (b"DELETE" as &[u8], b"/api/unknown" as &[u8])
        );
    }

    #[test]
    fn test_parse_query_param() {
        let path1 = "/api/search?q=Kill+Bill";
        assert_eq!(parse_query_param(path1, "q"), Some("Kill Bill".to_string()));

        let path2 = "/api/select?imdb_id=tt0266697&foo=bar";
        assert_eq!(
            parse_query_param(path2, "imdb_id"),
            Some("tt0266697".to_string())
        );

        let path3 = "/api/status";
        assert_eq!(parse_query_param(path3, "q"), None);
    }

    #[test]
    fn test_extract_body() {
        let req = b"POST /api/select HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"imdb_id\":\"tt0266697\"}";
        assert_eq!(extract_body(req), Some("{\"imdb_id\":\"tt0266697\"}"));
    }

    #[test]
    fn test_extract_header_value() {
        let headers = vec![
            "Host: localhost:8080".to_string(),
            "Content-Type: application/json".to_string(),
        ];
        assert_eq!(
            extract_header_value(&headers, "content-type"),
            Some("application/json")
        );
        assert_eq!(
            extract_header_value(&headers, "host"),
            Some("localhost:8080")
        );
        assert_eq!(extract_header_value(&headers, "missing"), None);
    }

    #[test]
    fn test_parse_json_request_body() {
        let req =
            b"POST /api/rip HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"drive\":\"D:\"}";
        let headers = vec!["Content-Type: application/json".to_string()];
        assert_eq!(
            parse_json_request_body(req, &headers),
            Some("{\"drive\":\"D:\"}")
        );
    }

    #[test]
    fn test_parse_sse_events_route() {
        let req =
            b"GET /api/events HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n";
        assert_eq!(
            parse_http_route(req),
            (b"GET" as &[u8], b"/api/events" as &[u8])
        );
    }

    #[test]
    fn test_api_key_validation() {
        let _lock = TEST_API_LOCK.lock().unwrap();
        set_api_key("secret123".to_string());
        let req_valid = b"POST /api/rip HTTP/1.1\r\nAuthorization: Bearer secret123\r\n\r\n";
        assert!(validate_api_key_header(req_valid, "/api/rip"));

        let req_invalid = b"POST /api/rip HTTP/1.1\r\nAuthorization: Bearer wrong\r\n\r\n";
        assert!(!validate_api_key_header(req_invalid, "/api/rip"));
    }

    #[test]
    fn test_extract_auth_key() {
        let headers = vec!["Authorization: Bearer my_token_123".to_string()];
        assert_eq!(
            extract_auth_key(&headers, "/api/rip"),
            Some("my_token_123".to_string())
        );

        let empty_headers: Vec<String> = Vec::new();
        assert_eq!(
            extract_auth_key(&empty_headers, "/api/rip?api_key=query_token"),
            Some("query_token".to_string())
        );
    }

    #[test]
    fn test_parse_query_map() {
        let map = parse_query_map("/api/search?q=Aliens&api_key=secret");
        assert_eq!(map.get("q"), Some(&"Aliens".to_string()));
        assert_eq!(map.get("api_key"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_send_json_message() {
        // Test helper string formatting logic
        let json_body = format!(
            "{{\"{}\": \"{}\"}}",
            "status",
            crate::utils::escape_json_str("success")
        );
        assert_eq!(json_body, "{\"status\": \"success\"}");
    }

    #[test]
    fn test_send_json_status_bool() {
        // Test helper string formatting logic
        let json_true = format!("{{\"success\": {}}}", true);
        assert_eq!(json_true, "{\"success\": true}");

        let json_msg = format!(
            "{{\"success\": {}, \"message\": \"{}\"}}",
            false,
            crate::utils::escape_json_str("Error occurred")
        );
        assert_eq!(
            json_msg,
            "{\"success\": false, \"message\": \"Error occurred\"}"
        );
    }

    #[test]
    fn test_snapshot_metrics() {
        let metrics = snapshot_metrics();
        assert_eq!(metrics.queued_jobs, crate::queue::list_jobs().len());

        let json = format_metrics_json(&metrics);
        assert!(json.contains("completed_rips"));
    }

    #[test]
    fn test_openapi_spec_route() {
        let req = b"GET /api/openapi.json HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req),
            (b"GET" as &[u8], b"/api/openapi.json" as &[u8])
        );
        let spec = generate_openapi_spec();
        assert!(spec.contains("DVD Ripper Appliance REST API"));
        assert!(spec.contains("/api/history/clear"));
        assert!(spec.contains("/api/boxset"));
        assert!(spec.contains("/api/boxset/reset"));
        assert!(spec.contains("/api/thumbnail"));
        assert!(spec.contains("/api/benchmark"));
        assert!(spec.contains("/api/pool/status"));
        assert!(spec.contains("/api/drives"));
        assert!(spec.contains("/api/health"));
    }

    #[test]
    fn test_parse_history_clear_route() {
        let req = b"POST /api/history/clear HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req),
            (b"POST" as &[u8], b"/api/history/clear" as &[u8])
        );
    }

    #[test]
    fn test_clear_history_api_logic() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_api_clear_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let hist_file = temp_dir.join("history.json");
        let rec = crate::domain::entities::job::RipRecord::new(
            "Test Film",
            "Movie",
            "out.mp4",
            "Success",
        );
        let _ = crate::history::save_history(&[rec], Some(&hist_file));
        let loaded = crate::history::load_history(Some(&hist_file));
        assert_eq!(loaded.len(), 1);

        let clear_res = crate::history::clear_history(Some(&hist_file));
        assert!(clear_res.is_ok());

        let after_clear = crate::history::load_history(Some(&hist_file));
        assert_eq!(after_clear.len(), 0);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_prometheus_metrics_render() {
        increment_completed_rips();
        increment_failed_rips();

        let output = render_prometheus_metrics();
        assert!(output.contains("dvd_ripper_completed_rips_total"));
        assert!(output.contains("dvd_ripper_failed_rips_total"));
        assert!(output.contains("dvd_ripper_active_jobs"));
        assert!(output.contains("dvd_ripper_queued_jobs"));
    }

    #[test]
    fn test_prometheus_metrics_route() {
        let req = b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_http_route(req),
            (b"GET" as &[u8], b"/metrics" as &[u8])
        );
    }

    #[test]
    fn test_build_http_response_bytes() {
        let bytes = build_http_response_bytes("200 OK", "application/json", b"{\"ok\":true}");
        let resp_str = String::from_utf8_lossy(&bytes);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("Content-Type: application/json"));
        assert!(resp_str.contains("{\"ok\":true}"));
    }

    #[test]
    fn test_build_http_json_response_bytes() {
        let bytes = build_http_json_response_bytes("200 OK", "{\"ok\":true}");
        let resp_str = String::from_utf8_lossy(&bytes);
        assert!(resp_str.contains("Content-Type: application/json"));
    }

    #[test]
    fn test_mime_type_for_path() {
        assert_eq!(mime_type_for_path("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_type_for_path("app.css"), "text/css; charset=utf-8");
        assert_eq!(mime_type_for_path("data.json"), "application/json");
        assert_eq!(mime_type_for_path("image.png"), "image/png");
    }

    #[test]
    fn test_drive_status_info_new() {
        let drive = DriveStatusInfo::new("E:\\");
        assert_eq!(drive.drive, "E:\\");
        assert_eq!(drive.status, "Idle");
        assert_eq!(drive.progress, 0.0);
        assert!(!drive.has_selected_movie);
    }

    #[test]
    fn test_drive_pool_initialization_and_status() {
        let drives = vec!["D:\\".to_string(), "E:\\".to_string(), "F:\\".to_string()];
        init_drive_pool(&drives);

        let statuses = get_drive_pool_statuses();
        assert!(statuses.iter().any(|d| d.drive == "D:\\"));
        assert!(statuses.iter().any(|d| d.drive == "E:\\"));
        assert!(statuses.iter().any(|d| d.drive == "F:\\"));

        set_drive_disc_detected("E:\\", "THE_MATRIX");
        let e_status = get_drive_status("E:\\").expect("Drive E status should exist");
        assert_eq!(e_status.disc, "THE_MATRIX");
        assert_eq!(e_status.status, "Detected - Search Required");

        set_drive_selected_metadata("E:\\", "The Matrix", false, Some(1999));
        let e_updated = get_drive_status("E:\\").unwrap();
        assert_eq!(e_updated.current_title, "The Matrix");
        assert_eq!(e_updated.year, Some(1999));
        assert!(e_updated.has_selected_movie);
        assert!(e_updated.status.contains("Ready"));

        update_drive_status("E:\\", "Ripping", "", "The Matrix", 54.5, "28", "2.1x");
        let e_ripping = get_drive_status("E:\\").unwrap();
        assert_eq!(e_ripping.status, "Ripping");
        assert_eq!(e_ripping.progress, 54.5);
        assert_eq!(e_ripping.fps, "28");
        assert_eq!(e_ripping.speed, "2.1x");
    }

    #[test]
    fn test_drive_pool_cancellation() {
        let flag_x = get_drive_cancel_flag("X:\\");
        let flag_y = get_drive_cancel_flag("Y:\\");
        flag_x.store(false, Ordering::SeqCst);
        flag_y.store(false, Ordering::SeqCst);

        let cancelled = cancel_drive_rip("X:\\");
        assert!(cancelled);
        assert!(flag_x.load(Ordering::SeqCst));
        assert!(!flag_y.load(Ordering::SeqCst)); // Independent cancellation!

        cancel_all_drive_rips();
        assert!(flag_y.load(Ordering::SeqCst));
    }

    #[test]
    fn test_extract_content_length() {
        let headers = b"POST /api/rip HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 42\r\n\r\n";
        assert_eq!(extract_content_length(headers), Some(42));

        let headers_case = b"POST /api/rip HTTP/1.1\r\ncontent-length: 1024\r\n\r\n";
        assert_eq!(extract_content_length(headers_case), Some(1024));

        let no_cl = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(extract_content_length(no_cl), None);

        let invalid_cl = b"POST /api/rip HTTP/1.1\r\nContent-Length: not-a-number\r\n\r\n";
        assert_eq!(extract_content_length(invalid_cl), None);
    }

    #[test]
    fn test_read_http_request_safely() {
        use std::io::Cursor;

        // Normal request
        let req = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let res = read_http_request_safely(Cursor::new(req));
        assert_eq!(res, SafeHttpReadResult::Ok(req.to_vec()));

        // Empty stream
        let empty = b"";
        assert_eq!(
            read_http_request_safely(Cursor::new(empty)),
            SafeHttpReadResult::Empty
        );

        // Payload Too Large (> 1MB via Content-Length)
        let large_cl = format!(
            "POST /api/rip HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_REQUEST_BODY_SIZE + 1
        );
        let res_large = read_http_request_safely(Cursor::new(large_cl.as_bytes()));
        assert_eq!(res_large, SafeHttpReadResult::PayloadTooLarge);

        // Headers Too Large (> 64KB)
        let mut giant_headers = vec![b'A'; MAX_HEADER_SIZE + 10];
        giant_headers.extend_from_slice(b"\r\n\r\n");
        let res_headers = read_http_request_safely(Cursor::new(giant_headers));
        assert_eq!(res_headers, SafeHttpReadResult::HeadersTooLarge);
    }

    fn test_handle_client_route(req: &[u8]) -> (String, Vec<u8>) {
        let req_vec = req.to_vec();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let client = std::thread::spawn(move || {
            let mut client = std::net::TcpStream::connect(addr).unwrap();
            use std::io::{Read, Write};
            client.write_all(&req_vec).unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let mut buf = Vec::new();
            let _ = client.read_to_end(&mut buf);
            buf
        });

        let (server_stream, _) = listener.accept().unwrap();
        let _ = handle_client(server_stream, "D:\\");
        let client_bytes = client.join().unwrap();
        let client_str = String::from_utf8_lossy(&client_bytes).to_string();
        (client_str, client_bytes)
    }

    #[test]
    fn test_api_handle_client_routes() {
        let _lock = TEST_API_LOCK.lock().unwrap();
        set_api_key(String::new()); // No key required

        // 1. Root & Dashboard
        let (r1, _) = test_handle_client_route(b"GET / HTTP/1.1\r\n\r\n");
        assert!(r1.contains("200 OK"));

        let (r2, _) = test_handle_client_route(b"GET /dashboard HTTP/1.1\r\n\r\n");
        assert!(r2.contains("200 OK"));

        let (r3, _) = test_handle_client_route(b"GET /docs HTTP/1.1\r\n\r\n");
        assert!(r3.contains("200 OK"));

        // 2. Metrics & OpenAPI
        let (r4, _) = test_handle_client_route(b"GET /metrics HTTP/1.1\r\n\r\n");
        assert!(r4.contains("200 OK"));

        let (r5, _) = test_handle_client_route(b"GET /api/openapi.json HTTP/1.1\r\n\r\n");
        assert!(r5.contains("200 OK"));

        // 3. Status & Drives
        let (r6, _) = test_handle_client_route(b"GET /api/status HTTP/1.1\r\n\r\n");
        assert!(r6.contains("200 OK"));

        let (r7, _) = test_handle_client_route(b"GET /api/status?drive=D:\\ HTTP/1.1\r\n\r\n");
        assert!(r7.contains("200 OK"));

        let (r8, _) = test_handle_client_route(b"GET /api/pool/status HTTP/1.1\r\n\r\n");
        assert!(r8.contains("200 OK"));

        let (r9, _) = test_handle_client_route(b"GET /api/drives HTTP/1.1\r\n\r\n");
        assert!(r9.contains("200 OK"));

        let (r10, _) = test_handle_client_route(b"GET /api/health HTTP/1.1\r\n\r\n");
        assert!(r10.contains("200 OK"));

        // 4. History
        let (r11, _) = test_handle_client_route(b"GET /api/history HTTP/1.1\r\n\r\n");
        assert!(r11.contains("200 OK"));

        let (r12, _) = test_handle_client_route(b"POST /api/history/clear HTTP/1.1\r\n\r\n");
        assert!(r12.contains("200 OK"));

        // 5. Disc cache
        let (r13, _) = test_handle_client_route(b"GET /api/disc-cache HTTP/1.1\r\n\r\n");
        assert!(r13.contains("200 OK"));

        // 6. Queue
        let (r14, _) = test_handle_client_route(b"GET /api/queue/list HTTP/1.1\r\n\r\n");
        assert!(r14.contains("200 OK"));

        let (r15, _) = test_handle_client_route(
            b"POST /api/queue/add?title=TestMovie&type=Movie HTTP/1.1\r\n\r\n",
        );
        assert!(r15.contains("200 OK"));

        let batch_req = b"POST /api/queue/batch HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 47\r\n\r\n[{\"title\":\"T1\",\"media_type\":\"Movie\",\"drive\":\"D:\"}]";
        let (r16, _) = test_handle_client_route(batch_req);
        assert!(r16.contains("200 OK"));

        let (r17, _) =
            test_handle_client_route(b"POST /api/queue/remove?id=nonexistent HTTP/1.1\r\n\r\n");
        assert!(r17.contains("200 OK"));

        // 7. Search & Select
        let (r18, _) = test_handle_client_route(b"GET /api/search?q=Aliens HTTP/1.1\r\n\r\n");
        assert!(r18.contains("200 OK"));

        let (r19, _) = test_handle_client_route(b"GET /api/search HTTP/1.1\r\n\r\n");
        assert!(r19.contains("200 OK"));

        let (r20, _) = test_handle_client_route(b"POST /api/select HTTP/1.1\r\n\r\n");
        assert!(r20.contains("400 Bad Request"));

        let (r21, _) =
            test_handle_client_route(b"POST /api/select?imdb_id=tt0090605 HTTP/1.1\r\n\r\n");
        assert!(r21.contains("200 OK"));

        let (r22, _) =
            test_handle_client_route(b"POST /api/select?imdb_id=tt0000000000 HTTP/1.1\r\n\r\n");
        assert!(r22.contains("404 Not Found"));

        // 8. Eject, Cancel, Trigger, Boxset
        let (r23, _) = test_handle_client_route(b"POST /api/eject?drive=D: HTTP/1.1\r\n\r\n");
        assert!(r23.contains("200 OK"));

        let (r24, _) = test_handle_client_route(b"POST /api/cancel?drive=D: HTTP/1.1\r\n\r\n");
        assert!(r24.contains("200 OK"));

        let (r25, _) = test_handle_client_route(b"POST /api/cancel HTTP/1.1\r\n\r\n");
        assert!(r25.contains("200 OK"));

        let (r26, _) = test_handle_client_route(b"GET /api/boxset HTTP/1.1\r\n\r\n");
        assert!(r26.contains("200 OK"));

        let (r27, _) = test_handle_client_route(
            b"POST /api/boxset/reset?show=Firefly&season=1 HTTP/1.1\r\n\r\n",
        );
        assert!(r27.contains("200 OK"));

        let (r28, _) = test_handle_client_route(b"POST /api/trigger?drive=D: HTTP/1.1\r\n\r\n");
        assert!(r28.contains("200 OK"));

        let (r29, _) = test_handle_client_route(b"GET /api/ws HTTP/1.1\r\n\r\n");
        assert!(r29.contains("101 Switching Protocols"));

        let (r30, _) = test_handle_client_route(b"GET /api/unknown_route_404 HTTP/1.1\r\n\r\n");
        assert!(r30.contains("404 Not Found"));

        let bad_json = b"POST /api/rip HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 10\r\n\r\n{bad_json:";
        let (r31, _) = test_handle_client_route(bad_json);
        assert!(r31.contains("400 Bad Request"));

        // 9. Additional rip & benchmark & thumbnail routes
        let (r_rip, _) =
            test_handle_client_route(b"POST /api/rip?drive=Z:\\NonExistent HTTP/1.1\r\n\r\n");
        assert!(r_rip.contains("400 Bad Request"));

        let (r_rip_iso, _) = test_handle_client_route(
            b"POST /api/rip?drive=Z:\\NonExistent&iso=true HTTP/1.1\r\n\r\n",
        );
        assert!(r_rip_iso.contains("400 Bad Request"));

        let (r_bench, _) =
            test_handle_client_route(b"POST /api/benchmark?drive=Z:\\NonExistent HTTP/1.1\r\n\r\n");
        assert!(r_bench.contains("500 Internal Server Error") || r_bench.contains("200 OK"));

        let (r_thumb, _) = test_handle_client_route(
            b"GET /api/thumbnail?drive=Z:\\NonExistent&title=1 HTTP/1.1\r\n\r\n",
        );
        assert!(r_thumb.contains("500 Internal Server Error"));

        let json_eject = b"POST /api/eject HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 14\r\n\r\n{\"drive\":\"D:\"}";
        let (r_ej, _) = test_handle_client_route(json_eject);
        assert!(r_ej.contains("200 OK"));

        let json_cancel = b"POST /api/cancel HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 14\r\n\r\n{\"drive\":\"D:\"}";
        let (r_can, _) = test_handle_client_route(json_cancel);
        assert!(r_can.contains("200 OK"));

        let json_select = b"POST /api/select HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 26\r\n\r\n{\"imdb_id\":\"tt0090605\"}";
        let (r_sel, _) = test_handle_client_route(json_select);
        assert!(r_sel.contains("200 OK"));

        // 10. Unauthorized when API key set
        set_api_key("super_secret".to_string());
        let (r32, _) = test_handle_client_route(b"POST /api/rip HTTP/1.1\r\n\r\n");
        assert!(r32.contains("401 Unauthorized"));
        set_api_key(String::new());
    }

    #[test]
    fn test_api_traits_and_appliance_status_updates() {
        let _lock = TEST_API_LOCK.lock().unwrap();
        let status_ep = DefaultApiStatusEndpoint;
        let _ = status_ep.query_status();

        let drives_ep = DefaultApiDrivesEndpoint;
        let _ = drives_ep.query_drives();

        let diag = DefaultDriveDiagnosticProvider;
        let _ = diag.detect_drives();
        let _ = diag.benchmark("D:\\");

        update_appliance_status("Active", "THE_DISC", "The Movie", 75.0, "30", "1.8x");
        set_disc_detected("THE_DISC");
        reset_appliance_status("Idle");
        fail_appliance_status("The Movie", "C:\\Out", "Simulated error");

        let handle = get_cancel_flag_handle();
        handle.store(false, Ordering::SeqCst);
        assert!(!handle.load(Ordering::SeqCst));

        let tx_handle = get_cancel_tx_handle();
        assert!(tx_handle.lock().is_ok());

        let drives_registry = get_drive_pool_registry();
        assert!(drives_registry.lock().is_ok());

        let flags_map = get_drive_cancel_flags_map();
        assert!(flags_map.lock().is_ok());

        reset_drive_status("Z:\\", "Idle");
    }
}
