#![windows_subsystem = "windows"]

/*!
 * @file main.rs
 * @brief DVD Ripper binary entry point supporting both GUI and CLI modes for Movies & TV Series.
 */

use anyhow::{Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

use dvd_ripper::cli::Args;
use dvd_ripper::daemon::run_daemon_with_container;
use dvd_ripper::dvd::{self, get_volume_label, normalize_dvd_path};
use dvd_ripper::ffmpeg::{detect_tv_episodes, resolve_output_path};
#[cfg(feature = "gui")]
use dvd_ripper::gui;
use dvd_ripper::imdb::{self, fetch_search_candidates, lookup_film_details, lookup_omdb_by_id};
use dvd_ripper::utils::{self, sanitize_filename};
use dvd_ripper::{AppContainer, AppInitializer};

/// Resolves film metadata, release year, running time, and cover poster bytes for CLI mode execution.
fn resolve_cli_metadata(
    args: &mut Args,
    volume_label: Option<&str>,
) -> (Option<String>, Option<u32>, Option<f64>, Option<Vec<u8>>) {
    // 0. Disc Fingerprint Cache Auto-Match
    let fp_hash = dvd::compute_disc_fingerprint(&args.input);
    if let Some(cached_meta) = imdb::lookup_fingerprint_cache(&fp_hash) {
        let clean_name = sanitize_filename(&cached_meta.title);
        let runtime_desc = cached_meta
            .runtime_secs
            .map(|r| format!(", {:.0} mins", r / 60.0))
            .unwrap_or_default();
        println!(
            "[Fingerprint Auto-Match] Found cached disc ID '{}': {} ({:?}{})",
            fp_hash, clean_name, cached_meta.year, runtime_desc
        );
        if cached_meta.is_series {
            args.tv = true;
        }
        return (
            Some(clean_name),
            cached_meta.year,
            cached_meta.runtime_secs,
            cached_meta.poster_bytes,
        );
    }

    // 1. Direct IMDb ID selection
    if let Some(ref imdb_id) = args.imdb_id {
        println!("Looking up exact IMDb ID: {}", imdb_id);
        if let Some(meta) = lookup_omdb_by_id(imdb_id) {
            let clean_name = sanitize_filename(&meta.title);
            let runtime_desc = meta
                .runtime_secs
                .map(|r| format!(", {:.0} mins", r / 60.0))
                .unwrap_or_default();
            println!(
                "IMDb ID Match: {} ({:?}{})",
                clean_name, meta.year, runtime_desc
            );
            if meta.is_series {
                println!("Media identified as TV Series.");
                args.tv = true;
            }
            return (
                Some(clean_name),
                meta.year,
                meta.runtime_secs,
                meta.poster_bytes,
            );
        } else {
            println!(
                "Warning: Could not fetch details for IMDb ID '{}'.",
                imdb_id
            );
        }
    }

    // 2. Search query or volume label detection
    let search_term = if let Some(ref q) = args.search {
        Some(q.clone())
    } else {
        volume_label.map(|l| l.to_string())
    };

    if let Some(query) = search_term {
        if args.search.is_some() {
            println!("Searching IMDb candidates for query: '{}'", query);
        } else {
            println!("Detected DVD Volume Label: {}", query);
        }

        let candidates = fetch_search_candidates(&query);
        let mut selected_candidate_id: Option<String> = None;

        if !candidates.is_empty() {
            if let Some(idx) = args.select_index {
                if idx >= 1 && idx <= candidates.len() {
                    let cand = &candidates[idx - 1];
                    println!(
                        "Selected candidate #{}/{} from CLI argument: {} ({:?}) [{}]",
                        idx,
                        candidates.len(),
                        cand.title,
                        cand.year,
                        cand.imdb_id
                    );
                    selected_candidate_id = Some(cand.imdb_id.clone());
                } else {
                    println!(
                        "Warning: Invalid --select-index {}, available range is 1-{}",
                        idx,
                        candidates.len()
                    );
                }
            } else if args.search.is_some() || candidates.len() > 1 {
                println!("\n--- IMDb Search Candidates for '{}' ---", query);
                for (i, cand) in candidates.iter().enumerate() {
                    let yr_str = cand.year.map(|y| format!(" ({})", y)).unwrap_or_default();
                    println!(
                        "  [{}] {} {} - [{}] ({})",
                        i + 1,
                        cand.title,
                        yr_str,
                        cand.imdb_id,
                        cand.type_field
                    );
                }
                println!("  [0] Skip candidate selection / use auto-detection");

                use std::io::{self, Write};
                print!(
                    "Select entry [1-{}, default 1, 0 to skip]: ",
                    candidates.len()
                );
                let _ = io::stdout().flush();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_ok() {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        selected_candidate_id = Some(candidates[0].imdb_id.clone());
                    } else if let Ok(num) = trimmed.parse::<usize>() {
                        if num >= 1 && num <= candidates.len() {
                            selected_candidate_id = Some(candidates[num - 1].imdb_id.clone());
                        } else if num == 0 {
                            println!("Skipped candidate selection.");
                        }
                    }
                }
            }
        }

        if let Some(imdb_id) = selected_candidate_id
            && let Some(meta) = lookup_omdb_by_id(&imdb_id)
        {
            let clean_name = sanitize_filename(&meta.title);
            let runtime_desc = meta
                .runtime_secs
                .map(|r| format!(", {:.0} mins", r / 60.0))
                .unwrap_or_default();
            println!(
                "Selected Metadata: {} ({:?}{})",
                clean_name, meta.year, runtime_desc
            );
            if meta.is_series {
                println!("Media identified as TV Series.");
                args.tv = true;
            }
            imdb::save_fingerprint_cache(&fp_hash, &meta);
            return (
                Some(clean_name),
                meta.year,
                meta.runtime_secs,
                meta.poster_bytes,
            );
        }

        // Fallback to lookup_film_details if no candidate selected or search returned empty
        match lookup_film_details(&query) {
            Ok(meta) => {
                let clean_name = sanitize_filename(&meta.title);
                let runtime_desc = meta
                    .runtime_secs
                    .map(|r| format!(", {:.0} mins", r / 60.0))
                    .unwrap_or_default();
                println!(
                    "Metadata Result: {} ({:?}{})",
                    clean_name, meta.year, runtime_desc
                );
                if meta.is_series {
                    println!("Media identified as TV Series.");
                    args.tv = true;
                }
                imdb::save_fingerprint_cache(&fp_hash, &meta);
                return (
                    Some(clean_name),
                    meta.year,
                    meta.runtime_secs,
                    meta.poster_bytes,
                );
            }
            Err(e) => {
                println!(
                    "Warning: Failed to look up metadata details for '{}': {}",
                    query, e
                );
            }
        }
    } else {
        println!("Warning: Could not detect DVD volume label and no search query provided.");
    }

    (None, None, None, None)
}

