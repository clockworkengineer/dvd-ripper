/**
 * @file daemon.rs
 * @brief Headless multi-drive auto-rip daemon watcher loop for embedded appliances & home media servers.
 */

use std::thread;
use std::time::Duration;
use anyhow::Result;

use crate::cli::Args;
use crate::dvd::{detect_dvd_drives, get_volume_label, normalize_dvd_path};

/// Spawns a watcher loop for a single specified optical drive.
fn spawn_drive_watcher(drive_path_str: String, args: Args, poll_interval_secs: u64) {
    thread::spawn(move || {
        let mut last_processed_label = String::new();
        loop {
            let dvd_path = normalize_dvd_path(&drive_path_str);
            if dvd_path.exists() {
                if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
                    if !label.is_empty() && label != last_processed_label {
                        println!("[Daemon Drive '{}'] New Disc Detected: {}", drive_path_str, label);
                        last_processed_label = label.clone();
                        crate::api::set_disc_detected(&label);

                        if let Some(ref broker) = args.mqtt_broker {
                            let _ = crate::mqtt::publish_mqtt_status(broker, &label, "Detected - Search Required", 0.0);
                        }
                        if let Some(ref webhook) = args.webhook_url {
                            let _ = crate::mqtt::send_webhook_notification(webhook, &label, "Detected", "New DVD disc inserted. Search and select movie to begin ripping.", args.webhook_secret.as_deref());
                        }
                        println!("[Daemon Drive '{}'] Disc inserted: '{}'. Awaiting movie search & selection to enable ripping.", drive_path_str, label);
                    }
                }
            } else {
                if !last_processed_label.is_empty() {
                    last_processed_label.clear();
                    let handle = crate::api::get_appliance_status_handle();
                    if let Ok(mut state) = handle.lock() {
                        state.reset();
                    }

                }
            }

            thread::sleep(Duration::from_secs(poll_interval_secs));
        }
    });
}

/// Launches the headless multi-drive appliance daemon loop polling optical drives for inserted DVDs.
pub fn run_daemon(args: Args, poll_interval_secs: u64) -> Result<()> {
    println!("=== DVD Ripper Embedded Multi-Drive Appliance Daemon Mode ===");
    let _ = crate::api::start_embedded_api_server(8080, args.input.clone());

    let detected = detect_dvd_drives();
    let drives_to_monitor = if args.input == "auto" && !detected.is_empty() {
        detected
    } else {
        vec![args.input.clone()]
    };

    println!("Monitoring {} optical drive(s) [{}] every {} seconds...", drives_to_monitor.len(), drives_to_monitor.join(", "), poll_interval_secs);
    println!("Press Ctrl+C to stop.\n");

    for drive in drives_to_monitor {
        spawn_drive_watcher(drive, args.clone(), poll_interval_secs);
    }

    // Keep main thread alive
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
