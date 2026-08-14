# DVD Ripper (`dvd-ripper`)

A fast, portable DVD backup utility written in Rust featuring both a **Portable Native Desktop GUI** (powered by [`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command Line Interface (CLI)**.

`dvd-ripper` uses FFmpeg's native `dvdvideo` demuxer to rip titles directly from optical DVD drives. It automatically retrieves volume labels, queries online database APIs (OMDb / IMDb) to identify movies & TV series, displays high-resolution poster thumbnails and plot summaries, automatically detects multi-episode TV series discs, structures output directories into named movie (`Films/`) or TV series (`TV/Show Name (Year)/Season SS/`) folders, and provides real-time progress bars and live log streaming.

---

## 🌟 Key Features

- **Portable Native Desktop GUI**: Pure Rust immediate-mode GUI (`eframe`/`egui`). Zero C++ or WebKit runtime dependencies. Compiles into a single lightweight executable.
- **Embedded Web REST API & Appliance Dashboard**: Built-in HTTP REST API and interactive HTML5 web dashboard (`http://localhost:8080`) providing live appliance status monitoring (`GET /api/status`), remote ripping control (`POST /api/rip`, `POST /api/cancel`), ripping history (`GET /api/history`), and optical disc tray ejection (`POST /api/eject`).
- **Headless Auto-Rip Daemon Appliance Mode**: Watcher loop (`--daemon`) that monitors optical drives, automatically detects inserted discs, fetches metadata, batch rips content, broadcasts smart home status, triggers webhooks, and ejects the tray upon completion.
- **Smart Home & Webhook Telemetry Notifications**: Native Home Assistant MQTT reporting (`--mqtt-broker`) and HTTP JSON Webhook notifications (`--webhook-url`) compatible with Discord, Slack, Ntfy, and Telegram.
- **Audio & Subtitle Track Stream Selection**: Multi-language audio track extraction (`--all-audio`, `--audio-lang`) and subtitle stream extraction (`--subtitles`, `--sub-lang`).
- **Output File Overwrite Protection**: Automatic duplicate file collision resolution (`--no-overwrite`) appending incremental numeric suffixes (`Title_1.mpg`, `Title_2.mpg`).
- **Portable Multi-OS Installer**: Built-in installer (`dvd-ripper-installer`) supporting user and system-wide installation, FFmpeg dependency auditing, and system PATH configuration on Windows, Linux, and macOS.
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
3. **OS**: Windows, Linux, or macOS.

---

## 🚀 Installation & Building

### Option 1: Automatic Cross-Platform Installer (Recommended)

DVD Ripper includes a portable, multi-OS installer (`dvd-ripper-installer`) that handles binary installation, FFmpeg auditing, system PATH configuration, and Linux systemd/udev service setup:

```bash
# Clone the repository
git clone https://github.com/your-username/dvd-ripper.git
cd dvd-ripper

# Build the release binaries
cargo build --release

# Run the installer (User mode by default)
cargo run --bin dvd-ripper-installer
```

#### Installer CLI Options:
```text
Usage: dvd-ripper-installer [OPTIONS]

Options:
      --system     Install system-wide (requires Administrator / root privileges)
      --user       Install for current user (default: %LOCALAPPDATA%\dvd-ripper\bin or ~/.local/bin)
  -d, --dir <DIR>  Custom target installation directory
  -u, --uninstall  Uninstall DVD Ripper from system
      --service    Install Linux systemd service and udev rules (Linux system-wide mode)
  -y, --yes        Non-interactive mode (automatically answer yes to prompts)
```

### Option 2: Direct Binary Execution

The compiled single executable will be saved at `target/release/dvd-ripper.exe` (or `target/release/dvd-ripper` on Linux/macOS). You can run or move this binary directly.

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
- **2. Media Metadata & Mode Settings**: Toggle **🎬 Movie Mode** or **📺 TV Series Mode**. Configure Show Name, Season #, Starting Episode #, Audio Track Languages, Subtitle Languages, Webhook Notifications, and **Batch Rip All Episodes**.
- **3. Ripping Process**: Click **▶ Batch Rip All Episodes** or **▶ Start Rip**. Monitor animated progress, FPS, and speed per episode.
- **4. Persistent Ripping History**: View and clear past rip records saved in `ripping_history.json`.
- **5. Log Console**: Scrollable live stream of FFmpeg output logs.

---

## 🌐 Embedded Web REST API Dashboard

When running in Daemon appliance mode (`--daemon`), an embedded web server is launched on port 8080 (`http://localhost:8080`):

| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/` | Serves interactive HTML5 Web Dashboard |
| `GET` | `/api/status` | Returns JSON status (`status`, `drive`, `disc`, `progress`) |
| `GET` | `/api/history` | Returns JSON array of past ripping records |
| `POST` | `/api/rip` | Triggers a ripping job remotely |
| `POST` | `/api/cancel` | Requests cancellation of active ripping job |
| `POST` | `/api/eject` | Ejects the optical drive tray |

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
      --webhook-url <WEBHOOK_URL>  Optional HTTP Webhook URL (Discord/Slack/Ntfy/Telegram) for JSON POST notifications
      --tv                         Enable TV series disc ripping mode
      --season <SEASON>            Season number for TV series mode [default: 1]
      --start-episode <EPISODE>   Starting episode number for first detected title on disc [default: 1]
      --all-episodes               Automatically batch rip all detected TV episode titles on the disc sequentially
      --all-audio                  Include all audio tracks from DVD title in output
      --audio-lang <LANG>          Preferred audio track language code (e.g. eng, fre, spa)
      --subtitles                  Extract subtitle tracks from DVD title into output container
      --sub-lang <LANG>            Preferred subtitle track language code (e.g. eng, fre, spa)
      --no-overwrite               Do not overwrite existing files (auto-append incremental numeric suffix)
  -h, --help                       Print help information
  -V, --version                    Print version
```

---

## 💡 CLI Examples

### 1. Batch Rip TV Series Disc with Subtitles & All Audio Tracks
Automatically detects all episode titles on the disc, maps all audio tracks and English subtitles, and rips them to `TV/`:
```bash
dvd-ripper.exe --cli D: --tv --season 1 --all-episodes --all-audio --subtitles --sub-lang eng
```

### 2. Headless Daemon Watcher with Smart Home Telemetry & Webhook Alerts
Monitors drive D:\ for disc insertions, auto-rips content, posts notifications to Discord webhook, and ejects disc when finished:
```bash
dvd-ripper.exe --daemon D:\ --webhook-url https://discord.com/api/webhooks/... --mqtt-broker 192.168.1.50:1883
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
├── main.rs         - Application entry point orchestrating GUI, CLI, or Daemon mode for Movies & TV Series
├── api.rs          - Embedded HTTP REST API & HTML5 Web UI appliance dashboard server
├── cli.rs          - Command-line argument schema (clap) with audio/subtitle/webhook options
├── daemon.rs       - Headless auto-rip watcher loop for optical disc insertion, metadata resolution, & auto-eject
├── dvd.rs          - Drive path normalization, Win32 & ISO-9660 volume label queries, & optical tray eject API
├── ffmpeg.rs       - Title probing, stream mapping, collision protection, & execution engine
├── gui.rs          - Native desktop GUI implementation (eframe/egui) with stream controls & history view
├── history.rs      - Persistent JSON ripping history database log (load, save, clear, record)
├── imdb.rs         - OMDb / IMDb API client, runtime & plot parsing models (FilmMetadata with is_series)
├── mqtt.rs         - MQTT smart home telemetry & HTTP Webhook notification engine
├── utils.rs        - Duration parsing, filename sanitization, season/disc label extraction, & DRY string helpers
└── bin/
    └── installer.rs - Portable multi-OS installer binary (dvd-ripper-installer)
```

---

## 🧪 Testing

Run the full test suite (26 unit tests):
```bash
cargo test
```
All unit tests use RAII temporary directory guards to ensure test files leave zero footprint on disk.

---

## 📜 License

MIT License or Apache-2.0. Feel free to modify and distribute.
