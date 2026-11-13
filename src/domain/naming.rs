//! Pure domain naming conventions for Plex/Jellyfin/Emby media library organization (SRP).

use crate::domain::entities::options::OutputNamingSpec;
use std::path::{Path, PathBuf};

/// Formats a sanitized media folder/file stem `<Title> (<Year>)` or `<Title>`.
pub fn format_media_stem(title: Option<&str>, year: Option<u32>, fallback: &str) -> String {
    let raw = title
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback);
    let clean = crate::utils::sanitize_filename(raw);
    let base = if clean.is_empty() {
        fallback.to_string()
    } else {
        clean
    };
    match year {
        Some(y) if y >= 1888 && !base.contains(&format!("({})", y)) => format!("{} ({})", base, y),
        _ => base,
    }
}

/// Resolves the output file extension (`iso`, `mkv`, or `mp4`) from a segregated [`OutputNamingSpec`].
pub fn resolve_container_extension(spec: &OutputNamingSpec) -> &'static str {
    if spec.iso {
        "iso"
    } else if spec.mkv {
        "mkv"
    } else {
        "mp4"
    }
}

/// Helper: Checks if the path component is named "films" (case-insensitive).
pub fn is_named_films(path: &Path) -> bool {
    path.file_name()
        .map(|s| s.to_string_lossy().eq_ignore_ascii_case("films"))
        .unwrap_or(false)
}

/// Helper: Checks if the path component is named "tv" or "tv shows" (case-insensitive).
pub fn is_named_tv(path: &Path) -> bool {
    path.file_name()
        .map(|s| {
            let lossy = s.to_string_lossy();
            lossy.eq_ignore_ascii_case("tv") || lossy.eq_ignore_ascii_case("tv shows")
        })
        .unwrap_or(false)
}

/// Resolves the root destination directory for Film/Movie rips.
/// Defaults to `<out_dir>/Films`. If `out_dir` already ends in `Films`, it is kept as-is.
/// If `out_dir` ends in `TV`, its parent sibling `/Films` is used.
pub fn resolve_films_root(out_dir: &str) -> PathBuf {
    let trimmed = out_dir.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return PathBuf::from("Films");
    }
    let p = Path::new(trimmed);
    if is_named_films(p) {
        p.to_path_buf()
    } else if is_named_tv(p) {
        match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("Films"),
            _ => PathBuf::from("Films"),
        }
    } else {
        p.join("Films")
    }
}

/// Resolves the root destination directory for TV series rips.
/// Defaults to `<out_dir>/TV`. If `out_dir` already ends in `TV`, it is kept as-is.
/// If `out_dir` ends in `Films`, its parent sibling `/TV` is used.
pub fn resolve_tv_root(out_dir: &str) -> PathBuf {
    let trimmed = out_dir.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return PathBuf::from("TV");
    }
    let p = Path::new(trimmed);
    if is_named_tv(p) {
        p.to_path_buf()
    } else if is_named_films(p) {
        match p.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.join("TV"),
            _ => PathBuf::from("TV"),
        }
    } else {
        p.join("TV")
    }
}

/// Resolves a movie output path purely from [`OutputNamingSpec`], title, and optional release year.
/// Always places film rips into the `/Films` subfolder of the configured base directory:
/// `<base_folder>/Films/<Title (Year)>/<Title (Year)>.<ext>`.
pub fn resolve_movie_output_path(
    spec: &OutputNamingSpec,
    title: Option<&str>,
    year: Option<u32>,
) -> PathBuf {
    if let Some(ref explicit) = spec.explicit_output
        && !explicit.trim().is_empty()
    {
        return PathBuf::from(explicit.trim());
    }
    let ext = resolve_container_extension(spec);
    let stem = format_media_stem(title, year, "DVD_Rip");
    let films_root = resolve_films_root(&spec.out_dir);
    films_root.join(&stem).join(format!("{}.{}", stem, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_naming_conventions() {
        let spec = OutputNamingSpec {
            out_dir: "/movies".to_string(),
            explicit_output: None,
            mkv: true,
            iso: false,
            no_overwrite: false,
        };
        let p = resolve_movie_output_path(&spec, Some("The Matrix"), Some(1999));
        assert!(p.to_string_lossy().ends_with("The Matrix (1999).mkv"));
        assert!(p.to_string_lossy().contains("Films"));
    }

    #[test]
    fn test_domain_naming_films_root_handling() {
        let base = PathBuf::from("C:\\Media\\DVD Rips");
        assert_eq!(
            resolve_films_root(&base.to_string_lossy()),
            base.join("Films")
        );
        let base_films = base.join("Films");
        assert_eq!(
            resolve_films_root(&base_films.to_string_lossy()),
            base.join("Films")
        );
        let base_tv = base.join("TV");
        assert_eq!(
            resolve_films_root(&base_tv.to_string_lossy()),
            base.join("Films")
        );
    }
}
