/**
 * @file main.rs
 * @brief DVD Ripper entry point supporting both GUI and CLI modes for Movies & TV Series.
 */

mod api;
mod cli;
mod daemon;
mod dvd;
mod ffmpeg;
#[cfg(feature = "gui")]
mod gui;
mod history;
mod imdb;
mod mqtt;
mod utils;

use std::path::PathBuf;
use anyhow::{anyhow, Result};
use clap::Parser;

use cli::Args;
use daemon::run_daemon;
use dvd::{get_volume_label, normalize_dvd_path};
use ffmpeg::{
    detect_tv_episodes, resolve_output_path, resolve_tv_output_path, run_ffmpeg_with_progress,
};
use imdb::{fetch_search_candidates, lookup_film_details, lookup_omdb_by_id};
use utils::sanitize_filename;

fn resolve_cli_metadata(args: &mut Args, volume_label: Option<&str>) -> (Option<String>, Option<u32>, Option<f64>, Option<Vec<u8>>) {
    // 0. Disc Fingerprint Cache Auto-Match
    let fp_hash = dvd::compute_disc_fingerprint(&args.input);
    if let Some(cached_meta) = imdb::lookup_fingerprint_cache(&fp_hash) {
        let clean_name = sanitize_filename(&cached_meta.title);
        let runtime_desc = cached_meta
            .runtime_secs
            .map(|r| format!(", {:.0} mins", r / 60.0))
            .unwrap_or_default();
        println!("[Fingerprint Auto-Match] Found cached disc ID '{}': {} ({:?}{})", fp_hash, clean_name, cached_meta.year, runtime_desc);
        if cached_meta.is_series {
            args.tv = true;
        }
        return (Some(clean_name), cached_meta.year, cached_meta.runtime_secs, cached_meta.poster_bytes);
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
            println!("IMDb ID Match: {} ({:?}{})", clean_name, meta.year, runtime_desc);
            if meta.is_series {
                println!("Media identified as TV Series.");
                args.tv = true;
            }
            return (Some(clean_name), meta.year, meta.runtime_secs, meta.poster_bytes);
        } else {
            println!("Warning: Could not fetch details for IMDb ID '{}'.", imdb_id);
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
                print!("Select entry [1-{}, default 1, 0 to skip]: ", candidates.len());
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

        if let Some(imdb_id) = selected_candidate_id {
            if let Some(meta) = lookup_omdb_by_id(&imdb_id) {
                let clean_name = sanitize_filename(&meta.title);
                let runtime_desc = meta
                    .runtime_secs
                    .map(|r| format!(", {:.0} mins", r / 60.0))
                    .unwrap_or_default();
                println!("Selected Metadata: {} ({:?}{})", clean_name, meta.year, runtime_desc);
                if meta.is_series {
                    println!("Media identified as TV Series.");
                    args.tv = true;
                }
                imdb::save_fingerprint_cache(&fp_hash, &meta);
                return (Some(clean_name), meta.year, meta.runtime_secs, meta.poster_bytes);
            }
        }

        // Fallback to lookup_film_details if no candidate selected or search returned empty
        match lookup_film_details(&query) {
            Ok(meta) => {
                let clean_name = sanitize_filename(&meta.title);
                let runtime_desc = meta
                    .runtime_secs
                    .map(|r| format!(", {:.0} mins", r / 60.0))
                    .unwrap_or_default();
                println!("Metadata Result: {} ({:?}{})", clean_name, meta.year, runtime_desc);
                if meta.is_series {
                    println!("Media identified as TV Series.");
                    args.tv = true;
                }
                imdb::save_fingerprint_cache(&fp_hash, &meta);
                return (Some(clean_name), meta.year, meta.runtime_secs, meta.poster_bytes);
            }
            Err(e) => {
                println!("Warning: Failed to look up metadata details for '{}': {}", query, e);
            }
        }
    } else {
        println!("Warning: Could not detect DVD volume label and no search query provided.");
    }

    (None, None, None, None)
}

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
        });

        if !is_cli {
            // Run native desktop GUI by default when feature is enabled
            if let Err(e) = gui::run_gui() {
                eprintln!("GUI Error: {}", e);
            }
            return Ok(());
        }
    }

    // CLI mode execution path
    let mut args = Args::parse();

    if args.daemon {
        return run_daemon(args, 10);
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

    // 3. Execution path for TV series vs Movie
    if args.tv {
        let show_name = title_name.as_deref().unwrap_or("TV Show");

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

            for (idx, ep) in episodes.iter().enumerate() {
                println!(
                    "\n=== Ripping Episode {}/{}: {} (Title #{}, {:.0} mins) ===",
                    idx + 1,
                    episodes.len(),
                    ep.formatted_name,
                    ep.title_num,
                    ep.duration_secs / 60.0
                );

                let ep_output = resolve_tv_output_path(
                    &args,
                    Some(show_name),
                    title_year,
                    args.season,
                    ep.episode_num,
                )?;

                let mut ep_args = args.clone();
                ep_args.title = ep.title_num;

                run_ffmpeg_with_progress(
                    &ep_args,
                    &dvd_path,
                    &ep_output,
                    &ep.formatted_name,
                    Some(ep.duration_secs),
                )?;
                last_output_file = Some(ep_output);
            }

            println!("\nSuccessfully completed batch rip of all episodes!");
        } else {
            let ep_num = if args.start_episode > 0 {
                args.start_episode
            } else {
                1
            };
            let ep_output = resolve_tv_output_path(
                &args,
                Some(show_name),
                title_year,
                args.season,
                ep_num,
            )?;
            println!("Output file will be saved to: {}", ep_output.display());

            let ep_name = format!("{} - S{:02}E{:02}", show_name, args.season, ep_num);
            run_ffmpeg_with_progress(
                &args,
                &dvd_path,
                &ep_output,
                &ep_name,
                film_runtime,
            )?;
            last_output_file = Some(ep_output);
        }
    } else {
        // Standard Movie execution path
        let absolute_output =
            resolve_output_path(&args, title_name.as_deref(), title_year)?;
        println!("Output file will be saved to: {}", absolute_output.display());

        let display_title = title_name.as_deref().unwrap_or("Unknown DVD Title");
        run_ffmpeg_with_progress(
            &args,
            &dvd_path,
            &absolute_output,
            display_title,
            film_runtime,
        )?;
        last_output_file = Some(absolute_output);
    }

    if let (Some(bytes), Some(out_file)) = (poster_bytes.as_ref(), last_output_file.as_ref()) {
        let _ = utils::save_cover_artworks(out_file, bytes);
    }

    println!("\nEjecting DVD disc from drive...");
    let _ = dvd::eject_disc(&dvd_path.to_string_lossy());

    Ok(())
}
