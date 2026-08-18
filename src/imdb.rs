/**
 * @file imdb.rs
 * @brief IMDb / OMDb suggestion API client and rich movie metadata models.
 */

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents full metadata details for a movie or TV show.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    pub is_series: bool,
    #[allow(dead_code)]
    pub total_seasons: Option<u32>,
    pub poster_bytes: Option<Vec<u8>>,
}



/// Represents a single movie/series search result from the OMDb database.
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
    #[serde(rename = "Type")]
    pub type_field: Option<String>,
    #[serde(rename = "totalSeasons")]
    pub total_seasons: Option<String>,
    #[serde(rename = "Response")]
    pub response: Option<String>,
}

/// Represents a single movie or TV show item returned from TMDB REST API.
#[derive(Deserialize, Debug)]
pub struct TmdbSearchResultItem {
    #[allow(dead_code)]
    pub id: Option<u64>,
    pub title: Option<String>,
    pub name: Option<String>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub vote_average: Option<f64>,
    pub media_type: Option<String>,
}

/// Represents the container object from TMDB search endpoints.
#[derive(Deserialize, Debug)]
pub struct TmdbSearchContainer {
    pub results: Option<Vec<TmdbSearchResultItem>>,
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

/// Parses a release year string (e.g. "1986", "1986–", "2021-05-14") into a 4-digit u32 year.
pub fn parse_year_from_str(s: &str) -> Option<u32> {
    let clean: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if clean.len() >= 4 {
        clean[..4].parse().ok()
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

use std::sync::OnceLock;

fn get_http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    })
}

/// Helper: Constructs a reqwest::Url with query parameter tuples.
pub fn build_api_url(endpoint: &str, params: &[(&str, &str)]) -> Result<reqwest::Url> {
    reqwest::Url::parse_with_params(endpoint, params).context("Failed to parse API URL")
}

/// Represents a structured candidate search result item for UI popup selection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SearchResultItem {
    pub title: String,
    pub year: Option<u32>,
    pub imdb_id: String,
    pub type_field: String,
    pub poster_url: Option<String>,
}

impl SearchResultItem {
    pub fn formatted_label(&self) -> String {
        let yr = self.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        let type_desc = if !self.type_field.is_empty() { format!(" [{}]", self.type_field) } else { String::new() };
        format!("{}{}{}", self.title, yr, type_desc)
    }
}

/// Represents a search result entry from OMDb search endpoint.
#[derive(Deserialize, Debug)]
pub struct OmdbSearchItem {
    #[serde(rename = "Title")]
    pub title: Option<String>,
    #[serde(rename = "Year")]
    pub year: Option<String>,
    #[serde(rename = "imdbID")]
    pub imdb_id: Option<String>,
    #[serde(rename = "Type")]
    pub type_field: Option<String>,
    #[serde(rename = "Poster")]
    pub poster: Option<String>,
}

/// Represents response from OMDb search endpoint (`s=`).
#[derive(Deserialize, Debug)]
pub struct OmdbSearchResponse {
    #[serde(rename = "Search")]
    pub search: Option<Vec<OmdbSearchItem>>,
    #[serde(rename = "Response")]
    pub response: Option<String>,
}

