/**
 * @file main.rs
 * @brief DVD Ripper CLI Utility entry point.
 */

mod cli;
mod dvd;
mod ffmpeg;
mod imdb;
mod utils;

use anyhow::{anyhow, Result};
use clap::Parser;

use cli::Args;
use dvd::{get_volume_label, normalize_dvd_path};
use ffmpeg::{resolve_output_path, run_ffmpeg_with_progress};
use imdb::lookup_film_details;
use utils::sanitize_filename;

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Resolve & validate DVD path
    let dvd_path = normalize_dvd_path(&args.input);
    if !dvd_path.exists() {
        return Err(anyhow!(
            "DVD drive or path does not exist: {}",
            dvd_path.display()
        ));
    }
    println!("Target DVD path: {}", dvd_path.display());

    // 2. Try to auto-detect film metadata from volume label
    let mut film_name = None;
    let mut film_year = None;

    if let Some(label) = get_volume_label(&dvd_path.to_string_lossy()) {
        println!("Detected DVD Volume Label: {}", label);
        match lookup_film_details(&label) {
            Ok((name, year)) => {
                let clean_name = sanitize_filename(&name);
                println!("Auto-detected Film Details: {} ({:?})", clean_name, year);
                film_name = Some(clean_name);
                film_year = year;
            }
            Err(e) => {
                println!(
                    "Warning: Failed to look up film details for label '{}': {}",
                    label, e
                );
            }
        }
    } else {
        println!("Warning: Could not detect DVD volume label.");
    }

    // 3. Resolve destination output path
    let absolute_output =
        resolve_output_path(&args, film_name.as_deref(), film_year)?;
    println!("Output file will be saved to: {}", absolute_output.display());

    // 4. Run FFmpeg process with live progress tracking
    let display_title = film_name.as_deref().unwrap_or("Unknown DVD Title");
    run_ffmpeg_with_progress(&args, &dvd_path, &absolute_output, display_title)?;

    Ok(())
}
