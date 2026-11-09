//! Single Responsibility (SRP/SOLID): Application CLI execution runner and interactive candidate resolver.

use crate::AppContainer;
use crate::cli::Args;
use crate::domain::ports::post_processor::PostProcessContext;
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;

/// Interactive and automated CLI metadata resolution coordinator.
pub struct CliMetadataResolver<'a> {
    container: &'a AppContainer,
}

impl<'a> CliMetadataResolver<'a> {
    pub fn new(container: &'a AppContainer) -> Self {
        Self { container }
    }

    /// Resolves film metadata, release year, running time, and cover poster bytes for CLI mode execution.
    pub fn resolve_metadata(
        &self,
        args: &mut Args,
        volume_label: Option<&str>,
    ) -> (Option<String>, Option<u32>, Option<f64>, Option<Vec<u8>>) {
        let dvd_path = std::path::Path::new(&args.input);
        let fp_hash = self.container.disc_inspector.compute_fingerprint(dvd_path);

        // 1. Direct explicit IMDb ID selection takes highest precedence
        if let Some(ref imdb_id) = args.imdb_id {
            println!("Looking up exact IMDb ID: {}", imdb_id);
            if let Some(meta) = crate::imdb::lookup_omdb_by_id(imdb_id) {
                let clean_name = crate::utils::sanitize_filename(&meta.title);
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

        // 2. Disc Fingerprint Cache Auto-Match
        if let Some(cached_meta) = crate::imdb::lookup_fingerprint_cache(&fp_hash) {
            let clean_name = crate::utils::sanitize_filename(&cached_meta.title);
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

            let candidates = crate::imdb::fetch_search_candidates(&query);
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
                && let Some(meta) = crate::imdb::lookup_omdb_by_id(&imdb_id)
            {
                let clean_name = crate::utils::sanitize_filename(&meta.title);
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
                crate::imdb::save_fingerprint_cache(&fp_hash, &meta);
                return (
                    Some(clean_name),
                    meta.year,
                    meta.runtime_secs,
                    meta.poster_bytes,
                );
            }

            // Fallback to lookup_film_details if no candidate selected or search returned empty
            match crate::imdb::lookup_film_details(&query) {
                Ok(meta) => {
                    let clean_name = crate::utils::sanitize_filename(&meta.title);
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
                    crate::imdb::save_fingerprint_cache(&fp_hash, &meta);
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
}

/// Orchestrates command-line application execution modes (Single Title, TV Series, ISO dump, Diagnostics).
pub struct CliApplicationRunner {
    container: Arc<AppContainer>,
}

impl CliApplicationRunner {
    pub fn new(container: Arc<AppContainer>) -> Self {
        Self { container }
    }

    /// Executes the CLI workflow based on parsed arguments.
    pub fn run(&self, mut args: Args) -> Result<()> {
        let dvd_path = crate::dvd::normalize_dvd_path(&args.input);
        if !dvd_path.exists() {
            return Err(anyhow!(
                "DVD drive or path does not exist: {}",
                dvd_path.display()
            ));
        }
        println!("Target DVD path: {}", dvd_path.display());

        if args.check_protection {
            println!("\n--- [Disc Copy Protection Diagnostic Report] ---");
            let report = self.container.disc_inspector.inspect_protection(&dvd_path);
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
            match self.container.drive_benchmark.benchmark(&dvd_path, 10) {
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

        let volume_label = self.container.disc_inspector.read_volume_label(&dvd_path);
        let resolver = CliMetadataResolver::new(&self.container);
        let (title_name, title_year, film_runtime, poster_bytes) =
            resolver.resolve_metadata(&mut args, volume_label.as_deref());

        if title_name.is_none() {
            return Err(anyhow!(
                "Ripping disabled: No movie searched and selected. Please specify --search <QUERY> or --imdb-id <ID> to select a movie."
            ));
        }

        let mut last_output_file: Option<PathBuf> = None;

        // Check disk space threshold guard with optional fallback directory
        let fb_dir = args.fallback_out_dir.as_deref().map(std::path::Path::new);
        let resolved_out_dir = crate::utils::check_disk_space_guard_with_fallback(
            std::path::Path::new(&args.out_dir),
            fb_dir,
            args.min_free_gb,
        )?;
        args.out_dir = resolved_out_dir.to_string_lossy().to_string();

        if args.iso {
            let absolute_output =
                crate::ffmpeg::resolve_output_path(&args, title_name.as_deref(), title_year)?;
            println!(
                "Archival ISO dump will be saved to: {}",
                absolute_output.display()
            );
            let mut builder =
                crate::application::dump_iso::DumpIsoRequest::builder(&dvd_path, &absolute_output)
                    .min_free_gb(args.min_free_gb);
            if let Some(ref label) = volume_label {
                builder = builder.volume_label(label);
            }
            if let Some(ref title) = title_name {
                builder = builder.display_title(title);
            }
            let request = builder.build()?;
            self.container
                .dump_iso_use_case()
                .execute(&request, None, None)?;
            println!("\nSuccessfully completed archival 1:1 ISO dump of disc!");
            last_output_file = Some(absolute_output);
        } else if args.tv {
            let show_name = title_name.as_deref().unwrap_or("TV Show");

            if args.auto_boxset && args.start_episode == 1 {
                let next_ep = crate::queue::get_next_boxset_episode(show_name, args.season);
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

                let episodes = crate::ffmpeg::detect_tv_episodes(
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
                let mut req_builder = crate::application::rip_tv_batch::RipTvBatchRequest::builder(
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
                .track_boxset(args.auto_boxset);

                if let Some(end_ep) = args.end_episode {
                    req_builder = req_builder.end_episode(end_ep);
                }

                if let Some(year) = title_year {
                    req_builder = req_builder.metadata(crate::domain::FilmMetadata {
                        title: show_name.to_string(),
                        year: Some(year),
                        is_series: true,
                        ..Default::default()
                    });
                }

                let request = req_builder.build()?;

                let ripped_count = self
                    .container
                    .rip_tv_batch_use_case()
                    .execute(&request, None, None)?;
                println!(
                    "\nSuccessfully completed batch rip of {} episodes!",
                    ripped_count
                );

                if let Some(first_ep) = episodes.first() {
                    let sample_output = crate::ffmpeg::resolve_tv_output_path_with_title(
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
                let ep_title = self
                    .container
                    .tv_metadata_provider
                    .find_episode_title(show_name, args.season, ep_num)
                    .ok()
                    .flatten();
                let ep_output = crate::ffmpeg::resolve_tv_output_path_with_title(
                    &args,
                    Some(show_name),
                    title_year,
                    args.season,
                    ep_num,
                    ep_title.as_deref(),
                )?;
                println!("Output file will be saved to: {}", ep_output.display());

                let ep_name = crate::utils::format_episode_name_with_title(
                    show_name,
                    args.season,
                    ep_num,
                    ep_title.as_deref(),
                );
                let mut req_builder =
                    crate::application::rip_single_title::RipSingleTitleRequest::builder(
                        &dvd_path, args.title, &ep_output,
                    )
                    .display_title(&ep_name)
                    .min_free_gb(args.min_free_gb);
                if let Some(dur) = film_runtime {
                    req_builder = req_builder.expected_duration(dur);
                }
                let req = req_builder.build()?;
                self.container
                    .rip_single_title_use_case()
                    .execute(&req, None, None)?;
                let _ = crate::ocr::process_subtitle_ocr_sidecar(&args, &ep_output, &ep_name);
                last_output_file = Some(ep_output);
            }
        } else {
            let absolute_output =
                crate::ffmpeg::resolve_output_path(&args, title_name.as_deref(), title_year)?;
            println!(
                "Output file will be saved to: {}",
                absolute_output.display()
            );

            let display_title = title_name.as_deref().unwrap_or("Unknown DVD Title");
            let mut req_builder =
                crate::application::rip_single_title::RipSingleTitleRequest::builder(
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
            self.container
                .rip_single_title_use_case()
                .execute(&req, None, None)?;
            let _ =
                crate::ocr::process_subtitle_ocr_sidecar(&args, &absolute_output, display_title);
            last_output_file = Some(absolute_output);
        }

        let post_title = title_name.unwrap_or_else(|| "DVD Rip".to_string());
        let post_context = PostProcessContext::from_args(
            &args,
            last_output_file,
            &post_title,
            title_year,
            dvd_path,
            poster_bytes,
        );
        let _ = self.container.post_processor.process(&post_context);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_runner_nonexistent_drive() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let runner = CliApplicationRunner::new(container);

        let mut args = Args::default();
        args.input = "Z:\\DefinitelyNonExistentDrive_987".to_string();

        let res = runner.run(args);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_cli_runner_benchmark_flag() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let runner = CliApplicationRunner::new(container);

        let temp = std::env::temp_dir();
        let mut args = Args::default();
        args.input = temp.to_string_lossy().to_string();
        args.benchmark = true;

        let res = runner.run(args);
        assert!(res.is_ok());
    }

    #[test]
    fn test_cli_runner_check_protection_flag_no_selection() {
        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let runner = CliApplicationRunner::new(container);

        let temp = std::env::temp_dir().join(format!(
            "test_unsearchable_xyz_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp);
        let mut args = Args::default();
        args.input = temp.to_string_lossy().to_string();
        args.check_protection = true;

        // Will print protection notes, then fail because no movie was selected
        let res = runner.run(args);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("No movie searched and selected")
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_cli_metadata_resolver_select_index() {
        let container = AppContainer::build_production("ffmpeg");
        let resolver = CliMetadataResolver::new(&container);

        let mut args = Args::default();
        args.input = "Z:\\Disc_Unique_Path_1".to_string();
        args.search = Some("Aliens".to_string());
        args.select_index = Some(1);

        let (title, year, _runtime, _bytes) = resolver.resolve_metadata(&mut args, None);
        if let Some(t) = title {
            assert!(!t.is_empty());
            assert!(year.is_some());
        }

        // Invalid select index falls back to lookup_film_details
        let mut args2 = Args::default();
        args2.input = "Z:\\Disc_Unique_Path_2".to_string();
        args2.search = Some("Aliens".to_string());
        args2.select_index = Some(999);
        let (title2, _, _, _) = resolver.resolve_metadata(&mut args2, None);
        if let Some(t2) = title2 {
            assert!(!t2.is_empty());
        }

        // Completely unmatched search query returns None
        let mut args3 = Args::default();
        args3.input = "Z:\\Disc_Unique_Path_3".to_string();
        args3.search = Some("xyznonexistentmoviequery999888".to_string());
        let (title3, _, _, _) = resolver.resolve_metadata(&mut args3, None);
        assert!(title3.is_none());
    }

    #[test]
    fn test_cli_metadata_resolver_exact_imdb_id() {
        let container = AppContainer::build_production("ffmpeg");
        let resolver = CliMetadataResolver::new(&container);

        let mut args = Args::default();
        args.input = format!(
            "Z:\\Unique_Test_Imdb_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        args.imdb_id = Some("tt0090605".to_string()); // Aliens (1986)

        let (title, year, _runtime, _bytes) = resolver.resolve_metadata(&mut args, None);
        if let Some(t) = title {
            assert!(!t.is_empty());
            assert_eq!(year, Some(1986));
        }

        // Nonexistent IMDb ID
        let mut args2 = Args::default();
        args2.input = format!(
            "Z:\\Unique_Test_Imdb_Nonexistent_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        args2.imdb_id = Some("tt00000000000".to_string());
        let (title2, _, _, _) = resolver.resolve_metadata(&mut args2, None);
        assert!(title2.is_none());
    }

    #[test]
    fn test_cli_runner_iso_execution() {
        let temp_dvd = std::env::temp_dir().join(format!(
            "test_cli_dvd_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let temp_out = std::env::temp_dir().join(format!(
            "test_cli_out_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let video_ts = temp_dvd.join("VIDEO_TS");
        let _ = std::fs::create_dir_all(&video_ts);
        let _ = std::fs::create_dir_all(&temp_out);
        let _ = std::fs::write(video_ts.join("VIDEO_TS.IFO"), b"DUMMY_IFO");

        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let runner = CliApplicationRunner::new(container);

        let mut args = Args::default();
        args.input = temp_dvd.to_string_lossy().to_string();
        args.out_dir = temp_out.to_string_lossy().to_string();
        args.search = Some("Aliens".to_string());
        args.select_index = Some(1);
        args.iso = true;
        args.min_free_gb = 0;

        // Will attempt archival ISO dump on dummy directory and exercise pipeline
        let _ = runner.run(args);

        let _ = std::fs::remove_dir_all(&temp_dvd);
        let _ = std::fs::remove_dir_all(&temp_out);
    }

    #[test]
    fn test_cli_runner_tv_execution() {
        let temp_dvd = std::env::temp_dir().join(format!(
            "test_cli_tv_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let temp_out = std::env::temp_dir().join(format!(
            "test_cli_tv_out_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dvd);
        let _ = std::fs::create_dir_all(&temp_out);

        let container = Arc::new(AppContainer::build_production("ffmpeg"));
        let runner = CliApplicationRunner::new(container);

        let mut args = Args::default();
        args.input = temp_dvd.to_string_lossy().to_string();
        args.out_dir = temp_out.to_string_lossy().to_string();
        args.search = Some("Firefly".to_string());
        args.select_index = Some(1);
        args.tv = true;
        args.season = 1;
        args.start_episode = 1;
        args.all_episodes = false;
        args.min_free_gb = 0;
        args.auto_boxset = true;

        // Will attempt to transcode single episode on dummy drive and gracefully return error
        let _ = runner.run(args);

        let _ = std::fs::remove_dir_all(&temp_dvd);
        let _ = std::fs::remove_dir_all(&temp_out);
    }
}
