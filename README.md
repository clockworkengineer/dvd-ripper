# DVD Ripper (`dvd-ripper`)

A fast, portable DVD backup utility written in Rust featuring both a **Portable Native Desktop GUI** (powered by [`eframe`/`egui`](https://github.com/emilk/egui)) and a full **Command Line Interface (CLI)**.

`dvd-ripper` uses FFmpeg's native `dvdvideo` demuxer to rip titles directly from optical DVD drives. It automatically retrieves volume labels, queries the IMDb API to identify movie titles and release years, structures output directories inside a configurable destination folder (default `"Films"`), and provides real-time progress bars and live log streaming.

---

## 🌟 Key Features

- **Portable Native Desktop GUI**: Pure Rust immediate-mode GUI (`eframe`/`egui`). Zero C++ or WebKit runtime dependencies. Compiles into a single lightweight executable.
- **Automatic IMDb Identification**: Reads the DVD volume label (via Windows Win32 API) and queries IMDb's suggestion database to resolve movie titles and release years (e.g., `Films/Thor Ragnarok (2017)/Thor Ragnarok (2017).mp4`).
- **Configurable Destination Folder**: Automatically places all ripped films into a configured directory (defaults to `"Films"`, customizable via GUI or `--out-dir`).
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
- **1. DVD Drive & Detection**: Set input drive letter (`D:\`) and click **"🔍 Detect DVD"** for async IMDb lookup.
- **2. Film Metadata & Output**: Edit detected title and year, set target directory (default `"Films"`), adjust title #, toggle H.264 re-encoding, and choose encoding presets.
- **3. Ripping Process**: Click **▶ Start Rip** to start or **⏹ Cancel** to stop. Monitor animated progress, FPS, and speed.
- **4. Log Console**: Scrollable live stream of FFmpeg output logs.

---

### 2. Command Line Interface (CLI Mode)

To run in terminal mode, pass `--cli` or standard command line arguments:

```bash
# Basic rip of Title 1 from drive D:\ to Films/ directory
cargo run -- --cli D:\
```

#### Command Line Syntax & Options:

```text
Usage: dvd-ripper.exe [OPTIONS] [INPUT]

Arguments:
  [INPUT]  DVD drive letter or root path (e.g., D: or D:\) [default: D:\]

Options:
  -o, --output <OUTPUT>    Custom output file path (overridden if IMDb details are auto-detected)
  -d, --out-dir <OUT_DIR>  Destination directory for ripped output [default: Films]
  -t, --title <TITLE>      Specific DVD title number to rip (e.g. 1, 2). Set to 0 (default) to auto-detect and select the title with the longest duration. [default: 0]
      --transcode          Re-encode video (H.264) and audio (AAC) instead of lossless copy
      --preset <PRESET>    FFmpeg preset for H.264 encoding (e.g. ultrafast, superfast, veryfast, fast, medium) [default: veryfast]
      --ffmpeg <FFMPEG>    Custom path to FFmpeg executable [default: ffmpeg]
      --cli                Force command-line interface mode instead of GUI
  -h, --help               Print help information
  -V, --version            Print version
```

---

## 💡 CLI Examples

### 1. Fast Lossless Stream Copy (Default)
Remuxes DVD streams without re-encoding into `Films/`:
```bash
dvd-ripper.exe --cli D: --title 1
```

### 2. Custom Output Directory
Rip into a specific output directory:
```bash
dvd-ripper.exe --cli D: -d "D:\MyMovies"
```

### 3. Transcode to MP4 (H.264 / AAC)
Re-encodes video to H.264 (CRF 22) and audio to AAC (128k):
```bash
dvd-ripper.exe --cli D: --transcode --preset fast
```

### 4. Custom FFmpeg Path
Specify custom FFmpeg location:
```bash
dvd-ripper.exe --cli D: --ffmpeg "C:\Tools\ffmpeg\bin\ffmpeg.exe"
```

---

## 🏗️ Project Architecture

```
src/
├── main.rs       - Application entry point orchestrating GUI or CLI launch
├── cli.rs        - Command-line argument schema (clap)
├── dvd.rs        - Drive path normalization & Win32 volume label queries
├── gui.rs        - Native desktop GUI implementation (eframe/egui)
├── imdb.rs       - IMDb Suggestion API client & data models
├── ffmpeg.rs     - FFmpeg command builder, process lifecycle, channel event emitter
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