/// Fetches list of candidate search matches for interactive user selection in the UI popup.
pub fn fetch_search_candidates(query: &str) -> Vec<SearchResultItem> {
    let client = get_http_client();
    let parsed_info = crate::utils::parse_season_disc_from_label(query);
    let mut search_terms = Vec::new();

    if !parsed_info.clean_title.is_empty() {
        search_terms.push(normalize_search_title(&parsed_info.clean_title));
    }
    let cleaned_raw = query.replace('_', " ").replace('-', " ").trim().to_lowercase();
    if !cleaned_raw.is_empty() && !search_terms.contains(&cleaned_raw) {
        search_terms.push(cleaned_raw);
    }
    if search_terms.is_empty() {
        search_terms.push(query.to_string());
    }

    let mut results = Vec::new();
    for term in search_terms {
        let encoded = term.replace(' ', "+");
        let url = format!("https://www.omdbapi.com/?s={}&apikey=trilogy", encoded);
        if let Ok(resp) = client.get(&url).send() {
            if let Ok(text) = resp.text() {
                if let Ok(omdb_search) = serde_json::from_str::<OmdbSearchResponse>(&text) {
                    if omdb_search.response.as_deref() == Some("True") {
                        if let Some(items) = omdb_search.search {
                            for item in items {
                                if let (Some(t), Some(id)) = (item.title, item.imdb_id) {
                                    let y = item.year.as_deref().and_then(|yr| yr.get(..4)).and_then(|yr| yr.parse::<u32>().ok());
                                    let type_field = item.type_field.unwrap_or_else(|| "movie".to_string());
                                    let poster_url = item.poster.filter(|p| p != "N/A");
                                    let candidate = SearchResultItem {
                                        title: t,
                                        year: y,
                                        imdb_id: id,
                                        type_field,
                                        poster_url,
                                    };
                                    if !results.contains(&candidate) {
                                        results.push(candidate);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if !results.is_empty() {
            break;
        }
    }
    results
}

/// Queries OMDb API by IMDb ID (`i=tt...`) for exact movie/show metadata.
pub fn lookup_omdb_by_id(imdb_id: &str) -> Option<FilmMetadata> {
    let client = get_http_client();
    let url = format!("https://www.omdbapi.com/?i={}&apikey=trilogy", imdb_id);
    let resp = client.get(&url).send().ok()?;
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

            let is_series = omdb.type_field.as_deref() == Some("series");
            let total_seasons = omdb
                .total_seasons
                .as_deref()
                .and_then(|s| s.parse::<u32>().ok());

            let mut poster_bytes = None;
            if let Some(ref p_url) = poster_url {
                if let Ok(p_resp) = client.get(p_url).send() {
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
                is_series,
                total_seasons,
                poster_bytes,
            });
        }
    }
    None
}

/// Queries OMDb search endpoint (`s=query`) and fetches exact metadata for the top matching IMDb result.
pub fn lookup_omdb_search(query: &str) -> Option<FilmMetadata> {
    let client = get_http_client();
    let encoded_query = query.replace(' ', "+");
    let url = format!("https://www.omdbapi.com/?s={}&apikey=trilogy", encoded_query);
    let resp = client.get(&url).send().ok()?;
    let text = resp.text().ok()?;
    let omdb_search: OmdbSearchResponse = serde_json::from_str(&text).ok()?;

    if omdb_search.response.as_deref() == Some("True") {
        if let Some(items) = omdb_search.search {
            if let Some(first) = items.first() {
                if let Some(ref imdb_id) = first.imdb_id {
                    return lookup_omdb_by_id(imdb_id);
                }
            }
        }
    }
    None
}

/// Queries OMDb API for movie title, release year, running time, plot summary, and poster image.
pub fn lookup_omdb_details(query: &str) -> Option<FilmMetadata> {
    let client = get_http_client();
    let encoded_query = query.replace(' ', "+");
    let url = format!("https://www.omdbapi.com/?t={}&apikey=trilogy", encoded_query);
    let resp = client.get(&url).send().ok()?;
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

            let is_series = omdb.type_field.as_deref() == Some("series");
            let total_seasons = omdb
                .total_seasons
                .as_deref()
                .and_then(|s| s.parse::<u32>().ok());

            let mut poster_bytes = None;
            if let Some(ref p_url) = poster_url {
                if let Ok(p_resp) = client.get(p_url).send() {
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
                is_series,
                total_seasons,
                poster_bytes,
            });
        }
    }

    // If exact title match failed, try search query endpoint (e.g. for volume titles like "kill bill vol1")
    lookup_omdb_search(query)
}

/// Helper: Normalizes a title query for metadata searches (expanding "dr" -> "doctor", "vol1" -> "vol 1", etc.).
pub fn normalize_search_title(title: &str) -> String {
    let mut clean = if title.starts_with("dr ") {
        title.replacen("dr ", "doctor ", 1)
    } else if title.starts_with("dr. ") {
        title.replacen("dr. ", "doctor ", 1)
    } else {
        title.to_string()
    };

    clean = clean.replace("vol1", "vol 1")
                 .replace("vol2", "vol 2")
                 .replace("vol3", "vol 3")
                 .replace("vol4", "vol 4");

    clean
}

/// Queries OMDb or IMDb APIs to resolve a raw DVD volume label or show title to full metadata.
pub fn lookup_film_details(query: &str) -> Result<FilmMetadata> {
    if query.starts_with("disc_") {
        if let Some(cached) = lookup_fingerprint_cache(query) {
            return Ok(cached);
        }
    }

    let parsed_info = crate::utils::parse_season_disc_from_label(query);
    let mut candidates = Vec::new();

    if !parsed_info.clean_title.is_empty() {
        let clean = normalize_search_title(&parsed_info.clean_title);
        candidates.push(clean.clone());

        // Also add candidate stripping trailing volume markers (e.g. "kill bill" from "kill bill vol 1")
        if let Some(root) = clean.split(" vol ").next() {
            let root_str = root.trim().to_string();
            if !root_str.is_empty() && root_str != clean {
                candidates.push(root_str);
            }
        }
    }

    let cleaned_raw: String = query
        .replace('_', " ")
        .replace('-', " ")
        .trim()
        .to_lowercase();

    if cleaned_raw.is_empty() {
        return Err(anyhow!("Cleaned query is empty"));
    }

    let cleaned = normalize_search_title(&cleaned_raw);

    if !candidates.contains(&cleaned) {
        candidates.push(cleaned.clone());
    }

    // Try root candidate stripping "vol"
    if let Some(root) = cleaned.split(" vol").next() {
        let root_str = root.trim().to_string();
        if !root_str.is_empty() && !candidates.contains(&root_str) {
            candidates.push(root_str);
        }
    }

    for cand in &candidates {
        if let Some(tmdb_res) = lookup_tmdb_details(cand) {
            return Ok(tmdb_res);
        }
        if let Some(omdb_res) = lookup_omdb_details(cand) {
            return Ok(omdb_res);
        }
    }

    if let Some(tmdb_res) = lookup_tmdb_details(query) {
        return Ok(tmdb_res);
    }

    if let Some(omdb_res) = lookup_omdb_details(query) {
        return Ok(omdb_res);
    }

    // Fallback to IMDb Suggest API
    let first_char = cleaned.chars().next().ok_or_else(|| anyhow!("Empty query"))?;

    let mut url = reqwest::Url::parse("https://sg.media-imdb.com")?;
    url.set_path(&format!("suggests/{}/{}.json", first_char, cleaned));

    let response_text = get_http_client()
        .get(url)
        .send()
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

/// Queries TMDB API for movie / TV show metadata details.
pub fn lookup_tmdb_details(query: &str) -> Option<FilmMetadata> {
    let client = get_http_client();
    let api_key = "3aec63790d50f3b9fc2efb4c15a8cf99";
    let url = reqwest::Url::parse_with_params(
        "https://api.themoviedb.org/3/search/multi",
        &[("api_key", api_key), ("query", query)],
    ).ok()?;

    let text = client.get(url).send().ok()?.text().ok()?;
    let resp: TmdbSearchContainer = serde_json::from_str(&text).ok()?;
    let item = resp.results?.into_iter().next()?;

    let title = item.title.or(item.name)?;
    let year = item
        .release_date
        .as_deref()
        .or(item.first_air_date.as_deref())
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<u32>().ok());
    let plot = item.overview.filter(|p| !p.is_empty());
    let rating = item.vote_average.map(|v| format!("{:.1}", v));
    let is_series = item.media_type.as_deref() == Some("tv");

    let mut poster_bytes = None;
    let mut poster_url = None;
    if let Some(path) = item.poster_path {
        let full_url = format!("https://image.tmdb.org/t/p/w500{}", path);
        poster_url = Some(full_url.clone());
        if let Ok(p_resp) = client.get(&full_url).send() {
            if let Ok(bytes) = p_resp.bytes() {
                poster_bytes = Some(bytes.to_vec());
            }
        }
    }

    Some(FilmMetadata {
        title,
        year,
        runtime_secs: None,
        poster_url,
        plot,
        genre: None,
        director: None,
        actors: None,
        rating,
        is_series,
        total_seasons: None,
        poster_bytes,
    })
}

/// Queries TMDB/OMDb to fetch specific TV show episode titles (e.g. S01E01 - Pilot).
#[allow(dead_code)]
pub fn fetch_tv_episode_title(show_name: &str, season: u32, episode_num: u32) -> String {
    let clean_show = crate::utils::sanitize_filename(show_name);
    let default_name = crate::utils::format_episode_name(&clean_show, season, episode_num);

    let client = get_http_client();
    let api_key = "3aec63790d50f3b9fc2efb4c15a8cf99";
    if let Ok(url) = reqwest::Url::parse_with_params(
        "https://api.themoviedb.org/3/search/tv",
        &[("api_key", api_key), ("query", &clean_show)],
    ) {
        if let Ok(resp) = client.get(url).send() {
            if let Ok(text) = resp.text() {
                if let Ok(data) = serde_json::from_str::<TmdbSearchContainer>(&text) {
                    if let Some(show) = data.results.and_then(|r| r.into_iter().next()) {
                        if let Some(show_id) = show.id {
                            let ep_url = format!(
                                "https://api.themoviedb.org/3/tv/{}/season/{}/episode/{}",
                                show_id, season, episode_num
                            );
                            if let Ok(ep_url_parsed) = reqwest::Url::parse_with_params(&ep_url, &[("api_key", api_key)]) {
                                if let Ok(ep_resp) = client.get(ep_url_parsed).send() {
                                    if let Ok(ep_text) = ep_resp.text() {
                                        if let Ok(ep_json) = serde_json::from_str::<serde_json::Value>(&ep_text) {
                                            if let Some(ep_name) = ep_json.get("name").and_then(|n| n.as_str()) {
                                                if !ep_name.is_empty() {
                                                    return format!("{} - S{:02}E{:02} - {}", clean_show, season, episode_num, crate::utils::sanitize_filename(ep_name));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    default_name
}

fn resolve_fingerprint_cache_path() -> PathBuf {
    let base = crate::utils::get_app_data_dir().join("fingerprints.json");
    let _ = crate::utils::ensure_parent_dir(&base);
    base
}

/// Looks up cached metadata by disc fingerprint hash string.
pub fn lookup_fingerprint_cache(hash: &str) -> Option<FilmMetadata> {
    let path = resolve_fingerprint_cache_path();
    if !path.exists() {
        return None;
    }
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, FilmMetadata>>(&content) {
            return map.get(hash).cloned();
        }
    }
    None
}

/// Saves disc fingerprint metadata mapping to local JSON cache.
pub fn save_fingerprint_cache(hash: &str, meta: &FilmMetadata) {
    let path = resolve_fingerprint_cache_path();
    let mut map: std::collections::HashMap<String, FilmMetadata> = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    map.insert(hash.to_string(), meta.clone());
    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = std::fs::write(path, json);
    }
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

    #[test]
    fn test_search_result_item_formatted_label() {
        let item = SearchResultItem {
            imdb_id: "tt0090605".to_string(),
            title: "Aliens".to_string(),
            year: Some(1986),
            type_field: "movie".to_string(),
            poster_url: None,
        };
        assert_eq!(item.formatted_label(), "Aliens (1986) [movie]");
    }

    #[test]
    fn test_lookup_dr_who_normalization() {
        let meta = lookup_film_details("dr who").unwrap();
        assert_eq!(meta.title, "Doctor Who");
        assert_eq!(meta.year, Some(2005));
        assert!(meta.is_series);
    }

    #[test]
    fn test_lookup_kill_bill_vol1_volume_label() {
        let meta = lookup_film_details("KILL_BILL_VOL1").unwrap();
        assert!(meta.title.contains("Kill Bill"));
        assert!(meta.year.is_some());
    }

    #[test]
    fn test_fetch_search_candidates_multiple_results() {
        let candidates = fetch_search_candidates("Kill Bill");
        assert!(!candidates.is_empty());
        assert!(candidates.iter().any(|c| c.title.contains("Kill Bill")));
    }

    #[test]
    fn test_fingerprint_cache_persistence() {
        let dummy_hash = "disc_test12345";
        let meta = FilmMetadata {
            title: "Test Cached Movie".to_string(),
            year: Some(2026),
            ..Default::default()
        };
        save_fingerprint_cache(dummy_hash, &meta);
        let cached = lookup_fingerprint_cache(dummy_hash);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().title, "Test Cached Movie");
    }

    #[test]
    fn test_build_api_url() {
        let url = build_api_url("https://api.omdbapi.com", &[("apikey", "demo"), ("t", "Aliens")]).unwrap();
        assert_eq!(url.as_str(), "https://api.omdbapi.com/?apikey=demo&t=Aliens");
    }

    #[test]
    fn test_parse_year_from_str() {
        assert_eq!(parse_year_from_str("1986"), Some(1986));
        assert_eq!(parse_year_from_str("1986–"), Some(1986));
        assert_eq!(parse_year_from_str("2021-05-14"), Some(2021));
        assert_eq!(parse_year_from_str("invalid"), None);
    }
}
