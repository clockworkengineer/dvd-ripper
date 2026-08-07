use std::path::PathBuf;
use std::process::Command;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde::Deserialize;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

#[derive(Parser, Debug)]
#[command(
    name = "dvd-ripper",
    version,
    about = "Rips a DVD title using FFmpeg's dvdvideo demuxer and creates an MPEG/MPEG-4 file"
)]
struct Args {
    /// DVD drive letter or root path (e.g., D: or D:\)
    #[arg(default_value = "D:\\")]
    input: String,

    /// Output file path. Defaults to output.mp4 (or output.mpg for copy). Overridden if film details are auto-detected.
    #[arg(short, long)]
    output: Option<String>,

    /// Specific DVD title number to rip (e.g. 1). 0 defaults to auto-select Title 1.
    #[arg(short, long, default_value_t = 1)]
    title: u32,

    /// Re-encode the video/audio instead of doing a fast lossless stream copy
    #[arg(long)]
    transcode: bool,

    /// FFmpeg preset for H.264 encoding (e.g. veryfast, superfast, ultrafast, fast, medium)
    #[arg(long, default_value = "veryfast")]
    preset: String,

    /// Custom path to FFmpeg executable
    #[arg(long, default_value = "ffmpeg")]
    ffmpeg: String,
}

#[derive(Deserialize, Debug)]
struct ImdbEntry {
    l: String,
    y: Option<u32>,
    q: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ImdbSuggestResponse {
    d: Vec<ImdbEntry>,
}

fn get_volume_label(root_path: &str) -> Option<String> {
    let mut path_wide: Vec<u16> = root_path.encode_utf16().collect();
    if !path_wide.ends_with(&[b'\\' as u16]) {
        path_wide.push(b'\\' as u16);
    }
    path_wide.push(0);

    let mut volume_name = [0u16; 260];
    
    unsafe {
        let result = GetVolumeInformationW(
            PCWSTR::from_raw(path_wide.as_ptr()),
            Some(&mut volume_name),
            None,
            None,
            None,
            None,
        );
        if result.is_ok() {
            let name = String::from_utf16_lossy(&volume_name);
            let trimmed = name.trim_matches(char::from(0)).trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn lookup_film_details(query: &str) -> Result<(String, Option<u32>)> {
    let cleaned: String = query
        .replace('_', " ")
        .replace('-', " ")
        .trim()
        .to_lowercase();
    
    if cleaned.is_empty() {
        return Err(anyhow!("Cleaned query is empty"));
    }

    let first_char = cleaned.chars().next().ok_or_else(|| anyhow!("Empty query"))?;
    
    let mut url = reqwest::Url::parse("https://sg.media-imdb.com")?;
    url.set_path(&format!("suggests/{}/{}.json", first_char, cleaned));

    let response_text = reqwest::blocking::get(url)
        .context("Failed to send request to IMDb Suggest API")?
        .text()
        .context("Failed to read response body from IMDb Suggest API")?;

    let start_idx = response_text.find('{')
        .ok_or_else(|| anyhow!("Invalid JSONP response from IMDb: opening bracket not found"))?;
    let end_idx = response_text.rfind('}')
        .ok_or_else(|| anyhow!("Invalid JSONP response from IMDb: closing bracket not found"))?;

    if start_idx >= end_idx {
        return Err(anyhow!("Invalid JSONP response bounds"));
    }

    let json_str = &response_text[start_idx..=end_idx];
    let parsed: ImdbSuggestResponse = serde_json::from_str(json_str)
        .context("Failed to parse IMDb Suggest JSON response")?;

    let best_match = parsed.d.iter()
        .find(|entry| entry.q.as_deref() == Some("feature"))
        .or_else(|| parsed.d.first())
        .ok_or_else(|| anyhow!("No match found on IMDb for query: {}", query))?;

    Ok((best_match.l.clone(), best_match.y))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            ':' => ' ', // Replace colons with spaces
            '\\' | '/' | '*' | '?' | '"' | '<' | '>' | '|' => '_', // Replace other invalid characters
            _ => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_duration(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        let hours: f64 = parts[0].trim().parse().ok()?;
        let minutes: f64 = parts[1].trim().parse().ok()?;
        let seconds: f64 = parts[2].trim().parse().ok()?;
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    } else {
        None
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Resolve DVD path
    let mut dvd_path = PathBuf::from(&args.input);
    if dvd_path.to_string_lossy().ends_with(':') {
        dvd_path = PathBuf::from(format!("{}\\", args.input));
    }

    if !dvd_path.exists() {
        return Err(anyhow!("DVD drive or path does not exist: {}", dvd_path.display()));
    }

    println!("Target DVD path: {}", dvd_path.display());

    // Try to auto-detect film name and year from the volume label
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
                println!("Warning: Failed to look up film details for label '{}': {}", label, e);
            }
        }
    } else {
        println!("Warning: Could not detect DVD volume label.");
    }

    // 2. Resolve output path to absolute path
    let extension = if args.transcode { "mp4" } else { "mpg" };
    let output_path = if let Some(ref name) = film_name {
        let segment = if let Some(year) = film_year {
            format!("{} ({})", name, year)
        } else {
            name.clone()
        };
        PathBuf::from(format!("{}/{}.{}", segment, segment, extension))
    } else if let Some(ref out) = args.output {
        PathBuf::from(out)
    } else {
        PathBuf::from(format!("output.{}", extension))
    };

    let absolute_output = if output_path.is_absolute() {
        output_path.clone()
    } else {
        std::env::current_dir()?.join(&output_path)
    };
    
    // Ensure parent directory of output exists
    if let Some(parent) = absolute_output.parent() {
        std::fs::create_dir_all(parent).context("Failed to create output parent directory")?;
    }
    println!("Output file will be saved to: {}", absolute_output.display());

    // 3. Build FFmpeg command
    let mut cmd = Command::new(&args.ffmpeg);
    
    // Use FFmpeg's native DVD-Video demuxer
    cmd.arg("-f").arg("dvdvideo");
    
    // Specify the title number
    if args.title > 0 {
        cmd.arg("-title").arg(args.title.to_string());
    }

    // Input is the DVD drive path
    cmd.arg("-i").arg(&dvd_path);

    // Map all video and audio streams
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
        // Force DVD-Video program stream format for compatibility (handles AC3 audio well in MPEG container)
        cmd.arg("-f").arg("dvd");
    }

    // Overwrite output file if it exists
    cmd.arg("-y");
    cmd.arg(&absolute_output);

    println!("\nRunning command: {} {:?}", args.ffmpeg, cmd.get_args().collect::<Vec<_>>());

    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn FFmpeg process. Is it installed and in your PATH?")?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("Failed to capture FFmpeg stderr"))?;

    use std::io::{BufRead, BufReader, Read};
    let mut reader = BufReader::new(stderr);
    let mut total_seconds: Option<f64> = None;

    let target_film_info = film_name.as_deref().unwrap_or("Unknown DVD Title");
    println!("\nRipping Film: {}", target_film_info);

    let mut line_bytes = Vec::new();
    loop {
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

        let line = String::from_utf8_lossy(&line_bytes);

        if total_seconds.is_none() {
            if let Some(idx) = line.find("Duration: ") {
                let sub = &line[idx + 10..];
                if let Some(comma_idx) = sub.find(',') {
                    let duration_str = sub[..comma_idx].trim();
                    if let Some(secs) = parse_duration(duration_str) {
                        total_seconds = Some(secs);
                    }
                }
            }
        }

        if let Some(idx) = line.find("time=") {
            let sub = &line[idx + 5..];
            let time_str = sub.split_whitespace().next().unwrap_or("").trim();
            if let Some(secs) = parse_duration(time_str) {
                let mut speed = "N/A".to_string();
                if let Some(s_idx) = line.find("speed=") {
                    let s_sub = &line[s_idx + 6..];
                    speed = s_sub.split_whitespace().next().unwrap_or("N/A").trim().to_string();
                }

                let mut fps = "N/A".to_string();
                if let Some(f_idx) = line.find("fps=") {
                    let f_sub = &line[f_idx + 4..];
                    fps = f_sub.split_whitespace().next().unwrap_or("N/A").trim().to_string();
                }

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
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
            }
        }
    }

    let status = child.wait().context("Failed to wait on FFmpeg process")?;
    println!();

    if status.success() {
        println!("\nSuccess! DVD ripped successfully to: {}", absolute_output.display());
    } else {
        return Err(anyhow!("FFmpeg exited with non-zero status code: {:?}", status.code()));
    }

    Ok(())
}
