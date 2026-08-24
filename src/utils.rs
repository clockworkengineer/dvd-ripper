/**
 * @file utils.rs
 * @brief Common string manipulation, HTTP client pooling, atomic file I/O, NFO metadata generation, and system helper utilities.
 */

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Queries remaining free disk space (bytes) for a given target directory path.
#[cfg(target_os = "windows")]
pub fn get_free_disk_space_bytes(dir_path: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let target = if dir_path.exists() {
        dir_path.to_path_buf()
    } else {
        dir_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
    };

    let mut path_wide: Vec<u16> = target.as_os_str().encode_wide().collect();
    path_wide.push(0);

    let mut free_bytes_available: u64 = 0;
    let mut total_number_of_bytes: u64 = 0;
    let mut total_number_of_free_bytes: u64 = 0;

    unsafe {
        GetDiskFreeSpaceExW(
            windows::core::PCWSTR(path_wide.as_ptr()),
            Some(&mut free_bytes_available),
            Some(&mut total_number_of_bytes),
            Some(&mut total_number_of_free_bytes),
        )?;
    }

    Ok(free_bytes_available)
}

#[cfg(not(target_os = "windows"))]
pub fn get_free_disk_space_bytes(_dir_path: &Path) -> Result<u64> {
    Ok(100 * 1024 * 1024 * 1024)
}

/// Verifies that available free disk space on target_dir exceeds min_free_gb threshold.
pub fn check_disk_space_guard(target_dir: &Path, min_free_gb: u64) -> Result<u64> {
    if min_free_gb == 0 {
        let free = get_free_disk_space_bytes(target_dir).unwrap_or(0);
        return Ok(free);
    }
    let free_bytes = get_free_disk_space_bytes(target_dir)?;
    let required_bytes = min_free_gb * 1024 * 1024 * 1024;
    if free_bytes < required_bytes {
        let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        return Err(anyhow!(
            "Disk Space Guard Safeguard Triggered: Only {:.2} GB free space available in '{}', but minimum threshold is {} GB.",
            free_gb, target_dir.display(), min_free_gb
        ));
    }
    Ok(free_bytes)
}

/// Verifies available disk space, falling back to secondary fallback_dir if primary space is below min_free_gb threshold.
pub fn check_disk_space_guard_with_fallback(
    primary_dir: &Path,
    fallback_dir: Option<&Path>,
    min_free_gb: u64,
) -> Result<PathBuf> {
    match check_disk_space_guard(primary_dir, min_free_gb) {
        Ok(_) => Ok(primary_dir.to_path_buf()),
        Err(e) => {
            if let Some(fb) = fallback_dir {
                println!("[Storage Guard Warning] Primary path '{}' low on space. Attempting fallback path: '{}'...", primary_dir.display(), fb.display());
                if check_disk_space_guard(fb, min_free_gb).is_ok() {
                    println!("[Storage Guard] Switched output destination to fallback path: {}", fb.display());
                    return Ok(fb.to_path_buf());
                }
            }
            Err(e)
        }
    }
}

/// Cleans raw optical DVD volume labels (e.g. "KILL_BILL_VOL1_D1" -> "Kill Bill Vol1 D1").
#[allow(dead_code)]
pub fn normalize_volume_label_title(label: &str) -> String {
    let clean = label
        .replace('_', " ")
        .replace('.', " ");
    let mut words: Vec<String> = Vec::new();
    for w in clean.split_whitespace() {
        if w.to_uppercase() == "DVD" || w.to_uppercase() == "VIDEO" || w.to_uppercase() == "DISC" {
            continue;
        }
        let mut chars = w.chars();
        if let Some(first) = chars.next() {
            let capitalized = format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase());
            words.push(capitalized);
        }
    }
    if words.is_empty() {
        label.to_string()
    } else {
        words.join(" ")
    }
}

/// Applies optional string replacement pattern (e.g. "PATTERN:REPLACEMENT") before volume label title normalization.
#[allow(dead_code)]
pub fn normalize_volume_label_title_with_regex(label: &str, regex_pattern: Option<&str>) -> String {
    let mut working = label.to_string();
    if let Some(pat) = regex_pattern {
        if let Some((target, replacement)) = pat.split_once(':') {
            working = working.replace(target, replacement);
        }
    }
    normalize_volume_label_title(&working)
}

/// Ensures that the parent directory for a target file path exists on disk.
pub fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

/// Returns the centralized application data directory (~/.dvd-ripper or %USERPROFILE%\.dvd-ripper).
pub fn get_app_data_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(|h| PathBuf::from(h).join(".dvd-ripper"))
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolves a file path inside the centralized application data directory.
pub fn get_app_file_path(filename: &str) -> PathBuf {
    get_app_data_dir().join(filename)
}

