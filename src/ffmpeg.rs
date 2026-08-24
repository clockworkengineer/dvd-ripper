/**
 * @file ffmpeg.rs
 * @brief FFmpeg process invocation and real-time progress parsing.
 */

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Context, Result};

use crate::cli::Args;
use crate::utils::{extract_kv_field, format_episode_name, format_title_folder_name, parse_duration};

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
    let extension = if args.mkv {
        "mkv"
    } else if is_transcode_enabled(args) {
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

    let target_path = ensure_absolute_parent_dir(&args.out_dir, rel_or_abs_file, args.no_overwrite)?;
    crate::utils::ensure_path_contained(Path::new(&args.out_dir), &target_path)
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
    } else if is_transcode_enabled(args) {
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

    let target_path = ensure_absolute_parent_dir(root_dir, rel_file, args.no_overwrite)?;
    crate::utils::ensure_path_contained(Path::new(root_dir), &target_path)
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
pub fn format_title_selection_summary(detected_title: u32, duration_opt: Option<f64>, expected_runtime_secs: Option<f64>) -> String {
    let dur_str = duration_opt.map(|d| format!(" ({:.0} mins)", d / 60.0)).unwrap_or_default();
    if expected_runtime_secs.is_some() {
        format!("Auto-selected Title #{}{} (matched running time)", detected_title, dur_str)
    } else {
        format!("Auto-selected Title #{}{} (longest duration on DVD)", detected_title, dur_str)
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

/// Determines whether output target should use Matroska MKV container format.
pub fn is_mkv_output(args: &Args, output_path: &Path) -> bool {
    let profile = args.profile.to_lowercase();
    args.mkv
        || profile == "archival"
        || output_path
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("mkv"))
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
        cmd.arg("-metadata:s:a:0").arg("title=Stereo AAC (Normalized)");

        cmd.arg("-map").arg("0:a:0?");
        cmd.arg("-c:a:1").arg("copy");
        cmd.arg("-metadata:s:a:1").arg("title=5.1 Surround Passthrough");
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

/// Constructs a vector of video filter string elements based on resolution scaling, deinterlacing, and denoise flags.
pub fn build_video_filter_chain(args: &Args, profile: &str) -> Vec<String> {
    let mut vf_filters = Vec::new();
    if profile == "mobile" {
        vf_filters.push("scale=-2:720".to_string());
    }
    if args.deinterlace {
        let algo = match args.deinterlace_algo.as_deref().unwrap_or("bwdif").to_lowercase().as_str() {
            "yadif" => "yadif=1:-1:0",
            "w3fdif" => "w3fdif=filter=complex",
            _ => "bwdif=mode=send_frame:parity=auto:deint=all",
        };
        vf_filters.push(algo.to_string());
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
        vf_filters.push(format!("zscale=transfer=linear,tonemap=tonemap={}:desat=0", curve));
    }
    vf_filters
}

/// Trait defining a strategy provider for GPU hardware accelerated video encoding (OCP/SOLID).
pub trait HwAccelProvider {
    fn name(&self) -> &str;
    fn apply(&self, cmd: &mut Command, preset: &str);
}

pub struct NvencHwAccelProvider;
impl HwAccelProvider for NvencHwAccelProvider {
    fn name(&self) -> &str { "nvenc" }
    fn apply(&self, cmd: &mut Command, preset: &str) {
        cmd.arg("-c:v").arg("h264_nvenc");
        cmd.arg("-preset").arg(preset);
    }
}

pub struct QsvHwAccelProvider;
impl HwAccelProvider for QsvHwAccelProvider {
    fn name(&self) -> &str { "qsv" }
    fn apply(&self, cmd: &mut Command, preset: &str) {
        cmd.arg("-c:v").arg("h264_qsv");
        cmd.arg("-preset").arg(preset);
    }
}

pub struct VaapiHwAccelProvider;
impl HwAccelProvider for VaapiHwAccelProvider {
    fn name(&self) -> &str { "vaapi" }
    fn apply(&self, cmd: &mut Command, _preset: &str) {
        cmd.arg("-vaapi_device").arg("/dev/dri/renderD128");
        cmd.arg("-c:v").arg("h264_vaapi");
    }
}

pub struct V4l2HwAccelProvider;
impl HwAccelProvider for V4l2HwAccelProvider {
    fn name(&self) -> &str { "v4l2" }
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

/// Probes all titles on the DVD drive, using fast single-pass probing with fallback to sequential probing.
pub fn probe_dvd_titles(
    ffmpeg_path: &str,
    dvd_path: &Path,
    cancel_flag: Option<&std::sync::atomic::AtomicBool>,
) -> Vec<DvdTitleInfo> {
    let fast_results = probe_dvd_titles_fast(ffmpeg_path, dvd_path);
    if fast_results.iter().any(|t| t.duration_secs >= 300.0) {
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

/// Probes the DVD drive to find the title number and probed duration best matching expected_runtime_secs, or with the longest duration.
pub fn detect_best_title_info(
    ffmpeg_path: &str,
    dvd_path: &Path,
    expected_runtime_secs: Option<f64>,
) -> (u32, Option<f64>) {
    let titles = probe_dvd_titles(ffmpeg_path, dvd_path, None);
    if titles.is_empty() {
        return (1, None);
    }

    let mut best_title = 1u32;
    let mut best_duration = None;
    let mut best_diff = f64::MAX;
    let mut max_duration = 0.0f64;

    for t in titles {
        if let Some(target) = expected_runtime_secs {
            let diff = (t.duration_secs - target).abs();
            if diff < best_diff {
                best_diff = diff;
                best_title = t.title_num;
                best_duration = Some(t.duration_secs);
            }
        } else if t.duration_secs > max_duration {
            max_duration = t.duration_secs;
            best_title = t.title_num;
            best_duration = Some(t.duration_secs);
        }
    }

    (best_title, best_duration)
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
        let vf_filters = build_video_filter_chain(args, &profile);

        if profile == "mobile" {
            if !vf_filters.is_empty() {
                cmd.arg("-vf").arg(vf_filters.join(","));
            }
            cmd.arg("-c:v").arg("libx264");
            cmd.arg("-preset").arg(&args.preset);
            cmd.arg("-crf").arg("24");
            cmd.arg("-c:a").arg("aac");
            cmd.arg("-b:a").arg("128k");
        } else if profile == "plex" {
            if !vf_filters.is_empty() {
                cmd.arg("-vf").arg(vf_filters.join(","));
            }
            cmd.arg("-c:v").arg(resolve_video_codec_name(&codec));
            cmd.arg("-preset").arg(&args.preset);
            cmd.arg("-crf").arg("20");
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
                    cmd.arg("-c:v").arg("h264_vaapi");
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "nvenc" => {
                    if !vf_filters.is_empty() {
                        cmd.arg("-vf").arg(vf_filters.join(","));
                    }
                    cmd.arg("-c:v").arg("h264_nvenc");
                    cmd.arg("-preset").arg(&args.preset);
                    cmd.arg("-c:a").arg("aac");
                    cmd.arg("-b:a").arg("128k");
                }
                "qsv" => {
                    if !vf_filters.is_empty() {
                        cmd.arg("-vf").arg(vf_filters.join(","));
                    }
                    cmd.arg("-c:v").arg("h264_qsv");
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
    BenchmarkFinished(crate::dvd::DriveBenchmarkReport),
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
    let (resolved_title, probed_duration) = if args.title == 0 {
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

        let (detected, duration_opt) = detect_best_title_info(&args.ffmpeg, dvd_path, expected_runtime_secs);
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
    } else {
        (args.title, None)
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

    if absolute_output.exists() {
        std::fs::remove_file(absolute_output)
            .with_context(|| format!("Failed to remove existing output file {}", absolute_output.display()))?;
    }

    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .context("Failed to spawn FFmpeg process. Is it installed and in your PATH?")?;

    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to capture FFmpeg stderr"))?;

    let mut total_seconds: Option<f64> = probed_duration.or(expected_runtime_secs);
    let mut demux_error = false;
    let mut empty_output = false;
    let mut css_error = false;

    let mut buf = [0u8; 1024];
    let mut line_bytes = Vec::new();

    loop {
        if let Some(ref flag) = cancel_flag {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
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
                let _ = child.wait();
                let msg = "Ripping process cancelled by user.".to_string();
                if let Some(ref sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(msg.clone()));
                }
                return Err(anyhow!(msg));
            }
        }

        let n = match stderr.read(&mut buf) {
            Ok(0) => {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            Ok(n) => n,
            Err(_) => {
                if let Ok(Some(_)) = child.try_wait() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
        };

        for &b in &buf[..n] {
            if b == b'\r' || b == b'\n' {
                if !line_bytes.is_empty() {
                    let line = String::from_utf8_lossy(&line_bytes).to_string();
                    line_bytes.clear();

                    if let Some(ref sender) = tx {
                        let _ = sender.send(ProgressEvent::Log(line.clone()));
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
                        if let Some(secs) = parse_duration(clean_duration) {
                            // Require duration >= 5 minutes (300s) to filter out short chapters, menus, & sub-streams
                            if secs >= 300.0 {
                                match total_seconds {
                                    Some(current) => {
                                        // Refine total_seconds only if candidate duration is within 35% of current estimate
                                        if (secs - current).abs() < current * 0.35 {
                                            total_seconds = Some(secs);
                                        }
                                    }
                                    None => {
                                        total_seconds = Some(secs);
                                    }
                                }
                            }
                        }
                    }

                    // Parse time=, speed=, and fps= using DRY helper
                    if let Some(time_str) = extract_kv_field(&line, "time=") {
                        let clean_time = time_str.trim_start_matches('-');
                        if let Some(secs) = parse_duration(clean_time) {
                            let speed = extract_kv_field(&line, "speed=").unwrap_or("N/A").to_string();
                            let fps = extract_kv_field(&line, "fps=").unwrap_or("N/A").to_string();

                            if let Some(total) = total_seconds {
                                if total > 0.0 {
                                    let percent = (secs / total * 100.0).min(100.0).max(0.0);

                                    crate::api::update_appliance_status("Ripping", "", display_title, percent, &fps, &speed);

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
                        }
                    }
                }
            } else {
                line_bytes.push(b);
            }
        }
    }

    let status = child.wait().context("Failed to wait on FFmpeg process")?;

    if status.success() {
        let output_size = std::fs::metadata(absolute_output)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if output_size == 0 || demux_error || empty_output || css_error {
            let err_msg = if css_error {
                format!(
                    "FFmpeg cannot rip this encrypted DVD because CSS support is unavailable. Install libdvdcss and retry (output: {}).",
                    absolute_output.display()
                )
            } else {
                format!(
                    "FFmpeg did not produce a valid rip at {} (output size: {} bytes; DVD demux error: {})",
                    absolute_output.display(),
                    output_size,
                    demux_error
                )
            };
            crate::api::fail_appliance_status(display_title, "", &err_msg);
            if let Some(ref sender) = tx {
                let _ = sender.send(ProgressEvent::Error(err_msg.clone()));
            } else {
                eprintln!("\n{}", err_msg);
            }
            return Err(anyhow!(err_msg));
        }

        crate::api::update_appliance_status("Completed", "", display_title, 100.0, "N/A", "N/A");
        if let Some(ref sender) = tx {
            let _ = sender.send(ProgressEvent::Progress {
                percent: 100.0,
                fps: "N/A".to_string(),
                speed: "N/A".to_string(),
            });
        }
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
        assert!(path.parent().map_or(false, |p| p.exists()));
    }

    #[test]
    fn test_build_ffmpeg_command_title_argument() {
        let args = Args::default();

        let output_path = PathBuf::from("Films/Test/Test.mp4");
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

        let output_path = PathBuf::from("Films/Test/Test.mp4");
        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), &output_path, 1);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

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
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

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

        let dummy_file = temp.0.join("output.mp4");
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
    fn test_build_ffmpeg_command_chapters_mapping() {
        let args = Args {
            chapters: true,
            ..Default::default()
        };

        let cmd = build_ffmpeg_command(&args, Path::new("D:\\"), Path::new("test.mp4"), 1);
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

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
        let cmd_args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();

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
    fn test_format_episode_helpers() {
        assert_eq!(format_episode_code(1, 5), "S01E05");
        assert_eq!(format_episode_filename("The Office", 1, 5), "The Office - S01E05");
    }

    #[test]
    fn test_format_tv_season_folder() {
        let folder = format_tv_season_folder("The Office", Some(2005), 1);
        assert_eq!(folder, PathBuf::from("TV").join("The Office (2005)").join("Season 01"));
    }

    #[test]
    fn test_format_movie_folder() {
        let folder = format_movie_folder("Aliens", Some(1986));
        assert_eq!(folder, PathBuf::from("Films").join("Aliens (1986)"));
    }

    #[test]
    fn test_build_video_filter_chain() {
        let args = Args { deinterlace: true, denoise: true, ..Default::default() };
        let filters = build_video_filter_chain(&args, "mobile");
        assert_eq!(filters.len(), 3);
        assert_eq!(filters[0], "scale=-2:720");
    }

    #[test]
    fn test_is_mkv_output() {
        let args_mkv = Args { mkv: true, ..Default::default() };
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
        assert_eq!(format_hwaccel_vf_chain("vaapi", &filters), Some("scale=-2:720,bwdif,format=nv12,hwupload".to_string()));
        assert_eq!(format_hwaccel_vf_chain("copy", &filters), Some("scale=-2:720,bwdif".to_string()));
        assert_eq!(format_hwaccel_vf_chain("vaapi", &[]), Some("format=nv12,hwupload".to_string()));
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
        assert_eq!(msg, "Auto-selected Title #3 (120 mins) (longest duration on DVD)");
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
}
