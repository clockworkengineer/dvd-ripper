# Media Organization & Output Directory Specification - `dvd-ripper`

> **Document Version**: 1.0.0  
> **Target Standard**: Plex Media Server, Jellyfin, Emby, & Kodi Compliance

---

## 1. Overview

`dvd-ripper` organizes ripped optical media into clean, standardized directory structures designed for 100% direct compatibility with modern home media servers (**Plex**, **Jellyfin**, **Emby**, and **Kodi**).

Media is systematically separated into two root categories:
- **`Films/`** for standalone theatrical and direct-to-video films.
- **`TV/`** for episodic television series, mini-series, and anime.

---

## 2. Standard Directory Layouts

### 2.1 Feature Films (`/Films`)

Each movie is placed in its own dedicated subfolder bearing the sanitized movie title and 4-digit release year:

```text
<out_dir>/
└── Films/
    └── <Movie Title> (<Year>)/
        ├── <Movie Title> (<Year>).mp4          # Video stream (or .mkv / .mpg)
        ├── <Movie Title> (<Year>).nfo          # Kodi/Jellyfin XML metadata sidecar
        ├── <Movie Title> (<Year>).en.srt       # Optional OCR text subtitle sidecar
        ├── poster.jpg                          # High-resolution poster artwork
        └── fanart.jpg                          # Background fanart (if available)
```

#### Real-World Example:
```text
D:\Media\Films\Kill Bill Vol. 1 (2003)\Kill Bill Vol. 1 (2003).mp4
D:\Media\Films\Kill Bill Vol. 1 (2003)\Kill Bill Vol. 1 (2003).nfo
D:\Media\Films\Kill Bill Vol. 1 (2003)\Kill Bill Vol. 1 (2003).en.srt
D:\Media\Films\Kill Bill Vol. 1 (2003)\poster.jpg
```

---

### 2.2 Television Series (`/TV`)

Multi-episode discs and TV box sets follow the standard two-tier show and season hierarchy:

```text
<out_dir>/
└── TV/
    └── <Show Name> (<Year>)/
        ├── tvshow.nfo                          # Series-level metadata sidecar
        ├── poster.jpg                          # Series poster artwork
        └── Season <SS>/
            ├── <Show Name> - S<SS>E01.mp4      # Episode 1
            ├── <Show Name> - S<SS>E01.nfo      # Episode 1 metadata sidecar
            ├── <Show Name> - S<SS>E01.en.srt   # Episode 1 subtitle sidecar
            ├── <Show Name> - S<SS>E02.mp4      # Episode 2
            └── <Show Name> - S<SS>E03.mp4      # Episode 3
```

#### Real-World Example:
```text
D:\Media\TV\The Office (2005)\Season 01\The Office - S01E01.mp4
D:\Media\TV\The Office (2005)\Season 01\The Office - S01E01.nfo
D:\Media\TV\The Office (2005)\Season 01\The Office - S01E02.mp4
```

---

## 3. Intelligent Sibling Directory Resolution

To prevent confusing directory structures (such as `Films/Films/...` or `Films/TV/...`), `dvd-ripper` features built-in sibling directory resolution:

| User Configured `out_dir` | Ripping Target | Resolved Output Path | Behavior / Rationale |
|---|---|---|---|
| `D:\Media` | Film | `D:\Media\Films\<Title> (<Year>)\...` | Automatically appends `Films/` |
| `D:\Media` | TV Series | `D:\Media\TV\<Show> (<Year>)\Season SS\...` | Automatically appends `TV/` |
| `D:\Media\Films` | Film | `D:\Media\Films\<Title> (<Year>)\...` | Preserves existing `Films` folder without duplicate `Films/Films` |
| `D:\Media\Films` | TV Series | `D:\Media\TV\<Show> (<Year>)\Season SS\...` | **Intelligently routes to sibling `TV` folder** |
| `D:\Media\TV` | TV Series | `D:\Media\TV\<Show> (<Year>)\Season SS\...` | Preserves existing `TV` folder without duplicate `TV/TV` |
| `D:\Media\TV` | Film | `D:\Media\Films\<Title> (<Year>)\...` | **Intelligently routes to sibling `Films` folder** |
| `.` (or empty) | Film | `Films\<Title> (<Year>)\...` | Relative path inside current working directory |
| `.` (or empty) | TV Series | `TV\<Show> (<Year>)\...` | Relative path inside current working directory |

---

## 4. Default OS Directory Locations

When launching without custom directory configuration, `dvd-ripper` selects a sensible, user-accessible storage folder:

- **Windows**: `%USERPROFILE%\Videos\DVD Rips` (e.g. `C:\Users\JohnDoe\Videos\DVD Rips`)
- **Linux**: `$HOME/Videos/DVD Rips` (or `$HOME/Movies/DVD Rips` if Videos is absent)
- **macOS**: `$HOME/Movies/DVD Rips`

In the Desktop GUI, you can instantly:
- Click **`📂 Browse...`** to pick any folder using the native OS file picker.
- Click **`↗ Open`** to open the active destination in Windows File Explorer or Linux file manager.
- Click **`↺ Reset`** to revert to the default OS Videos directory.

---

## 5. Metadata & Sidecar Files

### 5.1 NFO Metadata Files (`.nfo`)
When online metadata is fetched (via OMDb / TMDb), an XML `.nfo` file is written alongside the media file:

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<movie>
  <title>Kill Bill: Vol. 1</title>
  <originaltitle>Kill Bill: Vol. 1</originaltitle>
  <year>2003</year>
  <plot>After awakening from a four-year coma, a former assassin wreaks vengeance...</plot>
  <director>Quentin Tarantino</director>
  <rating>8.2</rating>
  <id>tt0266697</id>
</movie>
```

### 5.2 Artwork Files
High-resolution poster thumbnails fetched from online APIs are written to `poster.jpg` and `folder.jpg`, allowing instant visual indexing on network shares and DLNA servers.

### 5.3 Subtitle Sidecars (`.srt`)
When `--sub-external-srt` or `--ocr` is enabled, subtitle streams are converted to SubRip text sidecars (`<Title>.<lang>.srt`). This ensures universal playback on mobile and smart TV clients without requiring CPU-intensive video transcoding on the media server.

---

## 6. Automated Media Server Library Refresh Triggers

Upon rip completion, `dvd-ripper` can trigger library scans via HTTP API:

- **Plex**: `http://<PLEX_IP>:32400/library/sections/all/refresh?X-Plex-Token=<TOKEN>`
- **Jellyfin**: `http://<JELLYFIN_IP>:8096/Library/Refresh?api_key=<KEY>`
- **Emby**: `http://<EMBY_IP>:8096/emby/Library/Refresh?api_key=<KEY>`

Configure these endpoints in your `dvd-ripper.toml` or via CLI flags:

```bash
dvd-ripper --plex-url "http://192.168.1.100:32400" --plex-token "your_token"
```
