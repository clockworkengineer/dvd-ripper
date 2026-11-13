# Media Organization & Output Directory Specification - `dvd-ripper`

> **Document Version**: 1.1.0  
> **Applies to**: `dvd-ripper` v0.4.0+  
> **Target Standard**: Plex Media Server, Jellyfin, Emby, & Kodi Compliance  

---

## 1. Overview

`dvd-ripper` organizes ripped optical media into clean, standardized directory structures designed for 100% direct compatibility with modern home media servers (**Plex**, **Jellyfin**, **Emby**, and **Kodi**).

Media is systematically separated into two root categories:
- **Feature Films / Movies**: Formatted as `<out_dir>/<films_dir>/<Movie Title> (<Year>)/<Movie Title> (<Year>).<ext>`
- **Television Series**: Formatted as `<out_dir>/<tv_dir>/<Show Name> (<Year>)/Season <SS>/<Show Name> - S<SS>E<EE> - <Title>.<ext>`

Both subfolder names (`films_dir` and `tv_dir`) default to `"Films"` and `"TV"` respectively, and can be customized via CLI parameters (`--films-dir`, `--tv-dir`), `config.toml`, or the desktop GUI Settings tab.

---

## 2. Standard Directory Layouts

### 2.1 Feature Films (`<films_dir>`)

Each movie is placed in its own dedicated subfolder bearing the sanitized movie title and 4-digit release year:

```text
<out_dir>/
└── <films_dir>/                              # Default: "Films" (configurable: "Movies", "Cinema")
    └── <Movie Title> (<Year>)/
        ├── <Movie Title> (<Year>).mp4          # Video stream (or .mkv / .iso)
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

### 2.2 Television Series (`<tv_dir>`)

Multi-episode discs and TV box sets follow the standard two-tier show and season hierarchy:

```text
<out_dir>/
└── <tv_dir>/                                 # Default: "TV" (configurable: "Series", "TV Shows")
    └── <Show Name> (<Year>)/
        ├── tvshow.nfo                          # Series-level metadata sidecar
        ├── poster.jpg                          # Series poster artwork
        └── Season <SS>/
            ├── <Show Name> - S<SS>E01 - Pilot.mp4  # Episode 1 with TVMaze title
            ├── <Show Name> - S<SS>E01.nfo          # Episode 1 metadata sidecar
            ├── <Show Name> - S<SS>E01.en.srt       # Episode 1 subtitle sidecar
            ├── <Show Name> - S<SS>E02.mp4          # Episode 2
            └── <Show Name> - S<SS>E03.mp4          # Episode 3
