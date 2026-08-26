# Desktop Graphical User Interface (GUI) Guide

`dvd-ripper` features a portable immediate-mode Desktop GUI powered by [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) and [`egui`](https://github.com/emilk/egui). It provides a responsive interface with zero C++ runtime or WebKit dependencies, compiling directly into a single standalone executable.

---

## 1. Launching the Desktop GUI

By default, launching `dvd-ripper` without command-line flags opens the Graphical User Interface:

```bash
# Launch Desktop GUI directly
dvd-ripper
```

If launching from a terminal or script where command-line flags might be present, you can launch without flags or build with `--features gui` (enabled by default).

---

## 2. Interface Overview & Tab Navigation

The desktop interface is organized into **6 core workflow sections and tab views**:

```text
┌────────────────────────────────────────────────────────────────────────┐
│ 📀 DVD Ripper Desktop                                        [ℹ️ About]│
├────────────────────────────────────────────────────────────────────────┤
│ [Main Rip Tab] [Box Set Manager] [Benchmark] [History Log] [Settings]  │
├────────────────────────────────────────────────────────────────────────┤
│ 1. Drive & Input:  Drive: [ auto ▾ ]   Out Dir: [ Films ]             │
│ 2. Metadata:      Title: [ Kill Bill ] Year: [ 2003 ] [🔍 Search OMDb] │
│ 3. Transcode:     [✓] Transcode   Codec: [ H.264 ▾ ]  Profile: [ Standard ▾ ]
│ 4. Audio & Subs:  [✓] All Audio   [✓] Subtitles    Format: [ dvdsub ▾ ] │
│ 5. Controls:      [🔍 Scan Disc]  [▶ Start Rip]    [⏹ Cancel]           │
├────────────────────────────────────────────────────────────────────────┤
│ Progress: [=====================>                  ] 45.2% (28.5 fps) │
│ Console:  [16:04:12] [INFO] FFmpeg demuxer initialized...              │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2.1 ℹ️ About Box Modal

Clicking the **`ℹ️ About`** button in the top-right header opens the application info dialog displaying:
- Application version (`v0.1.2`) and description.
- Dual open-source licenses (`MIT OR Apache-2.0`).
- Direct hyperlink to the **[GitHub Repository](https://github.com/roberttizz1/dvd-ripper)**.
- Direct hyperlink to **[Buy Me a Coffee](https://buymeacoffee.com/roberttizz1)** support page.
- Configured system FFmpeg executable path.

---

## 3. Main Rip Panel Setup

### 3.1 Input Drive & Output Directory Selection
- **Input Drive**: Dropdown menu allowing selection of `auto` drive detection, explicit optical drive letters (`D:\`, `E:\`), or Linux block devices (`/dev/sr0`).
- **Base Output Directory (`out_dir`)**: Specify destination folder root (defaults to `Films` for movies or `TV` for series).

### 3.2 Movie Metadata & Candidate Search Modal
- **Movie Title & Release Year**: Enter the title name and release year for Plex/Jellyfin naming compatibility.
- **🔍 Search OMDb / IMDb Candidate Modal**: Click **Search OMDb** to open the interactive search candidate popup.
  - Queries OMDb and IMDb APIs.
  - Displays movie posters, release years, IMDb IDs (`tt0266697`), and plot summaries.
  - Clicking **Select** populates the title, year, genre, director, cast, plot summary, and fetches high-resolution poster artwork.

### 3.3 Transcoding & Video Filter Quality Suite
- **Transcode Toggle (`--transcode`)**: Enable to re-encode video streams using `libx264`, `libx265`, or `libsvtav1`. Leave unchecked for fast, lossless remuxing (`-c copy`).
- **MKV Container (`--mkv`)**: Force output into a Matroska (`.mkv`) container format instead of MP4 (`.mp4`).
- **Video Codec Selector**: Choose between `h264`, `hevc`, `av1`, or `copy`.
- **Profile Presets**: Choose preset profiles (`standard`, `archival`, `plex`, `mobile`).
- **Deinterlacing Suite**: Enable motion-adaptive deinterlacing (`--deinterlace`) and select algorithms (`bwdif`, `yadif`, `w3fdif`) to remove comb artifacts from 480i/576i streams.
- **3D Denoising Filter**: Enable spatial/temporal noise reduction (`--denoise hqdn3d`) to clean up analog tape or film grain noise.

### 3.4 Audio & Subtitle Stream Selection
- **Include All Audio Tracks (`--all-audio`)**: Preserve all multi-language audio streams present on the DVD.
- **EBU R128 Audio Normalization (`--normalize-audio`)**: Apply loudness normalization filter (`loudnorm`) for consistent volume across scenes.
- **Dual Audio Streams (`--dual-audio`)**: Output both normalized 2.0 AAC stereo and raw 5.1 AC3 passthrough audio streams.
- **Subtitle Extraction (`--subtitles`)**: Extract DVD bitmap subtitle streams (`dvdsub`) or convert to text SRT format (`subrip`).

---

## 4. TV Series Multi-Episode Batch Ripping

To rip TV show season box sets:

1. Check the **📺 TV Series Mode** checkbox.
2. Enter the **Show Title** (e.g. `The Office`) and **Season Number** (e.g. `1`).
3. Check **📦 Auto BoxSet** mode to enable cumulative episode numbering across multi-disc box sets.
4. Click **🔍 Scan Disc**: The GUI invokes `detect_tv_episodes()` to probe disc titles, automatically discarding intros, menu loops, and duplicate "Play All" tracks.
5. Review detected episode list (Title index, duration in minutes, start episode assignment).
6. Click **▶ Batch Rip All Episodes** to execute automated sequential extraction.

---

## 5. Progress Tracking, Live Logs & Instant Cancellation

- **Real-Time Progress Bar**: Visual percentage bar (0.0% to 100.0%) updated via thread-safe channels.
- **Performance Indicators**: Live FPS rendering counter and encoding speed multiplier (`2.4x`).
- **Ring Buffer Log Console**: Live scrollable terminal log window capturing FFmpeg stdout/stderr diagnostics.
- **Instant Job Cancellation (`⏹ Cancel Rip`)**: Click to send an emergency abort signal (`cancel_flag`), immediately terminating background FFmpeg child processes without leaving corrupted temporary files.

---

## 6. Auxiliary GUI Tabs

### 6.1 📦 Box Set Manager Tab
View active multi-disc TV show season tracking records stored in `~/.dvd-ripper/boxsets.json`.
- Displays Show Title, Season Number, Last Ripped Episode Number, Total Discs Processed, and Last Updated timestamp.
- Includes **Reset Show Counter** button to reset episode numbering when starting a fresh season rip.

### 6.2 ⚡ Optical Drive Speed Benchmark Tab
Execute raw sector throughput diagnostics:
- Click **⚡ Run Benchmark** to launch a 10-second sector read speed test.
- Displays read throughput (MB/s), demuxer FPS, and drive rating (`RipLocked`, `Standard 8x`, or `High Speed 16x+`).

### 6.3 📜 History Log Tab
Browse persistent historical ripping records from `ripping_history.json`:
- View past completed, cancelled, or failed rip jobs.
- Displays timestamps, target movie titles, runtime duration, output file paths, and file size in megabytes.
- Includes **Clear History Log** button.
