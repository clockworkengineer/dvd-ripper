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

/// DRY helper: Formats a film/show folder name, appending `(Year)` if provided.
pub fn format_title_folder_name(name: &str, year: Option<u32>) -> String {
    if let Some(y) = year {
        format!("{} ({})", name, y)
    } else {
        name.to_string()
    }
}

/// DRY helper: Formats a standard TV episode name (e.g. `Show Name - S01E02`).
pub fn format_episode_name(show_name: &str, season: u32, ep_num: u32) -> String {
    format!("{} - S{:02}E{:02}", show_name, season, ep_num)
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
        assert_eq!(format_title_folder_name("The Office", None), "The Office");
        assert_eq!(format_episode_name("Doctor Who", 1, 3), "Doctor Who - S01E03");
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