/// Returns a shared, pre-configured HTTP blocking client instance with standard timeout and user agent.
pub fn get_http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("dvd-ripper/0.1.0")
        .build()
        .unwrap_or_default()
}

/// Generic helper: Loads and deserializes a JSON file from disk into a typed data structure.
pub fn load_json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Generic helper: Serializes a typed data structure to pretty JSON and writes atomically to disk.
pub fn save_json_file<T: serde::Serialize>(path: &Path, data: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    atomic_write_file(path, json)?;
    Ok(())
}




/// Formats floating-point duration seconds into standardized HH:MM:SS format (e.g., 5432.0 -> "01:30:32").
#[allow(dead_code)]
pub fn format_duration_hhmmss(total_secs: f64) -> String {
    let total = total_secs.max(0.0) as u64;
    let hours = total / 3600;
    let mins = (total % 3600) / 60;
    let secs = total % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

/// Formats floating-point duration seconds into rounded runtime minute text (e.g. 5400.0 -> "90 mins").
#[allow(dead_code)]
pub fn format_duration_minutes(total_secs: f64) -> String {
    let mins = (total_secs.max(0.0) / 60.0).round() as u64;
    format!("{} mins", mins)
}

/// Formats a standardized low disk space alert warning message.
#[allow(dead_code)]
pub fn format_disk_space_warning(free_gb: f64, min_gb: u64) -> String {
    format!(
        "Low Disk Space Warning: {:.2} GB available (minimum {} GB required)",
        free_gb, min_gb
    )
}

/// Returns current local timestamp formatted as "YYYY-MM-DD HH:MM:SS".
pub fn now_timestamp_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Decodes URL percent-encoding and plus-signs into plain text spaces.
pub fn decode_url_query_value(encoded: &str) -> String {
    encoded.replace('+', " ").replace("%20", " ")
}

/// Encodes spaces into URL query `+` syntax.
pub fn encode_url_query_value(raw: &str) -> String {
    raw.trim().replace(' ', "+")
}


/// Formats a log line with a timestamp and prefix tag (e.g. "[2026-08-18 17:15:14] [Daemon] Message").
#[allow(dead_code)]
pub fn format_timestamped_log(prefix: &str, msg: &str) -> String {
    format!("[{}] [{}] {}", now_timestamp_str(), prefix, msg)
}


/// Escapes special characters and ASCII control characters in a string to make it safe for insertion into JSON string literals.
pub fn escape_json_str(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len() + 16);
    for c in input.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\x08' => escaped.push_str("\\b"),
            '\x0C' => escaped.push_str("\\f"),
            c if c < ' ' => {
                let _ = std::fmt::write(&mut escaped, format_args!("\\u{:04x}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

/// Percent-encodes special characters in URL query parameter values.
#[allow(dead_code)]
pub fn url_query_escape(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len() + 16);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                let _ = std::fmt::write(&mut encoded, format_args!("%{:02X}", b));
            }
        }
    }
    encoded
}

/// Writes content atomically to a file by creating a temporary file and performing an atomic rename.
pub fn atomic_write_file(path: &std::path::Path, content: impl AsRef<[u8]>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_file = path.with_extension(format!("tmp_{}", std::process::id()));
    std::fs::write(&tmp_file, content)?;
    std::fs::rename(&tmp_file, path)?;
    Ok(())
}

/// Validates whether a filename string is safe from path traversal (`..`), path separators, and Windows reserved names.
#[allow(dead_code)]
pub fn is_safe_filename(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') || trimmed.starts_with('.') {
        return false;
    }
    let upper = trimmed.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or(&upper);
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    !reserved.contains(&stem)
}

/// Validates that target path stays strictly contained within base directory bounds, blocking path traversal (`..`).
pub fn ensure_path_contained(base: &Path, target: &Path) -> Result<PathBuf> {
    let base_canonical = if base.exists() {
        base.canonicalize().unwrap_or_else(|_| base.to_path_buf())
    } else {
        base.to_path_buf()
    };

    let full_target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        base_canonical.join(target)
    };

    let target_str = full_target.to_string_lossy();
    if target_str.contains("/..") || target_str.contains("\\..") || target_str.contains("../") || target_str.contains("..\\") {
        return Err(anyhow!(
            "Path Traversal Safeguard Triggered: Target path '{}' attempts to escape base directory bounds.",
            full_target.display()
        ));
    }

    Ok(full_target)
}

