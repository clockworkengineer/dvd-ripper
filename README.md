# DVD Ripper (`dvd-ripper`)

A fast, portable DVD backup utility written in Rust featuring both a **Portable Native Desktop GUI** (powered by [`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command Line Interface (CLI)**.

`dvd-ripper` uses FFmpeg's native `dvdvideo` demuxer to rip titles directly from optical DVD drives. It automatically retrieves volume labels, queries online database APIs (OMDb / IMDb) to identify movies & TV series, displays high-resolution poster thumbnails and plot summaries, automatically detects multi-episode TV series discs, structures output directories into named movie (`Films/`) or TV series (`TV/Show Name (Year)/Season SS/`) folders, and provides real-time progress bars and live log streaming.

---

## 🌟 Key Features

- **Portable Native Desktop GUI**: Pure Rust immediate-mode GUI (`eframe`/`egui`). Zero C++ or WebKit runtime dependencies. Compiles into a single lightweight executable.
- **Embedded Web REST API & Appliance Dashboard**: Built-in HTTP REST API and HTML5 web dashboard (`http://localhost:8080`) providing live appliance monitoring, ripping history, and remote optical disc ejection control.
- **Headless Auto-Rip Daemon Appliance Mode**: Watcher loop (`--daemon`) that monitors optical drives, automatically detects inserted discs, fetches metadata, batch rips content, broadcasts smart home status, and ejects the tray upon completion.
- **MQTT Smart Home Telemetry**: Native Home Assistant telemetry reporting (`--mqtt-broker`) for real-time progress and disc status updates.
- **Persistent Ripping History**: Tracks all completed and cancelled backup events in a structured `ripping_history.json` database log.
- **TV Series Multi-Episode Ripping Mode**: Probes disc titles to detect individual TV episodes, automatically filtering out short intros (<10m) and "Play All" composite titles. Automatically structures files into Plex/Jellyfin standard TV naming: `TV/<Show Name> (<Year>)/Season <SS>/<Show Name> - S<SS>E<EE>.<ext>`.
- **Automatic Movie & TV Show Identification**: Reads DVD volume labels (via Windows Win32 API / Linux ISO-9660 reader), queries OMDb and IMDb APIs to fetch title, release year, genre, director, cast, IMDb rating, plot summary, and official poster thumbnail. Automatically sets TV mode when a TV series disc is detected.
- **Smart Movie Title Selection**: Probes DVD titles with optimized FFmpeg flags (`-analyzeduration 500000 -probesize 500000`) and selects the DVD title that best matches the movie's expected running time (or longest duration), bypassing studio intros, FBI warnings, and bonus features.
- **Hardware Acceleration Support**: Supports embedded and GPU hardware video acceleration (`--hwaccel` copy, v4l2m2m, vaapi, nvenc, qsv).
- **Fast Lossless Remuxing (Default)**: Losslessly copies raw video and audio streams (`-c copy`) into an MPEG program stream container for maximum extraction speed.
- **H.264 / AAC Transcoding**: Optional high-quality transcoding mode (`--transcode`) using `libx264` and `aac` with customizable encoding presets.
- **Live Progress & Monitoring**: Smooth progress bar (0–100%), real-time FPS & Speed indicators, interactive **Start** & **Cancel** controls, and a live log console.
- **Dual Execution Modes**: Launching without flags opens the native GUI window. Command-line users can pass `--cli`, `--daemon`, or standard CLI arguments for terminal workflows and automation scripts.

---

## 📋 Prerequisites

1. **Rust Toolchain**: Rust 2024 edition (`cargo` & `rustc`). Install via [rustup.rs](https://rustup.rs/).
2. **FFmpeg**: Must be installed and available in system `PATH` (or specified via `--ffmpeg <path>`). FFmpeg must be built with DVD reading support (`dvdvideo` demuxer enabled).
3. **OS**: Windows or Linux (Volume label queries use Win32 API on Windows and ISO-9660 Primary Volume Descriptor reading on Linux).

---

## 🚀 Installation & Building

Clone the repository and build using Cargo:

```bash
git clone https://github.com/your-username/dvd-ripper.git
cd dvd-ripper
cargo build --release
```

The compiled single executable will be saved at `target/release/dvd-ripper.exe`.

---

## 🖥️ Usage

### 1. Graphical User Interface (GUI Mode)

Running `dvd-ripper` without flags (or via `cargo run`) launches the portable desktop GUI:

```bash
cargo run
```

Or double-click `dvd-ripper.exe`.

#### GUI Features:
- **1. DVD Drive & Detection**: Set input drive letter (`D:\`) and click **"🔍 Detect DVD"** for async metadata lookup.
- **2. Media Metadata & Mode Settings**: Toggle **🎬 Movie Mode** or **📺 TV Series Mode**. Configure Show Name, Season #, Starting Episode #, **Batch Rip All Episodes**, and view detected episode title lists.
- **3. Ripping Process**: Click **▶ Batch Rip All Episodes** or **▶ Start Rip**. Monitor animated progress, FPS, and speed per episode.
- **4. Persistent Ripping History**: View and clear past rip records saved in `ripping_history.json`.
- **5. Log Console**: Scrollable live stream of FFmpeg output logs.

---

### 2. Embedded Appliance Daemon Mode

To run in headless auto-ripping appliance daemon mode:

```bash
dvd-ripper.exe --daemon --input D:\ --mqtt-broker 192.168.1.50:1883
```

This starts the embedded Web REST API server on `http://localhost:8080`, polls the drive every 10 seconds, auto-detects inserted media, batch rips the disc content, publishes Home Assistant telemetry over MQTT, records history, and ejects the tray upon completion.

---

### 3. Command Line Interface (CLI Mode)

To run in terminal mode, pass `--cli` or standard command line arguments:

```bash
# Basic rip from drive D:\ (auto-detects title based on running time / duration)
cargo run -- --cli D:\
```

#### Command Line Syntax & Options:

```text
Usage: dvd-ripper.exe [OPTIONS] [INPUT]

Arguments:
  [INPUT]  DVD drive letter or root path (e.g., D: or D:\) [default: D:\]

Options:
  -o, --output <OUTPUT>            Custom output file path (overridden if metadata details are auto-detected)
  -d, --out-dir <OUT_DIR>          Destination directory for ripped output [default: Films]
  -t, --title <TITLE>              Specific DVD title number to rip (e.g. 1, 2). Set to 0 (default) to auto-detect title matching running time / longest duration. [default: 0]
      --transcode                  Re-encode video (H.264) and audio (AAC) instead of lossless copy
      --preset <PRESET>            FFmpeg preset for H.264 encoding (e.g. ultrafast, superfast, veryfast, fast, medium) [default: veryfast]
      --ffmpeg <FFMPEG>            Custom path to FFmpeg executable [default: ffmpeg]
      --hwaccel <HWACCEL>          Hardware acceleration mode for transcoding (copy, v4l2m2m, vaapi, nvenc, qsv) [default: copy]
      --cli                        Force command-line interface mode instead of GUI
      --daemon                     Run as a headless embedded appliance daemon watching optical drive insertion
      --mqtt-broker <MQTT_BROKER>  Optional MQTT broker address (e.g. 192.168.1.50:1883) for smart home telemetry
      --tv                         Enable TV series disc ripping mode
      --season <SEASON>            Season number for TV series mode [default: 1]
      --start-episode <EPISODE>   Starting episode number for first detected title on disc [default: 1]
      --all-episodes               Automatically batch rip all detected TV episode titles on the disc sequentially
  -h, --help                       Print help information
  -V, --version                    Print version
```

---

## 💡 CLI Examples

### 1. Batch Rip TV Series Disc (All Episodes)
Automatically detects all episode titles on the disc and rips them to `TV/The Office (2005)/Season 01/The Office - S01E01.mpg`, `S01E02.mpg`, etc.:
```bash
dvd-ripper.exe --cli D: --tv --season 1 --all-episodes
```

### 2. Headless Daemon Watcher with Smart Home Telemetry
Monitors drive D:\ for disc insertions, auto-rips movies and TV series, publishes status to MQTT broker, and ejects disc when finished:
```bash
dvd-ripper.exe --daemon --input D:\ --mqtt-broker 192.168.1.50:1883
```

### 3. Movie Auto-Title Detection & Fast Remux (Default)
Auto-selects main title based on movie running time and losslessly remuxes into `Films/`:
```bash
dvd-ripper.exe --cli D:
```

### 4. Transcode TV Episodes with NVENC Hardware Acceleration
Re-encodes TV episodes using NVIDIA NVENC hardware acceleration:
```bash
dvd-ripper.exe --cli D: --tv --season 1 --all-episodes --transcode --hwaccel nvenc --preset fast
```

---

## 🏗️ Project Architecture

```
src/
├── main.rs       - Application entry point orchestrating GUI, CLI, or Daemon mode for Movies & TV Series
├── api.rs        - Embedded HTTP REST API & HTML5 Web UI appliance dashboard server
├── cli.rs        - Command-line argument schema (clap) with --tv, --season, --start-episode, --daemon, --mqtt-broker
├── daemon.rs     - Headless auto-rip watcher loop for optical disc insertion, metadata resolution, & auto-eject
├── dvd.rs        - Drive path normalization, Win32 & ISO-9660 volume label queries, & optical tray eject API
├── ffmpeg.rs     - Title probing (probe_dvd_titles, detect_best_title & detect_tv_episodes), path resolution, & execution engine
├── gui.rs        - Native desktop GUI implementation (eframe/egui) with Movie/TV toggles, posters, & history view
├── history.rs    - Persistent JSON ripping history database log (load, save, clear, record)
├── imdb.rs       - OMDb / IMDb API client, runtime & plot parsing models (FilmMetadata with is_series)
├── mqtt.rs       - MQTT smart home telemetry publisher for Home Assistant status reporting
└── utils.rs      - Duration parsing, filename sanitization, season/disc label extraction, & DRY string helpers
```

---

## 🧪 Testing

Run the test suite:
```bash
cargo test
```
All unit tests use RAII temporary directory guards to ensure test files leave zero footprint on disk.

---

## 📜 License

MIT License or Apache-2.0. Feel free to modify and distribute.
