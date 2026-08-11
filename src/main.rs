/**
 * @file main.rs
 * @brief DVD Ripper entry point supporting both GUI and CLI modes for Movies & TV Series.
 */

mod cli;
mod dvd;
mod ffmpeg;
mod gui;
mod imdb;
mod utils;

use anyhow::{anyhow, Result};
use clap::Parser;

use cli::Args;
use dvd::{get_volume_label, normalize_dvd_path};
use ffmpeg::{
    detect_tv_episodes, resolve_output_path, resolve_tv_output_path, run_ffmpeg_with_progress,
};
use gui::run_gui;
use imdb::lookup_film_details;
use utils::{
    find_next_start_episode, infer_start_episode_from_label, parse_season_disc_from_label,
    sanitize_filename,
};

fn main() -> Result<()> {
    // Check raw args to see if user requested CLI mode or passed specific CLI parameters
    let raw_args: Vec<String> = std::env::args().collect();
    let is_cli = raw_args.iter().any(|arg| {
        arg == "--cli"
            || arg == "-h"
            || arg == "--help"
            || arg == "-V"
            || arg == "--version"
            || arg == "--tv"
    });

    if !is_cli {
        // Run native desktop GUI by default
        if let Err(e) = run_gui() {
            eprintln!("GUI Error: {}", e);
        }
        return Ok(());
    }

    // CLI mode execution path
    let mut args = Args::parse();

    // 1. Resolve & validate DVD path
    let dvd_path = normalize_dvd_path(&args.input);
    if !dvd_path.exists() {
        return Err(anyhow!(
            "DVD drive or path does not exist: {}",
            dvd_path.display()
        ));
    }
    println!("Target DVD path: {}", dvd_path.display());

    // 2. Try to auto-detect media metadata from volume label
    let mut title_name = None;
    let mut title_year = None;
    let mut film_runtime = None;
    let volume_label = get_volume_label(&dvd_path.to_string_lossy());

    if let Some(ref label) = volume_label {
        println!("Detected DVD Volume Label: {}", label);
        match lookup_film_details(label) {
            Ok(meta) => {
                let clean_name = sanitize_filename(&meta.title);
                let runtime_desc = meta
                    .runtime_secs
                    .map(|r| format!(", {:.0} mins", r / 60.0))
                    .unwrap_or_default();
                println!(
                    "Auto-detected Metadata: {} ({:?}{})",
                    clean_name, meta.year, runtime_desc
                );
                title_name = Some(clean_name);
                title_year = meta.year;
                film_runtime = meta.runtime_secs;

                if meta.is_series {
                    println!("Media identified as TV Series.");
                    args.tv = true;
                }
            }
            Err(e) => {
                println!(
                    "Warning: Failed to look up metadata details for label '{}': {}",
                    label, e
                );
            }
        }
    } else {
        println!("Warning: Could not detect DVD volume label.");
    }

    // 3. Execution path for TV series vs Movie
    if args.tv {
        let show_name = title_name.as_deref().unwrap_or("TV Show");

        if let Some(ref lbl) = volume_label {
            let parsed = parse_season_disc_from_label(lbl);
            if let Some(s) = parsed.season {
                args.season = s;
            }
        }

        if args.start_episode == 1 {
            let label_inferred = volume_label
                .as_deref()
                .and_then(|lbl| infer_start_episode_from_label(lbl, 3));

            if let Some(ep) = label_inferred {
                println!(
                    "Inferred Start Episode #{} directly from DVD Volume Label ({}).",
                    ep,
                    volume_label.as_deref().unwrap_or("")
                );
                args.start_episode = ep;
            } else {
                let auto_ep = find_next_start_episode(&args.out_dir, show_name, title_year, args.season);
                if auto_ep > 1 {
                    println!(
                        "Detected existing episodes in Season {:02} folder. Auto-setting Start Episode to #{}.",
                        args.season, auto_ep
                    );
                    args.start_episode = auto_ep;
                }
            }
        }
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
    }

    Ok(())
}
