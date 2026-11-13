use crate::domain::entities::media::FilmMetadata;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Escapes XML special characters for safety in NFO documents.
pub fn quick_xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Resolves sidecar `.nfo` metadata file path from video output path.
pub fn resolve_nfo_path(video_path: &Path) -> PathBuf {
    let parent = video_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    parent.join(format!("{}.nfo", stem))
}

/// Generates a Kodi/Jellyfin/Plex compatible XML `.nfo` sidecar file from fine-grained metadata fields.
#[allow(clippy::too_many_arguments)]
pub fn generate_nfo_file_detailed(
    output_file: &Path,
    title: &str,
    year: Option<u32>,
    plot: Option<&str>,
    rating: Option<&str>,
    director: Option<&str>,
    media_type: &str,
    tags: Option<&str>,
) -> Result<()> {
    if let Some(parent) = output_file.parent() {
        std::fs::create_dir_all(parent)?;
        let nfo_path = resolve_nfo_path(output_file);

        let tag = if media_type.to_lowercase().contains("tv") {
            "tvshow"
        } else {
            "movie"
        };
        let year_str = year
            .map(|y| format!("  <year>{}</year>\n", y))
            .unwrap_or_default();
        let plot_str = plot
            .map(|p| format!("  <plot>{}</plot>\n", quick_xml_escape(p)))
            .unwrap_or_default();
        let rating_str = rating
            .map(|r| format!("  <rating>{}</rating>\n", quick_xml_escape(r)))
            .unwrap_or_default();
        let director_str = director
            .map(|d| format!("  <director>{}</director>\n", quick_xml_escape(d)))
            .unwrap_or_default();

        let tags_str = if let Some(tags_csv) = tags {
            let mut buf = String::new();
            for t in tags_csv.split(',') {
                let clean_t = t.trim();
                if !clean_t.is_empty() {
                    buf.push_str(&format!("  <tag>{}</tag>\n", quick_xml_escape(clean_t)));
                }
            }
            buf
        } else {
            String::new()
        };

        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <{tag}>\n\
             \x20\x20<title>{}</title>\n\
             {}{}{}{}{}\
             </{tag}>\n",
            quick_xml_escape(title),
            year_str,
            plot_str,
            rating_str,
            director_str,
            tags_str,
            tag = tag
        );

        std::fs::write(&nfo_path, content)?;
        println!(
            "[NFO Metadata] Generated sidecar XML: {}",
            nfo_path.display()
        );
    }
    Ok(())
}

/// Generates a Kodi/Jellyfin/Plex compatible XML `.nfo` sidecar file alongside the transcoded video from FilmMetadata.
pub fn generate_nfo_file(
    output_file: &Path,
    metadata: &FilmMetadata,
    media_type: &str,
) -> Result<()> {
    generate_nfo_file_detailed(
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
        assert_eq!(resolve_nfo_path(p3), PathBuf::from(".\\output.nfo"));
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