```

---

## 3. Configurable Subfolder Names (`--films-dir` & `--tv-dir`)

Users can customize the subfolder names to match existing NAS/homelab folder taxonomies:

### 3.1 CLI Options
```bash
# Saves films to C:\Media\Movies and TV series to C:\Media\Series
dvd-ripper -i D:\ -d "C:\Media" --films-dir "Movies" --tv-dir "Series"
```

### 3.2 Configuration File (`~/.dvd-ripper/config.toml`)
```toml
out_dir = "C:\\Media"
films_dir = "Movies"
tv_dir = "Series"
```

### 3.3 GUI Configuration
In the **⚙️ Settings & Preferences** tab under **📁 Storage & Paths**, adjust:
- **Films Subfolder Name**: e.g. `Movies`
- **TV Subfolder Name**: e.g. `Series`

The active path routing is immediately reflected live on the **🎬 Disc Extraction** tab:
`Rips will be saved to: C:\Media\Movies (Movies) or C:\Media\Series (Series)`.

---

## 4. Intelligent Sibling Directory Resolution

To prevent confusing directory structures (such as `Films/Films/...` or `Movies/Series/...`), `dvd-ripper` implements automated sibling directory resolution ([`src/domain/naming.rs`](../src/domain/naming.rs)):

| User Configured `out_dir` | `films_dir` | `tv_dir` | Target | Resolved Output Path | Behavior / Rationale |
|---|---|---|---|---|---|
| `D:\Media` | `Films` | `TV` | Film | `D:\Media\Films\<Title> (<Year>)\...` | Base folder; appends `Films/` |
| `D:\Media` | `Films` | `TV` | TV | `D:\Media\TV\<Show> (<Year>)\Season SS\...` | Base folder; appends `TV/` |
| `D:\Media` | `Movies` | `Series` | Film | `D:\Media\Movies\<Title> (<Year>)\...` | Base folder; appends custom `Movies/` |
| `D:\Media` | `Movies` | `Series` | TV | `D:\Media\Series\<Show> (<Year>)\Season SS\...` | Base folder; appends custom `Series/` |
| `D:\Media\Films` | `Films` | `TV` | Film | `D:\Media\Films\<Title> (<Year>)\...` | Preserves existing `Films` folder without duplicate `Films/Films` |
| `D:\Media\Films` | `Films` | `TV` | TV | `D:\Media\TV\<Show> (<Year>)\Season SS\...` | **Intelligently routes to sibling `TV` folder** |
| `D:\Media\Movies` | `Movies` | `Series` | TV | `D:\Media\Series\<Show> (<Year>)\Season SS\...` | **Intelligently routes to sibling `Series` folder** |
| `D:\Media\TV` | `Films` | `TV` | Film | `D:\Media\Films\<Title> (<Year>)\...` | **Intelligently routes to sibling `Films` folder** |
| `D:\Media\Series` | `Movies` | `Series` | Film | `D:\Media\Movies\<Title> (<Year>)\...` | **Intelligently routes to sibling `Movies` folder** |
| `.` (or empty) | `Films` | `TV` | Film | `Films\<Title> (<Year>)\...` | Relative path inside current working directory |

---

## 5. Default OS Directory Locations

When launching without custom directory configuration, `dvd-ripper` selects a sensible, user-accessible storage folder:
- **Windows**: `%USERPROFILE%\Videos\DVD Rips` (e.g. `C:\Users\JohnDoe\Videos\DVD Rips`)
- **Linux**: `$HOME/Videos/DVD Rips` (or `$HOME/Movies/DVD Rips` if Videos is absent)
- **macOS**: `$HOME/Movies/DVD Rips`

In the Desktop GUI:
- Click **`📂 Browse...`** to pick any folder using the native OS file picker.
- Click **`↗ Open`** to open the active destination in Windows File Explorer or Linux file manager.
- Click **`↺ Reset`** to revert to the default OS Videos directory.

---

## 6. Metadata & Sidecar Files

### 6.1 NFO Metadata Files (`.nfo`)
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

### 6.2 Artwork Files
High-resolution poster thumbnails fetched from online APIs are written to `poster.jpg` and `folder.jpg`, allowing instant visual indexing on network shares and DLNA servers.

### 6.3 Subtitle Sidecars (`.srt`)
When `--sub-external-srt` or `--ocr` is enabled, subtitle streams are converted to SubRip text sidecars (`<Title>.<lang>.srt`). This ensures universal playback on mobile and smart TV clients without requiring CPU-intensive video transcoding on the media server.

---

## 7. Automated Media Server Library Refresh Triggers

Upon rip completion, `dvd-ripper` can trigger library scans via HTTP API:
- **Plex**: `http://<PLEX_IP>:32400/library/sections/all/refresh?X-Plex-Token=<TOKEN>`
- **Jellyfin**: `http://<JELLYFIN_IP>:8096/Library/Refresh?api_key=<KEY>`
- **Emby**: `http://<EMBY_IP>:8096/emby/Library/Refresh?api_key=<KEY>`

Configure these endpoints in your `dvd-ripper.toml` or via CLI flags:
```bash
dvd-ripper --plex-url "http://192.168.1.100:32400" --plex-token "your_token"
```

For complete technical specifications on custom templates and sanitization rules, see [`docs/CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md`](CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md).
