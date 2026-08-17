/**
 * @file ffmpeg.rs
 * @brief FFmpeg process invocation and real-time progress parsing.
 */

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Context, Result};

use crate::cli::Args;
use crate::utils::{extract_kv_field, format_episode_name, format_title_folder_name, parse_duration};

/// Helper: Ensures parent directories exist and returns absolute path with optional collision incrementing.
fn ensure_absolute_parent_dir(base_dir: &str, path: PathBuf, no_overwrite: bool) -> Result<PathBuf> {
    let mut absolute_output = if path.is_absolute() {
        path
    } else {
        let target = PathBuf::from(base_dir).join(path);
        if target.is_absolute() {
            target
        } else {
            std::env::current_dir()?.join(target)
        }
    };

    if no_overwrite && absolute_output.exists() {
        let parent = absolute_output.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let stem = absolute_output.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = absolute_output.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();

        let mut counter = 1;
        loop {
            let candidate_name = if ext.is_empty() {
                format!("{}_{}", stem, counter)
            } else {
                format!("{}_{}.{}", stem, counter, ext)
            };
            let candidate_path = parent.join(candidate_name);
            if !candidate_path.exists() {
                absolute_output = candidate_path;
                break;
            }
            counter += 1;
        }
    }

    if let Some(parent) = absolute_output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output parent directory")?;
    }

    Ok(absolute_output)
}

/// Resolves the absolute output file path based on detected film metadata, configured output directory, or user CLI args.
pub fn resolve_output_path(
    args: &Args,
    film_name: Option<&str>,
    film_year: Option<u32>,
) -> Result<PathBuf> {
    let extension = if args.mkv {
        "mkv"
    } else if args.transcode {
        "mp4"
    } else {
        "mpg"
    };
    let rel_or_abs_file = if let Some(name) = film_name {
        let segment = format_title_folder_name(name, film_year);
        PathBuf::from(format!("{}/{}.{}", segment, segment, extension))
    } else if let Some(ref out) = args.output {
        PathBuf::from(out)
    } else {
        PathBuf::from(format!("output.{}", extension))
    };

    ensure_absolute_parent_dir(&args.out_dir, rel_or_abs_file, args.no_overwrite)
}

/// Resolves the absolute output file path for a TV series episode (e.g. TV/The Office (2005)/Season 01/The Office - S01E01.mpg).
pub fn resolve_tv_output_path(
    args: &Args,
    show_name: Option<&str>,
    show_year: Option<u32>,
    season: u32,
    episode_num: u32,
) -> Result<PathBuf> {
    let extension = if args.mkv {
        "mkv"
    } else if args.transcode {
        "mp4"
    } else {
        "mpg"
    };
    let name = show_name.unwrap_or("Unknown Show");
    let show_folder = format_title_folder_name(name, show_year);
    let season_folder = format!("Season {:02}", season);
    let filename = format!("{}.{}", format_episode_name(name, season, episode_num), extension);

    let root_dir = if args.out_dir == "Films" {
        "TV"
    } else {
        &args.out_dir
    };

    let rel_file = PathBuf::from(root_dir)
        .join(show_folder)
        .join(season_folder)
        .join(filename);

    ensure_absolute_parent_dir(root_dir, rel_file, args.no_overwrite)
}

/// Structure representing a detected TV episode title on a DVD disc.
#[derive(Debug, Clone)]
pub struct TvEpisodeInfo {
    pub title_num: u32,
    pub episode_num: u32,
    pub duration_secs: f64,
    pub formatted_name: String,
}

/// Structure representing basic probed information for a DVD title track.
#[derive(Debug, Clone)]
pub struct DvdTitleInfo {
    pub title_num: u32,
    pub duration_secs: f64,
}

