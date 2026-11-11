//! Multi-disc stitching, Blu-ray (`BDMV`) command builders, Audio CD FLAC command builders,
//! and UTF-8 `.m3u8` extended playlist generators.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use crate::cli::Args;
use crate::ffmpeg::resolve_video_codec_name;

/// Builds an FFmpeg `concat` demuxer manifest string for stitching multi-disc movie parts.
pub fn build_multi_disc_concat_list(part_files: &[PathBuf]) -> String {
    let mut manifest = String::new();
    for path in part_files {
        let escaped = path.to_string_lossy().replace('\'', "'\\''");
        manifest.push_str(&format!("file '{}'\n", escaped));
    }
    manifest
}

/// Losslessly merges multi-disc movie files (e.g., Disc 1 + Disc 2 extended editions) using FFmpeg `-f concat`.
pub fn merge_multi_disc_files(
    ffmpeg_path: &str,
    part_files: &[PathBuf],
    output_path: &Path,
) -> Result<PathBuf> {
    if part_files.len() < 2 {
        return Err(anyhow!(
            "At least 2 disc part files are required for multi-disc merging."
        ));
    }
    for part in part_files {
        if !part.exists() {
            return Err(anyhow!(
                "Multi-disc part file does not exist: {}",
                part.display()
            ));
        }
    }
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let list_path = std::env::temp_dir().join(format!(
        "dvd_ripper_concat_{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let manifest = build_multi_disc_concat_list(part_files);
    std::fs::write(&list_path, manifest)?;

    let mut cmd = Command::new(ffmpeg_path);
    crate::utils::configure_silent_command(&mut cmd);
    let status = cmd
        .stdin(std::process::Stdio::null())
        .arg("-nostdin")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&list_path)
        .arg("-c")
        .arg("copy")
        .arg(output_path)
        .status();

    let _ = std::fs::remove_file(&list_path);

    let exit = status.context("Failed to spawn FFmpeg concat process")?;
    if !exit.success() {
        return Err(anyhow!(
            "FFmpeg multi-disc concat failed with status: {}",
            exit
        ));
    }

    Ok(output_path.to_path_buf())
}

/// Builds an FFmpeg Command for ripping a Blu-ray (`BDMV`) playlist via the `bluray:` protocol.
pub fn build_bluray_ffmpeg_command(
    args: &Args,
    disc_path: &Path,
    playlist_id: u32,
    output_path: &Path,
) -> Command {
    let mut cmd = Command::new(&args.ffmpeg);
    crate::utils::configure_silent_command(&mut cmd);
    cmd.arg("-nostdin");
    cmd.arg("-stats");
    cmd.arg("-playlist").arg(playlist_id.to_string());
    if let Some(angle) = args.angle
        && angle > 0
    {
        cmd.arg("-angle").arg(angle.to_string());
    }
    cmd.arg("-i")
        .arg(format!("bluray:{}", disc_path.to_string_lossy()));
    cmd.arg("-map").arg("0:v:0?");
    if args.all_audio {
        cmd.arg("-map").arg("0:a?");
    } else {
        cmd.arg("-map").arg("0:a:0?");
    }
    if args.subtitles {
        cmd.arg("-map").arg("0:s?");
    }
    if args.chapters {
        cmd.arg("-map_chapters").arg("0");
    }

    let profile = args.profile.to_lowercase();
    let codec = args.codec.to_lowercase();
    if profile == "archival" || codec == "copy" || !args.transcode {
        cmd.arg("-c").arg("copy");
    } else {
        cmd.arg("-c:v").arg(resolve_video_codec_name(&codec));
        cmd.arg("-preset").arg(&args.preset);
        let crf = args
            .crf
            .map(|c| c.min(51).to_string())
            .unwrap_or_else(|| "20".to_string());
        cmd.arg("-crf").arg(crf);
        cmd.arg("-c:a").arg("aac");
        cmd.arg("-b:a").arg("192k");
    }
    cmd.arg("-y");
    cmd.arg(output_path);
    cmd
}

/// Builds an FFmpeg Command to rip an Audio CD track (`.cda` / `.wav` / `libcdio`) to lossless FLAC with Vorbis tags.
pub fn build_audio_cd_track_command(
    ffmpeg_path: &str,
    source_track: &Path,
    track_num: u32,
    album_title: &str,
    track_title: &str,
    output_flac: &Path,
) -> Command {
    let mut cmd = Command::new(ffmpeg_path);
    crate::utils::configure_silent_command(&mut cmd);
    cmd.arg("-nostdin");
    cmd.arg("-y");
    cmd.arg("-i").arg(source_track);
    cmd.arg("-vn");
    cmd.arg("-c:a").arg("flac");
    cmd.arg("-compression_level").arg("8");
    cmd.arg("-metadata").arg(format!("track={}", track_num));
    if !album_title.trim().is_empty() {
        cmd.arg("-metadata")
            .arg(format!("album={}", album_title.trim()));
    }
    if !track_title.trim().is_empty() {
        cmd.arg("-metadata")
            .arg(format!("title={}", track_title.trim()));
    }
    cmd.arg(output_flac);
    cmd
}

/// Generates a UTF-8 `.m3u8` extended playlist for ripped Audio CD or Concert Chapter tracks.
pub fn generate_m3u8_playlist(tracks: &[(String, f64, PathBuf)]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for (title, duration_secs, path) in tracks {
        let secs = duration_secs.max(0.0).round() as i64;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("track.flac");
        out.push_str(&format!("#EXTINF:{},{}\n{}\n", secs, title, file_name));
    }
    out
}