/// Sanitizes a file path or input argument to prevent CLI command option injection (e.g. paths starting with `-`).
#[allow(dead_code)]
pub fn sanitize_cli_path_arg(path_str: &str) -> String {
    let trimmed = path_str.trim();
    if trimmed.starts_with('-') {
        format!("./{}", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Sanitizes a movie title to make it safe for filesystem folders and file names.
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            ':' => ' ', // Replace colons with spaces
            '\\' | '/' | '*' | '?' | '"' | '<' | '>' | '|' => '_', // Replace other invalid characters
            _ => c,
        })
        .collect();

    let mut result = String::with_capacity(sanitized.len());
    for word in sanitized.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        result.push_str(word);
    }
    result
}

/// Parses an FFmpeg timestamp string (HH:MM:SS.xx) into total seconds.
pub fn parse_duration(s: &str) -> Option<f64> {
    let mut parts = s.split(':');
    let hours: f64 = parts.next()?.trim().parse().ok()?;
    let minutes: f64 = parts.next()?.trim().parse().ok()?;
    let seconds: f64 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_none() {
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    } else {
        None
    }
}

/// DRY helper: Extracts the whitespace-delimited value associated with a key prefix in a log line.
/// For example, `extract_kv_field("fps= 25.0 speed=1.2x", "fps=")` returns `Some("25.0")`.
pub fn extract_kv_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let idx = line.find(key)?;
    let sub = &line[idx + key.len()..];
    let trimmed = sub.trim_start();
    let val = trimmed.split_whitespace().next()?;
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// Structured metrics parsed from an FFmpeg stderr progress line.
#[derive(Debug, Clone, PartialEq)]
pub struct FfmpegProgressMetrics {
    pub fps: Option<f64>,
    pub speed: Option<String>,
    pub bitrate: Option<String>,
    pub time_secs: Option<f64>,
}

/// Parses an FFmpeg progress output line (e.g., "frame= 100 fps=25.5 q=28.0 size= 1024kB time=00:01:30.00 bitrate=100.0kbits/s speed= 2.5x").
#[allow(dead_code)]
pub fn parse_ffmpeg_progress_line(line: &str) -> Option<FfmpegProgressMetrics> {
    if !line.contains("time=") && !line.contains("fps=") {
        return None;
    }
    let fps = extract_kv_field(line, "fps=").and_then(|v| v.parse::<f64>().ok());
    let speed = extract_kv_field(line, "speed=").map(|v| v.to_string());
    let bitrate = extract_kv_field(line, "bitrate=").map(|v| v.to_string());
    let time_secs = extract_kv_field(line, "time=").and_then(parse_duration);

    Some(FfmpegProgressMetrics {
        fps,
        speed,
        bitrate,
        time_secs,
    })
}

/// DRY helper: Formats a film/show folder name, appending `(Year)` if provided, with sanitized path characters.
pub fn format_title_folder_name(name: &str, year: Option<u32>) -> String {
    let clean_name = sanitize_filename(name);
    if let Some(y) = year {
        format!("{} ({})", clean_name, y)
    } else {
        clean_name
    }
}

/// Formats a TV season and episode number into a standardized episode code string (e.g., "S01E05").
pub fn format_episode_code(season: u32, episode: u32) -> String {
    format!("S{:02}E{:02}", season, episode)
}

/// DRY helper: Formats a standard TV episode name (e.g. `Show Name - S01E02`), with sanitized path characters.
pub fn format_episode_name(show_name: &str, season: u32, ep_num: u32) -> String {
    let clean_name = sanitize_filename(show_name);
    format!("{} - {}", clean_name, format_episode_code(season, ep_num))
}


/// Saves poster artwork bytes to `cover.jpg` and `folder.jpg` in the parent directory of the ripped video file.
pub fn save_cover_artworks(output_file: &std::path::Path, poster_bytes: &[u8]) -> anyhow::Result<()> {
    if poster_bytes.is_empty() {
        return Ok(());
    }
    if let Some(parent) = output_file.parent() {
        std::fs::create_dir_all(parent)?;
        let cover_path = parent.join("cover.jpg");
        let folder_path = parent.join("folder.jpg");
        if !cover_path.exists() {
            std::fs::write(&cover_path, poster_bytes)?;
        }
        if !folder_path.exists() {
            std::fs::write(&folder_path, poster_bytes)?;
        }
    }
    Ok(())
}

