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

/// Helper: Infers the starting episode number directly from the DVD volume label (e.g., DISC 3 or BBCDVD1757).
pub fn infer_start_episode_from_label(volume_label: &str, eps_per_disc: u32) -> Option<u32> {
    let label_upper = volume_label.to_uppercase();

    let default_eps = if eps_per_disc > 0 { eps_per_disc } else { 6 };

    // 1. Look for explicit disc patterns like DISC3, DISC_3, D3, VOL3, VOL_3, PART3
    let re_patterns = ["DISC", "VOL", "PART", "DISK"];

    for pat in &re_patterns {
        if let Some(idx) = label_upper.find(pat) {
            let sub = &label_upper[idx + pat.len()..];
            let num_str: String = sub
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(disc_num) = num_str.parse::<u32>() {
                if disc_num > 0 {
                    return Some((disc_num - 1) * default_eps + 1);
                }
            }
        }
    }

    // 2. Look for catalog codes ending in sequential numbers (e.g. BBCDVD1755 = Disc 1, 1756 = Disc 2, 1757 = Disc 3)
    let trailing_digits: String = label_upper
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    if trailing_digits.len() >= 2 {
        if let Ok(num) = trailing_digits.parse::<u32>() {
            if num >= 1000 {
                let base = num - (num % 10) + 5;
                let base_num = if num >= base { base } else { num };
                let disc_num = (num - base_num) + 1;
                return Some((disc_num - 1) * default_eps + 1);
            }
        }
    }

    None
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

    #[test]
    fn test_infer_start_episode_from_label() {
        assert_eq!(infer_start_episode_from_label("BBCDVD1755", 3), Some(1));
        assert_eq!(infer_start_episode_from_label("BBCDVD1756", 3), Some(4));
        assert_eq!(infer_start_episode_from_label("BBCDVD1757", 3), Some(7));
        assert_eq!(infer_start_episode_from_label("DRWHO_DISC3", 3), Some(7));
        assert_eq!(infer_start_episode_from_label("SHOW_S01_VOL2", 3), Some(4));
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
}
