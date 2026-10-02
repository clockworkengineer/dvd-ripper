/**
 * @file daemon.rs
 * @brief Headless multi-drive auto-rip daemon watcher loop for embedded appliances & home media servers.
 */

use std::thread;
use std::time::Duration;
use anyhow::Result;

use crate::cli::Args;
use crate::dvd::{detect_dvd_drives, get_volume_label, normalize_dvd_path};

/// Single Responsibility (SRP/SOLID): Encapsulates optical drive state change polling.
pub struct DaemonDriveWatcher {
    pub drive_path_str: String,
    pub last_processed_label: String,
}

impl DaemonDriveWatcher {
    pub fn new(drive_path_str: String) -> Self {
        Self {
            drive_path_str,
            last_processed_label: String::new(),
        }
    }

    pub fn poll_drive_state(&mut self) -> Option<String> {
        let dvd_path = normalize_dvd_path(&self.drive_path_str);
        if dvd_path.exists() {
            if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
                if !label.is_empty() && label != self.last_processed_label {
                    self.last_processed_label = label.clone();
                    return Some(label);
                }
            }
        } else if !self.last_processed_label.is_empty() {
            self.last_processed_label.clear();
        }
        None
    }
}

/// Spawns a watcher loop for a single specified optical drive.
fn spawn_drive_watcher(drive_path_str: String, args: Args, poll_interval_secs: u64) {
    thread::spawn(move || {
        let mut watcher = DaemonDriveWatcher::new(drive_path_str.clone());
        loop {
            if let Some(label) = watcher.poll_drive_state() {
                println!("[Daemon Drive '{}'] New Disc Detected: {}", drive_path_str, label);
                crate::api::set_drive_disc_detected(&drive_path_str, &label);

                if let Some(ref broker) = args.mqtt_broker {
                    let _ = crate::mqtt::publish_mqtt_status(broker, &label, "Detected - Search Required", 0.0);
                }
                if let Some(ref webhook) = args.webhook_url {
                    let msg = format!("New DVD disc inserted into drive {}. Awaiting movie search & selection.", drive_path_str);
                    let _ = crate::mqtt::send_webhook_notification(webhook, &label, "Detected", &msg, args.webhook_secret.as_deref());
                }

                if args.auto_rip {
                    println!("[Daemon Drive '{}'] ⚡ Auto-Rip enabled. Identifying metadata for '{}'...", drive_path_str, label);
                    let cleaned = crate::dvd::clean_volume_label(&label);
                    let query = crate::utils::normalize_volume_label_title(&cleaned);
                    let candidates = crate::imdb::fetch_search_candidates(&query);
                    if let Some(cand) = candidates.first() {
                        let is_series = cand.type_field.to_lowercase() == "series";
                        println!("[Daemon Drive '{}'] Auto-matched: '{}' ({:?}, Series: {})", drive_path_str, cand.title, cand.year, is_series);
                        crate::api::set_drive_selected_metadata(&drive_path_str, &cand.title, is_series, cand.year);
                        if let Err(e) = crate::api::execute_rip_for_drive(&drive_path_str, Some(args.clone())) {
                            eprintln!("[Daemon Drive '{}'] Auto-rip failed to start: {}", drive_path_str, e);
                        } else {
                            println!("[Daemon Drive '{}'] ▶ Auto-rip started for '{}'", drive_path_str, cand.title);
                        }
                    } else {
                        println!("[Daemon Drive '{}'] No high-confidence match found for '{}'. Awaiting manual selection.", drive_path_str, label);
                    }
                } else {
                    println!("[Daemon Drive '{}'] Disc inserted: '{}'. Awaiting movie search & selection to enable ripping.", drive_path_str, label);
                }
            }

            thread::sleep(Duration::from_secs(poll_interval_secs));
        }
    });
}

/// Launches the headless multi-drive appliance daemon loop polling optical drives for inserted DVDs.
pub fn run_daemon(args: Args, poll_interval_secs: u64) -> Result<()> {
    println!("=== DVD Ripper Embedded Multi-Drive Appliance Daemon Mode ===");

    let detected = detect_dvd_drives();
    let drives_to_monitor = if let Some(ref pool) = args.drive_pool {
        crate::dvd::resolve_drive_pool_handles(Some(pool))
    } else if args.input == "auto" && !detected.is_empty() {
        detected
    } else {
        vec![args.input.clone()]
    };

    println!("Initializing optical drive pool with {} drive(s): [{}]", drives_to_monitor.len(), drives_to_monitor.join(", "));
    crate::api::init_drive_pool(&drives_to_monitor);

    let default_drive = drives_to_monitor.first().cloned().unwrap_or_else(|| "auto".to_string());
    let _ = crate::api::start_embedded_api_server(8080, default_drive);

    println!("Monitoring {} optical drive(s) [{}] every {} seconds...", drives_to_monitor.len(), drives_to_monitor.join(", "), poll_interval_secs);
    if args.auto_rip {
        println!("🚀 Unattended Auto-Rip Mode: ACTIVE (automatic title identification, background ripping & tray auto-eject)");
    } else {
        println!("🔒 Controlled Selection Workflow: ACTIVE (disc insertion paused until movie selection via Web UI or CLI)");
    }
    println!("Press Ctrl+C to stop.\n");

    for drive in drives_to_monitor {
        spawn_drive_watcher(drive, args.clone(), poll_interval_secs);
    }

    // Keep main thread alive
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
