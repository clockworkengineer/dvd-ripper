use anyhow::Result;
/**
 * @file daemon.rs
 * @brief Headless multi-drive auto-rip daemon watcher loop for embedded appliances & home media servers.
 */
use std::thread;
use std::time::Duration;

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
        self.poll_drive_state_with_inspector(None)
    }

    pub fn poll_drive_state_with_inspector(
        &mut self,
        inspector: Option<&dyn crate::domain::ports::optical_drive::DiscInspector>,
    ) -> Option<String> {
        let dvd_path = normalize_dvd_path(&self.drive_path_str);
        if dvd_path.exists() {
            let label_opt = if let Some(ins) = inspector {
                ins.read_volume_label(&dvd_path)
            } else {
                get_volume_label(&dvd_path.to_string_lossy())
            };
            if let Some(label) = label_opt
                && !label.is_empty()
                && label != self.last_processed_label
            {
                self.last_processed_label = label.clone();
                return Some(label);
            }
        } else if !self.last_processed_label.is_empty() {
            self.last_processed_label.clear();
        }
        None
    }
}

/// Spawns a watcher loop for a single specified optical drive.
fn spawn_drive_watcher(
    drive_path_str: String,
    args: Args,
    poll_interval_secs: u64,
    container: std::sync::Arc<crate::AppContainer>,
) {
    thread::spawn(move || {
        let mut watcher = DaemonDriveWatcher::new(drive_path_str.clone());
        loop {
            if let Some(label) =
                watcher.poll_drive_state_with_inspector(Some(container.disc_inspector.as_ref()))
            {
                process_detected_disc(&drive_path_str, &label, &args, &container);
            }

            thread::sleep(Duration::from_secs(poll_interval_secs));
        }
    });
}

/// Processes a detected disc event for the specified optical drive.
pub fn process_detected_disc(
    drive_path_str: &str,
    label: &str,
    args: &Args,
    container: &crate::AppContainer,
) {
    println!(
        "[Daemon Drive '{}'] New Disc Detected: {}",
        drive_path_str, label
    );
    crate::api::set_drive_disc_detected(drive_path_str, label);

    if let Some(ref broker) = args.mqtt_broker {
        let _ = crate::mqtt::publish_mqtt_status(
            broker,
            label,
            "Detected - Search Required",
            0.0,
        );
    }
    if let Some(ref webhook) = args.webhook_url {
        let msg = format!(
            "New DVD disc inserted into drive {}. Awaiting movie search & selection.",
            drive_path_str
        );
        let _ = crate::mqtt::send_webhook_notification(
            webhook,
            label,
            "Detected",
            &msg,
            args.webhook_secret.as_deref(),
        );
    }

    if args.auto_rip {
        println!(
            "[Daemon Drive '{}'] ⚡ Auto-Rip enabled. Identifying metadata for '{}'...",
            drive_path_str, label
        );
        let physical_drive = crate::infrastructure::optical::PhysicalOpticalDrive::new(
            drive_path_str,
            &args.ffmpeg,
        );
        let probe_uc = container.probe_disc_use_case();
        let probe_res = probe_uc.execute(&physical_drive);

        let (matched_title, is_series, matched_year) = if let Ok(ref res) = probe_res
            && let Some(ref meta) = res.metadata
        {
            (Some(meta.title.clone()), meta.is_series, meta.year)
        } else {
            let cleaned = crate::dvd::clean_volume_label(label);
            let query = crate::utils::normalize_volume_label_title(&cleaned);
            let candidates = crate::imdb::fetch_search_candidates(&query);
            if let Some(cand) = candidates.first() {
                (
                    Some(cand.title.clone()),
                    cand.type_field.to_lowercase() == "series",
                    cand.year,
                )
            } else {
                (None, false, None)
            }
        };

        if let Some(title) = matched_title {
            println!(
                "[Daemon Drive '{}'] Auto-matched: '{}' ({:?}, Series: {})",
                drive_path_str, title, matched_year, is_series
            );
            crate::api::set_drive_selected_metadata(
                drive_path_str,
                &title,
                is_series,
                matched_year,
            );
            if let Err(e) =
                crate::api::execute_rip_for_drive(drive_path_str, Some(args.clone()))
            {
                eprintln!(
                    "[Daemon Drive '{}'] Auto-rip failed to start: {}",
                    drive_path_str, e
                );
            } else {
                println!(
                    "[Daemon Drive '{}'] ▶ Auto-rip started for '{}'",
                    drive_path_str, title
                );
            }
        } else {
            println!(
                "[Daemon Drive '{}'] No high-confidence match found for '{}'. Awaiting manual selection.",
                drive_path_str, label
            );
        }
    } else {
        println!(
            "[Daemon Drive '{}'] Disc inserted: '{}'. Awaiting movie search & selection to enable ripping.",
            drive_path_str, label
        );
    }
}

/// Launches the headless multi-drive appliance daemon loop polling optical drives for inserted DVDs.
#[allow(dead_code)]
pub fn run_daemon(args: Args, poll_interval_secs: u64) -> Result<()> {
    let container = std::sync::Arc::new(crate::AppContainer::build_from_args(&args));
    run_daemon_with_container(args, poll_interval_secs, container)
}

/// Resolves the optical drives to monitor based on CLI arguments and hardware detection.
pub fn resolve_daemon_drives(args: &Args) -> Vec<String> {
    let detected = detect_dvd_drives();
    if let Some(ref pool) = args.drive_pool {
        crate::dvd::resolve_drive_pool_handles(Some(pool))
    } else if args.input == "auto" && !detected.is_empty() {
        detected
    } else {
        vec![args.input.clone()]
    }
}

