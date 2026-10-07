/**
 * @file tvmaze.rs
 * @brief TVMaze REST API client for fetching TV series episode names and metadata.
 */

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Represents a TV episode returned from TVMaze.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TvMazeEpisode {
    pub id: u64,
    pub name: String,
    pub season: u32,
    pub number: Option<u32>,
    pub runtime: Option<u32>,
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TvMazeEmbedded {
    episodes: Vec<TvMazeEpisode>,
}

#[derive(Debug, Deserialize)]
struct TvMazeShowResponse {
    pub id: u64,
    pub name: String,
    #[serde(rename = "_embedded")]
    pub embedded: Option<TvMazeEmbedded>,
}

/// Parses TVMaze JSON response into a list of TV episodes.
pub fn parse_tvmaze_response(json_str: &str) -> Result<Vec<TvMazeEpisode>> {
    let show: TvMazeShowResponse = serde_json::from_str(json_str)
        .context("Failed to parse TVMaze show response")?;
    Ok(show.embedded.map(|e| e.episodes).unwrap_or_default())
}

/// Fetches all episodes for a show from TVMaze using the public singlesearch API.
pub fn fetch_tvmaze_episodes(show_name: &str) -> Result<Vec<TvMazeEpisode>> {
    let clean_query = show_name.trim();
    if clean_query.is_empty() {
        return Ok(Vec::new());
    }

    let url = format!(
        "https://api.tvmaze.com/singlesearch/shows?q={}&embed=episodes",
        crate::utils::url_query_escape(clean_query)
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let response = client
        .get(&url)
        .header("User-Agent", "dvd-ripper/0.2.0")
        .send()?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let text = response.text()?;
    parse_tvmaze_response(&text)
}

/// Finds the episode title for a specific season and episode number.
pub fn find_episode_title(episodes: &[TvMazeEpisode], season: u32, episode_num: u32) -> Option<String> {
    episodes
        .iter()
        .find(|ep| ep.season == season && ep.number == Some(episode_num))
        .map(|ep| ep.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tvmaze_response_valid() {
        let sample_json = r#"{
            "id": 526,
            "name": "The Office",
            "_embedded": {
                "episodes": [
                    {
                        "id": 49788,
                        "name": "Pilot",
                        "season": 1,
                        "number": 1,
                        "runtime": 30,
                        "summary": "<p>A documentary crew arrives at Dunder Mifflin.</p>"
                    },
                    {
                        "id": 49789,
                        "name": "Diversity Day",
                        "season": 1,
                        "number": 2,
                        "runtime": 30,
                        "summary": "<p>Michael holds a diversity seminar.</p>"
                    }
                ]
            }
        }"#;

        let episodes = parse_tvmaze_response(sample_json).unwrap();
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].name, "Pilot");
        assert_eq!(episodes[0].season, 1);
        assert_eq!(episodes[0].number, Some(1));

        let title_e1 = find_episode_title(&episodes, 1, 1);
        assert_eq!(title_e1.as_deref(), Some("Pilot"));

        let title_e2 = find_episode_title(&episodes, 1, 2);
        assert_eq!(title_e2.as_deref(), Some("Diversity Day"));

        let title_e3 = find_episode_title(&episodes, 1, 3);
        assert_eq!(title_e3, None);
    }

    #[test]
    fn test_parse_tvmaze_response_no_episodes() {
        let sample_json = r#"{"id": 999, "name": "Empty Show"}"#;
        let episodes = parse_tvmaze_response(sample_json).unwrap();
        assert!(episodes.is_empty());
    }
}
