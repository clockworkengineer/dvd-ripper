/**
 * @file imdb.rs
 * @brief IMDb suggestion API client and data deserialization structs.
 */

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

/// Represents a single movie search result from the IMDb suggestion database.
#[derive(Deserialize, Debug)]
pub struct ImdbEntry {
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

/// Queries the IMDb Suggest API to resolve a raw DVD volume label to a movie name and year.
pub fn lookup_film_details(query: &str) -> Result<(String, Option<u32>)> {
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

    Ok((best_match.l.clone(), best_match.y))
}
