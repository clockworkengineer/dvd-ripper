/**
 * @file daemon.rs
 * @brief Headless auto-rip daemon watcher loop for embedded appliances & home media servers.
 */

use std::thread;
use std::time::Duration;
use anyhow::Result;

use crate::cli::Args;
use crate::dvd::{get_volume_label, normalize_dvd_path};
use crate::ffmpeg::{detect_tv_episodes, resolve_output_path, resolve_tv_output_path, run_ffmpeg_with_progress};
use crate::history::record_rip_event;
use crate::imdb::lookup_film_details;

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

                    if let Some(ref broker) = args.mqtt_broker {
                        let _ = crate::mqtt::publish_mqtt_status(broker, &label, "Detected", 0.0);
                    }
                    if let Some(ref webhook) = args.webhook_url {
                        let _ = crate::mqtt::send_webhook_notification(webhook, &label, "Detected", "New DVD disc detected on optical drive");
                    }

                    let mut job_args = args.clone();
                    let meta_res = lookup_film_details(&label);

                    let title_name = if let Ok(meta) = meta_res {
                        println!("[Daemon Metadata] Found: {} ({})", meta.title, meta.year.unwrap_or(0));
                        if meta.is_series {
                            job_args.tv = true;
                            job_args.out_dir = "TV".to_string();
                        }
                        meta.title
                    } else {
                        label.clone()
                    };

                    if job_args.tv {
                        println!("[Daemon] Starting TV series batch rip...");
                        let episodes = detect_tv_episodes(
                            &job_args.ffmpeg,
                            &dvd_path,
                            &title_name,
                            job_args.season,
                            job_args.start_episode,
                            None,
                        );
                        for ep in &episodes {
                            if let Ok(out_path) = resolve_tv_output_path(
                                &job_args,
                                Some(&title_name),
                                None,
                                job_args.season,
                                ep.episode_num,
                            ) {
                                println!("[Daemon Ripping] Episode -> {}", out_path.display());
                                if let Some(ref broker) = args.mqtt_broker {
                                    let _ = crate::mqtt::publish_mqtt_status(broker, &ep.formatted_name, "Ripping", 50.0);
                                }
                                if run_ffmpeg_with_progress(
                                    &job_args,
                                    &dvd_path,
                                    &out_path,
                                    &ep.formatted_name,
                                    Some(ep.duration_secs),
                                ).is_ok() {
                                    let _ = record_rip_event(&ep.formatted_name, "TV Series", &out_path.to_string_lossy(), "Success");
                                }
                            }
                        }
                    } else {
                        if let Ok(out_path) = resolve_output_path(&job_args, Some(&title_name), None) {
                            println!("[Daemon Ripping] Movie -> {}", out_path.display());
                            if let Some(ref broker) = args.mqtt_broker {
                                let _ = crate::mqtt::publish_mqtt_status(broker, &title_name, "Ripping", 50.0);
                            }
                            if run_ffmpeg_with_progress(
                                &job_args,
                                &dvd_path,
                                &out_path,
                                &title_name,
                                None,
                            ).is_ok() {
                                let _ = record_rip_event(&title_name, "Movie", &out_path.to_string_lossy(), "Success");
                            }
                        }
                    }

                    println!("[Daemon Event] Ripping completed for disc: {}", label);
                    if let Some(ref broker) = args.mqtt_broker {
                        let _ = crate::mqtt::publish_mqtt_status(broker, &label, "Completed", 100.0);
                    }
                    if let Some(ref webhook) = args.webhook_url {
                        let _ = crate::mqtt::send_webhook_notification(webhook, &label, "Completed", "Successfully finished backup of DVD disc");
                    }
                    println!("[Daemon Event] Ejecting optical disc tray...");
                    let _ = crate::dvd::eject_disc(&args.input);
                    println!();
                }
            } else {
                last_processed_label.clear();
            }
        } else {
            last_processed_label.clear();
        }

        thread::sleep(Duration::from_secs(poll_interval_secs));
    }
}
