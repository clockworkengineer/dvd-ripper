/**
 * @file daemon.rs
 * @brief Headless auto-rip daemon watcher loop for embedded appliances & home media servers.
 */

use std::thread;
use std::time::Duration;
use anyhow::Result;

use crate::cli::Args;
use crate::dvd::{get_volume_label, normalize_dvd_path};

/// Launches the headless appliance daemon loop polling optical drives for inserted DVDs.
pub fn run_daemon(args: Args, poll_interval_secs: u64) -> Result<()> {
    println!("=== DVD Ripper Embedded Appliance Daemon Mode ===");
    println!("Monitoring drive '{}' every {} seconds...", args.input, poll_interval_secs);
    let _ = crate::api::start_embedded_api_server(8080, args.input.clone());
    println!("Press Ctrl+C to stop.\n");

    let mut last_processed_label = String::new();

    loop {
        let dvd_path = normalize_dvd_path(&args.input);
        if dvd_path.exists() {
            if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
                if !label.is_empty() && label != last_processed_label {
                    println!("[Daemon Event] New Disc Detected: {}", label);
                    last_processed_label = label.clone();
                    crate::api::set_disc_detected(&label);

                    if let Some(ref broker) = args.mqtt_broker {
                        let _ = crate::mqtt::publish_mqtt_status(broker, &label, "Detected - Search Required", 0.0);
                    }
                    if let Some(ref webhook) = args.webhook_url {
                        let _ = crate::mqtt::send_webhook_notification(webhook, &label, "Detected", "New DVD disc inserted. Search and select movie to begin ripping.");
                    }
                    println!("[Daemon Event] Disc inserted: '{}'. Awaiting movie search & selection to enable ripping.", label);
                }
            }
        } else {
            if !last_processed_label.is_empty() {
                last_processed_label.clear();
                let handle = crate::api::get_appliance_status_handle();
                if let Ok(mut state) = handle.lock() {
                    state.disc.clear();
                    state.current_title.clear();
                    state.has_selected_movie = false;
                    state.status = "Idle".to_string();
                }
            }
        }

        thread::sleep(Duration::from_secs(poll_interval_secs));
    }
}