/// Launches the daemon loop using the provided AppContainer.
pub fn run_daemon_with_container(
    args: Args,
    poll_interval_secs: u64,
    container: std::sync::Arc<crate::AppContainer>,
) -> Result<()> {
    println!("=== DVD Ripper Embedded Multi-Drive Appliance Daemon Mode ===");

    let drives_to_monitor = resolve_daemon_drives(&args);

    println!(
        "Initializing optical drive pool with {} drive(s): [{}]",
        drives_to_monitor.len(),
        drives_to_monitor.join(", ")
    );
    crate::api::init_drive_pool(&drives_to_monitor);

    let default_drive = drives_to_monitor
        .first()
        .cloned()
        .unwrap_or_else(|| "auto".to_string());
    let _ = crate::api::start_embedded_api_server(8080, default_drive);

    println!(
        "Monitoring {} optical drive(s) [{}] every {} seconds...",
        drives_to_monitor.len(),
        drives_to_monitor.join(", "),
        poll_interval_secs
    );
    if args.auto_rip {
        println!(
            "🚀 Unattended Auto-Rip Mode: ACTIVE (automatic title identification, background ripping & tray auto-eject)"
        );
    } else {
        println!(
            "🔒 Controlled Selection Workflow: ACTIVE (disc insertion paused until movie selection via Web UI or CLI)"
        );
    }
    println!("Press Ctrl+C to stop.\n");

    for drive in drives_to_monitor {
        spawn_drive_watcher(drive, args.clone(), poll_interval_secs, container.clone());
    }

    // Keep main thread alive
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    struct MockInspector {
        label: Option<String>,
    }

    impl crate::domain::ports::optical_drive::DiscInspector for MockInspector {
        fn read_volume_label(&self, _drive_path: &Path) -> Option<String> {
            self.label.clone()
        }
        fn compute_fingerprint(&self, _drive_path: &Path) -> String {
            "mock_fp".to_string()
        }
        fn inspect_protection(
            &self,
            _drive_path: &Path,
        ) -> crate::domain::entities::disc::DiscCopyProtectionReport {
            Default::default()
        }
    }

    #[test]
    fn test_daemon_drive_watcher_nonexistent_drive() {
        let mut watcher = DaemonDriveWatcher::new("Z:\\NonExistent_Drive_12345".to_string());
        assert_eq!(watcher.drive_path_str, "Z:\\NonExistent_Drive_12345");
        assert_eq!(watcher.last_processed_label, "");

        let res = watcher.poll_drive_state();
        assert_eq!(res, None);
    }

    #[test]
    fn test_daemon_drive_watcher_with_inspector_and_deduplication() {
        let temp_dir = std::env::temp_dir();
        let path_str = temp_dir.to_string_lossy().to_string();
        let mut watcher = DaemonDriveWatcher::new(path_str);

        let inspector = MockInspector {
            label: Some("ALIENS_DISC".to_string()),
        };

        // First poll: detects new disc
        let res1 = watcher.poll_drive_state_with_inspector(Some(&inspector));
        assert_eq!(res1, Some("ALIENS_DISC".to_string()));
        assert_eq!(watcher.last_processed_label, "ALIENS_DISC");

        // Second poll with same disc: deduplicates and returns None
        let res2 = watcher.poll_drive_state_with_inspector(Some(&inspector));
        assert_eq!(res2, None);

        // Third poll with empty label: returns None
        let empty_inspector = MockInspector { label: Some("".to_string()) };
        let res3 = watcher.poll_drive_state_with_inspector(Some(&empty_inspector));
        assert_eq!(res3, None);

        // Fourth poll with new disc label: returns Some("MATRIX_DISC")
        let new_inspector = MockInspector {
            label: Some("MATRIX_DISC".to_string()),
        };
        let res4 = watcher.poll_drive_state_with_inspector(Some(&new_inspector));
        assert_eq!(res4, Some("MATRIX_DISC".to_string()));
        assert_eq!(watcher.last_processed_label, "MATRIX_DISC");

        // Nonexistent path clears last_processed_label
        let mut dead_watcher = DaemonDriveWatcher::new("Z:\\NonExistentDrive".to_string());
        dead_watcher.last_processed_label = "OLD_LABEL".to_string();
        let res5 = dead_watcher.poll_drive_state_with_inspector(Some(&inspector));
        assert_eq!(res5, None);
        assert_eq!(dead_watcher.last_processed_label, "");
    }

    #[test]
    fn test_daemon_resolve_drives() {
        let mut args1 = Args::default();
        args1.drive_pool = Some("D:,E:,F:".to_string());
        let drives1 = resolve_daemon_drives(&args1);
        assert_eq!(drives1, vec!["D:", "E:", "F:"]);

        let mut args2 = Args::default();
        args2.input = "X:".to_string();
        let drives2 = resolve_daemon_drives(&args2);
        assert_eq!(drives2, vec!["X:"]);
    }

    #[test]
    fn test_daemon_process_detected_disc_manual_and_auto() {
        let container = std::sync::Arc::new(crate::AppContainer::build_production("ffmpeg"));

        // 1. Manual mode with webhook & mqtt
        let mut args_manual = Args::default();
        args_manual.webhook_url = Some("http://127.0.0.1:18888/webhook".to_string());
        args_manual.mqtt_broker = Some("127.0.0.1:11883".to_string());
        args_manual.auto_rip = false;

        process_detected_disc("D:\\", "TEST_DISC_MANUAL", &args_manual, &container);

        // 2. Auto rip mode with match
        let mut args_auto = Args::default();
        args_auto.auto_rip = true;
        process_detected_disc("D:\\", "Aliens", &args_auto, &container);

        // 3. Auto rip mode with no match
        process_detected_disc("D:\\", "xyznonexistentdisc999888", &args_auto, &container);
    }
}
