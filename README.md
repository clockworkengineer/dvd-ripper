# DVD Ripper (`dvd-ripper`)

A fast, portable DVD backup utility written in Rust featuring both a **Portable Native Desktop GUI** (powered by [`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command Line Interface (CLI)**.

`dvd-ripper` uses FFmpeg's native `dvdvideo` demuxer to rip titles directly from optical DVD drives. It automatically retrieves volume labels, queries online database APIs (OMDb / IMDb) to identify movies & TV series, displays high-resolution poster thumbnails and plot summaries, automatically detects multi-episode TV series discs, structures output directories into named movie (`Films/`) or TV series (`TV/Show Name (Year)/Season SS/`) folders, and provides real-time progress bars and live log streaming.

---

## 🌟 Key Features

- **Portable Native Desktop GUI**: Pure Rust immediate-mode GUI (`eframe`/`egui`). Zero C++ or WebKit runtime dependencies. Compiles into a single lightweight executable.
- **TV Series Multi-Episode Ripping Mode**: Probes disc titles to detect individual TV episodes, automatically filtering out short intros (<10m) and "Play All" composite titles. Automatically structures files into Plex/Jellyfin standard TV naming: `TV/<Show Name> (<Year>)/Season <SS>/<Show Name> - S<SS>E<EE>.<ext>`.
- **Automatic Movie & TV Show Identification**: Reads DVD volume labels (via Windows Win32 API), queries OMDb and IMDb APIs to fetch title, release year, genre, director, cast, IMDb rating, plot summary, and official poster thumbnail. Automatically sets TV mode when a TV series disc is detected.
- **Smart Movie Title Selection**: Probes DVD titles with optimized FFmpeg flags (`-analyzeduration 500000 -probesize 500000`) and selects the DVD title that best matches the movie's expected running time (or longest duration), bypassing studio intros, FBI warnings, and bonus features.
- **Movie & TV Poster / Plot Display**: Dynamically loads and renders official poster thumbnails, episode lists, and plot descriptions directly in the desktop application interface.
- **Fast Lossless Remuxing (Default)**: Losslessly copies raw video and audio streams (`-c copy`) into an MPEG program stream container for maximum extraction speed.
- **H.264 / AAC Transcoding**: Optional high-quality transcoding mode (`--transcode`) using `libx264` and `aac` with customizable encoding presets.
- **Live Progress & Monitoring**: Smooth progress bar (0–100%), real-time FPS & Speed indicators, interactive **Start** & **Cancel** controls, and a live log console.
- **Dual Execution Modes**: Launching without flags opens the native GUI window. Command-line users can pass `--cli` or standard CLI arguments for terminal workflows and automation scripts.

---

## 📋 Prerequisites

1. **Rust Toolchain**: Rust 2024 edition (`cargo` & `rustc`). Install via [rustup.rs](https://rustup.rs/).
2. **FFmpeg**: Must be installed and available in system `PATH` (or specified via `--ffmpeg <path>`). FFmpeg must be built with DVD reading support (`dvdvideo` demuxer enabled).
3. **OS**: Windows (volume label detection uses Windows Win32 API).

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
- **4. Log Console**: Scrollable live stream of FFmpeg output logs.

---

### 2. Command Line Interface (CLI Mode)

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
      --cli                        Force command-line interface mode instead of GUI
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

### 2. Rip Specific TV Episode with Start Episode Offset
Rip disc starting at Episode 5 of Season 2:
```bash
dvd-ripper.exe --cli D: --tv --season 2 --start-episode 5 --all-episodes
```

### 3. Movie Auto-Title Detection & Fast Remux (Default)
Auto-selects main title based on movie running time and losslessly remuxes into `Films/`:
```bash
dvd-ripper.exe --cli D:
```

### 4. Transcode TV Episodes to H.264 / AAC MP4
Re-encodes TV episodes to H.264 MP4 format:
```bash
dvd-ripper.exe --cli D: --tv --season 1 --all-episodes --transcode --preset fast
```

---

## 🏗️ Project Architecture

```
src/
├── main.rs       - Application entry point orchestrating GUI or CLI launch for Movies & TV Series
├── cli.rs        - Command-line argument schema (clap) with --tv, --season, --start-episode, --all-episodes
├── dvd.rs        - Drive path normalization & Win32 volume label queries
├── gui.rs        - Native desktop GUI implementation with Movie / TV Series mode toggles & episode lists
├── imdb.rs       - OMDb / IMDb API client, runtime & plot parsing models (FilmMetadata with is_series)
├── ffmpeg.rs     - Fast title probing (detect_best_title & detect_tv_episodes), resolve_tv_output_path, batch process lifecycle
└── utils.rs      - Duration parsing, filename sanitization & DRY string extraction helpers
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