/// Attempts single-pass fast probing of all DVD title tracks on disc in one process call.
pub fn probe_dvd_titles_fast(
    ffmpeg_path: &str,
    dvd_path: &Path,
) -> Vec<DvdTitleInfo> {
    let mut titles = Vec::new();
    let output = Command::new(ffmpeg_path)
        .stdin(std::process::Stdio::null())
        .arg("-analyzeduration")
        .arg("500000")
        .arg("-probesize")
        .arg("500000")
        .arg("-f")
        .arg("dvdvideo")
        .arg("-i")
        .arg(dvd_path)
        .output();

    if let Ok(out) = output {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut current_title_num: Option<u32> = None;

        for line in stderr.lines() {
            let line_trimmed = line.trim();
            if line_trimmed.starts_with("Program ") || line_trimmed.starts_with("title ") || line_trimmed.starts_with("Title ") {
                if let Some(num_str) = line_trimmed.split_whitespace().nth(1) {
                    let clean_num = num_str.trim_matches(':').trim_matches('#');
                    if let Ok(n) = clean_num.parse::<u32>() {
                        current_title_num = Some(n);
                    }
                }
            }

            if let Some(duration_str) = extract_kv_field(line, "Duration: ") {
                let clean_duration = duration_str.trim_end_matches(',');
                if let Some(secs) = parse_duration(clean_duration) {
                    let t_num = current_title_num.unwrap_or(titles.len() as u32 + 1);
                    titles.push(DvdTitleInfo {
                        title_num: t_num,
                        duration_secs: secs,
                    });
                    current_title_num = None;
                }
            }
        }
    }

    titles
}

/// Probes all titles on the DVD drive, using fast single-pass probing with fallback to sequential probing.
pub fn probe_dvd_titles(
    ffmpeg_path: &str,
    dvd_path: &Path,
    cancel_flag: Option<&std::sync::atomic::AtomicBool>,
) -> Vec<DvdTitleInfo> {
    let fast_results = probe_dvd_titles_fast(ffmpeg_path, dvd_path);
    if !fast_results.is_empty() {
        return fast_results;
    }

    let mut titles = Vec::new();
    let mut consecutive_failures = 0;

    for t in 1..=99 {
        if let Some(flag) = cancel_flag {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
        }
        let output = Command::new(ffmpeg_path)
            .stdin(std::process::Stdio::null())
            .arg("-analyzeduration")
            .arg("500000")
            .arg("-probesize")
            .arg("500000")
            .arg("-f")
            .arg("dvdvideo")
            .arg("-title")
            .arg(t.to_string())
            .arg("-i")
            .arg(dvd_path)
            .output();

        match output {
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let mut found_duration = false;
                for line in stderr.lines() {
                    if let Some(duration_str) = extract_kv_field(line, "Duration: ") {
                        let clean_duration = duration_str.trim_end_matches(',');
                        if let Some(secs) = parse_duration(clean_duration) {
                            found_duration = true;
                            titles.push(DvdTitleInfo {
                                title_num: t,
                                duration_secs: secs,
                            });
                        }
                    }
                }
                if !found_duration {
                    consecutive_failures += 1;
                } else {
                    consecutive_failures = 0;
                }
            }
            Err(_) => {
                consecutive_failures += 1;
            }
        }

        if consecutive_failures >= 3 {
            break;
        }
    }

    titles
}

/// Probes all titles on the DVD drive and filters out intros/outros (<10m) and Play-All composite titles.
pub fn detect_tv_episodes(
    ffmpeg_path: &str,
    dvd_path: &Path,
    show_name: &str,
    season: u32,
    start_ep: u32,
    cancel_flag: Option<&std::sync::atomic::AtomicBool>,
) -> Vec<TvEpisodeInfo> {
    let all_titles = probe_dvd_titles(ffmpeg_path, dvd_path, cancel_flag);
    let title_durations: Vec<(u32, f64)> = all_titles
        .into_iter()
        .filter(|t| t.duration_secs >= 600.0) // Only consider titles >= 10 minutes (600 seconds)
        .map(|t| (t.title_num, t.duration_secs))
        .collect();

    if title_durations.is_empty() {
        return Vec::new();
    }

    // Filter out "Play All" composite titles if multiple single episode titles exist
    let mut durations_only: Vec<f64> = title_durations.iter().map(|(_, d)| *d).collect();
    durations_only.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_duration = durations_only[durations_only.len() / 2];

    let total_titles = title_durations.len();
    let mut episodes = Vec::new();
    let mut current_ep = start_ep;

    for (title_num, secs) in title_durations {
        if total_titles > 1 && secs > median_duration * 1.6 {
            continue;
        }

        let formatted_name = format!("{} - S{:02}E{:02}", show_name, season, current_ep);
        episodes.push(TvEpisodeInfo {
            title_num,
            episode_num: current_ep,
            duration_secs: secs,
            formatted_name,
        });
        current_ep += 1;
    }

    episodes
}

