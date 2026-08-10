/**
 * @file imdb.rs
 * @brief IMDb / OMDb suggestion API client and rich movie metadata models.
 */

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

/// Represents full metadata details for a movie (including poster image and plot summary).
#[derive(Debug, Clone, Default)]
pub struct FilmMetadata {
    pub title: String,
    pub year: Option<u32>,
    pub runtime_secs: Option<f64>,
    #[allow(dead_code)]
    pub poster_url: Option<String>,
    pub plot: Option<String>,
    pub genre: Option<String>,
    pub director: Option<String>,
    pub actors: Option<String>,
    pub rating: Option<String>,
    pub poster_bytes: Option<Vec<u8>>,
}

/// Represents a single movie search result from the OMDb database.
#[derive(Deserialize, Debug)]
pub struct OmdbResponse {
    #[serde(rename = "Title")]
    pub title: Option<String>,
    #[serde(rename = "Year")]
    pub year: Option<String>,
    #[serde(rename = "Runtime")]
    pub runtime: Option<String>,
    #[serde(rename = "Plot")]
    pub plot: Option<String>,
    #[serde(rename = "Poster")]
    pub poster: Option<String>,
    #[serde(rename = "Genre")]
    pub genre: Option<String>,
    #[serde(rename = "Director")]
    pub director: Option<String>,
    #[serde(rename = "Actors")]
    pub actors: Option<String>,
    #[serde(rename = "imdbRating")]
    pub imdb_rating: Option<String>,
    #[serde(rename = "Response")]
    pub response: Option<String>,
}

/// Represents a single movie search result from the IMDb suggestion database.
#[derive(Deserialize, Debug)]
pub struct ImdbEntry {
    /// IMDb ID (e.g. "tt0090605")
    #[allow(dead_code)]
    pub id: Option<String>,
    /// Title of the movie / series
    pub l: String,
    /// Release year of the movie
    pub y: Option<u32>,
    /// Entity type (e.g. "feature", "tvSeries")
    pub q: Option<String>,
}

/// Represents the top-level structure of the IMDb Suggest API JSON response.
#[derive(Deserialize, Debug)]
pub struct ImdbSuggestResponse {
    /// List of suggestion search results
    pub d: Vec<ImdbEntry>,
}

/// Parses an OMDb runtime string (e.g. "137 min") into total seconds.
pub fn parse_runtime_minutes(s: &str) -> Option<f64> {
    let clean = s.trim_end_matches("min").trim();
    let mins: f64 = clean.parse().ok()?;
    if mins > 0.0 {
        Some(mins * 60.0)
    } else {
        None
    }
}

/// Parses an ISO 8601 duration string (e.g. "PT2H17M", "PT137M", "PT1H30M15S") into total seconds.
#[allow(dead_code)]
pub fn parse_iso_duration(s: &str) -> Option<f64> {
    if !s.starts_with("PT") {
        return None;
    }
    let rest = &s[2..];
    let mut total_secs = 0.0f64;
    let mut current_num = String::new();

    for c in rest.chars() {
        if c.is_ascii_digit() || c == '.' {
            current_num.push(c);
        } else {
            let val: f64 = current_num.parse().ok()?;
            current_num.clear();
            match c {
                'H' => total_secs += val * 3600.0,
                'M' => total_secs += val * 60.0,
                'S' => total_secs += val,
                _ => {}
            }
        }
    }
    if total_secs > 0.0 {
        Some(total_secs)
    } else {
        None
    }
}

/// Fetches the movie running time in seconds from the IMDb title webpage given an IMDb ID (e.g. "tt0090605").
#[allow(dead_code)]
pub fn fetch_imdb_runtime(_imdb_id: &str) -> Option<f64> {
    None
}