/// Main entry point executing GUI application or CLI transcoding process.
fn main() -> Result<()> {
    #[cfg(feature = "gui")]
    {
        // Check raw args to see if user requested CLI mode or passed specific CLI parameters
        let raw_args: Vec<String> = std::env::args().collect();
        let is_cli = raw_args.iter().any(|arg| {
            arg == "--cli"
                || arg == "-h"
                || arg == "--help"
                || arg == "-V"
                || arg == "--version"
                || arg == "--tv"
                || arg == "--daemon"
                || arg == "-s"
                || arg == "--search"
                || arg == "--imdb-id"
                || arg == "--select-index"
                || arg == "--iso"
        });

        if !is_cli {
            // Run native desktop GUI by default when feature is enabled
            if let Err(e) = gui::run_gui() {
                eprintln!("GUI Error: {}", e);
            }
            return Ok(());
        }
    }

    // Attach to parent console if running from terminal in CLI mode on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        unsafe extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
        }
        AttachConsole(0xFFFFFFFF);
    }

    // CLI mode execution path
    let mut args = Args::parse();
    let app_initializer = AppInitializer;
    let _cfg = app_initializer.bootstrap(&mut args);
    let container = Arc::new(AppContainer::build_from_args(&args));

    if args.daemon {
        return run_daemon_with_container(args, 10, container);
    }

    // 1. Resolve & validate DVD path
    let dvd_path = normalize_dvd_path(&args.input);
    if !dvd_path.exists() {
        return Err(anyhow!(
            "DVD drive or path does not exist: {}",
            dvd_path.display()
        ));
    }
    println!("Target DVD path: {}", dvd_path.display());

    if args.check_protection {
        println!("\n--- [Disc Copy Protection Diagnostic Report] ---");
        let report = dvd::inspect_disc_copy_protection(&dvd_path);
        for note in &report.diagnostic_notes {
            println!("  • {}", note);
        }
        println!("------------------------------------------------\n");
    }

    if args.benchmark {
        println!("\n--- [Optical Drive Throughput Benchmark Diagnostic] ---");
        println!(
            "Running 10-second optical sector read speed test on '{}'...",
            dvd_path.display()
        );
        match dvd::run_drive_benchmark(&args.ffmpeg, &dvd_path, 10) {
            Ok(report) => {
                println!("  • Drive Path     : {}", report.drive_path);
                println!("  • Test Duration  : {} seconds", report.test_duration_secs);
                println!("  • Read Throughput: {:.2} MB/s", report.read_speed_mbps);
                println!(
                    "  • Demux Rate     : {:.2} MB/s ({} FPS)",
                    report.demux_speed_mbps, report.fps
                );
                println!("  • Performance    : {}", report.rating_summary);
            }
            Err(e) => {
                println!("  • Benchmark Error: {}", e);
            }
        }
        println!("--------------------------------------------------------\n");
        return Ok(());
    }

    // 2. Resolve metadata via IMDb search, ID selection, or volume label auto-detection
    let volume_label = get_volume_label(&dvd_path.to_string_lossy());
    let (title_name, title_year, film_runtime, poster_bytes) =
        resolve_cli_metadata(&mut args, volume_label.as_deref());

    if title_name.is_none() {
        return Err(anyhow!(
            "Ripping disabled: No movie searched and selected. Please specify --search <QUERY> or --imdb-id <ID> to select a movie."
        ));
    }

    let mut last_output_file: Option<PathBuf> = None;

    // Check disk space threshold guard with optional fallback directory
    let fb_dir = args.fallback_out_dir.as_deref().map(std::path::Path::new);
    let resolved_out_dir = dvd_ripper::utils::check_disk_space_guard_with_fallback(
        std::path::Path::new(&args.out_dir),
        fb_dir,
        args.min_free_gb,
    )?;
    args.out_dir = resolved_out_dir.to_string_lossy().to_string();

    // 3. Execution path for Archival 1:1 ISO dump vs TV series vs Movie
    if args.iso {
        let absolute_output = resolve_output_path(&args, title_name.as_deref(), title_year)?;
        println!(
            "Archival ISO dump will be saved to: {}",
            absolute_output.display()
        );
        let mut builder =
            dvd_ripper::application::dump_iso::DumpIsoRequest::builder(&dvd_path, &absolute_output)
                .min_free_gb(args.min_free_gb);
        if let Some(ref label) = volume_label {
            builder = builder.volume_label(label);
        }
        if let Some(ref title) = title_name {
            builder = builder.display_title(title);
        }
        let request = builder.build()?;
        container
            .dump_iso_use_case()
            .execute(&request, None, None)?;
        println!("\nSuccessfully completed archival 1:1 ISO dump of disc!");
        last_output_file = Some(absolute_output);
    } else if args.tv {
        let show_name = title_name.as_deref().unwrap_or("TV Show");

        if args.auto_boxset && args.start_episode == 1 {
            let next_ep = dvd_ripper::queue::get_next_boxset_episode(show_name, args.season);
            if next_ep > 1 {
                args.start_episode = next_ep;
                println!(
                    "Auto BoxSet Mode: Automatically offset starting episode to S{:02}E{:02} for show '{}'.",
                    args.season, next_ep, show_name
                );
            }
        }

        println!(
            "TV Mode: Show '{}', Season {:02}, Start Episode #{}.",
            show_name, args.season, args.start_episode
        );
        if args.all_episodes {
            println!(
                "\n--- TV Series Mode: Batch Ripping Season {} (Starting Episode S{:02}E{:02}) ---",
                args.season, args.season, args.start_episode
            );

            let episodes = detect_tv_episodes(
                &args.ffmpeg,
                &dvd_path,
                show_name,
                args.season,
                args.start_episode,
                None,
            );

            if episodes.is_empty() {
                return Err(anyhow!("No valid TV episode titles found on DVD disc."));
            }

            println!("Found {} episode titles on disc.", episodes.len());

            let ext = if args.mkv { "mkv" } else { "mp4" };
            let request = dvd_ripper::application::rip_tv_batch::RipTvBatchRequest::builder(
                &dvd_path,
                show_name,
                &args.out_dir,
            )
            .season(args.season)
            .start_episode(args.start_episode)
            .episodes(episodes.clone())
            .min_free_gb(args.min_free_gb)
            .file_extension(ext)
            .lookup_episode_titles(true)
            .track_boxset(args.auto_boxset)
            .build()?;

            let ripped_count = container
                .rip_tv_batch_use_case()
                .execute(&request, None, None)?;
            println!(
                "\nSuccessfully completed batch rip of {} episodes!",
                ripped_count
            );

            if let Some(first_ep) = episodes.first() {
                let sample_output = dvd_ripper::ffmpeg::resolve_tv_output_path_with_title(
                    &args,
                    Some(show_name),
                    title_year,
                    args.season,
                    first_ep.episode_num,
                    None,
                )
                .ok();
                last_output_file = sample_output;
            }
        } else {
            let ep_num = if args.start_episode > 0 {
                args.start_episode
            } else {
                1
            };
            let tvmaze_episodes =
                dvd_ripper::infrastructure::metadata::fetch_tvmaze_episodes(show_name)
                    .unwrap_or_default();
            let ep_title = dvd_ripper::infrastructure::metadata::find_episode_title(
                &tvmaze_episodes,
                args.season,
                ep_num,
            );
            let ep_output = dvd_ripper::ffmpeg::resolve_tv_output_path_with_title(
                &args,
                Some(show_name),
                title_year,
                args.season,
                ep_num,
                ep_title.as_deref(),
            )?;
            println!("Output file will be saved to: {}", ep_output.display());

            let ep_name = dvd_ripper::utils::format_episode_name_with_title(
                show_name,
                args.season,
                ep_num,
                ep_title.as_deref(),
            );
            let mut req_builder =
                dvd_ripper::application::rip_single_title::RipSingleTitleRequest::builder(
                    &dvd_path, args.title, &ep_output,
                )
                .display_title(&ep_name)
                .min_free_gb(args.min_free_gb);
            if let Some(dur) = film_runtime {
                req_builder = req_builder.expected_duration(dur);
            }
            let req = req_builder.build()?;
            container
                .rip_single_title_use_case()
                .execute(&req, None, None)?;
            let _ = dvd_ripper::ocr::process_subtitle_ocr_sidecar(&args, &ep_output, &ep_name);
            last_output_file = Some(ep_output);
        }
    } else {
        // Standard Movie execution path
        let absolute_output = resolve_output_path(&args, title_name.as_deref(), title_year)?;
        println!(
            "Output file will be saved to: {}",
            absolute_output.display()
        );

        let display_title = title_name.as_deref().unwrap_or("Unknown DVD Title");
        let mut req_builder =
            dvd_ripper::application::rip_single_title::RipSingleTitleRequest::builder(
                &dvd_path,
                args.title,
                &absolute_output,
            )
            .display_title(display_title)
            .min_free_gb(args.min_free_gb);
        if let Some(dur) = film_runtime {
            req_builder = req_builder.expected_duration(dur);
        }
        let req = req_builder.build()?;
        container
            .rip_single_title_use_case()
            .execute(&req, None, None)?;
        let _ =
            dvd_ripper::ocr::process_subtitle_ocr_sidecar(&args, &absolute_output, display_title);
        last_output_file = Some(absolute_output);
    }

    if let (Some(bytes), Some(out_file)) = (poster_bytes.as_ref(), last_output_file.as_ref()) {
        let _ = utils::save_cover_artworks(out_file, bytes);
    }

    if args.nfo
        && let Some(ref out_file) = last_output_file
    {
        let media_type = if args.tv { "TV Series" } else { "Movie" };
        let title = title_name.as_deref().unwrap_or("DVD Rip");
        let _ = utils::generate_nfo_file(
            out_file,
            title,
            title_year,
            None,
            None,
            None,
            media_type,
            args.tags.as_deref(),
        );
    }

    if args.checksum
        && let Some(ref out_file) = last_output_file
    {
        let _ = utils::generate_checksum_file(out_file);
    }

    if let Some(ref audit_path) = args.audit_log {
        let details = serde_json::json!({
            "title": title_name,
            "year": title_year,
            "dvd_path": dvd_path,
            "output_file": last_output_file
        });
        let _ = utils::append_audit_log_entry(
            std::path::Path::new(audit_path),
            "RipCompleted",
            &details,
        );
    }

    if let Some(days) = args.auto_cleanup_days {
        let _ = dvd_ripper::history::purge_expired_rip_records(days, None);
    }

    if let (Some(script), Some(out_file)) = (args.post_script.as_deref(), last_output_file.as_ref())
    {
        let media_type = if args.tv { "TV Series" } else { "Movie" };
        let title = title_name.as_deref().unwrap_or("DVD Rip");
        let _ = utils::run_post_processing_hook(script, out_file, title, media_type, title_year);
    }

    utils::trigger_media_server_scans(&args);

    if !args.no_eject {
        println!("\nEjecting DVD disc from drive...");
        let _ = dvd::eject_disc(&dvd_path.to_string_lossy());
        if let Some(secs) = args.eject_autoclose {
            println!(
                "[Drive Tray] Waiting {} seconds before auto-closing drive tray...",
                secs
            );
            std::thread::sleep(std::time::Duration::from_secs(secs));
            let _ = dvd::close_disc_tray(&dvd_path.to_string_lossy());
        }
    } else if args.spindown {
        println!("\nIssuing optical drive motor spindown signal...");
        let _ = dvd::spindown_drive(&dvd_path.to_string_lossy());
    }

    Ok(())
}