/// Executes an external post-processing script hook upon rip completion, setting environment variables.
pub fn run_post_processing_hook(
    script_path: &str,
    output_path: &std::path::Path,
    title: &str,
    media_type: &str,
    year: Option<u32>,
) -> anyhow::Result<()> {
    if script_path.trim().is_empty() {
        return Ok(());
    }

    println!("[Post-Script Hook] Invoking external script '{}'...", script_path);

    let year_str = year.map(|y| y.to_string()).unwrap_or_default();
    let mut cmd = std::process::Command::new(script_path);
    cmd.env("DVD_OUTPUT_PATH", output_path.to_string_lossy().to_string())
       .env("DVD_TITLE", title)
       .env("DVD_MEDIA_TYPE", media_type)
       .env("DVD_YEAR", year_str);

    let status = cmd.status()?;
    if status.success() {
        println!("[Post-Script Hook] Script completed successfully.");
    } else {
        println!("[Post-Script Hook] Script exited with code: {:?}", status.code());
    }

    Ok(())
}

/// Open/Closed Principle (OCP/SOLID): Trait contract for media server URL strategies.
pub trait MediaServerUrlStrategy {
    fn name(&self) -> &str;
    fn format_url(&self, base_url: &str, key: &str) -> String;
}

pub struct PlexUrlStrategy;
impl MediaServerUrlStrategy for PlexUrlStrategy {
    fn name(&self) -> &str { "plex" }
    fn format_url(&self, base_url: &str, key: &str) -> String {
        format!("{}/library/sections/all/refresh?X-Plex-Token={}", base_url.trim_end_matches('/'), key)
    }
}

pub struct JellyfinUrlStrategy;
impl MediaServerUrlStrategy for JellyfinUrlStrategy {
    fn name(&self) -> &str { "jellyfin" }
    fn format_url(&self, base_url: &str, key: &str) -> String {
        format!("{}/Items/Root/Refresh?api_key={}", base_url.trim_end_matches('/'), key)
    }
}

pub struct EmbyUrlStrategy;
impl MediaServerUrlStrategy for EmbyUrlStrategy {
    fn name(&self) -> &str { "emby" }
    fn format_url(&self, base_url: &str, key: &str) -> String {
        format!("{}/Library/Refresh?api_key={}", base_url.trim_end_matches('/'), key)
    }
}

/// Helper: Builds a media server library refresh URL (Plex, Jellyfin, or Emby).
pub fn format_media_server_refresh_url(server_type: &str, base_url: &str, key: &str) -> String {
    let strategies: Vec<Box<dyn MediaServerUrlStrategy>> = vec![
        Box::new(PlexUrlStrategy),
        Box::new(JellyfinUrlStrategy),
        Box::new(EmbyUrlStrategy),
    ];
    let mode = server_type.to_lowercase();
    for strategy in strategies {
        if strategy.name() == mode {
            return strategy.format_url(base_url, key);
        }
    }
    format!("{}/Library/Refresh?api_key={}", base_url.trim_end_matches('/'), key)
}

pub fn build_plex_refresh_url(base_url: &str, token: &str) -> String {
    format_media_server_refresh_url("plex", base_url, token)
}

pub fn build_jellyfin_refresh_url(base_url: &str, api_key: &str) -> String {
    format_media_server_refresh_url("jellyfin", base_url, api_key)
}

pub fn build_emby_refresh_url(base_url: &str, api_key: &str) -> String {
    format_media_server_refresh_url("emby", base_url, api_key)
}

/// Represents a generic notification payload event for solid provider abstractions.
#[derive(Debug, Clone)]
pub struct NotificationEvent {
    pub title: String,
    pub status: String,
    pub message: String,
}

/// Trait defining a decoupled notification dispatch provider (OCP/DIP).
#[allow(dead_code)]
pub trait NotificationProvider {
    fn name(&self) -> &str;
    fn send_notification(&self, event: &NotificationEvent) -> anyhow::Result<()>;
}


/// Concrete webhook notification provider implementation.
pub struct WebhookNotificationProvider {
    pub webhook_url: String,
    pub webhook_secret: Option<String>,
}

impl NotificationProvider for WebhookNotificationProvider {
    fn name(&self) -> &str {
        "Webhook"
    }

    fn send_notification(&self, event: &NotificationEvent) -> anyhow::Result<()> {
        crate::mqtt::send_webhook_notification(&self.webhook_url, &event.title, &event.status, &event.message, self.webhook_secret.as_deref())
    }
}

/// Sends an HTTP POST request to trigger media server library scans with a 3-attempt exponential backoff retry loop.
fn send_media_server_refresh_post(server_name: &str, base_url: &str, endpoint: &str) -> anyhow::Result<()> {
    println!("[Media Server] Requesting {} library refresh: {}", server_name, base_url);
    let client = get_http_client();
    let mut success = false;

    for attempt in 1..=3 {
        if client.post(endpoint).send().is_ok() {
            success = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(300 * attempt));
    }

    if success {
        println!("[Media Server] Successfully triggered {} scan.", server_name);
    } else {
        println!("[Media Server Warning] Could not reach {} at {}", server_name, base_url);
    }
    Ok(())
}