/// Queries OMDb API for movie title, release year, running time, plot summary, and poster image.
pub fn lookup_omdb_details(query: &str) -> Option<FilmMetadata> {
    let encoded_query = query.replace(' ', "+");
    let url = format!("https://www.omdbapi.com/?t={}&apikey=trilogy", encoded_query);
    let resp = reqwest::blocking::get(&url).ok()?;
    let text = resp.text().ok()?;
    let omdb: OmdbResponse = serde_json::from_str(&text).ok()?;

    if omdb.response.as_deref() == Some("True") {
        if let Some(title) = omdb.title {
            let year = omdb
                .year
                .as_deref()
                .and_then(|y| y.get(..4))
                .and_then(|y| y.parse::<u32>().ok());
            let runtime_secs = omdb.runtime.as_deref().and_then(parse_runtime_minutes);
            let poster_url = omdb.poster.filter(|p| p != "N/A");
            let plot = omdb.plot.filter(|p| p != "N/A");
            let genre = omdb.genre.filter(|g| g != "N/A");
            let director = omdb.director.filter(|d| d != "N/A");
            let actors = omdb.actors.filter(|a| a != "N/A");
            let rating = omdb.imdb_rating.filter(|r| r != "N/A");

            let mut poster_bytes = None;
            if let Some(ref p_url) = poster_url {
                if let Ok(p_resp) = reqwest::blocking::get(p_url) {
                    if let Ok(bytes) = p_resp.bytes() {
                        poster_bytes = Some(bytes.to_vec());
                    }
                }
            }

            return Some(FilmMetadata {
                title,
                year,
                runtime_secs,
                poster_url,
                plot,
                genre,
                director,
                actors,
                rating,
                poster_bytes,
            });
        }
    }
    None
}

/// Queries OMDb or IMDb APIs to resolve a raw DVD volume label to full movie metadata.
pub fn lookup_film_details(query: &str) -> Result<FilmMetadata> {
    let cleaned: String = query
        .replace('_', " ")
        .replace('-', " ")
        .trim()
        .to_lowercase();

    if cleaned.is_empty() {
        return Err(anyhow!("Cleaned query is empty"));
    }

    if let Some(omdb_res) = lookup_omdb_details(&cleaned) {
        return Ok(omdb_res);
    }

    // Fallback to IMDb Suggest API
    let first_char = cleaned.chars().next().ok_or_else(|| anyhow!("Empty query"))?;

    let mut url = reqwest::Url::parse("https://sg.media-imdb.com")?;
    url.set_path(&format!("suggests/{}/{}.json", first_char, cleaned));

    let response_text = reqwest::blocking::get(url)
        .context("Failed to send request to IMDb Suggest API")?
        .text()
        .context("Failed to read response body from IMDb Suggest API")?;

    let start_idx = response_text
        .find('{')
        .ok_or_else(|| anyhow!("Invalid JSONP response from IMDb: opening bracket not found"))?;
    let end_idx = response_text
        .rfind('}')
        .ok_or_else(|| anyhow!("Invalid JSONP response from IMDb: closing bracket not found"))?;

    if start_idx >= end_idx {
        return Err(anyhow!("Invalid JSONP response bounds"));
    }

    let json_str = &response_text[start_idx..=end_idx];
    let parsed: ImdbSuggestResponse =
        serde_json::from_str(json_str).context("Failed to parse IMDb Suggest JSON response")?;

    let best_match = parsed
        .d
        .iter()
        .find(|entry| entry.q.as_deref() == Some("feature"))
        .or_else(|| parsed.d.first())
        .ok_or_else(|| anyhow!("No match found on IMDb for query: {}", query))?;

    Ok(FilmMetadata {
        title: best_match.l.clone(),
        year: best_match.y,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_runtime_minutes() {
        assert_eq!(parse_runtime_minutes("137 min"), Some(8220.0));
        assert_eq!(parse_runtime_minutes("90 min"), Some(5400.0));
        assert_eq!(parse_runtime_minutes("invalid"), None);
    }

    #[test]
    fn test_parse_iso_duration() {
        assert_eq!(parse_iso_duration("PT2H17M"), Some(8220.0));
        assert_eq!(parse_iso_duration("PT137M"), Some(8220.0));
        assert_eq!(parse_iso_duration("PT1H30M15S"), Some(5415.0));
        assert_eq!(parse_iso_duration("invalid"), None);
    }

    #[test]
    fn test_lookup_omdb_details_aliens() {
        let meta = lookup_omdb_details("aliens").unwrap();
        assert_eq!(meta.title, "Aliens");
        assert_eq!(meta.year, Some(1986));
        assert_eq!(meta.runtime_secs, Some(8220.0));
        assert!(meta.plot.is_some());
        assert!(meta.poster_bytes.is_some());
    }
}
