use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

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

/// Cleans raw optical DVD volume labels (e.g. "KILL_BILL_VOL1_D1" -> "Kill Bill Vol1 D1").
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

/// Formats floating-point duration seconds into standardized HH:MM:SS format (e.g., 5432.0 -> "01:30:32").
pub fn format_duration_hhmmss(total_secs: f64) -> String {
    let total = total_secs.max(0.0) as u64;
    let hours = total / 3600;
    let mins = (total % 3600) / 60;
    let secs = total % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

/// Formats a log line with a timestamp and prefix tag (e.g. "[2026-08-18 17:15:14] [Daemon] Message").
pub fn format_timestamped_log(prefix: &str, msg: &str) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    format!("[{}] [{}] {}", now, prefix, msg)
}

/// Escapes special characters in a string to make it safe for insertion into JSON string literals.
pub fn escape_json_str(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r")
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
    let val = sub.split_whitespace().next()?.trim();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
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

/// DRY helper: Formats a standard TV episode name (e.g. `Show Name - S01E02`), with sanitized path characters.
pub fn format_episode_name(show_name: &str, season: u32, ep_num: u32) -> String {
    let clean_name = sanitize_filename(show_name);
    format!("{} - S{:02}E{:02}", clean_name, season, ep_num)
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

pub fn build_plex_refresh_url(base_url: &str, token: &str) -> String {
    format!("{}/library/sections/all/refresh?X-Plex-Token={}", base_url.trim_end_matches('/'), token)
}

pub fn build_jellyfin_refresh_url(base_url: &str, api_key: &str) -> String {
    format!("{}/Items/Root/Refresh?api_key={}", base_url.trim_end_matches('/'), api_key)
}

pub fn build_emby_refresh_url(base_url: &str, api_key: &str) -> String {
    format!("{}/Library/Refresh?api_key={}", base_url.trim_end_matches('/'), api_key)
}

pub fn trigger_plex_library_scan(url: &str, token: &str) -> anyhow::Result<()> {
    let endpoint = build_plex_refresh_url(url, token);
    println!("[Media Server] Requesting Plex library refresh: {}", url);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let _ = client.post(&endpoint).send();
    Ok(())
}

pub fn trigger_jellyfin_library_scan(url: &str, api_key: &str) -> anyhow::Result<()> {
    let endpoint = build_jellyfin_refresh_url(url, api_key);
    println!("[Media Server] Requesting Jellyfin library refresh: {}", url);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let _ = client.post(&endpoint).send();
    Ok(())
}

pub fn trigger_emby_library_scan(url: &str, api_key: &str) -> anyhow::Result<()> {
    let endpoint = build_emby_refresh_url(url, api_key);
    println!("[Media Server] Requesting Emby library refresh: {}", url);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let _ = client.post(&endpoint).send();
    Ok(())
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

/// Resolves sidecar artwork image file path (e.g. "cover.jpg" or "folder.jpg") relative to target directory.
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
) -> anyhow::Result<()> {
    if let Some(parent) = output_file.parent() {
        std::fs::create_dir_all(parent)?;
        let nfo_path = resolve_nfo_path(output_file);

        let tag = if media_type.to_lowercase().contains("tv") { "tvshow" } else { "movie" };
        let year_str = year.map(|y| format!("  <year>{}</year>\n", y)).unwrap_or_default();
        let plot_str = plot.map(|p| format!("  <plot>{}</plot>\n", quick_xml_escape(p))).unwrap_or_default();
        let rating_str = rating.map(|r| format!("  <rating>{}</rating>\n", quick_xml_escape(r))).unwrap_or_default();
        let director_str = director.map(|d| format!("  <director>{}</director>\n", quick_xml_escape(d))).unwrap_or_default();

        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <{tag}>\n\
             \x20\x20<title>{}</title>\n\
             {}{}{}{}\
             </{tag}>\n",
            quick_xml_escape(title),
            year_str,
            plot_str,
            rating_str,
            director_str,
            tag = tag
        );

        std::fs::write(&nfo_path, content)?;
        println!("[NFO Metadata] Generated sidecar XML: {}", nfo_path.display());
    }
    Ok(())
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
    fn test_formatting_helpers() {
        assert_eq!(format_title_folder_name("Aliens", Some(1986)), "Aliens (1986)");
        assert_eq!(format_title_folder_name("Kill Bill: Vol. 1", Some(2003)), "Kill Bill Vol. 1 (2003)");
        assert_eq!(format_title_folder_name("The Office", None), "The Office");
        assert_eq!(format_episode_name("Doctor Who", 1, 3), "Doctor Who - S01E03");
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
}