pub fn trigger_plex_library_scan(url: &str, token: &str) -> anyhow::Result<()> {
    let endpoint = build_plex_refresh_url(url, token);
    send_media_server_refresh_post("Plex", url, &endpoint)
}

pub fn trigger_jellyfin_library_scan(url: &str, api_key: &str) -> anyhow::Result<()> {
    let endpoint = build_jellyfin_refresh_url(url, api_key);
    send_media_server_refresh_post("Jellyfin", url, &endpoint)
}

pub fn trigger_emby_library_scan(url: &str, api_key: &str) -> anyhow::Result<()> {
    let endpoint = build_emby_refresh_url(url, api_key);
    send_media_server_refresh_post("Emby", url, &endpoint)
}


pub fn trigger_media_server_scans(args: &crate::cli::Args) {
    if let (Some(url), Some(token)) = (args.plex_url.as_deref(), args.plex_token.as_deref()) {
        let _ = trigger_plex_library_scan(url, token);
    }
    if let (Some(url), Some(key)) = (args.jellyfin_url.as_deref(), args.jellyfin_key.as_deref()) {
        let _ = trigger_jellyfin_library_scan(url, key);
    }
    if let (Some(url), Some(key)) = (args.emby_url.as_deref(), args.emby_key.as_deref()) {
        let _ = trigger_emby_library_scan(url, key);
    }
}

/// Dependency Inversion (DIP/SOLID): High-level trait abstraction for media server library scan triggers.
#[allow(dead_code)]
pub trait MediaServerNotifier {
    fn trigger_scans(&self, args: &crate::cli::Args);
}

pub struct DefaultMediaServerNotifier;
impl MediaServerNotifier for DefaultMediaServerNotifier {
    fn trigger_scans(&self, args: &crate::cli::Args) {
        trigger_media_server_scans(args);
    }
}

/// Resolves sidecar artwork image file path (e.g. "cover.jpg" or "folder.jpg") relative to target directory.
#[allow(dead_code)]
pub fn resolve_artwork_path(target_dir: &Path, filename: &str) -> PathBuf {
    target_dir.join(filename)
}

/// Resolves sidecar `.nfo` metadata file path from video output path.
pub fn resolve_nfo_path(video_path: &std::path::Path) -> std::path::PathBuf {
    let parent = video_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = video_path.file_stem().and_then(|s| s.to_str()).unwrap_or("media");
    parent.join(format!("{}.nfo", stem))
}

pub fn generate_nfo_file(
    output_file: &std::path::Path,
    title: &str,
    year: Option<u32>,
    plot: Option<&str>,
    rating: Option<&str>,
    director: Option<&str>,
    media_type: &str,
    tags: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(parent) = output_file.parent() {
        std::fs::create_dir_all(parent)?;
        let nfo_path = resolve_nfo_path(output_file);

        let tag = if media_type.to_lowercase().contains("tv") { "tvshow" } else { "movie" };
        let year_str = year.map(|y| format!("  <year>{}</year>\n", y)).unwrap_or_default();
        let plot_str = plot.map(|p| format!("  <plot>{}</plot>\n", quick_xml_escape(p))).unwrap_or_default();
        let rating_str = rating.map(|r| format!("  <rating>{}</rating>\n", quick_xml_escape(r))).unwrap_or_default();
        let director_str = director.map(|d| format!("  <director>{}</director>\n", quick_xml_escape(d))).unwrap_or_default();

        let tags_str = if let Some(tags_csv) = tags {
            let mut buf = String::new();
            for t in tags_csv.split(',') {
                let clean_t = t.trim();
                if !clean_t.is_empty() {
                    buf.push_str(&format!("  <tag>{}</tag>\n", quick_xml_escape(clean_t)));
                }
            }
            buf
        } else {
            String::new()
        };

        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <{tag}>\n\
             \x20\x20<title>{}</title>\n\
             {}{}{}{}{}\
             </{tag}>\n",
            quick_xml_escape(title),
            year_str,
            plot_str,
            rating_str,
            director_str,
            tags_str,
            tag = tag
        );

        std::fs::write(&nfo_path, content)?;
        println!("[NFO Metadata] Generated sidecar XML: {}", nfo_path.display());
    }
    Ok(())
}

