use crate::domain::entities::media::FilmMetadata;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Resolves sidecar `.nfo` metadata file path from video output path.
pub fn resolve_nfo_path(video_path: &Path) -> PathBuf {
    let parent = video_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = video_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    parent.join(format!("{}.nfo", stem))
}

/// Generates a Kodi/Jellyfin/Plex compatible XML `.nfo` sidecar file alongside the transcoded video.
pub fn generate_nfo_file(
    output_file: &Path,
    metadata: &FilmMetadata,
    media_type: &str,
) -> Result<()> {
    crate::utils::generate_nfo_file(
        output_file,
        &metadata.title,
        metadata.year,
        metadata.plot.as_deref(),
        metadata.rating.as_deref(),
        metadata.director.as_deref(),
        media_type,
        metadata.genre.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_nfo_path() {
        let p1 = Path::new("C:\\Videos\\Movie.mp4");
        assert_eq!(resolve_nfo_path(p1), PathBuf::from("C:\\Videos\\Movie.nfo"));

        let p2 = Path::new("Show.mkv");
        assert_eq!(resolve_nfo_path(p2), PathBuf::from("Show.nfo"));

        let p3 = Path::new("");
        assert_eq!(resolve_nfo_path(p3), PathBuf::from("output.nfo"));
    }

    #[test]
    fn test_generate_nfo_file_integration() {
        let temp = std::env::temp_dir().join(format!("nfo_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        let out_video = temp.join("TestFilm.mp4");

        let meta = FilmMetadata {
            title: "Test Film".to_string(),
            year: Some(2022),
            plot: Some("Great film".to_string()),
            rating: Some("8.5".to_string()),
            director: Some("Director Name".to_string()),
            genre: Some("Drama".to_string()),
            ..Default::default()
        };

        let res = generate_nfo_file(&out_video, &meta, "Movie");
        assert!(res.is_ok());

        let nfo = temp.join("TestFilm.nfo");
        assert!(nfo.exists());
        let content = std::fs::read_to_string(&nfo).unwrap();
        assert!(content.contains("<title>Test Film</title>"));
        assert!(content.contains("<year>2022</year>"));

        let _ = std::fs::remove_dir_all(&temp);
    }
}
