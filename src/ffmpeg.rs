/**
 * @file ffmpeg.rs
 * @brief FFmpeg process invocation and real-time progress parsing.
 */

use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Context, Result};

use crate::cli::Args;
use crate::utils::{extract_kv_field, parse_duration};

/// Resolves the absolute output file path based on detected film metadata, configured output directory, or user CLI args.
pub fn resolve_output_path(
    args: &Args,
    film_name: Option<&str>,
    film_year: Option<u32>,
) -> Result<PathBuf> {
    let extension = if args.transcode { "mp4" } else { "mpg" };
    let rel_or_abs_file = if let Some(name) = film_name {
        let segment = if let Some(year) = film_year {
            format!("{} ({})", name, year)
        } else {
            name.to_string()
        };
        PathBuf::from(format!("{}/{}.{}", segment, segment, extension))
    } else if let Some(ref out) = args.output {
        PathBuf::from(out)
    } else {
        PathBuf::from(format!("output.{}", extension))
    };

    ensure_absolute_parent_dir(&args.out_dir, rel_or_abs_file)
}

/// Resolves the absolute output file path for a TV series episode (e.g. TV/The Office (2005)/Season 01/The Office - S01E01.mpg).
pub fn resolve_tv_output_path(
    args: &Args,
    show_name: Option<&str>,
    show_year: Option<u32>,
    season: u32,
    episode_num: u32,
) -> Result<PathBuf> {
    let extension = if args.transcode { "mp4" } else { "mpg" };
    let name = show_name.unwrap_or("Unknown Show");
    let show_folder = if let Some(year) = show_year {
        format!("{} ({})", name, year)
    } else {
        name.to_string()
    };
    let season_folder = format!("Season {:02}", season);
    let filename = format!("{} - S{:02}E{:02}.{}", name, season, episode_num, extension);

    let root_dir = if args.out_dir == "Films" {
        "TV"
    } else {
        &args.out_dir
    };

    let rel_file = PathBuf::from(root_dir)
        .join(show_folder)
        .join(season_folder)
        .join(filename);

    ensure_absolute_parent_dir(root_dir, rel_file)
}

/// Helper: Ensures parent directories exist and returns absolute path.
fn ensure_absolute_parent_dir(base_dir: &str, path: PathBuf) -> Result<PathBuf> {
    let absolute_output = if path.is_absolute() {
        path
    } else {
        let target = PathBuf::from(base_dir).join(path);
        if target.is_absolute() {
            target
        } else {
            std::env::current_dir()?.join(target)
        }
    };

    if let Some(parent) = absolute_output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output parent directory")?;
    }

    Ok(absolute_output)
}

/// Structure representing a detected TV episode title on a DVD disc.
#[derive(Debug, Clone)]
pub struct TvEpisodeInfo {
    pub title_num: u32,
    pub episode_num: u32,
    pub duration_secs: f64,
    pub formatted_name: String,
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
    let mut title_durations: Vec<(u32, f64)> = Vec::new();
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
                            // Only consider titles >= 10 minutes (600 seconds)
                            if secs >= 600.0 {
                                title_durations.push((t, secs));
                            }
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
    let mut best_title = 1u32;
    let mut best_diff = f64::MAX;
    let mut max_duration = 0.0f64;
    let mut consecutive_failures = 0;

    for t in 1..=99 {
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
                            if let Some(target) = expected_runtime_secs {
                                let diff = (secs - target).abs();
                                if diff < best_diff {
                                    best_diff = diff;
                                    best_title = t;
                                }
                            } else if secs > max_duration {
                                max_duration = secs;
                                best_title = t;
                            }
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
    cmd.arg("-map").arg("0:a?");

    if args.transcode {
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
                cmd.arg("-c:v").arg("libx264");
                cmd.arg("-preset").arg(&args.preset);
                cmd.arg("-crf").arg("22");
                cmd.arg("-c:a").arg("aac");
                cmd.arg("-b:a").arg("128k");
            }
        }
    } else {
        cmd.arg("-c").arg("copy");
        cmd.arg("-f").arg("dvd");
    }

    cmd.arg("-y");
    cmd.arg(absolute_output);

    cmd
}

/// Event emitted during FFmpeg ripping process.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Log(String),
    Metadata(crate::imdb::FilmMetadata),
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
        let mut byte = [0u8; 1];
        let mut read_bytes = 0;

        loop {
            match reader.read_exact(&mut byte) {
                Ok(_) => {
                    read_bytes += 1;
                    if byte[0] == b'\r' || byte[0] == b'\n' {
                        break;
                    }
                    line_bytes.push(byte[0]);
                }
                Err(_) => break,
            }
        }

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

                    if tx.is_none() {
                        let width = 30;
                        let filled = ((percent / 100.0) * width as f64).round() as usize;
                        let empty = width - filled;
                        print!(
                            "\rProgress: [{}{}] {:.1}% | FPS: {} | Speed: {}",
                            "█".repeat(filled),
                            "░".repeat(empty),
                            percent,
                            fps,
                            speed
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
}
