use serde::{Deserialize, Serialize};

/// Type of media represented on disc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    TvSeries,
}

/// Unified Film / TV metadata entity.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FilmMetadata {
    pub title: String,
    pub year: Option<u32>,
    pub runtime_secs: Option<f64>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub poster_bytes: Option<Vec<u8>>,
    pub is_series: bool,
    pub genre: Option<String>,
    pub director: Option<String>,
    pub actors: Option<String>,
    pub rating: Option<String>,
    pub total_seasons: Option<u32>,
}

/// Lightweight candidate result from online search queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchCandidate {
    pub title: String,
    pub year: Option<u32>,
    pub imdb_id: String,
    pub type_field: String,
    pub poster_url: Option<String>,
}

impl SearchCandidate {
    pub fn formatted_label(&self) -> String {
        let yr = self.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        let type_desc = if !self.type_field.is_empty() {
            format!(" [{}]", self.type_field)
        } else {
            String::new()
        };
        format!("{}{}{}", self.title, yr, type_desc)
    }
}

/// Episodic metadata describing a single television show episode.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TvEpisodeMetadata {
    pub season: u32,
    pub episode_num: u32,
    pub title: String,
    pub summary: Option<String>,
    pub runtime_minutes: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_type_serialization() {
        assert_eq!(MediaType::Movie, MediaType::Movie);
        assert_ne!(MediaType::Movie, MediaType::TvSeries);
        let serialized = serde_json::to_string(&MediaType::Movie).unwrap();
        let deserialized: MediaType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, MediaType::Movie);

        let tv_ser = serde_json::to_string(&MediaType::TvSeries).unwrap();
        let tv_de: MediaType = serde_json::from_str(&tv_ser).unwrap();
        assert_eq!(tv_de, MediaType::TvSeries);
    }

    #[test]
    fn test_film_metadata_defaults_and_clone() {
        let mut meta = FilmMetadata::default();
        assert_eq!(meta.title, "");
        assert_eq!(meta.year, None);
        assert!(!meta.is_series);

        meta.title = "The Matrix".to_string();
        meta.year = Some(1999);
        meta.runtime_secs = Some(8160.0);
        meta.plot = Some("A computer hacker learns about the true nature of reality.".to_string());
        meta.poster_url = Some("https://example.com/matrix.jpg".to_string());
        meta.poster_bytes = Some(vec![1, 2, 3]);
        meta.is_series = false;
        meta.genre = Some("Sci-Fi".to_string());
        meta.director = Some("Lana Wachowski, Lilly Wachowski".to_string());
        meta.actors = Some("Keanu Reeves, Laurence Fishburne".to_string());
        meta.rating = Some("8.7".to_string());
        meta.total_seasons = None;

        let cloned = meta.clone();
        assert_eq!(meta, cloned);

        let json = serde_json::to_string(&meta).unwrap();
        let de: FilmMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, de);
    }

    #[test]
    fn test_search_candidate_formatted_label() {
        let c1 = SearchCandidate {
            title: "Inception".to_string(),
            year: Some(2010),
            imdb_id: "tt1375666".to_string(),
            type_field: "movie".to_string(),
            poster_url: Some("http://img.png".to_string()),
        };
        assert_eq!(c1.formatted_label(), "Inception (2010) [movie]");

        let c2 = SearchCandidate {
            title: "Unknown Show".to_string(),
            year: None,
            imdb_id: "tt9999999".to_string(),
            type_field: String::new(),
            poster_url: None,
        };
        assert_eq!(c2.formatted_label(), "Unknown Show");

        let c3 = SearchCandidate {
            title: "Breaking Bad".to_string(),
            year: Some(2008),
            imdb_id: "tt0903747".to_string(),
            type_field: "series".to_string(),
            poster_url: None,
        };
        assert_eq!(c3.formatted_label(), "Breaking Bad (2008) [series]");

        let json = serde_json::to_string(&c1).unwrap();
        let de: SearchCandidate = serde_json::from_str(&json).unwrap();
        assert_eq!(c1, de);
    }

    #[test]
    fn test_tv_episode_metadata() {
        let ep = TvEpisodeMetadata {
            season: 2,
            episode_num: 5,
            title: "Test Episode".to_string(),
            summary: Some("Episode summary".to_string()),
            runtime_minutes: Some(45),
        };
        let cloned = ep.clone();
        assert_eq!(ep, cloned);
        assert_eq!(format!("{:?}", ep), format!("{:?}", cloned));

        let json = serde_json::to_string(&ep).unwrap();
        let de: TvEpisodeMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(ep, de);
    }
}