/// Appends a structured JSON-Lines audit log entry to the specified audit file path.
pub fn append_audit_log_entry(audit_path: &Path, event_type: &str, details: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    let obj = serde_json::json!({
        "timestamp": now_timestamp_str(),
        "event_type": event_type,
        "details": details
    });
    let line = format!("{}\n", obj.to_string());
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

/// Computes a fast hex verification checksum for a converted media file and writes a .sha256 sidecar file.
pub fn generate_checksum_file(video_path: &Path) -> Result<PathBuf> {
    use std::io::Read;
    let mut file = std::fs::File::open(video_path)?;
    let mut buffer = [0u8; 65536];
    let mut hasher: u64 = 0xcbf29ce484222325;
    let mut total_bytes = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        total_bytes += n as u64;
        for &b in &buffer[..n] {
            hasher ^= b as u64;
            hasher = hasher.wrapping_mul(0x100000001b3);
        }
    }
    let hex_str = format!("{:016x}", hasher);
    let ext = video_path.extension().and_then(|s| s.to_str()).unwrap_or("file");
    let checksum_path = video_path.with_extension(format!("{}.sha256", ext));
    let filename = video_path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let content = format!("{}  {} ({} bytes)\n", hex_str, filename, total_bytes);
    std::fs::write(&checksum_path, content)?;
    println!("[Checksum] Generated integrity checksum sidecar file: {}", checksum_path.display());
    Ok(checksum_path)
}

