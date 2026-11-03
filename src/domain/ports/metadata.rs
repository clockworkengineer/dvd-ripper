use crate::domain::entities::media::{FilmMetadata, SearchCandidate, TvEpisodeMetadata};
use anyhow::Result;

/// Abstraction for online and offline film/TV metadata lookups.
pub trait MetadataProvider: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<SearchCandidate>>;
    fn get_details_by_id(&self, id: &str) -> Result<Option<FilmMetadata>>;
    fn get_details_by_title_and_year(
        &self,
        title: &str,
        year: Option<u32>,
    ) -> Result<Option<FilmMetadata>>;
}

/// Interface Segregation (ISP/SOLID): Abstraction dedicated to TV series episodic catalog queries.
pub trait TvSeriesMetadataProvider: Send + Sync {
    /// Fetches all episodes cataloged for the given television show name.
    fn fetch_episodes(&self, series_name: &str) -> Result<Vec<TvEpisodeMetadata>>;

    /// Finds the episode title for a specific season and episode number.
    fn find_episode_title(
        &self,
        series_name: &str,
        season: u32,
        episode_num: u32,
    ) -> Result<Option<String>> {
        let episodes = self.fetch_episodes(series_name)?;
        Ok(episodes
            .into_iter()
            .find(|ep| ep.season == season && ep.episode_num == episode_num)
            .map(|ep| ep.title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTvSeriesMetadataProvider {
        episodes: Vec<TvEpisodeMetadata>,
    }

    impl TvSeriesMetadataProvider for MockTvSeriesMetadataProvider {
        fn fetch_episodes(&self, _series_name: &str) -> Result<Vec<TvEpisodeMetadata>> {
            Ok(self.episodes.clone())
        }
    }

    #[test]
    fn test_find_episode_title_match() {
        let provider = MockTvSeriesMetadataProvider {
            episodes: vec![
                TvEpisodeMetadata {
                    season: 1,
                    episode_num: 1,
                    title: "Pilot".to_string(),
                    summary: None,
                    runtime_minutes: Some(22),
                },
                TvEpisodeMetadata {
                    season: 1,
                    episode_num: 2,
                    title: "Diversity Day".to_string(),
                    summary: None,
                    runtime_minutes: Some(22),
                },
            ],
        };

        assert_eq!(
            provider.find_episode_title("The Office", 1, 2).unwrap(),
            Some("Diversity Day".to_string())
        );
        assert_eq!(
            provider.find_episode_title("The Office", 1, 3).unwrap(),
            None
        );
        assert_eq!(
            provider.find_episode_title("The Office", 2, 1).unwrap(),
            None
        );
    }
}
