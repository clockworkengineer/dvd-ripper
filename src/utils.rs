/**
 * @file utils.rs
 * @brief Helper utility functions for parsing durations, sanitizing filenames, and extracting string fields.
 */

/// Sanitizes a movie title to make it safe for filesystem folders and file names.
pub fn sanitize_filename(name: &str) -> String {
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

/// Parses an FFmpeg timestamp string (HH:MM:SS.xx) into total seconds.
pub fn parse_duration(s: &str) -> Option<f64> {
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

/// Scans the target TV season folder to find the highest existing episode number, returning next start episode (max_ep + 1).
pub fn find_next_start_episode(
    out_dir: &str,
    show_name: &str,
    show_year: Option<u32>,
    season: u32,
) -> u32 {
    let root_dir = if out_dir == "Films" { "TV" } else { out_dir };
    let show_folder = if let Some(year) = show_year {
        format!("{} ({})", show_name, year)
    } else {
        show_name.to_string()
    };
    let season_folder = format!("Season {:02}", season);
    let target_dir = std::path::PathBuf::from(root_dir)
        .join(show_folder)
        .join(season_folder);

    if !target_dir.exists() {
        return 1;
    }

    let mut max_ep = 0u32;
    let season_pattern = format!("S{:02}E", season);

    if let Ok(entries) = std::fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Some(idx) = filename.find(&season_pattern) {
                let sub = &filename[idx + season_pattern.len()..];
                let ep_num_str: String = sub.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(ep_num) = ep_num_str.parse::<u32>() {
                    if ep_num > max_ep {
                        max_ep = ep_num;
                    }
                }
            }
        }
    }

    if max_ep > 0 {
        max_ep + 1
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    struct TestTempDir(std::path::PathBuf);

    impl TestTempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("dvd_ripper_utils_test_{}_{}", name, std::process::id()));
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
    fn test_find_next_start_episode() {
        let temp = TestTempDir::new("next_start_ep");
        let season_dir = temp.0.join("Doctor Who (2005)").join("Season 01");
        fs::create_dir_all(&season_dir).unwrap();

        File::create(season_dir.join("Doctor Who - S01E01.mpg")).unwrap();
        File::create(season_dir.join("Doctor Who - S01E02.mpg")).unwrap();
        File::create(season_dir.join("Doctor Who - S01E03.mpg")).unwrap();

        let out_dir_str = temp.0.to_string_lossy().to_string();
        let next_ep = find_next_start_episode(&out_dir_str, "Doctor Who", Some(2005), 1);
        assert_eq!(next_ep, 4);
    }
}