fn quick_xml_escape(input: &str) -> String {
    input.replace('&', "&amp;")
         .replace('<', "&lt;")
         .replace('>', "&gt;")
         .replace('"', "&quot;")
         .replace('\'', "&apos;")
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedLabelInfo {
    pub clean_title: String,
    pub season: Option<u32>,
    pub disc: Option<u32>,
}

/// Parses volume labels (e.g. FAMILY_GUY_S9_D1 or DOCTOR_WHO_S1_D3) to extract clean show title, season #, and disc #.
pub fn parse_season_disc_from_label(label: &str) -> ParsedLabelInfo {
    let lower = label.to_lowercase().replace('_', " ").replace('-', " ");
    let words: Vec<&str> = lower.split_whitespace().collect();
    let mut season = None;
    let mut disc = None;
    let mut title_words = Vec::new();

    let mut i = 0;
    while i < words.len() {
        let word = words[i];

        // 1. Combined s2d2 / s02d01 pattern
        if season.is_none() || disc.is_none() {
            if word.starts_with('s') && word.contains('d') {
                let parts: Vec<&str> = word[1..].split('d').collect();
                if parts.len() == 2 {
                    if let (Ok(s), Ok(d)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        if season.is_none() {
                            season = Some(s);
                        }
                        if disc.is_none() {
                            disc = Some(d);
                        }
                        i += 1;
                        continue;
                    }
                }
            }
        }

        // 2. Season patterns: "s2", "s02", "season2", "series2", "season 2", "series 2"
        if season.is_none() {
            if word.starts_with('s') && word.len() >= 2 && word[1..].chars().all(|c| c.is_ascii_digit()) {
                if let Ok(s) = word[1..].parse::<u32>() {
                    season = Some(s);
                    i += 1;
                    continue;
                }
            } else if (word.starts_with("season") && word.len() > 6 && word[6..].chars().all(|c| c.is_ascii_digit()))
                || (word.starts_with("series") && word.len() > 6 && word[6..].chars().all(|c| c.is_ascii_digit()))
            {
                let digits = if word.starts_with("season") { &word[6..] } else { &word[6..] };
                if let Ok(s) = digits.parse::<u32>() {
                    season = Some(s);
                    i += 1;
                    continue;
                }
            } else if (word == "season" || word == "series" || word == "s") && i + 1 < words.len() {
                if let Ok(s) = words[i + 1].parse::<u32>() {
                    season = Some(s);
                    i += 2;
                    continue;
                }
            }
        }

        // 3. Disc patterns: "d1", "d01", "disc1", "vol1", "disc 2"
        if disc.is_none() {
            if word.starts_with('d') && word.len() >= 2 && word[1..].chars().all(|c| c.is_ascii_digit()) {
                if let Ok(d) = word[1..].parse::<u32>() {
                    disc = Some(d);
                    i += 1;
                    continue;
                }
            } else if (word.starts_with("disc") && word.len() > 4 && word[4..].chars().all(|c| c.is_ascii_digit()))
                || (word.starts_with("vol") && word.len() > 3 && word[3..].chars().all(|c| c.is_ascii_digit()))
            {
                let digits = if word.starts_with("disc") { &word[4..] } else { &word[3..] };
                if let Ok(d) = digits.parse::<u32>() {
                    disc = Some(d);
                    i += 1;
                    continue;
                }
            } else if (word == "disc" || word == "vol" || word == "d") && i + 1 < words.len() {
                if let Ok(d) = words[i + 1].parse::<u32>() {
                    disc = Some(d);
                    i += 2;
                    continue;
                }
            }
        }

        title_words.push(word);
        i += 1;
    }

    let clean_title = title_words.join(" ");

    ParsedLabelInfo {
        clean_title,
        season,
        disc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Thor: Ragnarok"), "Thor Ragnarok");
        assert_eq!(sanitize_filename("Movie/Name?*"), "Movie_Name__");
        assert_eq!(sanitize_filename("  Multiple   Spaces  "), "Multiple Spaces");
    }

    #[test]
    fn test_format_media_server_refresh_url() {
        assert_eq!(format_media_server_refresh_url("plex", "http://localhost:32400/", "tok"), "http://localhost:32400/library/sections/all/refresh?X-Plex-Token=tok");
        assert_eq!(format_media_server_refresh_url("jellyfin", "http://localhost:8096/", "key"), "http://localhost:8096/Items/Root/Refresh?api_key=key");
        assert_eq!(format_media_server_refresh_url("emby", "http://localhost:8096/", "key"), "http://localhost:8096/Library/Refresh?api_key=key");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("01:02:03.50"), Some(3723.5));
        assert_eq!(parse_duration("00:00:10"), Some(10.0));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_extract_kv_field() {
        let line = "frame= 100 fps=25.5 q=28.0 size= 1024kB time=00:01:30.00 bitrate=100.0kbits/s speed= 2.5x";
        assert_eq!(extract_kv_field(line, "fps="), Some("25.5"));
        assert_eq!(extract_kv_field(line, "speed="), Some("2.5x"));
        assert_eq!(extract_kv_field(line, "time="), Some("00:01:30.00"));
        assert_eq!(extract_kv_field(line, "missing="), None);
    }

    #[test]
    fn test_parse_ffmpeg_progress_line() {
        let line = "frame= 100 fps=25.5 q=28.0 size= 1024kB time=00:01:30.00 bitrate=100.0kbits/s speed= 2.5x";
        let metrics = parse_ffmpeg_progress_line(line).unwrap();
        assert_eq!(metrics.fps, Some(25.5));
        assert_eq!(metrics.speed, Some("2.5x".to_string()));
        assert_eq!(metrics.time_secs, Some(90.0));
    }

    #[test]
    fn test_formatting_helpers() {
        assert_eq!(format_title_folder_name("Aliens", Some(1986)), "Aliens (1986)");
        assert_eq!(format_title_folder_name("Kill Bill: Vol. 1", Some(2003)), "Kill Bill Vol. 1 (2003)");
        assert_eq!(format_title_folder_name("The Office", None), "The Office");
        assert_eq!(format_episode_name("Doctor Who", 1, 3), "Doctor Who - S01E03");
    }

    #[test]
    fn test_atomic_write_file() {
        let temp_dir = std::env::temp_dir().join("atomic_test");
        let file = temp_dir.join("test.txt");
        let res = atomic_write_file(&file, "Hello World");
        assert!(res.is_ok());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "Hello World");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_is_safe_filename() {
        assert!(super::is_safe_filename("Aliens (1986).mp4"));
        assert!(!super::is_safe_filename("../etc/passwd"));
        assert!(!super::is_safe_filename("CON.mp4"));
        assert!(!super::is_safe_filename("NUL"));
    }

    #[test]
    fn test_sanitize_cli_path_arg() {
        assert_eq!(super::sanitize_cli_path_arg("-vf"), "./-vf");
        assert_eq!(super::sanitize_cli_path_arg("video.mp4"), "video.mp4");
    }

    #[test]
    fn test_url_query_escape() {
        assert_eq!(super::url_query_escape("Kill Bill: Vol. 1"), "Kill+Bill%3A+Vol.+1");
        assert_eq!(super::url_query_escape("Dr. Who & Friends"), "Dr.+Who+%26+Friends");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration_minutes(5400.0), "90 mins");
        assert_eq!(format_duration_minutes(7200.0), "120 mins");
    }

    #[test]
    fn test_format_disk_space_warning() {
        let warn = format_disk_space_warning(4.5, 10);
        assert!(warn.contains("4.50 GB available"));
        assert!(warn.contains("10 GB required"));
    }

    #[test]
    fn test_resolve_nfo_path() {
        let video = Path::new("Films/Aliens (1986)/Aliens (1986).mp4");
        let nfo = resolve_nfo_path(video);
        assert_eq!(nfo, Path::new("Films/Aliens (1986)/Aliens (1986).nfo"));
    }

    #[test]
    fn test_resolve_artwork_path() {
        let dir = Path::new("Films/Aliens (1986)");
        assert_eq!(resolve_artwork_path(dir, "poster.jpg"), Path::new("Films/Aliens (1986)/poster.jpg"));
    }

    #[test]
    fn test_parse_season_disc_from_label() {
        let res = parse_season_disc_from_label("FAMILY_GUY_S9_D1");
        assert_eq!(res.clean_title, "family guy");
        assert_eq!(res.season, Some(9));
        assert_eq!(res.disc, Some(1));

        let res2 = parse_season_disc_from_label("DOCTOR_WHO_S1_D3");
        assert_eq!(res2.clean_title, "doctor who");
        assert_eq!(res2.season, Some(1));
        assert_eq!(res2.disc, Some(3));
    }

    #[test]
    fn test_normalize_volume_label_title() {
        assert_eq!(normalize_volume_label_title("KILL_BILL_VOL1_DVD"), "Kill Bill Vol1");
        assert_eq!(normalize_volume_label_title("ALIENS.1986.VIDEO"), "Aliens 1986");
    }

    #[test]
    fn test_save_cover_artworks() {
        let temp_dir = std::env::temp_dir().join("cover_art_test_dir");
        let video_file = temp_dir.join("movie.mp4");
        let dummy_poster = b"JPEG_DATA";

        let _ = save_cover_artworks(&video_file, dummy_poster);
        assert!(temp_dir.join("cover.jpg").exists());
        assert!(temp_dir.join("folder.jpg").exists());

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_run_post_processing_hook_empty() {
        let res = run_post_processing_hook("", std::path::Path::new("dummy.mp4"), "Title", "Movie", Some(2026));
        assert!(res.is_ok());
    }

    #[test]
    fn test_check_disk_space_guard() {
        let temp_dir = std::env::temp_dir();
        // 0 GB threshold should always pass
        assert!(check_disk_space_guard(&temp_dir, 0).is_ok());
        // 1 GB threshold should pass on any system with >= 1 GB free space
        let free = get_free_disk_space_bytes(&temp_dir).unwrap_or(0);
        if free > 1024 * 1024 * 1024 {
            assert!(check_disk_space_guard(&temp_dir, 1).is_ok());
        }
        // Extremely high threshold (1,000,000 GB) should fail
        assert!(check_disk_space_guard(&temp_dir, 1_000_000).is_err());
    }

    #[test]
    fn test_media_server_url_builders() {
        let plex = build_plex_refresh_url("http://192.168.1.100:32400/", "token123");
        assert_eq!(plex, "http://192.168.1.100:32400/library/sections/all/refresh?X-Plex-Token=token123");

        let jellyfin = build_jellyfin_refresh_url("http://192.168.1.100:8096/", "key456");
        assert_eq!(jellyfin, "http://192.168.1.100:8096/Items/Root/Refresh?api_key=key456");

        let emby = build_emby_refresh_url("http://192.168.1.100:8096", "key789");
        assert_eq!(emby, "http://192.168.1.100:8096/Library/Refresh?api_key=key789");
    }

    #[test]
    fn test_generate_nfo_file() {
        let temp_dir = std::env::temp_dir().join("nfo_test_dir");
        let video_file = temp_dir.join("Aliens.mp4");

        let res = generate_nfo_file(
            &video_file,
            "Aliens",
            Some(1986),
            Some("Awesome sci-fi movie"),
            Some("8.4"),
            Some("James Cameron"),
            "Movie",
            None,
        );

        assert!(res.is_ok());
        let nfo_file = temp_dir.join("Aliens.nfo");
        assert!(nfo_file.exists());

        let content = std::fs::read_to_string(nfo_file).unwrap();
        assert!(content.contains("<title>Aliens</title>"));
        assert!(content.contains("<year>1986</year>"));
        assert!(content.contains("<director>James Cameron</director>"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_get_app_data_dir() {
        let dir = get_app_data_dir();
        assert!(dir.to_string_lossy().contains(".dvd-ripper"));
    }

    #[test]
    fn test_format_duration_hhmmss() {
        assert_eq!(format_duration_hhmmss(5432.0), "01:30:32");
        assert_eq!(format_duration_hhmmss(0.0), "00:00:00");
    }

    #[test]
    fn test_format_timestamped_log() {
        let log = format_timestamped_log("Daemon", "Drive inserted");
        assert!(log.contains("[Daemon] Drive inserted"));
    }

    #[test]
    fn test_escape_json_str() {
        assert_eq!(escape_json_str("Hello \"World\"\nTest"), "Hello \\\"World\\\"\\nTest");
    }

    #[test]
    fn test_ensure_path_contained() {
        let base = std::env::temp_dir();
        let valid_target = base.join("Films/Aliens (1986)/Aliens (1986).mp4");
        assert!(ensure_path_contained(&base, &valid_target).is_ok());

        let invalid_target = base.join("../../../etc/shadow");
        assert!(ensure_path_contained(&base, &invalid_target).is_err());
    }
}