/// Probes the DVD drive to find the title number best matching expected_runtime_secs, or with the longest duration.
pub fn detect_best_title(
    ffmpeg_path: &str,
    dvd_path: &Path,
    expected_runtime_secs: Option<f64>,
) -> u32 {
    let titles = probe_dvd_titles(ffmpeg_path, dvd_path, None);
    if titles.is_empty() {
        return 1;
    }

    let mut best_title = 1u32;
    let mut best_diff = f64::MAX;
    let mut max_duration = 0.0f64;

    for t in titles {
        if let Some(target) = expected_runtime_secs {
            let diff = (t.duration_secs - target).abs();
            if diff < best_diff {
                best_diff = diff;
                best_title = t.title_num;
            }
        } else if t.duration_secs > max_duration {
            max_duration = t.duration_secs;
            best_title = t.title_num;
        }
    }

    best_title
}

/// Helper for longest duration title detection.
#[allow(dead_code)]
pub fn detect_longest_title(ffmpeg_path: &str, dvd_path: &Path) -> u32 {
    detect_best_title(ffmpeg_path, dvd_path, None)
}

/// Builds the FFmpeg Command configured with arguments according to CLI options.
pub fn build_ffmpeg_command(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    resolved_title: u32,
) -> Command {
    let mut cmd = Command::new(&args.ffmpeg);

    cmd.arg("-f").arg("dvdvideo");

    if resolved_title > 0 {
        cmd.arg("-title").arg(resolved_title.to_string());
    }

    cmd.arg("-i").arg(dvd_path);
    cmd.arg("-map").arg("0:v");

    // Audio stream mapping
    if args.dual_audio {
        cmd.arg("-map").arg("0:a:0?");
        cmd.arg("-c:a:0").arg("aac");
        cmd.arg("-b:a:0").arg("192k");
        cmd.arg("-ac:a:0").arg("2");
        if args.normalize_audio {
            cmd.arg("-filter:a:0").arg("loudnorm=I=-16:TP=-1.5:LRA=11");
        }
        cmd.arg("-metadata:s:a:0").arg("title=Stereo AAC (Normalized)");

        cmd.arg("-map").arg("0:a:0?");
        cmd.arg("-c:a:1").arg("copy");
        cmd.arg("-metadata:s:a:1").arg("title=5.1 Surround Passthrough");
    } else if args.all_audio {
        cmd.arg("-map").arg("0:a");
        if args.normalize_audio {
            cmd.arg("-filter:a").arg("loudnorm=I=-16:TP=-1.5:LRA=11");
        }
    } else if let Some(ref lang) = args.audio_lang {
        cmd.arg("-map").arg(format!("0:a:m:language:{}", lang));
        if args.normalize_audio {
            cmd.arg("-filter:a").arg("loudnorm=I=-16:TP=-1.5:LRA=11");
        }
    } else {
        cmd.arg("-map").arg("0:a?");
        if args.normalize_audio {
            cmd.arg("-filter:a").arg("loudnorm=I=-16:TP=-1.5:LRA=11");
        }
    }

    // Subtitle stream mapping
    if args.subtitles {
        if let Some(ref lang) = args.sub_lang {
            cmd.arg("-map").arg(format!("0:s:m:language:{}", lang));
        } else {
            cmd.arg("-map").arg("0:s?");
        }
        let sub_codec = match args.sub_format.as_deref().unwrap_or("dvdsub").to_lowercase().as_str() {
            "subrip" | "srt" => "subrip",
            _ => "dvdsub",
        };
        cmd.arg("-c:s").arg(sub_codec);
    }

    let profile = args.profile.to_lowercase();
    let codec = args.codec.to_lowercase();
    let is_mkv = args.mkv
        || profile == "archival"
        || absolute_output
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("mkv"));

    if profile == "archival" {
        cmd.arg("-c").arg("copy");
        cmd.arg("-f").arg("matroska");
    } else if args.transcode || profile == "plex" || profile == "mobile" {
        if profile == "mobile" {
            cmd.arg("-vf").arg("scale=-2:720");
            cmd.arg("-c:v").arg("libx264");
            cmd.arg("-preset").arg(&args.preset);
            cmd.arg("-crf").arg("24");
            cmd.arg("-c:a").arg("aac");
            cmd.arg("-b:a").arg("128k");
        } else if profile == "plex" {
            if codec == "hevc" || codec == "h265" {
                cmd.arg("-c:v").arg("libx265");
            } else if codec == "av1" {
                cmd.arg("-c:v").arg("libsvtav1");
            } else {
                cmd.arg("-c:v").arg("libx264");
            }
            cmd.arg("-preset").arg(&args.preset);
            cmd.arg("-crf").arg("20");
            cmd.arg("-c:a").arg("aac");
            cmd.arg("-b:a").arg("192k");
        } else {
            match args.hwaccel.to_lowercase().as_str() {
                "v4l2" | "v4l2m2m" => {
                    cmd.arg("-c:v").arg("h264_v4l2m2m");
                    cmd.arg("-b:v").arg("4M");
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "vaapi" => {
                    cmd.arg("-vaapi_device").arg("/dev/dri/renderD128");
                    cmd.arg("-vf").arg("format=nv12,hwupload");
                    cmd.arg("-c:v").arg("h264_vaapi");
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "nvenc" => {
                    cmd.arg("-c:v").arg("h264_nvenc");
                    cmd.arg("-preset").arg(&args.preset);
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "qsv" => {
                    cmd.arg("-c:v").arg("h264_qsv");
                    cmd.arg("-preset").arg(&args.preset);
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                _ => {
                    if codec == "hevc" || codec == "h265" {
                        cmd.arg("-c:v").arg("libx265");
                    } else if codec == "av1" {
                        cmd.arg("-c:v").arg("libsvtav1");
                    } else {
                        cmd.arg("-c:v").arg("libx264");
                    }
                    cmd.arg("-preset").arg(&args.preset);
                    cmd.arg("-crf").arg("22");
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
            }
        }

        if is_mkv {
            cmd.arg("-f").arg("matroska");
        }
    } else {
        cmd.arg("-c").arg("copy");
        if is_mkv {
            cmd.arg("-f").arg("matroska");
        } else {
            cmd.arg("-f").arg("dvd");
        }
    }

    cmd.arg("-y");
    cmd.arg(absolute_output);

    cmd
}

/// Event emitted during FFmpeg ripping process or async GUI metadata lookup.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Log(String),
    Metadata(crate::imdb::FilmMetadata),
    SearchResults(Vec<crate::imdb::SearchResultItem>),
    TvEpisodesDetected(Vec<TvEpisodeInfo>),
    Progress {
        percent: f64,
        fps: String,
        speed: String,
    },
    Success(PathBuf),
    Error(String),
}

/// Executes FFmpeg child process and parses output line-by-line to render dynamic progress bar.
pub fn run_ffmpeg_with_progress(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    display_title: &str,
    expected_runtime_secs: Option<f64>,
) -> Result<()> {
    run_ffmpeg_with_channel(
        args,
        dvd_path,
        absolute_output,
        display_title,
        expected_runtime_secs,
        None,
        None,
        None,
        false,
    )
}

/// Executes FFmpeg child process, sending events over a channel and allowing cancellation.
pub fn run_ffmpeg_with_channel(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    display_title: &str,
    expected_runtime_secs: Option<f64>,
    tx: Option<std::sync::mpsc::Sender<ProgressEvent>>,
    cancel_rx: Option<std::sync::mpsc::Receiver<()>>,
    cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    is_batch: bool,
) -> Result<()> {
    let resolved_title = if args.title == 0 {
        let msg = if let Some(target) = expected_runtime_secs {
            format!(
                "Auto-detecting DVD title matching movie running time ({:.0} mins)...",
                target / 60.0
            )
        } else {
            "Auto-detecting title with longest duration on DVD...".to_string()
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Log(msg));
        } else {
            println!("\n{}", msg);
            std::io::stdout().flush().ok();
        }

        let detected = detect_best_title(&args.ffmpeg, dvd_path, expected_runtime_secs);
        let msg2 = if expected_runtime_secs.is_some() {
            format!("Auto-selected Title #{} (matched running time)", detected)
        } else {
            format!("Auto-selected Title #{} (longest duration)", detected)
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Log(msg2));
        } else {
            println!("{}", msg2);
            std::io::stdout().flush().ok();
        }
        detected
    } else {
        args.title
    };

    let mut cmd = build_ffmpeg_command(args, dvd_path, absolute_output, resolved_title);

    let mode_desc = if args.transcode {
        "Transcoding to high-quality H.264 video & AAC audio...".to_string()
    } else {
        "Performing fast, lossless stream copy (remuxing)...".to_string()
    };

    let cmd_line = format!("Running command: {} {:?}", args.ffmpeg, cmd.get_args().collect::<Vec<_>>());
    let rip_line = format!("Ripping Film: {}", display_title);

    if tx.is_none() {
        println!("\n{}", mode_desc);
        println!("\n{}", cmd_line);
        println!("\n{}", rip_line);
    } else if let Some(ref sender) = tx {
        let _ = sender.send(ProgressEvent::Log(mode_desc));
        let _ = sender.send(ProgressEvent::Log(cmd_line));
        let _ = sender.send(ProgressEvent::Log(rip_line));
    }

    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .context("Failed to spawn FFmpeg process. Is it installed and in your PATH?")?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to capture FFmpeg stderr"))?;

    let mut reader = BufReader::new(stderr);
    let mut total_seconds: Option<f64> = None;

    let mut line_bytes = Vec::new();
    loop {
        if let Some(ref flag) = cancel_flag {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = child.kill();
                let msg = "Ripping process cancelled by user.".to_string();
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(msg.clone()));
                }
                return Err(anyhow!(msg));
            }
        }

        if let Some(ref rx) = cancel_rx {
            if rx.try_recv().is_ok() {
                let _ = child.kill();
                let msg = "Ripping process cancelled by user.".to_string();
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(msg.clone()));
                }
                return Err(anyhow!(msg));
            }
        }

        line_bytes.clear();
        let read_bytes = match reader.read_until(b'\r', &mut line_bytes) {
            Ok(n) => n,
            Err(_) => 0,
        };

        if read_bytes == 0 {
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        let line = String::from_utf8_lossy(&line_bytes).to_string();

        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Log(line.clone()));
        }

        // Detect overall duration from initialization log
        if total_seconds.is_none() {
            if let Some(duration_str) = extract_kv_field(&line, "Duration: ") {
                let clean_duration = duration_str.trim_end_matches(',');
                if let Some(secs) = parse_duration(clean_duration) {
                    total_seconds = Some(secs);
                }
            }
        }

        // Parse time=, speed=, and fps= using DRY helper
        if let Some(time_str) = extract_kv_field(&line, "time=") {
            if let Some(secs) = parse_duration(time_str) {
                let speed = extract_kv_field(&line, "speed=").unwrap_or("N/A").to_string();
                let fps = extract_kv_field(&line, "fps=").unwrap_or("N/A").to_string();

                if let Some(total) = total_seconds {
                    let percent = (secs / total * 100.0).min(100.0).max(0.0);

                    crate::api::update_appliance_status("Ripping", "", display_title, percent, &fps, &speed);

                    if tx.is_none() {
                        let width = 30;
                        let filled = ((percent / 100.0) * width as f64).round() as usize;
                        let empty = width - filled;
                        println!(
                            "[Daemon Progress] [{}{}] {:.1}% | FPS: {} | Speed: {} | {}",
                            "█".repeat(filled),
                            "░".repeat(empty),
                            percent,
                            fps,
                            speed,
                            display_title
                        );
                        std::io::stdout().flush().ok();
                    } else if let Some(ref sender) = tx {
                        let _ = sender.send(ProgressEvent::Progress {
                            percent,
                            fps: fps.clone(),
                            speed: speed.clone(),
                        });
                    }
                }
            }
        }
    }

    let status = child.wait().context("Failed to wait on FFmpeg process")?;

    if status.success() {
        let succ_msg = format!("Success! DVD ripped successfully to: {}", absolute_output.display());
        if tx.is_none() {
            println!("\n\n{}", succ_msg);
        } else if let Some(ref sender) = tx {
            if is_batch {
                let _ = sender.send(ProgressEvent::Log(format!("Successfully finished episode: {}", absolute_output.display())));
            } else {
                let _ = sender.send(ProgressEvent::Success(absolute_output.to_path_buf()));
            }
        }
        Ok(())
    } else {
        let err_msg = format!("FFmpeg exited with non-zero status code: {:?}", status.code());
        if tx.is_none() {
            eprintln!("\n{}", err_msg);
        } else if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Error(err_msg.clone()));
        }
        Err(anyhow!(err_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("dvd_ripper_test_{}_{}", name, std::process::id()));
            let _ = fs::remove_dir_all(&path);
            let _ = fs::create_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_resolve_tv_output_path() {
        let temp = TestTempDir::new("tv_out_dir");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let args = Args {
            out_dir: out_dir_str,
            tv: true,
            ..Default::default()
        };

        let path = resolve_tv_output_path(&args, Some("The Office"), Some(2005), 1, 3).unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("The Office (2005)"));
        assert!(path_str.contains("Season 01"));
        assert!(path_str.contains("The Office - S01E03.mpg"));
    }

    #[test]
    fn test_resolve_output_path_custom_out_dir() {
        let temp = TestTempDir::new("custom_out_dir");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let args = Args {
            out_dir: out_dir_str,
            transcode: true,
            ..Default::default()
        };

        let path = resolve_output_path(&args, None, None).unwrap();
        assert!(path.to_string_lossy().contains("output.mp4"));
        assert!(path.parent().map_or(false, |p| p.exists()));
    }

    #[test]
    fn test_build_ffmpeg_command_title_argument() {
        let args = Args::default();

        let output_path = PathBuf::from("Films/Test/Test.mpg");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 3);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

        assert!(cmd_args.contains(&"-title".to_string()));
        let title_idx = cmd_args.iter().position(|r| r == "-title").unwrap();
        assert_eq!(cmd_args[title_idx + 1], "3");
    }

    #[test]
    fn test_build_ffmpeg_command_audio_subtitle_options() {
        let args = Args {
            all_audio: true,
            subtitles: true,
            sub_lang: Some("eng".to_string()),
            ..Default::default()
        };

        let output_path = PathBuf::from("Films/Test/Test.mpg");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

        assert!(cmd_args.contains(&"0:a".to_string()));
        assert!(cmd_args.contains(&"0:s:m:language:eng".to_string()));
        assert!(cmd_args.contains(&"dvdsub".to_string()));
    }

    #[test]
    fn test_resolve_output_path_no_overwrite() {
        let temp = TestTempDir::new("no_overwrite_test");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let dummy_file = temp.0.join("output.mpg");
        std::fs::write(&dummy_file, b"existing").unwrap();

        let args = Args {
            out_dir: out_dir_str,
            no_overwrite: true,
            ..Default::default()
        };

        let path = resolve_output_path(&args, None, None).unwrap();
        assert!(path.to_string_lossy().contains("output_1.mpg"));
    }

    #[test]
    fn test_build_ffmpeg_command_hwaccel_modes() {
        let output_path = PathBuf::from("output.mp4");

        let args_nvenc = Args {
            transcode: true,
            hwaccel: "nvenc".to_string(),
            preset: "fast".to_string(),
            ..Default::default()
        };
        let cmd = build_ffmpeg_command(&args_nvenc, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(cmd_args.contains(&"h264_nvenc".to_string()));

        let args_vaapi = Args {
            transcode: true,
            hwaccel: "vaapi".to_string(),
            ..Default::default()
        };
        let cmd_vaapi = build_ffmpeg_command(&args_vaapi, Path::new("D:\\"), &output_path, 1);
        let cmd_args_vaapi: Vec<String> = cmd_vaapi.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(cmd_args_vaapi.contains(&"h264_vaapi".to_string()));

        let args_qsv = Args {
            transcode: true,
            hwaccel: "qsv".to_string(),
            ..Default::default()
        };
        let cmd_qsv = build_ffmpeg_command(&args_qsv, Path::new("D:\\"), &output_path, 1);
        let cmd_args_qsv: Vec<String> = cmd_qsv.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(cmd_args_qsv.contains(&"h264_qsv".to_string()));
    }

    #[test]
    fn test_resolve_output_path_mkv() {
        let temp = TestTempDir::new("mkv_out_dir");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let args = Args {
            out_dir: out_dir_str,
            mkv: true,
            ..Default::default()
        };

        let path = resolve_output_path(&args, Some("Aliens"), Some(1986)).unwrap();
        assert!(path.to_string_lossy().contains("Aliens (1986).mkv"));
    }

    #[test]
    fn test_build_ffmpeg_command_mkv_subtitles() {
        let args = Args {
            mkv: true,
            subtitles: true,
            sub_lang: Some("eng".to_string()),
            ..Default::default()
        };

        let output_path = PathBuf::from("Films/Test/Test.mkv");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

        assert!(cmd_args.contains(&"-f".to_string()));
        assert!(cmd_args.contains(&"matroska".to_string()));
        assert!(cmd_args.contains(&"dvdsub".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_subrip_subtitles() {
        let args = Args {
            subtitles: true,
            sub_format: Some("subrip".to_string()),
            ..Default::default()
        };

        let output_path = PathBuf::from("Films/Test/Test.mkv");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

        assert!(cmd_args.contains(&"subrip".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_codecs_and_profiles() {
        let args_hevc = Args {
            transcode: true,
            codec: "hevc".to_string(),
            ..Default::default()
        };
        let cmd_hevc = build_ffmpeg_command(&args_hevc, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let args_vec_hevc: Vec<String> = cmd_hevc.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(args_vec_hevc.contains(&"libx265".to_string()));

        let args_av1 = Args {
            transcode: true,
            codec: "av1".to_string(),
            ..Default::default()
        };
        let cmd_av1 = build_ffmpeg_command(&args_av1, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let args_vec_av1: Vec<String> = cmd_av1.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(args_vec_av1.contains(&"libsvtav1".to_string()));

        let args_archival = Args {
            profile: "archival".to_string(),
            ..Default::default()
        };
        let cmd_archival = build_ffmpeg_command(&args_archival, Path::new("D:\\"), Path::new("test.mkv"), 1);
        let args_vec_archival: Vec<String> = cmd_archival.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(args_vec_archival.contains(&"matroska".to_string()));
        assert!(args_vec_archival.contains(&"copy".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_audio_normalization_and_dual_audio() {
        let args_norm = Args {
            normalize_audio: true,
            ..Default::default()
        };
        let cmd_norm = build_ffmpeg_command(&args_norm, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let vec_norm: Vec<String> = cmd_norm.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(vec_norm.contains(&"loudnorm=I=-16:TP=-1.5:LRA=11".to_string()));

        let args_dual = Args {
            dual_audio: true,
            normalize_audio: true,
            ..Default::default()
        };
        let cmd_dual = build_ffmpeg_command(&args_dual, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let vec_dual: Vec<String> = cmd_dual.get_args().map(|s| s.to_string_lossy().to_string()).collect();
        assert!(vec_dual.contains(&"title=Stereo AAC (Normalized)".to_string()));
        assert!(vec_dual.contains(&"title=5.1 Surround Passthrough".to_string()));
    }
}
