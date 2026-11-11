use anyhow::{Context, Result, anyhow};
/**
 * @file ffmpeg.rs
 * @brief FFmpeg process invocation and real-time progress parsing.
 */
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::Args;
use crate::utils::{extract_kv_field, format_title_folder_name, parse_duration};

/// Formats a TV season and episode number into a standardized episode code string (e.g., "S01E05").
#[allow(dead_code)]
pub fn format_episode_code(season: u32, episode: u32) -> String {
    crate::utils::format_episode_code(season, episode)
}

/// Formats a TV show name, season, and episode number into a standardized media filename (e.g., "The Office - S01E05").
#[allow(dead_code)]
pub fn format_episode_filename(show_name: &str, season: u32, episode: u32) -> String {
    crate::utils::format_episode_name(show_name, season, episode)
}

/// Helper: Checks if the path component is named "films" (case-insensitive)
fn is_named_films(path: &Path) -> bool {
    path.file_name()
        .map(|s| s.to_string_lossy().eq_ignore_ascii_case("films"))
        .unwrap_or(false)
}

/// Helper: Checks if the path component is named "tv" or "tv shows" (case-insensitive)
fn is_named_tv(path: &Path) -> bool {
    path.file_name()
        .map(|s| {
            let lossy = s.to_string_lossy();
            lossy.eq_ignore_ascii_case("tv") || lossy.eq_ignore_ascii_case("tv shows")
        })
        .unwrap_or(false)
}

/// Resolves the root destination directory for Film/Movie rips.
/// Defaults to `<out_dir>/Films`. If `out_dir` already ends in `Films`, it is kept as-is.
/// If `out_dir` ends in `TV`, its parent sibling `/Films` is used.
pub fn resolve_films_root(out_dir: &str) -> PathBuf {
    let trimmed = out_dir.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return PathBuf::from("Films");
    }
    let p = Path::new(trimmed);
    if is_named_films(p) {
        p.to_path_buf()
    } else if is_named_tv(p) {
        match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("Films"),
            _ => PathBuf::from("Films"),
        }
    } else {
        p.join("Films")
    }
}

/// Resolves the root destination directory for TV series rips.
/// Defaults to `<out_dir>/TV`. If `out_dir` already ends in `TV`, it is kept as-is.
/// If `out_dir` ends in `Films`, its parent sibling `/TV` is used.
pub fn resolve_tv_root(out_dir: &str) -> PathBuf {
    let trimmed = out_dir.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return PathBuf::from("TV");
    }
    let p = Path::new(trimmed);
    if is_named_tv(p) {
        p.to_path_buf()
    } else if is_named_films(p) {
        match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("TV"),
            _ => PathBuf::from("TV"),
        }
    } else {
        p.join("TV")
    }
}

