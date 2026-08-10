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

    let absolute_output = if rel_or_abs_file.is_absolute() {
        rel_or_abs_file
    } else {
        let target = PathBuf::from(&args.out_dir).join(rel_or_abs_file);
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

/// Builds the FFmpeg Command configured with arguments according to CLI options.
pub fn build_ffmpeg_command(args: &Args, dvd_path: &Path, absolute_output: &Path) -> Command {
    let mut cmd = Command::new(&args.ffmpeg);

    cmd.arg("-f").arg("dvdvideo");

    if args.title > 0 {
        cmd.arg("-title").arg(args.title.to_string());
    }

    cmd.arg("-i").arg(dvd_path);
    cmd.arg("-map").arg("0:v");
    cmd.arg("-map").arg("0:a?");

    if args.transcode {
        println!("\nTranscoding to high-quality H.264 video & AAC audio...");
        cmd.arg("-c:v").arg("libx264");
        cmd.arg("-preset").arg(&args.preset);
        cmd.arg("-crf").arg("22");
        cmd.arg("-c:a").arg("aac");
        cmd.arg("-b:a").arg("128k");
    } else {
        println!("\nPerforming fast, lossless stream copy (remuxing)...");
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
) -> Result<()> {
    run_ffmpeg_with_channel(args, dvd_path, absolute_output, display_title, None, None)
}

/// Executes FFmpeg child process, sending events over a channel and allowing cancellation.
pub fn run_ffmpeg_with_channel(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    display_title: &str,
    tx: Option<std::sync::mpsc::Sender<ProgressEvent>>,
    cancel_rx: Option<std::sync::mpsc::Receiver<()>>,
) -> Result<()> {
    let mut cmd = build_ffmpeg_command(args, dvd_path, absolute_output);

    let cmd_line = format!("Running command: {} {:?}", args.ffmpeg, cmd.get_args().collect::<Vec<_>>());
    println!("\n{}", cmd_line);
    if let Some(ref sender) = tx {
        let _ = sender.send(ProgressEvent::Log(cmd_line));
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

    let rip_line = format!("Ripping Film: {}", display_title);
    println!("\n{}", rip_line);
    if let Some(ref sender) = tx {
        let _ = sender.send(ProgressEvent::Log(rip_line));
    }

    let mut line_bytes = Vec::new();
    loop {
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

                    if let Some(ref sender) = tx {
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
    println!();

    if status.success() {
        let succ_msg = format!("Success! DVD ripped successfully to: {}", absolute_output.display());
        println!("\n{}", succ_msg);
        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Success(absolute_output.to_path_buf()));
        }
        Ok(())
    } else {
        let err_msg = format!("FFmpeg exited with non-zero status code: {:?}", status.code());
        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Error(err_msg.clone()));
        }
        Err(anyhow!(err_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_output_path_default_out_dir() {
        let args = Args {
            input: "D:\\".to_string(),
            output: None,
            out_dir: "Films".to_string(),
            title: 1,
            transcode: false,
            preset: "veryfast".to_string(),
            ffmpeg: "ffmpeg".to_string(),
            cli: false,
        };

        let path = resolve_output_path(&args, Some("The Matrix"), Some(1999)).unwrap();
        assert!(path.ends_with("Films/The Matrix (1999)/The Matrix (1999).mpg") || path.ends_with("Films\\The Matrix (1999)\\The Matrix (1999).mpg"));
    }

    #[test]
    fn test_resolve_output_path_custom_out_dir() {
        let args = Args {
            input: "D:\\".to_string(),
            output: None,
            out_dir: "MyMovies".to_string(),
            title: 1,
            transcode: true,
            preset: "veryfast".to_string(),
            ffmpeg: "ffmpeg".to_string(),
            cli: false,
        };

        let path = resolve_output_path(&args, None, None).unwrap();
        assert!(path.ends_with("MyMovies/output.mp4") || path.ends_with("MyMovies\\output.mp4"));
    }
}
