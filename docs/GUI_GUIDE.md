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

The desktop interface is organized into **7 core workflow sections and tab views**:

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
│ Target Path: 📁 C:\Media\Films\Kill Bill (2003)\Kill Bill (2003).mp4  │
│ Progress: [=====================>                  ] 45.2% (28.5 fps) │
│ Console:  [16:04:12] [INFO] FFmpeg demuxer initialized...              │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2.1 ℹ️ About Box Modal

Clicking the **`ℹ️ About`** button in the top-right header opens the application info dialog displaying:
- Application version (`v0.4.0`) and description.
- Dual open-source licenses (`MIT OR Apache-2.0`).
- Direct hyperlink to the **[GitHub Repository](https://github.com/clockworkengineer/dvd-ripper)**.
- Direct hyperlink to **[Buy Me a Coffee](https://buymeacoffee.com/roberttizz1)** support page.
- Configured system FFmpeg executable path.

---

## 3. Main Rip Panel Setup

### 3.1 Input Drive & Output Directory Selection
- **Input Drive**: Dropdown menu allowing selection of `auto` drive detection, explicit optical drive letters (`D:\`, `E:\`), or Linux block devices (`/dev/sr0`).
- **Base Output Directory (`out_dir`)**: Configures the base media library directory (defaults to `%USERPROFILE%\Videos\DVD Rips` on Windows or `~/Videos/DVD Rips` on Linux/macOS).
  - **`📂 Browse...`**: Opens the system-native folder picker dialog.
  - **`↗ Open`**: Immediately launches Windows File Explorer or the Linux desktop file manager in the active output folder.
  - **`↺ Reset`**: Restores the output folder setting to the OS default Videos directory.
  - **Routing Caption**: An informational indicator displays: `ℹ Films are saved to /Films and TV Shows to /TV within this directory` (or custom names configured in Settings). TV series rips automatically resolve to a sibling `TV` folder if `Films` is selected.
  - **Dynamic Output Path Preview**: A prominent status badge displays the exact resolved destination path in real time as the movie title, year, or container settings are edited (e.g. `📁 Output: C:\Media\Films\Kill Bill (2003)\Kill Bill (2003).mp4`).

### 3.2 Movie Metadata & Candidate Search Modal
- **Movie Title & Release Year**: Enter the title name and release year for Plex/Jellyfin naming compatibility.
- **🔍 Search OMDb / IMDb Candidate Modal**: Click **Search OMDb** to open the interactive search candidate popup.
  - Queries OMDb and IMDb APIs.
  - Displays movie posters, release years, IMDb IDs (`tt0266697`), and plot summaries.
  - Clicking **Select** populates the title, year, genre, director, cast, plot summary, and fetches high-resolution poster artwork.

### 3.3 Transcoding & Video Filter Quality Suite
- **Transcoding Engine Selector**:
  - **Auto Fallback (Recommended)**: Automatically tries FFmpeg first. If an invalid IFO, demuxer crash, or copy protection bad sector is encountered, it seamlessly falls back to the Native Rust IFO Cell Extractor, HandBrakeCLI, or MakeMKV.
  - **FFmpeg (`ffmpeg`)**: Full filter pipeline with motion-adaptive deinterlacing, 3D denoising, crop detection, and GPU hardware acceleration.
  - **Native IFO Extractor (`native`)**: Pure Rust sector cell extractor that reads DVD Program Chains (`VTS_PGCITI`) directly, bypassing FFmpeg demuxer bugs and ARccOS bad-sector traps with `.BUP` backup table support.
  - **HandBrakeCLI (`handbrake`)**: Advanced transcoding using HandBrake's native `libdvdnav` menu navigation and motion-adaptive decombing.
  - **MakeMKV (`makemkv`)**: Bit-for-bit lossless robot-mode remuxing into Matroska (`.mkv`) with optical drive hardware bus key decryption (AACS, BD+, CSS).
- **Transcode Toggle (`--transcode`)**: Enable to re-encode video streams using `libx264`, `libx265`, or `libsvtav1`. Leave unchecked for fast, lossless remuxing (`-c copy`).
- **Single-Pass Dual Output (`--dual-output`)**: Enable to simultaneously generate both a bit-for-bit archival master (`.mkv`/`.iso`) and a lightweight compressed streaming file (`.mp4`) in a single read pass, cutting laser exposure and mechanical wear by 50%.
- **Drive Acoustic Profile Selector (`--acoustic-profile`)**: Select spindle noise and speed negotiation:
  - **🔇 Silent**: Throttles spindle to ~4x (~5,540 KB/s) for whisper-quiet living room ripping.
  - **⚖️ Balanced**: Standard ~8x (~11,080 KB/s) operation.
  - **🚀 Turbo**: Maximum 16x+ hardware drive capability for fastest possible archival ingest.
- **Cadence-Aware 3:2 Pulldown IVTC (`--detelecine`)**: Automatically inspects MPEG-2 RFF/TFF flags and applies field matching + frame decimation to eliminate judder and recover pristine 23.976p film.
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

To rip TV show season box sets with custom episode ranges:

1. Switch to the **📺 TV Box Sets** tab in the navigation bar.
2. Enter the **Show Title** (e.g. `The Office`) and **Season Number** (e.g. `1`).
3. Set your target **Episode Range**:
   - **`Start Ep #`**: Starting episode number on this DVD (e.g. `1` for Disc 1, `5` for Disc 2).
   - **`End Ep #`**: Ending episode number to extract (e.g. `4`). When episodes are scanned, the UI automatically calculates the natural ending episode, but you can adjust it to restrict extraction to a specific sub-range.
   - A live range badge displays the exact span, e.g. `(S01E01..S01E04 • 4 eps)`.
4. Check **📦 Auto BoxSet** mode to enable automatic episode offset tracking across multi-disc box sets (`boxsets.json`).
5. Click **🔍 Scan Disc for Episodes**: The heuristics engine automatically discards menu loops, bumpers, and duplicate "Play All" tracks.
6. Review the discovered episode tracks:
   - Included episodes within the Start–End range are marked with a green checkmark (`✓`).
   - Episodes outside the range are struck through and marked `[Excluded]` (`✗`).
   - Click the camera icon (`📸`) to generate real-time visual thumbnail previews.
7. Click **▶ Batch Rip Episodes (S01E01..S01E04 • 4 tracks)** to execute automated sequential transcoding.

---

## 5. Progress Tracking, Live Logs & Cooperative Cancellation

- **Real-Time Progress Bar**: Visual percentage bar (0.0% to 100.0%) updated via thread-safe crossbeam channels.
- **Performance Indicators**: Live FPS rendering counter and encoding speed multiplier (`2.4x`).
- **Ring Buffer Log Console**: Live scrollable terminal log window capturing FFmpeg stdout/stderr diagnostics.
- **Cooperative Cancellation Flow (`⏹ Cancel Ripping`)**:
  - **Ubiquitous Visibility**: Prominent red `⏹ Cancel Ripping` buttons are active across the top header bar, the live Progress & Telemetry bar, the Rip tab, and the TV Series batch rip tab.
  - **Complete UI Lockdown**: While ripping is in progress, all other UI elements (tab bar navigation, input fields, checkboxes, drag values, and action buttons) are systematically disabled to prevent accidental state modifications or conflicting operations. Only the Cancel button remains active.
  - **Confirmation Dialog**: Clicking **⏹ Cancel Ripping** or pressing **Escape** brings up a modal confirmation dialog to verify cancellation and prevent unintended aborts.
  - **Clean Termination**: Confirming cancellation sets an atomic `cancel_flag` observed cooperatively by background threads and child processes. The UI cleanly terminates FFmpeg, removes incomplete files, releases optical drive locks, and records `"Cancelled"` in `ripping_history.json`.

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

---

## 7. ⚙️ Global Settings & Preferences Tab

The **Settings Tab** provides centralized configuration for engine preferences, binary overrides, custom media taxonomy, and audio mastering profiles. All modifications are automatically persisted to `~/.dvd-ripper/config.toml`.

```text
┌────────────────────────────────────────────────────────────────────────┐
│ ⚙️ Global Settings & Configuration                                      │
├────────────────────────────────────────────────────────────────────────┤
│ 1. Engine & Binary Overrides:                                          │
│    Primary Engine: [ auto ▾ ]                                          │
│    FFmpeg Path:      [ C:\ffmpeg\bin\ffmpeg.exe                      ] │
│    HandBrake Path:   [ C:\HandBrake\HandBrakeCLI.exe                 ] │
│    MakeMKV Path:     [ C:\Program Files (x86)\MakeMKV\makemkvcon64.exe] │
│ 2. Storage & Media Taxonomy:                                           │
│    Base Output Dir:  [ C:\Media                                      ] │
│    Films Subfolder:  [ Movies              ]  (Default: Films)         │
│    TV Subfolder:     [ Series              ]  (Default: TV)            │
│    Taxonomy Preview: ℹ Films: C:\Media\Movies | TV: C:\Media\Series    │
│ 3. Audio & EBU R128 Loudness Targets:                                  │
│    Integrated Target LUFS: [-23.0 LUFS ] (Broadcast Standard)          │
│    True Peak Max:          [-1.5 dBFS  ] (Inter-sample peak safety)    │
│    Loudness Range LRA:     [ 11.0 LU   ] (Dynamic range envelope)      │
│ 4. System Maintenance:                                                 │
│    [↺ Reset to Defaults]                                               │
└────────────────────────────────────────────────────────────────────────┘
```

### 7.1 Engine & Binary Path Configuration
- **Primary Engine**: Choose the default extraction pipeline (`auto`, `ffmpeg`, `native`, `handbrake`, or `makemkv`).
- **Binary Overrides**: Explicitly specify absolute filesystem paths for external CLI tools when they are not located in the system `PATH`. Paths containing spaces or UNC network shares are fully supported.

### 7.2 Media Taxonomy & Folder Customization
- **Base Output Directory (`out_dir`)**: The root root library destination.
- **Films Subfolder Name (`films_dir`)**: Name of the movie destination subfolder (defaults to `"Films"`). Users preferring Plex or Jellyfin conventions can configure `"Movies"`, `"Cinema"`, or any localized folder name.
- **TV Subfolder Name (`tv_dir`)**: Name of the television series subfolder (defaults to `"TV"`). Users can customize this to `"TV Shows"`, `"Series"`, or `"Anime"`.
- **Sibling Directory Harmonization**: Sibling folder auto-correction honors these custom names bidirectionally.

### 7.3 Audio Loudness & Mastering Standards
- **EBU R128 Integrated Target (`target_lufs`)**: Configurable slider from `-24.0 LUFS` (EBU R128 television) to `-14.0 LUFS` (Spotify / YouTube web target). Default is `-23.0 LUFS`.
- **True Peak Threshold (`true_peak`)**: Limits maximum inter-sample peaks between `-2.0 dBFS` and `-0.5 dBFS` (default: `-1.5 dBFS`) to avoid DAC clipping during lossy compression.
- **Loudness Range Envelope (`loudness_range`)**: Sets target LRA between `7.0 LU` and `20.0 LU` (default: `11.0 LU`).

### 7.4 Reset to Factory Defaults
- Clicking **`↺ Reset to Defaults`** restores all engine paths, taxonomy folder names, and audio loudness parameters to their factory defaults while preserving rip history and boxset progress.
