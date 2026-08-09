# DVD Ripper CLI (`dvd-ripper`)

A fast, lightweight CLI utility written in Rust for backing up DVD titles to high-quality video files using FFmpeg's native `dvdvideo` demuxer. 

`dvd-ripper` automatically retrieves the volume label from your DVD drive, queries the IMDb API to identify the movie name and release year, structures output directories cleanly, and renders a live terminal progress bar during extraction.

---

## Features

- **Automatic IMDb Identification**: Reads the DVD volume label (Windows API) and fetches the matching movie title and release year from IMDb to automatically organize output files (e.g. `Thor Ragnarok (2017)/Thor Ragnarok (2017).mp4`).
- **Fast Lossless Remuxing (Default)**: Losslessly copies raw video and audio streams (`-c copy`) into an MPEG program stream container for blazing fast extraction speeds without quality loss.
- **H.264 / AAC Transcoding**: Optional high-quality transcoding mode (`--transcode`) using `libx264` and `aac` for maximum compatibility across devices.
- **FFmpeg `dvdvideo` Integration**: Utilizes FFmpeg's built-in `dvdvideo` demuxer for reliable reading directly from optical DVD drives or folder structures.
- **Live Terminal Progress Bar**: Parses FFmpeg runtime stderr output to show real-time percentage complete, encoding FPS, and copy/transcode speed.
- **Customizable**: Select specific DVD titles, adjust encoding presets, override output paths, or specify custom FFmpeg binary paths.

---

## Prerequisites

1. **Rust Toolchain**: Rust 2024 edition (Cargo & `rustc`). Install via [rustup.rs](https://rustup.rs/).
2. **FFmpeg**: Must be installed and available in your system `PATH` (or specified via `--ffmpeg <path>`). FFmpeg must be built with DVD reading support (`dvdvideo` demuxer enabled).
3. **OS**: Windows (volume label detection currently uses the Windows Win32 API).

---

## Installation

Clone the repository and build using Cargo:

```bash
git clone https://github.com/your-username/dvd-ripper.git
cd dvd-ripper
cargo build --release
```

The compiled binary will be located at `target/release/dvd-ripper.exe`.

---

## Usage

### Basic Usage

Rip default Title 1 from DVD drive `D:\` using fast lossless stream copy:

```bash
cargo run -- D:\
```

Or run the compiled executable directly:

```bash
dvd-ripper.exe D:\
```

### Command Line Options

```text
Usage: dvd-ripper.exe [OPTIONS] [INPUT]

Arguments:
  [INPUT]  DVD drive letter or root path (e.g., D: or D:\) [default: D:\]

Options:
  -o, --output <OUTPUT>    Output file path (overridden if IMDb details are auto-detected)
  -t, --title <TITLE>      Specific DVD title number to rip (e.g. 1) [default: 1]
      --transcode          Re-encode video (H.264) and audio (AAC) instead of lossless stream copy
      --preset <PRESET>    FFmpeg preset for H.264 encoding (e.g. ultrafast, superfast, veryfast, fast, medium) [default: veryfast]
      --ffmpeg <FFMPEG>    Custom path to FFmpeg executable [default: ffmpeg]
  -h, --help               Print help
  -V, --version            Print version
```

---

## Examples

### 1. Lossless Stream Copy (Default)
Fastest extraction method. Remuxes DVD streams without re-encoding:
```bash
dvd-ripper D: --title 1
```

### 2. Transcode to MP4 (H.264 / AAC)
Re-encodes video to H.264 (CRF 22) and audio to AAC (128k):
```bash
dvd-ripper D: --transcode --preset fast
```

### 3. Rip Specific Title with Custom Output
Specify a custom output path for a specific DVD title:
```bash
dvd-ripper E:\ -t 2 -o "my_movie.mp4" --transcode
```

### 4. Custom FFmpeg Binary Path
Provide an explicit path to FFmpeg if it is not in your system `PATH`:
```bash
dvd-ripper D: --ffmpeg "C:\Tools\ffmpeg\bin\ffmpeg.exe"
```

---

## How It Works

1. **Volume Detection & IMDb Lookup**: Obtains volume information via `GetVolumeInformationW`. Cleans the label string and queries IMDb's suggestion API to resolve titles like `THOR_RAGNAROK` to `"Thor Ragnarok (2017)"`.
2. **FFmpeg Command Generation**: Constructs and executes an FFmpeg process with `-f dvdvideo -title <N> -i <DRIVE>`.
3. **Log Parsing & Progress Updates**: Pipes FFmpeg's `stderr` in real time, calculating duration and updating terminal progress:
   ```text
   Progress: [████████████████████░░░░░░░░░░] 67.4% | FPS: 142.5 | Speed: 5.8x
   ```

---

## License

MIT License or Apache-2.0. Feel free to modify and distribute.