/// Helper: Ensures parent directories exist and returns absolute path with optional collision incrementing.
fn ensure_absolute_parent_dir(path: PathBuf, no_overwrite: bool) -> Result<PathBuf> {
    let mut absolute_output = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };

    if no_overwrite && absolute_output.exists() {
        let parent = absolute_output
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let stem = absolute_output
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = absolute_output
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

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

/// Helper: Determines whether transcoding (video/audio re-encoding & compression) is enabled.
pub fn is_transcode_enabled(args: &Args) -> bool {
    let profile = args.profile.to_lowercase();
    let codec = args.codec.to_lowercase();

    if profile == "archival" || codec == "copy" {
        false
    } else {
        args.transcode
            || profile == "standard"
            || profile == "plex"
            || profile == "mobile"
            || codec == "h264"
            || codec == "hevc"
            || codec == "h265"
            || codec == "av1"
    }
}

/// Resolves the absolute output file path based on detected film metadata, configured output directory, or user CLI args.
pub fn resolve_output_path(
    args: &Args,
    film_name: Option<&str>,
    film_year: Option<u32>,
) -> Result<PathBuf> {
    let extension = if args.iso {
        "iso"
    } else if args.mkv {
        "mkv"
    } else if is_transcode_enabled(args) {
        "mp4"
    } else {
        "mpg"
    };

    let films_root = resolve_films_root(&args.out_dir);

    let target_candidate = if let Some(name) = film_name {
        let clean_name = crate::utils::sanitize_filename(name);
        let segment = format_title_folder_name(&clean_name, film_year);
        films_root
            .join(&segment)
            .join(format!("{}.{}", segment, extension))
    } else if let Some(ref out) = args.output {
        let out_path = PathBuf::from(out);
        if out_path.is_absolute() {
            out_path
        } else {
            films_root.join(out_path)
        }
    } else {
        films_root.join(format!("output.{}", extension))
    };

    let target_path = ensure_absolute_parent_dir(target_candidate, args.no_overwrite)?;
    crate::utils::ensure_path_contained(&films_root, &target_path)
}

/// Resolves the absolute output file path for a TV series episode with optional episode title.
pub fn resolve_tv_output_path_with_title(
    args: &Args,
    show_name: Option<&str>,
    show_year: Option<u32>,
    season: u32,
    episode_num: u32,
    episode_title: Option<&str>,
) -> Result<PathBuf> {
    let extension = if args.mkv {
        "mkv"
    } else if is_transcode_enabled(args) {
        "mp4"
    } else {
        "mpg"
    };
    let name = show_name.unwrap_or("Unknown Show");
    let clean_show = crate::utils::sanitize_filename(name);
    let show_folder = format_title_folder_name(&clean_show, show_year);
    let season_folder = format!("Season {:02}", season);
    let base_name = crate::utils::format_episode_name_with_title(
        &clean_show,
        season,
        episode_num,
        episode_title,
    );
    let filename = format!("{}.{}", base_name, extension);

    let tv_root = resolve_tv_root(&args.out_dir);

    let target_candidate = tv_root.join(show_folder).join(season_folder).join(filename);

    let target_path = ensure_absolute_parent_dir(target_candidate, args.no_overwrite)?;
    crate::utils::ensure_path_contained(&tv_root, &target_path)
}

/// Resolves the absolute output file path for a TV series episode (e.g. TV/The Office (2005)/Season 01/The Office - S01E01.mpg).
#[allow(dead_code)]
pub fn resolve_tv_output_path(
    args: &Args,
    show_name: Option<&str>,
    show_year: Option<u32>,
    season: u32,
    episode_num: u32,
) -> Result<PathBuf> {
    resolve_tv_output_path_with_title(args, show_name, show_year, season, episode_num, None)
}

/// Structure representing a detected TV episode title on a DVD disc (re-exported from domain).
pub use crate::domain::entities::disc::TvEpisodeInfo;

/// Structure representing basic probed information for a DVD title track (re-exported from domain).
pub use crate::domain::entities::disc::DvdTitleInfo;

/// Attempts single-pass fast probing of all DVD title tracks on disc in one process call.
pub fn probe_dvd_titles_fast(ffmpeg_path: &str, dvd_path: &Path) -> Vec<DvdTitleInfo> {
    let mut titles = Vec::new();
    let mut cmd = Command::new(ffmpeg_path);
    crate::utils::configure_silent_command(&mut cmd);
    let output = cmd
        .stdin(std::process::Stdio::null())
        .arg("-analyzeduration")
        .arg("500000")
        .arg("-probesize")
        .arg("500000")
        .arg("-f")
        .arg("dvdvideo")
        .arg("-trim")
        .arg("0")
        .arg("-i")
        .arg(dvd_path)
        .output();

    if let Ok(out) = output {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut current_title_num: Option<u32> = None;

        for line in stderr.lines() {
            let line_trimmed = line.trim();
            if (line_trimmed.starts_with("Program ")
                || line_trimmed.starts_with("title ")
                || line_trimmed.starts_with("Title "))
                && let Some(num_str) = line_trimmed.split_whitespace().nth(1)
            {
                let clean_num = num_str.trim_matches(':').trim_matches('#');
                if let Ok(n) = clean_num.parse::<u32>() {
                    current_title_num = Some(n);
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

/// Interface Segregation (ISP/SOLID): Segregated trait for DVD title duration queries.
#[allow(dead_code)]
pub trait TitleDurationProvider {
    fn duration_secs(&self) -> f64;
}

impl TitleDurationProvider for DvdTitleInfo {
    fn duration_secs(&self) -> f64 {
        self.duration_secs
    }
}

/// Parses a single line of FFmpeg probe output for Duration patterns (e.g., "Duration: 01:23:45.67").
#[allow(dead_code)]
pub fn parse_title_duration_line(line: &str) -> Option<f64> {
    if let Some(dur_idx) = line.find("Duration: ") {
        let after = &line[dur_idx + 10..];
        let dur_str = after.split(',').next()?.trim();
        parse_duration(dur_str)
    } else {
        None
    }
}

/// Single Responsibility (SRP/SOLID): Encapsulates line-by-line FFmpeg progress output parsing.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct FfmpegProgressParser;

impl FfmpegProgressParser {
    #[allow(dead_code)]
    pub fn parse_line(&self, line: &str) -> Option<crate::utils::FfmpegProgressMetrics> {
        crate::utils::parse_ffmpeg_progress_line(line)
    }
}

/// Formats video filter strings for specific hardware acceleration modes (e.g. VAAPI format=nv12,hwupload).
#[allow(dead_code)]
pub fn format_hwaccel_vf_chain(hwaccel: &str, vf_filters: &[String]) -> Option<String> {
    match hwaccel.to_lowercase().as_str() {
        "vaapi" => {
            if vf_filters.is_empty() {
                Some("format=nv12,hwupload".to_string())
            } else {
                Some(format!("{},format=nv12,hwupload", vf_filters.join(",")))
            }
        }
        _ => {
            if vf_filters.is_empty() {
                None
            } else {
                Some(vf_filters.join(","))
            }
        }
    }
}

/// Resolves target FFmpeg subtitle codec string based on user requested format ("subrip", "srt", "dvdsub").
pub fn resolve_subtitle_codec(sub_format: Option<&str>) -> &'static str {
    match sub_format.unwrap_or("dvdsub").to_lowercase().as_str() {
        "subrip" | "srt" => "subrip",
        _ => "dvdsub",
    }
}

/// Formats a human-readable title auto-detection summary log message.
#[allow(dead_code)]
pub fn format_title_selection_summary(
    detected_title: u32,
    duration_opt: Option<f64>,
    expected_runtime_secs: Option<f64>,
) -> String {
    let dur_str = duration_opt
        .map(|d| format!(" ({:.0} mins)", d / 60.0))
        .unwrap_or_default();
    if expected_runtime_secs.is_some() {
        format!(
            "Auto-selected Title #{}{} (matched running time)",
            detected_title, dur_str
        )
    } else {
        format!(
            "Auto-selected Title #{}{} (longest duration on DVD)",
            detected_title, dur_str
        )
    }
}

/// Encoder quality parameters resolved from encoding profile and codec options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderDefaults {
    pub preset: &'static str,
    pub crf: &'static str,
    pub audio_bitrate: &'static str,
}

/// Trait defining encoder profile configuration contract (OCP/ISP).
#[allow(dead_code)]
pub trait EncoderProfileProvider {
    fn name(&self) -> &str;
    fn get_defaults(&self, profile: &str) -> EncoderDefaults;
}

/// Concrete standard encoder profile provider implementation.
#[derive(Debug, Default)]
pub struct StandardEncoderProfileProvider;

impl EncoderProfileProvider for StandardEncoderProfileProvider {
    fn name(&self) -> &str {
        "Standard FFmpeg Encoder Profile Provider"
    }

    fn get_defaults(&self, profile: &str) -> EncoderDefaults {
        resolve_encoder_defaults(profile)
    }
}

/// Resolves default encoder quality parameters (preset, CRF, audio bitrate) based on profile name.
#[allow(dead_code)]
pub fn resolve_encoder_defaults(profile: &str) -> EncoderDefaults {
    match profile.to_lowercase().as_str() {
        "mobile" => EncoderDefaults {
            preset: "fast",
            crf: "24",
            audio_bitrate: "128k",
        },
        "plex" => EncoderDefaults {
            preset: "medium",
            crf: "20",
            audio_bitrate: "192k",
        },
        _ => EncoderDefaults {
            preset: "medium",
            crf: "22",
            audio_bitrate: "128k",
        },
    }
}

/// Formats a standardized TV show season directory relative path (e.g., "TV/The Office (2005)/Season 01").
#[allow(dead_code)]
pub fn format_tv_season_folder(show_name: &str, year: Option<u32>, season: u32) -> PathBuf {
    let clean_show = crate::utils::sanitize_filename(show_name);
    let folder_name = format_title_folder_name(&clean_show, year);
    PathBuf::from("TV")
        .join(folder_name)
        .join(format!("Season {:02}", season))
}

/// Formats a standardized movie directory relative path (e.g., "Films/Aliens (1986)").
#[allow(dead_code)]
pub fn format_movie_folder(title: &str, year: Option<u32>) -> PathBuf {
    let clean_title = crate::utils::sanitize_filename(title);
    let folder_name = format_title_folder_name(&clean_title, year);
    PathBuf::from("Films").join(folder_name)
}

/// Resolves target FFmpeg library video encoder name from user codec string ("h264", "hevc", "av1").
pub fn resolve_video_codec_name(codec: &str) -> &'static str {
    match codec.to_lowercase().as_str() {
        "hevc" | "h265" => "libx265",
        "av1" => "libsvtav1",
        _ => "libx264",
    }
}

/// Open/Closed Principle (OCP/SOLID): Re-export video codec strategy resolvers from domain.
#[allow(unused_imports)]
pub use crate::domain::ports::transcoder::{DefaultVideoCodecResolver, VideoCodecResolver};

/// Determines whether output target should use Matroska MKV container format.
pub fn is_mkv_output(args: &Args, output_path: &Path) -> bool {
    let profile = args.profile.to_lowercase();
    args.mkv
        || profile == "archival"
        || output_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mkv"))
}

/// Applies audio mapping and audio normalization flags to an FFmpeg command.
pub fn apply_audio_options(cmd: &mut Command, args: &Args) {
    let target_lufs = args.norm_target.unwrap_or(-16);
    let norm_filter = format!("loudnorm=I={}:TP=-1.5:LRA=11", target_lufs);

    if let Some(ref downmix_mode) = args.audio_downmix {
        let filter_str = match downmix_mode.to_lowercase().as_str() {
            "mono" => "pan=mono|c0=c0",
            "dolphylogic" => "pan=stereo|FL=0.5*c0+0.707*c2+0.5*c4|FR=0.5*c1+0.707*c2+0.5*c5",
            "headphone" => "pan=stereo|FL=0.4*c0+0.4*c2+0.2*c4|FR=0.4*c1+0.4*c2+0.2*c5",
            _ => "pan=stereo|c0=c0|c1=c1",
        };
        cmd.arg("-af").arg(filter_str);
    }

    if let Some(track_idx) = args.audio_track {
        let stream_idx = if track_idx > 0 { track_idx - 1 } else { 0 };
        cmd.arg("-map").arg(format!("0:a:{}", stream_idx));
        if args.normalize_audio {
            cmd.arg("-filter:a").arg(&norm_filter);
        }
    } else if args.dual_audio {
        cmd.arg("-map").arg("0:a:0?");
        cmd.arg("-c:a:0").arg("aac");
        cmd.arg("-b:a:0").arg("192k");
        cmd.arg("-ac:a:0").arg("2");
        if args.normalize_audio {
            cmd.arg("-filter:a:0").arg(&norm_filter);
        }
        cmd.arg("-metadata:s:a:0")
            .arg("title=Stereo AAC (Normalized)");

        cmd.arg("-map").arg("0:a:0?");
        cmd.arg("-c:a:1").arg("copy");
        cmd.arg("-metadata:s:a:1")
            .arg("title=5.1 Surround Passthrough");
    } else if args.all_audio {
        cmd.arg("-map").arg("0:a");
        if args.normalize_audio {
            cmd.arg("-filter:a").arg(&norm_filter);
        }
    } else if let Some(ref lang) = args.audio_lang {
        cmd.arg("-map").arg(format!("0:a:m:language:{}", lang));
        if args.normalize_audio {
            cmd.arg("-filter:a").arg(&norm_filter);
        }
    } else if let Some(ref pref) = args.auto_audio_pref {
        let langs = parse_ranked_audio_languages(pref);
        for lang in langs {
            cmd.arg("-map").arg(format!("0:a:m:language:{}?", lang));
        }
        if args.normalize_audio {
            cmd.arg("-filter:a").arg(&norm_filter);
        }
    } else {
        cmd.arg("-map").arg("0:a?");
        if args.normalize_audio {
            cmd.arg("-filter:a").arg(&norm_filter);
        }
    }

    if let Some(ref title) = args.audio_title {
        cmd.arg("-metadata:s:a").arg(format!("title={}", title));
    }
}

/// Applies subtitle stream mapping and codec configuration to an FFmpeg command.
pub fn apply_subtitle_options(cmd: &mut Command, args: &Args) {
    if args.subtitles {
        if args.sub_forced_only {
            cmd.arg("-map").arg("0:s:m:disposition:forced?");
        } else {
            cmd.arg("-map").arg("0:s?");
        }
        let sub_codec = resolve_subtitle_codec(args.sub_format.as_deref());
        cmd.arg("-c:s").arg(sub_codec);
        if args.sub_default {
            cmd.arg("-disposition:s:0").arg("default");
        }
    }
}

/// Constructs a vector of video filter string elements based on resolution scaling, deinterlacing, IVTC, auto-crop, and denoise flags.
pub fn build_video_filter_chain(args: &Args, profile: &str) -> Vec<String> {
    build_video_filter_chain_with_crop(args, profile, None)
}

/// Constructs a video filter chain with an optional pre-probed crop geometry (e.g. "crop=720:360:0:60").
pub fn build_video_filter_chain_with_crop(
    args: &Args,
    profile: &str,
    detected_crop: Option<&str>,
) -> Vec<String> {
    let mut vf_filters = Vec::new();
    if profile == "mobile" {
        vf_filters.push("scale=-2:720".to_string());
    }
    if args.detelecine {
        vf_filters.push(
            "fieldmatch=order=auto:combmatch=full,yadif=deint=interlaced,decimate".to_string(),
        );
    } else if args.deinterlace {
        let algo = match args
            .deinterlace_algo
            .as_deref()
            .unwrap_or("bwdif")
            .to_lowercase()
            .as_str()
        {
            "yadif" => "yadif=1:-1:0",
            "w3fdif" => "w3fdif=filter=complex",
            _ => "bwdif=mode=send_frame:parity=auto:deint=all",
        };
        vf_filters.push(algo.to_string());
    }
    if let Some(crop_str) = detected_crop
        && !crop_str.is_empty()
    {
        vf_filters.push(crop_str.to_string());
    }
    if args.denoise {
        vf_filters.push("hqdn3d=4:3:6:4.5".to_string());
    }
    if args.sub_burnin {
        vf_filters.push("subtitles=0:s=0".to_string());
    }
    if let Some(ref algo) = args.tonemap {
        let curve = match algo.to_lowercase().as_str() {
            "hable" => "hable",
            "reinhard" => "reinhard",
            _ => "mobius",
        };
        vf_filters.push(format!(
            "zscale=transfer=linear,tonemap=tonemap={}:desat=0",
            curve
        ));
    }
    vf_filters
}

/// Parses FFmpeg `cropdetect` stderr output lines and returns the modal (most frequent) valid crop filter string (e.g. "crop=720:360:0:60").
pub fn parse_cropdetect_output(stderr: &str) -> Option<String> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for line in stderr.lines() {
        if let Some(idx) = line.find("crop=") {
            let token = line[idx..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches([',', '.', ';']);
            let parts: Vec<&str> = token.trim_start_matches("crop=").split(':').collect();
            if parts.len() == 4
                && let (Ok(w), Ok(h), Ok(_x), Ok(_y)) = (
                    parts[0].parse::<i32>(),
                    parts[1].parse::<i32>(),
                    parts[2].parse::<i32>(),
                    parts[3].parse::<i32>(),
                )
                && w > 64
                && h > 64
            {
                *counts.entry(token.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(crop, _)| crop)
}

/// Probes 3 sample points across a DVD title using FFmpeg `cropdetect` to find the optimal black-bar crop geometry.
pub fn detect_crop_dimensions(
    ffmpeg_path: &str,
    dvd_path: &Path,
    title_num: u32,
    duration_secs: Option<f64>,
) -> Option<String> {
    let dur = duration_secs.unwrap_or(1800.0).max(120.0);
    let sample_points = [dur * 0.15, dur * 0.50, dur * 0.75];
    let mut combined_stderr = String::new();

    for ts in sample_points {
        let seek_str = crate::utils::format_duration_hhmmss(ts);
        let mut cmd = Command::new(ffmpeg_path);
        crate::utils::configure_silent_command(&mut cmd);
        let out = cmd
            .stdin(std::process::Stdio::null())
            .arg("-nostdin")
            .arg("-ss")
            .arg(&seek_str)
            .arg("-f")
            .arg("dvdvideo")
            .arg("-title")
            .arg(title_num.max(1).to_string())
            .arg("-i")
            .arg(dvd_path)
            .arg("-vframes")
            .arg("15")
            .arg("-vf")
            .arg("cropdetect=24:16:0")
            .arg("-f")
            .arg("null")
            .arg("-")
            .output();
        if let Ok(output) = out {
            combined_stderr.push_str(&String::from_utf8_lossy(&output.stderr));
            combined_stderr.push('\n');
        }
    }
    parse_cropdetect_output(&combined_stderr)
}

/// Trait defining a strategy provider for GPU hardware accelerated video encoding (OCP/SOLID).
pub trait HwAccelProvider {
    fn name(&self) -> &str;
    fn apply(&self, cmd: &mut Command, preset: &str);
}

pub struct NvencHwAccelProvider;
impl HwAccelProvider for NvencHwAccelProvider {
    fn name(&self) -> &str {
        "nvenc"
    }
    fn apply(&self, cmd: &mut Command, preset: &str) {
        cmd.arg("-c:v").arg("h264_nvenc");
        cmd.arg("-preset").arg(preset);
    }
}

pub struct QsvHwAccelProvider;
impl HwAccelProvider for QsvHwAccelProvider {
    fn name(&self) -> &str {
        "qsv"
    }
    fn apply(&self, cmd: &mut Command, preset: &str) {
        cmd.arg("-c:v").arg("h264_qsv");
        cmd.arg("-preset").arg(preset);
    }
}

pub struct VaapiHwAccelProvider;
impl HwAccelProvider for VaapiHwAccelProvider {
    fn name(&self) -> &str {
        "vaapi"
    }
    fn apply(&self, cmd: &mut Command, _preset: &str) {
        cmd.arg("-vaapi_device").arg("/dev/dri/renderD128");
        cmd.arg("-c:v").arg("h264_vaapi");
    }
}

pub struct V4l2HwAccelProvider;
impl HwAccelProvider for V4l2HwAccelProvider {
    fn name(&self) -> &str {
        "v4l2"
    }
    fn apply(&self, cmd: &mut Command, _preset: &str) {
        cmd.arg("-c:v").arg("h264_v4l2m2m");
        cmd.arg("-b:v").arg("4M");
    }
}

/// Configures hardware-accelerated video encoder command arguments for supported GPUs (NVENC, QSV, VAAPI, V4L2).
#[allow(dead_code)]
pub fn apply_hwaccel_encoder(cmd: &mut Command, hwaccel: &str, preset: &str) {
    let providers: Vec<Box<dyn HwAccelProvider>> = vec![
        Box::new(NvencHwAccelProvider),
        Box::new(QsvHwAccelProvider),
        Box::new(VaapiHwAccelProvider),
        Box::new(V4l2HwAccelProvider),
    ];
    let mode = hwaccel.to_lowercase();
    for provider in providers {
        if provider.name() == mode || (mode == "v4l2m2m" && provider.name() == "v4l2") {
            provider.apply(cmd, preset);
            break;
        }
    }
}

/// Liskov Substitution Principle (LSP/SOLID): Trait contract for media process runner execution.
#[allow(dead_code)]
pub trait MediaProcessRunner {
    fn run_process(&self, cmd: &mut Command) -> Result<std::process::ExitStatus>;
}

#[derive(Debug, Default)]
pub struct SystemMediaProcessRunner;

impl MediaProcessRunner for SystemMediaProcessRunner {
    fn run_process(&self, cmd: &mut Command) -> Result<std::process::ExitStatus> {
        let mut child = cmd.spawn()?;
        let status = child.wait()?;
        Ok(status)
    }
}

pub use crate::infrastructure::optical::ifo_parser::{
    probe_dvd_cell_topologies_ifo, probe_dvd_titles_ifo, probe_title_details_ifo,
    resolve_vts_ttn_to_pgcn,
};

/// Parses FFmpeg stderr probe output into a structured [`TitleStreamDetails`](crate::domain::entities::disc::TitleStreamDetails).
pub fn parse_ffmpeg_stream_details(
    stderr: &str,
    title_num: u32,
) -> crate::domain::entities::disc::TitleStreamDetails {
    use crate::domain::entities::disc::{
        AudioStreamInfo, Chapter, SubtitleStreamInfo, TitleStreamDetails,
    };

    let mut details = TitleStreamDetails {
        title_num,
        angles: 1,
        ..Default::default()
    };

    for line in stderr.lines() {
        let trimmed = line.trim();
        if let Some(dur_str) = extract_kv_field(trimmed, "Duration: ") {
            let clean = dur_str.trim_end_matches(',');
            if let Some(secs) = parse_duration(clean) {
                details.duration_secs = secs;
            }
        } else if trimmed.starts_with("Chapter #") {
            // e.g. "Chapter #0:0: start 0.000000, end 300.500000"
            let start = extract_kv_field(trimmed, "start ")
                .and_then(|s| s.trim_end_matches(',').parse::<f64>().ok())
                .unwrap_or(0.0);
            let end = extract_kv_field(trimmed, "end ")
                .and_then(|s| s.trim_end_matches(',').parse::<f64>().ok())
                .unwrap_or(start);
            let idx = (details.chapters.len() + 1) as u32;
            details.chapters.push(Chapter {
                index: idx,
                start_secs: start,
                end_secs: end,
                title: format!("Chapter {:02}", idx),
            });
        } else if trimmed.starts_with("Stream #") && trimmed.contains("Audio:") {
            // e.g. "Stream #0:1[0x80](eng): Audio: ac3, 48000 Hz, 5.1(side), fltp, 448 kb/s"
            let lang = if let Some(l_start) = trimmed.find('(') {
                if let Some(l_end) = trimmed[l_start + 1..].find(')') {
                    let candidate = &trimmed[l_start + 1..l_start + 1 + l_end];
                    if candidate.len() <= 3 && candidate.chars().all(|c| c.is_ascii_alphabetic()) {
                        candidate.to_lowercase()
                    } else {
                        "und".to_string()
                    }
                } else {
                    "und".to_string()
                }
            } else {
                "und".to_string()
            };
            let after_audio = trimmed.split("Audio:").nth(1).unwrap_or("").trim();
            let codec = after_audio
                .split([',', ' '])
                .next()
                .unwrap_or("ac3")
                .to_string();
            let (channels, layout) = if after_audio.contains("5.1") {
                (6, "5.1".to_string())
            } else if after_audio.contains("stereo") || after_audio.contains("2.0") {
                (2, "2.0 Stereo".to_string())
            } else if after_audio.contains("mono") {
                (1, "1.0 Mono".to_string())
            } else {
                (2, "2.0".to_string())
            };
            let idx = details.audio_streams.len() as u32;
            details.audio_streams.push(AudioStreamInfo {
                stream_index: idx,
                codec,
                language: lang,
                channels,
                channel_layout: layout,
                title: None,
            });
        } else if trimmed.starts_with("Stream #") && trimmed.contains("Subtitle:") {
            let lang = if let Some(l_start) = trimmed.find('(') {
                if let Some(l_end) = trimmed[l_start + 1..].find(')') {
                    let candidate = &trimmed[l_start + 1..l_start + 1 + l_end];
                    if candidate.len() <= 3 && candidate.chars().all(|c| c.is_ascii_alphabetic()) {
                        candidate.to_lowercase()
                    } else {
                        "und".to_string()
                    }
                } else {
                    "und".to_string()
                }
            } else {
                "und".to_string()
            };
            let after_sub = trimmed.split("Subtitle:").nth(1).unwrap_or("").trim();
            let format = after_sub
                .split([',', ' '])
                .next()
                .unwrap_or("dvd_subtitle")
                .to_string();
            let forced = trimmed.to_lowercase().contains("forced");
            let idx = details.subtitle_streams.len() as u32;
            details.subtitle_streams.push(SubtitleStreamInfo {
                stream_index: idx,
                language: lang,
                format,
                forced,
            });
        }
    }

    details
}

/// Probes comprehensive stream and chapter details for a DVD title (fast binary IFO parser with FFmpeg fallback).
pub fn probe_title_stream_details(
    ffmpeg_path: &str,
    dvd_path: &Path,
    title_num: u32,
) -> crate::domain::entities::disc::TitleStreamDetails {
    if let Some(ifo_details) = probe_title_details_ifo(dvd_path, title_num) {
        return ifo_details;
    }

    let mut cmd = Command::new(ffmpeg_path);
    crate::utils::configure_silent_command(&mut cmd);
    if let Ok(output) = cmd
        .stdin(std::process::Stdio::null())
        .arg("-nostdin")
        .arg("-analyzeduration")
        .arg("1000000")
        .arg("-probesize")
        .arg("1000000")
        .arg("-f")
        .arg("dvdvideo")
        .arg("-trim")
        .arg("0")
        .arg("-title")
        .arg(title_num.max(1).to_string())
        .arg("-i")
        .arg(dvd_path)
        .output()
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return parse_ffmpeg_stream_details(&stderr, title_num);
    }

    crate::domain::entities::disc::TitleStreamDetails {
        title_num,
        angles: 1,
        ..Default::default()
    }
}

/// Probes the duration in seconds of a specific DVD title.
pub fn probe_single_title_duration(
    ffmpeg_path: &str,
    dvd_path: &Path,
    title_num: u32,
) -> Option<f64> {
    if let Some(titles) = probe_dvd_titles_ifo(dvd_path)
        && let Some(t) = titles.into_iter().find(|item| item.title_num == title_num)
        && t.duration_secs > 0.0
    {
        return Some(t.duration_secs);
    }

    let mut cmd = Command::new(ffmpeg_path);
    crate::utils::configure_silent_command(&mut cmd);
    let output = cmd
        .stdin(std::process::Stdio::null())
        .arg("-nostdin")
        .arg("-analyzeduration")
        .arg("500000")
        .arg("-probesize")
        .arg("500000")
        .arg("-f")
        .arg("dvdvideo")
        .arg("-trim")
        .arg("0")
        .arg("-title")
        .arg(title_num.to_string())
        .arg("-i")
        .arg(dvd_path)
        .output()
        .ok()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
        if let Some(dur_str) = extract_kv_field(line, "Duration: ") {
            let clean = dur_str.trim_end_matches(',');
            if let Some(secs) = parse_duration(clean)
                && secs > 0.0
            {
                return Some(secs);
            }
        }
    }
    None
}

/// Probes all titles on the DVD drive, using fast single-pass probing with fallback to sequential probing.
pub fn probe_dvd_titles(
    ffmpeg_path: &str,
    dvd_path: &Path,
    cancel_flag: Option<&std::sync::atomic::AtomicBool>,
) -> Vec<DvdTitleInfo> {
    if let Some(ifo_titles) = probe_dvd_titles_ifo(dvd_path)
        && ifo_titles.iter().any(|t| t.duration_secs >= 60.0)
    {
        return ifo_titles;
    }

    let fast_results = probe_dvd_titles_fast(ffmpeg_path, dvd_path);
    if fast_results.iter().any(|t| t.duration_secs >= 300.0) {
        return fast_results;
    }

    let mut titles = Vec::new();
    let mut consecutive_failures = 0;

    for t in 1..=99 {
        if let Some(flag) = cancel_flag
            && flag.load(std::sync::atomic::Ordering::SeqCst)
        {
            break;
        }
        let mut cmd = Command::new(ffmpeg_path);
        crate::utils::configure_silent_command(&mut cmd);
        let output = cmd
            .stdin(std::process::Stdio::null())
            .arg("-analyzeduration")
            .arg("500000")
            .arg("-probesize")
            .arg("500000")
            .arg("-f")
            .arg("dvdvideo")
            .arg("-trim")
            .arg("0")
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

        if consecutive_failures >= 15 {
            break;
        }
    }

    if titles.is_empty() {
        fast_results
    } else {
        titles
    }
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
    crate::domain::heuristics::detect_tv_episodes_from_titles(
        &all_titles,
        show_name,
        season,
        start_ep,
        None,
    )
}

/// Probes the DVD drive to find the title number and probed duration best matching expected_runtime_secs, or with the longest duration (re-exported from domain).
pub use crate::domain::heuristics::cluster_detector::rank_best_title_from_slice;

/// Probes the DVD drive to find the title number and probed duration best matching expected_runtime_secs, or with the longest duration.
pub fn detect_best_title_info(
    ffmpeg_path: &str,
    dvd_path: &Path,
    expected_runtime_secs: Option<f64>,
) -> (u32, Option<f64>) {
    if let Some(topologies) = probe_dvd_cell_topologies_ifo(dvd_path)
        && topologies.iter().any(|t| !t.cell_sectors.is_empty())
        && let Some(auth_title) = crate::domain::heuristics::select_authentic_main_title(
            &topologies,
            expected_runtime_secs,
        )
        && let Some(matched) = topologies.iter().find(|t| t.title_num == auth_title)
        && matched.duration_secs >= 300.0
    {
        return (matched.title_num, Some(matched.duration_secs));
    }

    let titles = probe_dvd_titles(ffmpeg_path, dvd_path, None);
    rank_best_title_from_slice(&titles, expected_runtime_secs)
}

/// Probes the DVD drive to find the title number best matching expected_runtime_secs, or with the longest duration.
pub fn detect_best_title(
    ffmpeg_path: &str,
    dvd_path: &Path,
    expected_runtime_secs: Option<f64>,
) -> u32 {
    detect_best_title_info(ffmpeg_path, dvd_path, expected_runtime_secs).0
}

/// Helper for longest duration title detection.
#[allow(dead_code)]
pub fn detect_longest_title(ffmpeg_path: &str, dvd_path: &Path) -> u32 {
    detect_best_title(ffmpeg_path, dvd_path, None)
}

/// Builds the FFmpeg Command configured to capture a single thumbnail frame snapshot.
pub fn build_thumbnail_command(
    ffmpeg_path: &str,
    dvd_path: &Path,
    title_num: u32,
    timestamp_secs: f64,
    output_thumb: &Path,
) -> Command {
    let mut cmd = Command::new(ffmpeg_path);
    crate::utils::configure_silent_command(&mut cmd);
    cmd.arg("-nostdin");
    cmd.arg("-y");

    let seek_str = if timestamp_secs > 0.0 {
        crate::utils::format_duration_hhmmss(timestamp_secs)
    } else {
        "00:02:00".to_string()
    };

    cmd.arg("-ss").arg(&seek_str);
    cmd.arg("-f").arg("dvdvideo");
    if title_num > 0 {
        cmd.arg("-title").arg(title_num.to_string());
    }
    cmd.arg("-i").arg(dvd_path);
    cmd.arg("-vframes").arg("1");
    cmd.arg("-q:v").arg("3");
    cmd.arg(output_thumb);
    cmd
}

/// Generates a video thumbnail JPEG snapshot from a DVD title track using FFmpeg.
pub fn generate_title_thumbnail(
    ffmpeg_path: &str,
    dvd_path: &Path,
    title_num: u32,
    timestamp_secs: f64,
    output_thumb: &Path,
) -> Result<PathBuf> {
    if let Some(parent) = output_thumb.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = build_thumbnail_command(
        ffmpeg_path,
        dvd_path,
        title_num,
        timestamp_secs,
        output_thumb,
    );
    let status = cmd
        .status()
        .context("Failed to spawn FFmpeg thumbnail snapshot command")?;
    if !status.success() {
        return Err(anyhow!(
            "FFmpeg thumbnail extraction failed with status: {}",
            status
        ));
    }

    Ok(output_thumb.to_path_buf())
}

/// Builds an FFmpeg Command that encodes a live 10-second preview clip with all active filters (crop, IVTC, deinterlace, CRF) applied.
pub fn build_preview_clip_command(
    args: &Args,
    dvd_path: &Path,
    title_num: u32,
    start_offset_secs: f64,
    duration_secs: u32,
    output_preview: &Path,
) -> Command {
    let mut cmd = Command::new(&args.ffmpeg);
    crate::utils::configure_silent_command(&mut cmd);
    cmd.arg("-nostdin");
    cmd.arg("-y");

    let seek_str = crate::utils::format_duration_hhmmss(start_offset_secs.max(30.0));
    cmd.arg("-ss").arg(&seek_str);
    cmd.arg("-f").arg("dvdvideo");
    cmd.arg("-trim").arg("0");
    if title_num > 0 {
        cmd.arg("-title").arg(title_num.to_string());
    }
    if let Some(angle) = args.angle
        && angle > 0
    {
        cmd.arg("-angle").arg(angle.to_string());
    }
    cmd.arg("-i").arg(dvd_path);
    cmd.arg("-t").arg(duration_secs.max(5).to_string());
    cmd.arg("-map").arg("0:v:0?");
    cmd.arg("-map").arg("0:a:0?");

    let profile = args.profile.to_lowercase();
    let detected_crop = if args.auto_crop {
        detect_crop_dimensions(&args.ffmpeg, dvd_path, title_num, None)
    } else {
        None
    };
    let vf_filters = build_video_filter_chain_with_crop(args, &profile, detected_crop.as_deref());
    if !vf_filters.is_empty() {
        cmd.arg("-vf").arg(vf_filters.join(","));
    }

    let codec = resolve_video_codec_name(&args.codec);
    let crf = args
        .crf
        .map(|c| c.min(51).to_string())
        .unwrap_or_else(|| "22".to_string());
    cmd.arg("-c:v").arg(codec);
    cmd.arg("-preset").arg("veryfast");
    cmd.arg("-crf").arg(crf);
    cmd.arg("-c:a").arg("aac");
    cmd.arg("-b:a").arg("128k");
    cmd.arg(output_preview);
    cmd
}

/// Generates a 10-second encoded video preview sample clip for instant visual verification of filters and quality.
pub fn generate_preview_clip(
    args: &Args,
    dvd_path: &Path,
    title_num: u32,
    start_offset_secs: f64,
    output_preview: &Path,
) -> Result<PathBuf> {
    if let Some(parent) = output_preview.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = build_preview_clip_command(
        args,
        dvd_path,
        title_num,
        start_offset_secs,
        10,
        output_preview,
    );
    let status = cmd
        .status()
        .context("Failed to spawn FFmpeg preview clip command")?;
    if !status.success() {
        return Err(anyhow!(
            "FFmpeg preview clip generation failed with status: {}",
            status
        ));
    }

    Ok(output_preview.to_path_buf())
}

/// Builds the FFmpeg Command configured with arguments according to CLI options.
pub fn build_ffmpeg_command(
    args: &Args,
    dvd_path: &Path,
    absolute_output: &Path,
    resolved_title: u32,
) -> Command {
    let mut cmd = Command::new(&args.ffmpeg);
    crate::utils::configure_silent_command(&mut cmd);
    cmd.arg("-nostdin");
    cmd.arg("-stats");

    if args.ignore_errors {
        cmd.arg("-fflags").arg("+genpts+discardcorrupt");
        cmd.arg("-err_detect").arg("ignore_err");
    }

    cmd.arg("-f").arg("dvdvideo");
    cmd.arg("-trim").arg("0");

    if resolved_title > 0 {
        cmd.arg("-title").arg(resolved_title.to_string());
    }
    if let Some(sc) = args.start_chapter
        && sc > 0
    {
        cmd.arg("-chapter_start").arg(sc.to_string());
    }
    if let Some(ec) = args.end_chapter
        && ec > 0
    {
        cmd.arg("-chapter_end").arg(ec.to_string());
    }
    if let Some(angle) = args.angle
        && angle > 0
    {
        cmd.arg("-angle").arg(angle.to_string());
    }

    cmd.arg("-i").arg(dvd_path);
    cmd.arg("-map").arg("0:v");

    if args.chapters {
        cmd.arg("-map_chapters").arg("0");
    }

    // Audio stream mapping
    apply_audio_options(&mut cmd, args);

    // Subtitle stream mapping
    if args.subtitles && args.sub_lang.is_some() {
        if let Some(ref lang) = args.sub_lang {
            cmd.arg("-map").arg(format!("0:s:m:language:{}", lang));
        }
        let sub_codec = resolve_subtitle_codec(args.sub_format.as_deref());
        cmd.arg("-c:s").arg(sub_codec);
    } else {
        apply_subtitle_options(&mut cmd, args);
    }

    let profile = args.profile.to_lowercase();
    let codec = args.codec.to_lowercase();
    let is_mkv = is_mkv_output(args, absolute_output);

    if profile == "archival" || codec == "copy" {
        cmd.arg("-c").arg("copy");
        if is_mkv {
            cmd.arg("-f").arg("matroska");
        } else {
            cmd.arg("-f").arg("dvd");
        }
    } else if is_transcode_enabled(args) {
        let detected_crop = if args.auto_crop {
            detect_crop_dimensions(&args.ffmpeg, dvd_path, resolved_title, None)
        } else {
            None
        };
        let vf_filters =
            build_video_filter_chain_with_crop(args, &profile, detected_crop.as_deref());
        let custom_crf = args.crf.map(|c| c.min(51).to_string());

        if profile == "mobile" {
            if !vf_filters.is_empty() {
                cmd.arg("-vf").arg(vf_filters.join(","));
            }
            cmd.arg("-c:v").arg("libx264");
            cmd.arg("-preset").arg(&args.preset);
            cmd.arg("-crf").arg(custom_crf.as_deref().unwrap_or("24"));
            cmd.arg("-c:a").arg("aac");
            cmd.arg("-b:a").arg("128k");
        } else if profile == "plex" {
            if !vf_filters.is_empty() {
                cmd.arg("-vf").arg(vf_filters.join(","));
            }
            cmd.arg("-c:v").arg(resolve_video_codec_name(&codec));
            cmd.arg("-preset").arg(&args.preset);
            cmd.arg("-crf").arg(custom_crf.as_deref().unwrap_or("20"));
            cmd.arg("-c:a").arg("aac");
            cmd.arg("-b:a").arg("192k");
        } else {
            match args.hwaccel.to_lowercase().as_str() {
                "v4l2" | "v4l2m2m" => {
                    if !vf_filters.is_empty() {
                        cmd.arg("-vf").arg(vf_filters.join(","));
                    }
                    cmd.arg("-c:v").arg("h264_v4l2m2m");
                    cmd.arg("-b:v").arg("4M");
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "vaapi" => {
                    cmd.arg("-vaapi_device").arg("/dev/dri/renderD128");
                    let vf_str = if vf_filters.is_empty() {
                        "format=nv12,hwupload".to_string()
                    } else {
                        format!("{},format=nv12,hwupload", vf_filters.join(","))
                    };
                    cmd.arg("-vf").arg(vf_str);
                    let enc = match codec.as_str() {
                        "hevc" | "h265" => "hevc_vaapi",
                        "av1" => "av1_vaapi",
                        _ => "h264_vaapi",
                    };
                    cmd.arg("-c:v").arg(enc);
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "nvenc" => {
                    if !vf_filters.is_empty() {
                        cmd.arg("-vf").arg(vf_filters.join(","));
                    }
                    let enc = match codec.as_str() {
                        "hevc" | "h265" => "hevc_nvenc",
                        "av1" => "av1_nvenc",
                        _ => "h264_nvenc",
                    };
                    cmd.arg("-c:v").arg(enc);
                    cmd.arg("-preset").arg(&args.preset);
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "qsv" => {
                    if !vf_filters.is_empty() {
                        cmd.arg("-vf").arg(vf_filters.join(","));
                    }
                    let enc = match codec.as_str() {
                        "hevc" | "h265" => "hevc_qsv",
                        "av1" => "av1_qsv",
                        _ => "h264_qsv",
                    };
                    cmd.arg("-c:v").arg(enc);
                    cmd.arg("-preset").arg(&args.preset);
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                _ => {
                    if !vf_filters.is_empty() {
                        cmd.arg("-vf").arg(vf_filters.join(","));
                    }
                    cmd.arg("-c:v").arg(resolve_video_codec_name(&codec));
                    cmd.arg("-preset").arg(&args.preset);
                    if let Some(target_mb) = args.target_size_mb
                        && target_mb > 0
                    {
                        let v_kbps = calculate_target_video_bitrate_kbps(target_mb, 5400.0, 128);
                        cmd.arg("-b:v").arg(format!("{}k", v_kbps));
                        cmd.arg("-maxrate").arg(format!("{}k", (v_kbps * 5) / 4));
                        cmd.arg("-bufsize").arg(format!("{}k", v_kbps * 2));
                    } else {
                        cmd.arg("-crf").arg(custom_crf.as_deref().unwrap_or("22"));
                    }
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

pub fn parse_ranked_audio_languages(pref_str: &str) -> Vec<String> {
    pref_str
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Event emitted during FFmpeg ripping process or async GUI metadata lookup.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Log(String),
    Metadata(crate::imdb::FilmMetadata),
    SearchResults(Vec<crate::imdb::SearchResultItem>),
    TvEpisodesDetected(Vec<TvEpisodeInfo>),
    TvEpisodeStarted {
        title_num: u32,
    },
    TvEpisodeCompleted {
        title_num: u32,
        output_path: PathBuf,
    },
    BenchmarkFinished(crate::dvd::DriveBenchmarkReport),
    DetectedTitle {
        title_num: u32,
        duration_secs: f64,
    },
    TitleThumbnailReady {
        title_num: u32,
        image_bytes: Vec<u8>,
    },
    TitleStreamsProbed(crate::domain::entities::disc::TitleStreamDetails),
    PreviewClipReady(PathBuf),
    Progress {
        percent: f64,
        fps: String,
        speed: String,
    },
    Success(PathBuf),
    Error(String),
}

/// Executes FFmpeg child process and parses output line-by-line to render dynamic progress bar.
#[allow(dead_code)]
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

/// Evaluates whether an FFmpeg rip execution resulted in a verified, successful output
/// despite potential trailing disc errors or demux EOF notices.
#[derive(Debug, PartialEq)]
pub enum RipOutcome {
    Success,
    TrailingSectorNotice {
        exit_code: Option<i32>,
        output_size_mb: f64,
        ripped_mins: f64,
    },
    FatalError(String),
}

/// Typed context for evaluating whether an FFmpeg rip execution produced a verified, complete output.
///
/// Replaces positional boolean/numeric tuples with named fields for clarity and self-documenting call sites.
#[derive(Debug, Clone)]
pub struct RipEvaluationContext<'a> {
    pub exit_status_success: bool,
    pub exit_code: Option<i32>,
    pub output_size: u64,
    pub last_progress_secs: f64,
    pub total_seconds: Option<f64>,
    pub demux_error: bool,
    pub empty_output: bool,
    pub css_error: bool,
    pub resolved_title: u32,
    pub output_path: &'a Path,
    pub last_error_lines: &'a [String],
    pub is_batch: bool,
}

impl<'a> RipEvaluationContext<'a> {
    /// Evaluates the rip telemetry and output file metrics into a structured [`RipOutcome`].
    pub fn evaluate(&self) -> RipOutcome {
        let min_valid_size = if self.is_batch {
            30_000_000
        } else {
            50_000_000
        };
        let is_premature_truncation = self
            .total_seconds
            .filter(|&total| total >= 300.0)
            .is_some_and(|total| {
                self.last_progress_secs > 0.0 && self.last_progress_secs < total * 0.85
            });
        let is_premature_demux_error = self.demux_error
            && (self.output_size < min_valid_size || is_premature_truncation);
        let is_fatal_error = self.output_size == 0
            || is_premature_demux_error
            || is_premature_truncation
            || self.empty_output
            || self.css_error;

        // Detect if the movie/episode feature was fully ripped to completion before optical drive EOF / bad sector traps
        let has_completed_feature = self.output_size >= min_valid_size
            && !self.css_error
            && !self.empty_output
            && (self.last_progress_secs >= 300.0)
            && (self.total_seconds.is_none()
                || self.last_progress_secs >= self.total_seconds.unwrap_or(0.0) * 0.85);

        if self.exit_status_success && !is_fatal_error {
            RipOutcome::Success
        } else if has_completed_feature {
            RipOutcome::TrailingSectorNotice {
                exit_code: self.exit_code,
                output_size_mb: self.output_size as f64 / 1_000_000.0,
                ripped_mins: self.last_progress_secs / 60.0,
            }
        } else {
            let err_msg = if self.css_error {
                format!(
                    "FFmpeg cannot rip this encrypted DVD because CSS support is unavailable. Install libdvdcss and retry (output: {}).",
                    self.output_path.display()
                )
            } else if is_premature_demux_error || is_premature_truncation {
                let pct = self
                    .total_seconds
                    .filter(|&t| t > 0.0)
                    .map(|t| (self.last_progress_secs / t * 100.0).clamp(0.0, 100.0))
                    .unwrap_or(0.0);
                format!(
                    "DVD Title #{} stopped early at {:.1}% ({:.1} mins / {:.1} MB) due to copy-protection bad sectors or demux read error. On copy-protected discs, select Title #2 or check detection.",
                    self.resolved_title,
                    pct,
                    self.last_progress_secs / 60.0,
                    self.output_size as f64 / 1_000_000.0
                )
            } else if !self.exit_status_success {
                let detail = if !self.last_error_lines.is_empty() {
                    format!(": {}", self.last_error_lines.join(" | "))
                } else {
                    String::new()
                };
                format!(
                    "FFmpeg exited with non-zero status code: {:?}{}",
                    self.exit_code, detail
                )
            } else {
                format!(
                    "FFmpeg did not produce a valid rip at {} (output size: {} bytes; DVD demux error: {})",
                    self.output_path.display(),
                    self.output_size,
                    self.demux_error
                )
            };
            RipOutcome::FatalError(err_msg)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_rip_outcome(
    exit_status_success: bool,
    exit_code: Option<i32>,
    output_size: u64,
    last_progress_secs: f64,
    total_seconds: Option<f64>,
    demux_error: bool,
    empty_output: bool,
    css_error: bool,
    resolved_title: u32,
    output_path: &Path,
    last_error_lines: &[String],
    is_batch: bool,
) -> RipOutcome {
    RipEvaluationContext {
        exit_status_success,
        exit_code,
        output_size,
        last_progress_secs,
        total_seconds,
        demux_error,
        empty_output,
        css_error,
        resolved_title,
        output_path,
        last_error_lines,
        is_batch,
    }
    .evaluate()
}

/// Executes FFmpeg child process, sending events over a channel and allowing cancellation.
#[allow(clippy::too_many_arguments)]
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
    let max_retries = if dvd_path.exists() {
        args.sector_retries.min(2)
    } else {
        0
    };
    let mut attempt: u32 = 0;

    loop {
        let mut attempt_args = args.clone();
        if attempt > 0 {
            attempt_args.ignore_errors = true;
            let retry_msg = format!(
                "Retrying rip (attempt {}/{}) with error-tolerant demux flags enabled after bad sector / demux interruption...",
                attempt, max_retries
            );
            if let Some(ref sender) = tx {
                let _ = sender.send(ProgressEvent::Log(retry_msg));
            } else {
                println!("{}", retry_msg);
            }
            std::thread::sleep(std::time::Duration::from_millis(600));
        }

        let (resolved_title, probed_duration) = if attempt_args.title == 0 {
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

            let (detected, duration_opt) =
                detect_best_title_info(&attempt_args.ffmpeg, dvd_path, expected_runtime_secs);
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
            (detected, duration_opt)
        } else if attempt_args.start_chapter.is_some()
            || attempt_args.end_chapter.is_some()
            || expected_runtime_secs.is_some_and(|d| d > 0.0)
        {
            (attempt_args.title, expected_runtime_secs)
        } else {
            let dur =
                probe_single_title_duration(&attempt_args.ffmpeg, dvd_path, attempt_args.title);
            (attempt_args.title, dur)
        };

        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Progress {
                percent: 0.0,
                fps: "N/A".to_string(),
                speed: "N/A".to_string(),
            });
        }

        let mut cmd =
            build_ffmpeg_command(&attempt_args, dvd_path, absolute_output, resolved_title);

        let mode_desc = if attempt_args.transcode {
            "Transcoding to high-quality H.264 video & AAC audio...".to_string()
        } else {
            "Performing fast, lossless stream copy (remuxing)...".to_string()
        };

        let cmd_line = format!(
            "Running command: {} {:?}",
            attempt_args.ffmpeg,
            cmd.get_args().collect::<Vec<_>>()
        );
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

        if absolute_output.exists() {
            std::fs::remove_file(absolute_output).with_context(|| {
                format!(
                    "Failed to remove existing output file {}",
                    absolute_output.display()
                )
            })?;
        }

        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());

        let raw_child = cmd
            .spawn()
            .context("Failed to spawn FFmpeg process. Is it installed and in your PATH?")?;
        let mut child = crate::infrastructure::ffmpeg::ChildGuard::new(raw_child);

        let mut stderr = child
            .as_mut()
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture FFmpeg stderr"))?;

        let mut total_seconds: Option<f64> = probed_duration.or(expected_runtime_secs);
        let mut demux_error = false;
        let mut empty_output = false;
        let mut css_error = false;
        let mut last_progress_secs: f64 = 0.0;
        let mut last_error_lines: std::collections::VecDeque<String> =
            std::collections::VecDeque::with_capacity(10);

        let mut buf = [0u8; 1024];
        let mut line_bytes = Vec::new();

        loop {
            if let Some(ref flag) = cancel_flag
                && flag.load(std::sync::atomic::Ordering::SeqCst)
            {
                let _ = child.as_mut().kill();
                let _ = child.as_mut().wait();
                if absolute_output.exists() {
                    let _ = std::fs::remove_file(absolute_output);
                }
                let msg = "Ripping process cancelled by user.".to_string();
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(msg.clone()));
                }
                return Err(anyhow!(msg));
            }

            if let Some(ref rx) = cancel_rx
                && rx.try_recv().is_ok()
            {
                let _ = child.as_mut().kill();
                let _ = child.as_mut().wait();
                if absolute_output.exists() {
                    let _ = std::fs::remove_file(absolute_output);
                }
                let msg = "Ripping process cancelled by user.".to_string();
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(msg.clone()));
                }
                return Err(anyhow!(msg));
            }

            let n = match stderr.read(&mut buf) {
                Ok(0) => {
                    if let Ok(Some(_)) = child.as_mut().try_wait() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                Ok(n) => n,
                Err(_) => {
                    if let Ok(Some(_)) = child.as_mut().try_wait() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
            };

            for &b in &buf[..n] {
                if b == b'\r' || b == b'\n' {
                    if !line_bytes.is_empty() {
                        // Zero-allocation byte-level progress extraction before String conversion
                        let byte_metrics = crate::infrastructure::ffmpeg::progress_parser::parse_ffmpeg_progress_bytes(&line_bytes)
                            .and_then(|slice| {
                                slice.time_secs.map(|secs| {
                                    let speed_str = slice.speed.unwrap_or("N/A").to_string();
                                    let fps_str = crate::infrastructure::ffmpeg::progress_parser::extract_kv_field_bytes(&line_bytes, b"fps=")
                                        .unwrap_or("N/A")
                                        .to_string();
                                    (secs, fps_str, speed_str)
                                })
                            });

                        let line = String::from_utf8_lossy(&line_bytes).to_string();
                        line_bytes.clear();

                        if let Some(ref sender) = tx {
                            let _ = sender.send(ProgressEvent::Log(line.clone()));
                        }

                        if line.contains("Error")
                            || line.contains("error")
                            || line.contains("failed")
                            || line.contains("Invalid")
                            || line.contains("invalid")
                        {
                            if last_error_lines.len() >= 5 {
                                last_error_lines.pop_front();
                            }
                            last_error_lines.push_back(line.clone());
                        }

                        if line.contains("Error during demuxing") {
                            demux_error = true;
                        }
                        if line.contains("Output file is empty") {
                            empty_output = true;
                        }
                        if line.contains("Encrypted DVD support unavailable")
                            || line.contains("No css library available")
                        {
                            css_error = true;
                        }

                        // Detect overall duration from initialization log or stream info
                        if let Some(duration_str) = extract_kv_field(&line, "Duration: ") {
                            let clean_duration = duration_str.trim_end_matches(',');
                            if let Some(secs) = parse_duration(clean_duration)
                                && secs > 0.0
                            {
                                let is_chapter_slice = attempt_args.start_chapter.is_some()
                                    || attempt_args.end_chapter.is_some();
                                if total_seconds.is_none()
                                    || total_seconds.unwrap_or(0.0) <= 0.0
                                    || (!is_chapter_slice
                                        && secs >= 60.0
                                        && (total_seconds.unwrap_or(0.0) - secs).abs() > 60.0)
                                {
                                    total_seconds = Some(secs);
                                }
                            }
                        }

                        // Apply zero-allocation byte-parsed metrics (with fallback to line helper for negative timestamps)
                        let parsed_progress = byte_metrics.or_else(|| {
                            extract_kv_field(&line, "time=").and_then(|time_str| {
                                let clean_time = time_str.trim_start_matches('-');
                                parse_duration(clean_time).map(|secs| {
                                    let speed = extract_kv_field(&line, "speed=")
                                        .unwrap_or("N/A")
                                        .to_string();
                                    let fps =
                                        extract_kv_field(&line, "fps=").unwrap_or("N/A").to_string();
                                    (secs, fps, speed)
                                })
                            })
                        });

                        if let Some((secs, fps, speed)) = parsed_progress {
                            last_progress_secs = secs;

                            // If encoding progresses past estimated total duration (e.g. unprobed credits/cells),
                            // dynamically expand total_seconds so progress doesn't stall at 100% prematurely.
                            if let Some(total) = total_seconds
                                && secs >= total
                            {
                                total_seconds = Some(secs + 30.0);
                            }

                            let percent = if let Some(total) = total_seconds {
                                if total > 0.0 {
                                    (secs / total * 100.0).clamp(0.0, 99.5)
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            };

                            crate::api::update_appliance_status(
                                "Ripping",
                                "",
                                display_title,
                                percent,
                                &fps,
                                &speed,
                            );

                            if tx.is_none() {
                                let width = 20;
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
                } else {
                    line_bytes.push(b);
                }
            }
        }

        if let Some(ref flag) = cancel_flag
            && flag.load(std::sync::atomic::Ordering::SeqCst)
        {
            let _ = child.as_mut().kill();
            let _ = child.as_mut().wait();
            if absolute_output.exists() {
                let _ = std::fs::remove_file(absolute_output);
            }
            let msg = "Ripping process cancelled by user.".to_string();
            return Err(anyhow!(msg));
        }

        let status = child
            .as_mut()
            .wait()
            .context("Failed to wait on FFmpeg process")?;
        child.disarm();
        let output_size = std::fs::metadata(absolute_output)
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let err_lines_vec: Vec<String> = last_error_lines.into_iter().collect();
        let outcome = RipEvaluationContext {
            exit_status_success: status.success(),
            exit_code: status.code(),
            output_size,
            last_progress_secs,
            total_seconds,
            demux_error,
            empty_output,
            css_error,
            resolved_title,
            output_path: absolute_output,
            last_error_lines: &err_lines_vec,
            is_batch,
        }
        .evaluate();

        match outcome {
            RipOutcome::Success => {
                crate::api::update_appliance_status(
                    "Completed",
                    "",
                    display_title,
                    100.0,
                    "N/A",
                    "N/A",
                );
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Progress {
                        percent: 100.0,
                        fps: "N/A".to_string(),
                        speed: "N/A".to_string(),
                    });
                }
                let succ_msg = format!(
                    "Success! DVD ripped successfully to: {}",
                    absolute_output.display()
                );
                if tx.is_none() {
                    println!("\n\n{}", succ_msg);
                } else if let Some(ref sender) = tx {
                    if is_batch {
                        let _ = sender.send(ProgressEvent::Log(format!(
                            "Successfully finished episode: {}",
                            absolute_output.display()
                        )));
                    } else {
                        let _ = sender.send(ProgressEvent::Success(absolute_output.to_path_buf()));
                    }
                }
                return Ok(());
            }
            RipOutcome::TrailingSectorNotice {
                exit_code,
                output_size_mb,
                ripped_mins,
            } => {
                let notice = format!(
                    "Notice: Optical drive encountered trailing bad sectors / EOF notice at end of disc ({:?}), but feature was successfully ripped ({:.1} MB, {:.0} mins).",
                    exit_code, output_size_mb, ripped_mins
                );
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Log(notice.clone()));
                } else {
                    println!("{}", notice);
                }
                crate::api::update_appliance_status(
                    "Completed",
                    "",
                    display_title,
                    100.0,
                    "N/A",
                    "N/A",
                );
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Progress {
                        percent: 100.0,
                        fps: "N/A".to_string(),
                        speed: "N/A".to_string(),
                    });
                }
                let succ_msg = format!(
                    "Success! DVD ripped successfully to: {}",
                    absolute_output.display()
                );
                if tx.is_none() {
                    println!("\n\n{}", succ_msg);
                } else if let Some(ref sender) = tx {
                    if is_batch {
                        let _ = sender.send(ProgressEvent::Log(format!(
                            "Successfully finished episode: {}",
                            absolute_output.display()
                        )));
                    } else {
                        let _ = sender.send(ProgressEvent::Success(absolute_output.to_path_buf()));
                    }
                }
                return Ok(());
            }
            RipOutcome::FatalError(err_msg) => {
                let is_retryable = !css_error
                    && (demux_error || err_msg.contains("stopped early"))
                    && attempt < max_retries;
                if is_retryable {
                    attempt += 1;
                    continue;
                }
                crate::api::fail_appliance_status(display_title, "", &err_msg);
                if tx.is_none() {
                    eprintln!("\n{}", err_msg);
                } else if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(err_msg.clone()));
                }
                return Err(anyhow!(err_msg));
            }
        }
    }
}

/// Result of a perceptual video quality sample probe (`SSIM` / `PSNR`) for automatic CRF selection.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityProbeResult {
    pub tested_crf: u8,
    pub mean_ssim: f64,
    pub psnr_db: Option<f64>,
    pub sample_size_bytes: u64,
}

/// Calculates the exact target video bitrate (`kbps`) required to fit a title of `duration_secs`
/// into `target_size_mb` Megabytes after accounting for audio stream overhead and container muxing.
pub fn calculate_target_video_bitrate_kbps(
    target_size_mb: u32,
    duration_secs: f64,
    audio_bitrate_kbps: u32,
) -> u32 {
    let eff_dur = duration_secs.max(60.0);
    // Reserve ~1.5% for MP4/MKV container overhead
    let usable_kbits = (target_size_mb as f64) * 8192.0 * 0.985;
    let total_kbps = (usable_kbits / eff_dur).round() as i64;
    let video_kbps = (total_kbps - (audio_bitrate_kbps as i64)).clamp(300, 15_000);
    video_kbps as u32
}

/// Parses FFmpeg `ssim` filter output (`All:0.985412`) from stderr.
pub fn parse_ffmpeg_ssim_output(stderr: &str) -> Option<f64> {
    let mut last_ssim = None;
    for line in stderr.lines() {
        if line.contains("SSIM")
            && let Some(all_part) = line.split("All:").nth(1)
        {
            let num_str: String = all_part
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(val) = num_str.parse::<f64>()
                && (0.0..=1.0).contains(&val)
            {
                last_ssim = Some(val);
            }
        }
    }
    last_ssim
}

/// Parses FFmpeg `psnr` filter output (`average:42.15`) from stderr.
pub fn parse_ffmpeg_psnr_output(stderr: &str) -> Option<f64> {
    let mut last_psnr = None;
    for line in stderr.lines() {
        if line.contains("PSNR")
            && let Some(avg_part) = line.split("average:").nth(1)
        {
            let num_str: String = avg_part
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(val) = num_str.parse::<f64>()
                && val > 0.0
            {
                last_psnr = Some(val);
            }
        }
    }
    last_psnr
}

/// Predicts the optimal Constant Rate Factor (`CRF`) to achieve `target_ssim` (e.g. `0.98`) using sample probing.
///
/// Inspired by `ab-av1`, performs a fast binary search over `[16..=28]` on a 3-second representative sample.
pub fn predict_optimal_crf(
    ffmpeg_path: &str,
    dvd_path: &Path,
    title_num: u32,
    codec: &str,
    target_ssim: f64,
) -> QualityProbeResult {
    let clamped_target = target_ssim.clamp(0.90, 0.999);
    let encoder = resolve_video_codec_name(codec);
    let mut low: u8 = 16;
    let mut high: u8 = 28;
    let mut best = QualityProbeResult {
        tested_crf: 20,
        mean_ssim: clamped_target,
        psnr_db: None,
        sample_size_bytes: 0,
    };

    let temp_dir = std::env::temp_dir();
    while low <= high {
        let mid = low + (high - low) / 2;
        let sample_file = temp_dir.join(format!(
            "dvd_ripper_crf_probe_{}_{}.mkv",
            std::process::id(),
            mid
        ));

        let mut encode_cmd = Command::new(ffmpeg_path);
        crate::utils::configure_silent_command(&mut encode_cmd);
        let encode_ok = encode_cmd
            .stdin(std::process::Stdio::null())
            .arg("-nostdin")
            .arg("-y")
            .arg("-ss")
            .arg("00:05:00")
            .arg("-f")
            .arg("dvdvideo")
            .arg("-title")
            .arg(title_num.max(1).to_string())
            .arg("-i")
            .arg(dvd_path)
            .arg("-t")
            .arg("3")
            .arg("-an")
            .arg("-sn")
            .arg("-c:v")
            .arg(encoder)
            .arg("-preset")
            .arg("veryfast")
            .arg("-crf")
            .arg(mid.to_string())
            .arg(&sample_file)
            .status()
            .is_ok_and(|s| s.success());

        if !encode_ok || !sample_file.exists() {
            let _ = std::fs::remove_file(&sample_file);
            // Analytical fallback curve when optical hardware or FFmpeg is offline
            let synthetic_crf = if clamped_target >= 0.99 {
                18
            } else if clamped_target >= 0.98 {
                20
            } else if clamped_target >= 0.965 {
                22
            } else {
                24
            };
            return QualityProbeResult {
                tested_crf: synthetic_crf,
                mean_ssim: clamped_target,
                psnr_db: Some(41.5),
                sample_size_bytes: 0,
            };
        }

        let sample_size = std::fs::metadata(&sample_file)
            .map(|m| m.len())
            .unwrap_or(0);

        let mut ssim_cmd = Command::new(ffmpeg_path);
        crate::utils::configure_silent_command(&mut ssim_cmd);
        let ssim_out = ssim_cmd
            .stdin(std::process::Stdio::null())
            .arg("-nostdin")
            .arg("-i")
            .arg(&sample_file)
            .arg("-ss")
            .arg("00:05:00")
            .arg("-f")
            .arg("dvdvideo")
            .arg("-title")
            .arg(title_num.max(1).to_string())
            .arg("-i")
            .arg(dvd_path)
            .arg("-t")
            .arg("3")
            .arg("-lavfi")
            .arg("ssim;[0:v][1:v]psnr")
            .arg("-f")
            .arg("null")
            .arg("-")
            .output();

        let _ = std::fs::remove_file(&sample_file);

        if let Ok(out) = ssim_out {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if let Some(ssim) = parse_ffmpeg_ssim_output(&stderr) {
                let psnr = parse_ffmpeg_psnr_output(&stderr);
                best = QualityProbeResult {
                    tested_crf: mid,
                    mean_ssim: ssim,
                    psnr_db: psnr,
                    sample_size_bytes: sample_size,
                };
                if ssim >= clamped_target {
                    // Quality meets target; try higher CRF (smaller file)
                    low = mid.saturating_add(1);
                } else {
                    if mid == 0 {
                        break;
                    }
                    high = mid - 1;
                }
                continue;
            }
        }
        break;
    }

    best
}

/// Generates an FFmpeg `;FFMETADATA1` chapter manifest with optional custom chapter names.
pub fn generate_ffmetadata_chapters(
    title_label: &str,
    chapters: &[crate::domain::entities::disc::Chapter],
    custom_names: Option<&[String]>,
) -> String {
    let escape_meta = |s: &str| -> String {
        s.replace('\\', "\\\\")
            .replace('=', "\\=")
            .replace(';', "\\;")
            .replace('#', "\\#")
            .replace('\n', " ")
    };

    let mut out = String::from(";FFMETADATA1\n");
    if !title_label.trim().is_empty() {
        out.push_str(&format!("title={}\n\n", escape_meta(title_label.trim())));
    }

    for (idx, ch) in chapters.iter().enumerate() {
        let start_ms = (ch.start_secs.max(0.0) * 1000.0).round() as u64;
        let end_ms = (ch.end_secs.max(ch.start_secs) * 1000.0).round() as u64;
        let ch_name = custom_names
            .and_then(|names| names.get(idx))
            .filter(|n| !n.trim().is_empty())
            .map(|n| n.as_str())
            .unwrap_or_else(|| {
                if ch.title.trim().is_empty() {
                    "Chapter"
                } else {
                    ch.title.trim()
                }
            });

        out.push_str("[CHAPTER]\n");
        out.push_str("TIMEBASE=1/1000\n");
        out.push_str(&format!("START={}\n", start_ms));
        out.push_str(&format!("END={}\n", end_ms));
        out.push_str(&format!("title={}\n\n", escape_meta(ch_name)));
    }

    out
}

pub use crate::infrastructure::ffmpeg::disc_formats::{
    build_audio_cd_track_command, build_bluray_ffmpeg_command, build_multi_disc_concat_list,
    generate_m3u8_playlist, merge_multi_disc_files,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dvd_ripper_test_{}_{}",
                name,
                std::process::id()
            ));
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
        assert!(path_str.contains("The Office - S01E03.mp4"));
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
        assert!(path.parent().is_some_and(|p| p.exists()));
    }

    #[test]
    fn test_build_ffmpeg_command_title_argument() {
        let args = Args::default();

        let output_path = PathBuf::from("Films/Test/Test.mp4");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 3);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

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

        let output_path = PathBuf::from("Films/Test/Test.mp4");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(cmd_args.contains(&"0:a".to_string()));
        assert!(cmd_args.contains(&"0:s:m:language:eng".to_string()));
        assert!(cmd_args.contains(&"dvdsub".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_deinterlace_and_denoise() {
        let args = Args {
            transcode: true,
            deinterlace: true,
            deinterlace_algo: Some("yadif".to_string()),
            denoise: true,
            ..Default::default()
        };

        let output_path = PathBuf::from("Films/Test/Test.mp4");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(cmd_args.contains(&"-vf".to_string()));
        let vf_idx = cmd_args.iter().position(|r| r == "-vf").unwrap();
        let vf_val = &cmd_args[vf_idx + 1];
        assert!(vf_val.contains("yadif=1:-1:0"));
        assert!(vf_val.contains("hqdn3d=4:3:6:4.5"));
    }

    #[test]
    fn test_resolve_output_path_no_overwrite() {
        let temp = TestTempDir::new("no_overwrite_test");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let dummy_file = temp.0.join("Films").join("output.mp4");
        std::fs::create_dir_all(dummy_file.parent().unwrap()).unwrap();
        std::fs::write(&dummy_file, b"existing").unwrap();

        let args = Args {
            out_dir: out_dir_str,
            no_overwrite: true,
            ..Default::default()
        };

        let path = resolve_output_path(&args, None, None).unwrap();
        assert!(path.to_string_lossy().contains("output_1.mp4"));
    }

    #[test]
    fn test_resolve_films_and_tv_root_defaults() {
        let films = resolve_films_root("Films");
        let tv = resolve_tv_root("Films");
        assert_eq!(films, PathBuf::from("Films"));
        assert_eq!(tv, PathBuf::from("TV"));

        let films_tv = resolve_films_root("TV");
        let tv_tv = resolve_tv_root("TV");
        assert_eq!(films_tv, PathBuf::from("Films"));
        assert_eq!(tv_tv, PathBuf::from("TV"));
    }

    #[test]
    fn test_resolve_films_and_tv_root_nested() {
        let base = PathBuf::from("C:\\Media\\DVD Rips");
        let base_str = base.to_string_lossy();
        assert_eq!(resolve_films_root(&base_str), base.join("Films"));
        assert_eq!(resolve_tv_root(&base_str), base.join("TV"));

        let base_films = base.join("Films");
        let base_films_str = base_films.to_string_lossy();
        assert_eq!(resolve_films_root(&base_films_str), base.join("Films"));
        assert_eq!(resolve_tv_root(&base_films_str), base.join("TV"));

        let base_tv = base.join("TV");
        let base_tv_str = base_tv.to_string_lossy();
        assert_eq!(resolve_films_root(&base_tv_str), base.join("Films"));
        assert_eq!(resolve_tv_root(&base_tv_str), base.join("TV"));
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
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_args.contains(&"h264_nvenc".to_string()));

        let args_vaapi = Args {
            transcode: true,
            hwaccel: "vaapi".to_string(),
            ..Default::default()
        };
        let cmd_vaapi = build_ffmpeg_command(&args_vaapi, Path::new("D:\\"), &output_path, 1);
        let cmd_args_vaapi: Vec<String> = cmd_vaapi
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_args_vaapi.contains(&"h264_vaapi".to_string()));

        let args_qsv = Args {
            transcode: true,
            hwaccel: "qsv".to_string(),
            ..Default::default()
        };
        let cmd_qsv = build_ffmpeg_command(&args_qsv, Path::new("D:\\"), &output_path, 1);
        let cmd_args_qsv: Vec<String> = cmd_qsv
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
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
    fn test_resolve_output_path_iso() {
        let temp = TestTempDir::new("iso_out_dir");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let args = Args {
            out_dir: out_dir_str,
            iso: true,
            ..Default::default()
        };

        let path = resolve_output_path(&args, Some("Aliens"), Some(1986)).unwrap();
        assert!(path.to_string_lossy().contains("Aliens (1986).iso"));
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
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

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
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(cmd_args.contains(&"subrip".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_chapters_mapping() {
        let args = Args {
            chapters: true,
            ..Default::default()
        };

        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(cmd_args.contains(&"-map_chapters".to_string()));
    }

    #[test]
    fn test_parse_ranked_audio_languages() {
        let langs = parse_ranked_audio_languages("eng, fre, spa ");
        assert_eq!(langs, vec!["eng", "fre", "spa"]);
    }

    #[test]
    fn test_build_ffmpeg_command_ranked_audio() {
        let args = Args {
            auto_audio_pref: Some("eng,fre,spa".to_string()),
            ..Default::default()
        };

        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(cmd_args.contains(&"0:a:m:language:eng?".to_string()));
        assert!(cmd_args.contains(&"0:a:m:language:fre?".to_string()));
        assert!(cmd_args.contains(&"0:a:m:language:spa?".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_codecs_and_profiles() {
        let args_hevc = Args {
            transcode: true,
            codec: "hevc".to_string(),
            ..Default::default()
        };
        let cmd_hevc =
            build_ffmpeg_command(&args_hevc, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let args_vec_hevc: Vec<String> = cmd_hevc
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(args_vec_hevc.contains(&"libx265".to_string()));

        let args_av1 = Args {
            transcode: true,
            codec: "av1".to_string(),
            ..Default::default()
        };
        let cmd_av1 = build_ffmpeg_command(&args_av1, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let args_vec_av1: Vec<String> = cmd_av1
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(args_vec_av1.contains(&"libsvtav1".to_string()));

        let args_archival = Args {
            profile: "archival".to_string(),
            ..Default::default()
        };
        let cmd_archival =
            build_ffmpeg_command(&args_archival, Path::new("D:\\"), Path::new("test.mkv"), 1);
        let args_vec_archival: Vec<String> = cmd_archival
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(args_vec_archival.contains(&"matroska".to_string()));
        assert!(args_vec_archival.contains(&"copy".to_string()));
    }

    #[test]
    fn test_format_episode_helpers() {
        assert_eq!(format_episode_code(1, 5), "S01E05");
        assert_eq!(
            format_episode_filename("The Office", 1, 5),
            "The Office - S01E05"
        );
    }

    #[test]
    fn test_format_tv_season_folder() {
        let folder = format_tv_season_folder("The Office", Some(2005), 1);
        assert_eq!(
            folder,
            PathBuf::from("TV")
                .join("The Office (2005)")
                .join("Season 01")
        );
    }

    #[test]
    fn test_format_movie_folder() {
        let folder = format_movie_folder("Aliens", Some(1986));
        assert_eq!(folder, PathBuf::from("Films").join("Aliens (1986)"));
    }

    #[test]
    fn test_build_video_filter_chain() {
        let args = Args {
            deinterlace: true,
            denoise: true,
            ..Default::default()
        };
        let filters = build_video_filter_chain(&args, "mobile");
        assert_eq!(filters.len(), 3);
        assert_eq!(filters[0], "scale=-2:720");
    }

    #[test]
    fn test_is_mkv_output() {
        let args_mkv = Args {
            mkv: true,
            ..Default::default()
        };
        assert!(is_mkv_output(&args_mkv, Path::new("output.mp4")));

        let args_normal = Args::default();
        assert!(is_mkv_output(&args_normal, Path::new("output.mkv")));
        assert!(!is_mkv_output(&args_normal, Path::new("output.mp4")));
    }

    #[test]
    fn test_resolve_video_codec_name() {
        assert_eq!(resolve_video_codec_name("hevc"), "libx265");
        assert_eq!(resolve_video_codec_name("h265"), "libx265");
        assert_eq!(resolve_video_codec_name("av1"), "libsvtav1");
        assert_eq!(resolve_video_codec_name("h264"), "libx264");
    }

    #[test]
    fn test_apply_hwaccel_encoder() {
        let mut cmd = Command::new("ffmpeg");
        apply_hwaccel_encoder(&mut cmd, "nvenc", "medium");
        // Verify helper executes without panic
    }

    #[test]
    fn test_parse_title_duration_line() {
        let line = "  Duration: 00:45:30.12, start: 0.000000, bitrate: 5500 kb/s";
        let parsed = parse_title_duration_line(line);
        assert_eq!(parsed, Some(2730.12));
    }

    #[cfg(unix)]
    #[test]
    fn test_successful_ffmpeg_without_output_is_failure() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TestTempDir::new("missing_output");
        let fake_ffmpeg = temp.0.join("fake-ffmpeg");
        fs::write(&fake_ffmpeg, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&fake_ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();

        let output = temp.0.join("rip.mp4");
        let args = Args {
            ffmpeg: fake_ffmpeg.to_string_lossy().to_string(),
            title: 1,
            ..Default::default()
        };

        let result = run_ffmpeg_with_progress(&args, Path::new("/dev/null"), &output, "Test", None);
        let error = result.expect_err("a successful process without an output must fail");
        assert!(error.to_string().contains("did not produce a valid rip"));
    }

    #[test]
    fn test_format_hwaccel_vf_chain() {
        let filters = vec!["scale=-2:720".to_string(), "bwdif".to_string()];
        assert_eq!(
            format_hwaccel_vf_chain("vaapi", &filters),
            Some("scale=-2:720,bwdif,format=nv12,hwupload".to_string())
        );
        assert_eq!(
            format_hwaccel_vf_chain("copy", &filters),
            Some("scale=-2:720,bwdif".to_string())
        );
        assert_eq!(
            format_hwaccel_vf_chain("vaapi", &[]),
            Some("format=nv12,hwupload".to_string())
        );
    }

    #[test]
    fn test_resolve_subtitle_codec() {
        assert_eq!(resolve_subtitle_codec(Some("srt")), "subrip");
        assert_eq!(resolve_subtitle_codec(Some("SubRip")), "subrip");
        assert_eq!(resolve_subtitle_codec(Some("dvdsub")), "dvdsub");
        assert_eq!(resolve_subtitle_codec(None), "dvdsub");
    }

    #[test]
    fn test_resolve_encoder_defaults() {
        let mobile = resolve_encoder_defaults("mobile");
        assert_eq!(mobile.preset, "fast");
        assert_eq!(mobile.crf, "24");

        let default_enc = resolve_encoder_defaults("custom");
        assert_eq!(default_enc.preset, "medium");
        assert_eq!(default_enc.crf, "22");
    }

    #[test]
    fn test_format_title_selection_summary() {
        let msg = format_title_selection_summary(3, Some(7200.0), None);
        assert_eq!(
            msg,
            "Auto-selected Title #3 (120 mins) (longest duration on DVD)"
        );
    }

    #[test]
    fn test_build_ffmpeg_command_audio_normalization_and_dual_audio() {
        let args_norm = Args {
            normalize_audio: true,
            ..Default::default()
        };
        let cmd_norm =
            build_ffmpeg_command(&args_norm, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let vec_norm: Vec<String> = cmd_norm
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(vec_norm.contains(&"loudnorm=I=-16:TP=-1.5:LRA=11".to_string()));

        let args_dual = Args {
            dual_audio: true,
            normalize_audio: true,
            ..Default::default()
        };
        let cmd_dual =
            build_ffmpeg_command(&args_dual, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let vec_dual: Vec<String> = cmd_dual
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(vec_dual.contains(&"title=Stereo AAC (Normalized)".to_string()));
        assert!(vec_dual.contains(&"title=5.1 Surround Passthrough".to_string()));
    }

    #[test]
    fn test_is_transcode_enabled() {
        let default_args = Args::default();
        assert!(is_transcode_enabled(&default_args));

        let copy_args = Args {
            codec: "copy".to_string(),
            ..Default::default()
        };
        assert!(!is_transcode_enabled(&copy_args));

        let archival_args = Args {
            profile: "archival".to_string(),
            ..Default::default()
        };
        assert!(!is_transcode_enabled(&archival_args));

        let explicit_transcode_args = Args {
            transcode: true,
            codec: "hevc".to_string(),
            ..Default::default()
        };
        assert!(is_transcode_enabled(&explicit_transcode_args));
    }

    #[test]
    fn test_probe_dvd_titles_ifo_nonexistent() {
        let result = probe_dvd_titles_ifo(Path::new("Z:\\nonexistent\\dvd"));
        assert!(result.is_none());
    }

    #[test]
    fn test_ranking_prefers_lowest_title_in_cluster() {
        let titles = vec![
            DvdTitleInfo {
                title_num: 77,
                duration_secs: 7664.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 7524.0,
            },
            DvdTitleInfo {
                title_num: 22,
                duration_secs: 7524.0,
            },
        ];
        let (best_title, duration) = rank_best_title_from_slice(&titles, None);
        assert_eq!(best_title, 2);
        assert_eq!(duration, Some(7524.0));
    }

    #[test]
    fn test_rank_best_title_john_wick_3_with_pal_speedup_metadata() {
        // John Wick 3 theatrical runtime is 131m = 7860s.
        // On PAL DVD (25fps), duration is 7524s (Title 2).
        // Decoy title 77 has duration 7664s with loop cells/bad sectors.
        let titles = vec![
            DvdTitleInfo {
                title_num: 77,
                duration_secs: 7664.0,
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 7524.0,
            },
            DvdTitleInfo {
                title_num: 22,
                duration_secs: 7524.0,
            },
            DvdTitleInfo {
                title_num: 70,
                duration_secs: 7524.0,
            },
        ];
        let (best_title, duration) = rank_best_title_from_slice(&titles, Some(7860.0));
        assert_eq!(best_title, 2);
        assert_eq!(duration, Some(7524.0));
    }

    #[test]
    fn test_rank_best_title_empty() {
        let titles = vec![];
        let (best_title, duration) = rank_best_title_from_slice(&titles, None);
        assert_eq!(best_title, 1);
        assert_eq!(duration, None);
    }

    #[test]
    fn test_evaluate_rip_outcome_john_wick_3_trailing_sectors() {
        // John Wick 3: 7547 seconds ripped, 952MB output, title duration 7312.4s.
        // At EOF, optical drive hits bad sector / demux EOF trap and exits non-zero (code 1).
        let out_path = Path::new("Films/John Wick Chapter 3 - Parabellum (2019)/movie.mp4");
        let outcome = evaluate_rip_outcome(
            false,
            Some(1),
            952_660_706,
            7546.96,
            Some(7312.4),
            true,
            false,
            false,
            2,
            out_path,
            &["Error during demuxing: Input/output error".to_string()],
            false,
        );
        match outcome {
            RipOutcome::TrailingSectorNotice {
                exit_code,
                output_size_mb,
                ripped_mins,
            } => {
                assert_eq!(exit_code, Some(1));
                assert!(output_size_mb > 900.0);
                assert!(ripped_mins > 120.0);
            }
            _ => panic!("Expected TrailingSectorNotice, got {:?}", outcome),
        }
    }

    #[test]
    fn test_evaluate_rip_outcome_success_with_demux_warning() {
        // Normal success with demux warning mid-stream or at end, but full size
        let out_path = Path::new("Films/Movie/movie.mp4");
        let outcome = evaluate_rip_outcome(
            true,
            Some(0),
            800_000_000,
            5000.0,
            Some(5000.0),
            true,
            false,
            false,
            1,
            out_path,
            &[],
            false,
        );
        assert_eq!(outcome, RipOutcome::Success);
    }

    #[test]
    fn test_evaluate_rip_outcome_early_demux_error() {
        // Early stop on decoy trap (only 20MB produced)
        let out_path = Path::new("Films/Movie/movie.mp4");
        let outcome = evaluate_rip_outcome(
            false,
            Some(1),
            20_000_000,
            120.0,
            Some(7200.0),
            true,
            false,
            false,
            1,
            out_path,
            &["Error during demuxing".to_string()],
            false,
        );
        match outcome {
            RipOutcome::FatalError(err) => {
                assert!(err.contains("stopped early"));
            }
            _ => panic!("Expected FatalError, got {:?}", outcome),
        }
    }

    #[test]
    fn test_build_ffmpeg_command_ignore_errors() {
        let args_enabled = Args {
            ignore_errors: true,
            ..Default::default()
        };
        let out_path = PathBuf::from("output.mp4");
        let cmd = build_ffmpeg_command(&args_enabled, Path::new("D:\\"), &out_path, 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_args.contains(&"-fflags".to_string()));
        assert!(cmd_args.contains(&"+genpts+discardcorrupt".to_string()));
        assert!(cmd_args.contains(&"-err_detect".to_string()));
        assert!(cmd_args.contains(&"ignore_err".to_string()));

        let args_disabled = Args {
            ignore_errors: false,
            ..Default::default()
        };
        let cmd2 = build_ffmpeg_command(&args_disabled, Path::new("D:\\"), &out_path, 1);
        let cmd2_args: Vec<String> = cmd2
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(!cmd2_args.contains(&"ignore_err".to_string()));
    }

    #[test]
    fn test_build_ffmpeg_command_hwaccel_hevc_and_av1() {
        let out_path = PathBuf::from("output.mp4");

        // NVENC HEVC & AV1
        let args_nvenc_hevc = Args {
            transcode: true,
            hwaccel: "nvenc".to_string(),
            codec: "hevc".to_string(),
            ..Default::default()
        };
        let cmd = build_ffmpeg_command(&args_nvenc_hevc, Path::new("D:\\"), &out_path, 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_args.contains(&"hevc_nvenc".to_string()));

        let args_nvenc_av1 = Args {
            transcode: true,
            hwaccel: "nvenc".to_string(),
            codec: "av1".to_string(),
            ..Default::default()
        };
        let cmd_av1 = build_ffmpeg_command(&args_nvenc_av1, Path::new("D:\\"), &out_path, 1);
        let cmd_av1_args: Vec<String> = cmd_av1
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_av1_args.contains(&"av1_nvenc".to_string()));

        // QSV HEVC
        let args_qsv_hevc = Args {
            transcode: true,
            hwaccel: "qsv".to_string(),
            codec: "hevc".to_string(),
            ..Default::default()
        };
        let cmd_qsv = build_ffmpeg_command(&args_qsv_hevc, Path::new("D:\\"), &out_path, 1);
        let cmd_qsv_args: Vec<String> = cmd_qsv
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_qsv_args.contains(&"hevc_qsv".to_string()));
    }

    #[test]
    fn test_resolve_tv_output_path_with_title() {
        let temp = TestTempDir::new("tv_output_title");
        let out_dir_str = temp.0.to_string_lossy().to_string();

        let args = Args {
            out_dir: out_dir_str,
            tv: true,
            ..Default::default()
        };

        let path = resolve_tv_output_path_with_title(
            &args,
            Some("The Office"),
            Some(2005),
            1,
            2,
            Some("Diversity Day"),
        )
        .unwrap();

        let path_str = path.to_string_lossy();
        assert!(path_str.contains("The Office (2005)"));
        assert!(path_str.contains("Season 01"));
        assert!(path_str.contains("The Office - S01E02 - Diversity Day.mp4"));
    }

    #[test]
    fn test_build_thumbnail_command() {
        let out_thumb = PathBuf::from("test_thumb.jpg");
        let cmd = build_thumbnail_command("ffmpeg", Path::new("D:\\"), 2, 120.0, &out_thumb);
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"-ss".to_string()));
        assert!(args.contains(&"00:02:00".to_string()));
        assert!(args.contains(&"-title".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"-vframes".to_string()));
        assert!(args.contains(&"1".to_string()));
    }

    #[test]
    fn test_ffmpeg_formatting_and_detection_helpers() {
        assert_eq!(format_episode_code(2, 4), "S02E04");
        assert_eq!(format_episode_filename("Firefly", 1, 2), "Firefly - S01E02");
        assert_eq!(
            format_tv_season_folder("Firefly", Some(2002), 1),
            PathBuf::from("TV").join("Firefly (2002)").join("Season 01")
        );
        assert_eq!(
            format_movie_folder("Aliens", Some(1986)),
            PathBuf::from("Films").join("Aliens (1986)")
        );

        let vf_chain = format_hwaccel_vf_chain("cuda", &["yadif".to_string()]);
        assert!(vf_chain.is_some());

        let summary = format_title_selection_summary(1, Some(5400.0), Some(5410.0));
        assert!(summary.contains("Title #1"));

        let langs = parse_ranked_audio_languages("eng,jpn,fra");
        assert_eq!(langs, vec!["eng", "jpn", "fra"]);

        let best_title = detect_best_title(
            "nonexistent_ffmpeg",
            Path::new("Z:\\NonExistent"),
            Some(5400.0),
        );
        assert!(best_title <= 99);

        let longest_title =
            detect_longest_title("nonexistent_ffmpeg", Path::new("Z:\\NonExistent"));
        assert!(longest_title <= 99);

        let best_info = detect_best_title_info(
            "nonexistent_ffmpeg",
            Path::new("Z:\\NonExistent"),
            Some(5400.0),
        );
        assert!(best_info.0 <= 99);

        let tv_eps = detect_tv_episodes(
            "nonexistent_ffmpeg",
            Path::new("Z:\\NonExistent"),
            "Firefly",
            1,
            1,
            None,
        );
        assert!(tv_eps.is_empty());

        let titles_fast = probe_dvd_titles_fast("nonexistent_ffmpeg", Path::new("Z:\\NonExistent"));
        assert!(titles_fast.is_empty());

        let titles_slow =
            probe_dvd_titles("nonexistent_ffmpeg", Path::new("Z:\\NonExistent"), None);
        assert!(titles_slow.is_empty());

        let dur =
            probe_single_title_duration("nonexistent_ffmpeg", Path::new("Z:\\NonExistent"), 1);
        assert!(dur.is_none());

        // apply_hwaccel_encoder tests for other accelerators
        let mut cmd_vaapi = Command::new("ffmpeg");
        apply_hwaccel_encoder(&mut cmd_vaapi, "vaapi", "fast");
        let mut cmd_vtb = Command::new("ffmpeg");
        apply_hwaccel_encoder(&mut cmd_vtb, "videotoolbox", "fast");
    }

    #[test]
    fn test_cropdetect_ivtc_stream_details_and_preview_commands() {
        // 1. parse_cropdetect_output
        let sample_stderr = "\
[Parsed_cropdetect_0 @ 0x1] x1:0 x2:719 y1:60 y2:419 w:720 h:352 x:0 y:64 pts:100 t:1.0 crop=720:352:0:64
[Parsed_cropdetect_0 @ 0x1] x1:0 x2:719 y1:58 y2:421 w:720 h:360 x:0 y:60 pts:200 t:2.0 crop=720:360:0:60
[Parsed_cropdetect_0 @ 0x1] x1:0 x2:719 y1:58 y2:421 w:720 h:360 x:0 y:60 pts:300 t:3.0 crop=720:360:0:60
";
        let modal_crop = parse_cropdetect_output(sample_stderr);
        assert_eq!(modal_crop, Some("crop=720:360:0:60".to_string()));

        // 2. IVTC + Crop filter chain
        let mut args = Args::default();
        args.transcode = true;
        args.detelecine = true;
        args.denoise = true;
        args.start_chapter = Some(2);
        args.end_chapter = Some(6);
        args.angle = Some(2);
        args.crf = Some(18);

        let vf = build_video_filter_chain_with_crop(&args, "standard", modal_crop.as_deref());
        assert_eq!(vf.len(), 3);
        assert!(vf[0].contains("fieldmatch"));
        assert_eq!(vf[1], "crop=720:360:0:60");
        assert!(vf[2].contains("hqdn3d"));

        // 3. build_ffmpeg_command with chapter range, angle, and custom CRF
        let cmd =
            build_ffmpeg_command(&args, Path::new("D:\\"), Path::new("C:\\Out\\Movie.mp4"), 1);
        let cmd_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(cmd_args.contains(&"-chapter_start".to_string()));
        assert!(cmd_args.contains(&"2".to_string()));
        assert!(cmd_args.contains(&"-chapter_end".to_string()));
        assert!(cmd_args.contains(&"6".to_string()));
        assert!(cmd_args.contains(&"-angle".to_string()));
        assert!(cmd_args.contains(&"18".to_string()));

        // 4. build_preview_clip_command
        let prev_cmd = build_preview_clip_command(
            &args,
            Path::new("D:\\"),
            1,
            120.0,
            10,
            Path::new("C:\\Temp\\preview.mp4"),
        );
        let prev_args: Vec<String> = prev_cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert!(prev_args.contains(&"-t".to_string()));
        assert!(prev_args.contains(&"10".to_string()));

        // 5. parse_ffmpeg_stream_details
        let probe_stderr = "\
  Duration: 01:45:00.00, start: 0.280000, bitrate: 5200 kb/s
    Chapter #0:0: start 0.000000, end 600.000000
    Chapter #0:1: start 600.000000, end 1200.000000
  Stream #0:0[0x1e0]: Video: mpeg2video (Main), yuv420p(tv), 720x480 [SAR 32:27 DAR 16:9], 29.97 fps
  Stream #0:1[0x80](eng): Audio: ac3, 48000 Hz, 5.1(side), fltp, 448 kb/s
  Stream #0:2[0x81](fra): Audio: ac3, 48000 Hz, stereo, fltp, 192 kb/s
  Stream #0:3[0x20](eng): Subtitle: dvd_subtitle
  Stream #0:4[0x21](spa): Subtitle: dvd_subtitle (forced)
";
        let details = parse_ffmpeg_stream_details(probe_stderr, 1);
        assert_eq!(details.duration_secs, 6300.0);
        assert_eq!(details.chapters.len(), 2);
        assert_eq!(details.audio_streams.len(), 2);
        assert_eq!(details.audio_streams[0].language, "eng");
        assert_eq!(details.audio_streams[0].channels, 6);
        assert_eq!(details.subtitle_streams.len(), 2);
        assert!(details.subtitle_streams[1].forced);
    }

    #[test]
    fn test_perceptual_quality_ffmetadata_concat_bluray_and_audiocd() {
        // 1. SSIM & PSNR output parsers
        let ssim_stderr =
            "[Parsed_ssim_0 @ 0x7f] SSIM Y:0.989123 U:0.992111 V:0.991000 All:0.986543 (18.7102)\n";
        assert_eq!(parse_ffmpeg_ssim_output(ssim_stderr), Some(0.986543));

        let psnr_stderr = "[Parsed_psnr_0 @ 0x7f] PSNR y:42.10 u:45.20 v:44.80 average:42.85 min:38.10 max:49.00\n";
        assert_eq!(parse_ffmpeg_psnr_output(psnr_stderr), Some(42.85));

        // 2. predict_optimal_crf fallback curve
        let probe_res = predict_optimal_crf(
            "nonexistent_ffmpeg",
            Path::new("Z:\\NonExistent"),
            1,
            "hevc",
            0.985,
        );
        assert_eq!(probe_res.tested_crf, 20);
        assert!((probe_res.mean_ssim - 0.985).abs() < 1e-6);

        // 3. generate_ffmetadata_chapters
        let chapters = vec![
            crate::domain::entities::disc::Chapter {
                index: 1,
                start_secs: 0.0,
                end_secs: 300.5,
                title: "Chapter 1".to_string(),
            },
            crate::domain::entities::disc::Chapter {
                index: 2,
                start_secs: 300.5,
                end_secs: 900.0,
                title: "Chapter 2".to_string(),
            },
        ];
        let custom_names = vec!["Prologue".to_string(), "The Council=Elrond".to_string()];
        let ffmeta = generate_ffmetadata_chapters(
            "The Fellowship of the Ring",
            &chapters,
            Some(&custom_names),
        );
        assert!(ffmeta.starts_with(";FFMETADATA1\n"));
        assert!(ffmeta.contains("title=The Fellowship of the Ring"));
        assert!(ffmeta.contains("START=0\nEND=300500\ntitle=Prologue"));
        assert!(ffmeta.contains("START=300500\nEND=900000\ntitle=The Council\\=Elrond"));

        // 4. build_multi_disc_concat_list
        let parts = vec![
            PathBuf::from("C:\\Movies\\LOTR_Part1.mkv"),
            PathBuf::from("C:\\Movies\\Director's_Part2.mkv"),
        ];
        let concat_txt = build_multi_disc_concat_list(&parts);
        assert!(concat_txt.contains("file 'C:\\Movies\\LOTR_Part1.mkv'"));
        assert!(concat_txt.contains("Director'\\''s_Part2.mkv"));

        // 5. build_bluray_ffmpeg_command & build_audio_cd_track_command & generate_m3u8_playlist
        let mut args = Args::default();
        args.transcode = true;
        args.codec = "hevc".to_string();
        args.all_audio = true;
        args.subtitles = true;
        args.angle = Some(1);
        let bd_cmd = build_bluray_ffmpeg_command(
            &args,
            Path::new("D:\\"),
            800,
            Path::new("C:\\Out\\Movie.mkv"),
        );
        let bd_args: Vec<String> = bd_cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(bd_args.contains(&"-playlist".to_string()));
        assert!(bd_args.contains(&"800".to_string()));
        assert!(bd_args.contains(&"bluray:D:\\".to_string()));

        let cd_cmd = build_audio_cd_track_command(
            "ffmpeg",
            Path::new("D:\\Track01.cda"),
            1,
            "Dark Side of the Moon",
            "Speak to Me",
            Path::new("C:\\Music\\01 - Speak to Me.flac"),
        );
        let cd_args: Vec<String> = cd_cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(cd_args.contains(&"flac".to_string()));
        assert!(cd_args.contains(&"album=Dark Side of the Moon".to_string()));

        let m3u8 = generate_m3u8_playlist(&[(
            "Speak to Me".to_string(),
            90.4,
            PathBuf::from("C:\\Music\\01 - Speak to Me.flac"),
        )]);
        assert!(m3u8.starts_with("#EXTM3U\n"));
        assert!(m3u8.contains("#EXTINF:90,Speak to Me\n01 - Speak to Me.flac"));

        // 6. calculate_target_video_bitrate_kbps (VidCoder target size calculator)
        let v_kbps = calculate_target_video_bitrate_kbps(1400, 7200.0, 192);
        assert!((1350..=1450).contains(&v_kbps));
    }

    #[test]
    fn test_evaluate_rip_outcome_rejects_60_percent_truncated_episode() {
        // Episode 2 is 2700s (45 mins), stops at 60% (1620s, 180 MB) with exit status 0
        let out_path = Path::new("TV/Show/Season 01/Show - S01E02.mp4");
        let outcome = evaluate_rip_outcome(
            true,
            Some(0),
            180_000_000,
            1620.0,
            Some(2700.0),
            false,
            false,
            false,
            2,
            out_path,
            &[],
            true,
        );
        match outcome {
            RipOutcome::FatalError(err) => {
                assert!(err.contains("stopped early at 60.0%"));
            }
            other => panic!("Expected FatalError for 60% truncated episode, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_vts_ttn_to_pgcn_maps_ptt_srpt_correctly() {
        // Build a synthetic VTS_01_0.IFO with VTS_PTT_SRPT at sector 1 (0x800)
        // mapping vts_ttn=1 -> pgcn=1 and vts_ttn=2 -> pgcn=3 (skipping Play-All pgcn=2)
        let mut vts_data = vec![0u8; 4096];
        // Pointer to VTS_PTT_SRPT at 0xC8..0xCC = sector 1
        vts_data[0xC8..0xCC].copy_from_slice(&1u32.to_be_bytes());
        let ptt_off = 2048;
        // nr_of_srpts = 2 titles
        vts_data[ptt_off..ptt_off + 2].copy_from_slice(&2u16.to_be_bytes());
        // Offset to TTU #1 = 16, Offset to TTU #2 = 20
        vts_data[ptt_off + 8..ptt_off + 12].copy_from_slice(&16u32.to_be_bytes());
        vts_data[ptt_off + 12..ptt_off + 16].copy_from_slice(&20u32.to_be_bytes());
        // TTU #1 first PTT: pgcn = 1, pgn = 1
        vts_data[ptt_off + 16..ptt_off + 18].copy_from_slice(&1u16.to_be_bytes());
        vts_data[ptt_off + 18..ptt_off + 20].copy_from_slice(&1u16.to_be_bytes());
        // TTU #2 first PTT: pgcn = 3, pgn = 1
        vts_data[ptt_off + 20..ptt_off + 22].copy_from_slice(&3u16.to_be_bytes());
        vts_data[ptt_off + 22..ptt_off + 24].copy_from_slice(&1u16.to_be_bytes());

        assert_eq!(resolve_vts_ttn_to_pgcn(&vts_data, 1), 1);
        assert_eq!(resolve_vts_ttn_to_pgcn(&vts_data, 2), 3);
        // Out-of-range vts_ttn falls back to vts_ttn
        assert_eq!(resolve_vts_ttn_to_pgcn(&vts_data, 4), 4);
    }
}
