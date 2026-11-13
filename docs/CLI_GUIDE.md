# DVD Ripper CLI User Manual & Command Reference

`dvd-ripper` provides a feature-complete command-line interface for automated DVD ripping, metadata tagging, hardware-accelerated video encoding, multi-disc TV box set batch processing, and optical drive diagnostics.

---

## 1. Syntax & Overview

```bash
dvd-ripper [OPTIONS]
```

By default, launching `dvd-ripper` without arguments opens the graphical desktop utility (if compiled with GUI support). To run in headless CLI mode or execute specific operations, pass any of the flags detailed below.

---

## 2. General & Drive Options

| Flag / Option | Default | Description |
|---|---|---|
| `-i`, `--input <PATH>` | `D:\` (or `/dev/sr0`) | Target DVD drive letter, mount point, or ISO path (use `auto` for drive auto-detection). |
| `-o`, `--output <FILE>` | *Auto* | Explicit destination file path. |
| `--out-dir <DIR>` | `Films` | Base output root directory. Movies are saved into `<out_dir>/<films-dir>/` and TV series into `<out_dir>/<tv-dir>/` with intelligent sibling resolution. |
| `--films-dir <NAME>` | `Films` | Configurable subfolder name for movie/film rips within the base output directory. |
| `--tv-dir <NAME>` | `TV` | Configurable subfolder name for TV series rips within the base output directory. |
| `--title <NUM>` | `0` | DVD title track index to rip (`0` = auto-detect title with matching movie runtime or longest duration). |
| `--config <PATH>` | *Auto* | Path to custom TOML configuration file. |
| `--cli` | `false` | Force headless command-line execution (suppress GUI launch). |
| `--daemon` | `false` | Run in background appliance daemon mode with REST API server & Home Assistant integration. |
| `--check-protection` | `false` | Run copy protection (CSS/CPPM) and bad-sector diagnostic scan on disc. |
| `--benchmark` | `false` | Run 10-second optical sector read speed throughput test (MB/s) and exit. |
| `--acoustic-profile <MODE>` | `balanced` | Optical spindle speed negotiation: `silent` (4x whisper-quiet), `balanced` (8x standard), or `turbo` (16x+ maximum). |

---

## 3. Video Encoding & Filter Options

| Flag / Option | Default | Description |
|---|---|---|
| `--transcode` | `false` | Enable video re-encoding (auto-enabled when selecting `h264`, `hevc`, `av1`, or preset profiles). |
| `--codec <CODEC>` | `h264` | Video codec: `h264` (AVC), `hevc` (H.265), `av1` (SVT-AV1), or `copy` (lossless stream copy). |
| `--profile <PROFILE>` | `standard` | Preset encoding profile: `standard`, `archival`, `plex`, or `mobile`. |
| `--preset <PRESET>` | `veryfast` | FFmpeg encoder speed preset: `ultrafast`, `superfast`, `veryfast`, `fast`, `medium`. |
| `--hwaccel <MODE>` | `copy` | Hardware acceleration mode: `copy` (software), `nvenc` (NVIDIA), `vaapi` (Intel/AMD Linux), `qsv` (Intel QuickSync), `v4l2` (Raspberry Pi). |
| `--deinterlace` | `false` | Apply motion-adaptive video deinterlacing filter (`-vf bwdif/yadif`) to eliminate comb artifacts. |
| `--deinterlace-algo <ALGO>`| `bwdif` | Deinterlacing algorithm selection: `bwdif` (best quality), `yadif`, or `w3fdif`. |
| `--detelecine` | `false` | Cadence-aware inverse telecine (IVTC) via field matching (`fieldmatch,decimate`) to recover native 23.976p film. |
| `--dual-output` | `false` | Single-pass dual output: concurrently creates bit-for-bit archival master (`.mkv`/`.iso`) and streaming transcode (`.mp4`). |
| `--denoise` | `false` | Apply 3D spatial/temporal denoising filter (`-vf hqdn3d`) to reduce film grain and analog noise. |
| `--sub-burnin` | `false` | Hard-burn subtitle text overlay directly onto video frames (`-vf subtitles=...`) for maximum compatibility. |
| `--tonemap <ALGO>` | *None* | Video SDR tonemapping curve selection (`mobius`, `hable`, `reinhard`) for HDR content conversion. |
| `--mkv` | `false` | Force Matroska (`.mkv`) output container format instead of MP4 (`.mp4`). |
| `--iso` | `false` | Create a 1:1 sector-by-sector archival ISO image dump directly from optical disc. |
| `--ignore-errors` | `false` | Enable fault-tolerant demuxing to bypass surface scratches and copy-protection bad sectors. |
| `--engine <ENGINE>` | `auto` | Transcoding/remuxing engine: `auto` (smart fallback cascade), `ffmpeg` (native demuxer), `native` (pure Rust IFO cell extractor), `handbrake` (HandBrakeCLI), or `makemkv` (MakeMKV robot mode). |
| `--handbrake-path <PATH>`| `HandBrakeCLI` | Custom executable path to `HandBrakeCLI` binary. |
| `--makemkv-path <PATH>` | `makemkvcon` | Custom executable path to `makemkvcon` binary. |

---

## 4. Audio, Subtitles & Metadata Options

| Flag / Option | Default | Description |
|---|---|---|
| `--all-audio` | `false` | Include all audio streams present on the DVD title track. |
| `--normalize-audio` | `false` | Apply EBU R128 loudness normalization (`-filter:a loudnorm=I=<TARGET>:TP=-1.5:LRA=11`). |
| `--norm-target <LUFS>` | `-16` | Configurable target LUFS loudness value (e.g. `-16`, `-14`, `-23 LUFS`). |
| `--audio-downmix <MODE>`| *None* | Audio downmix channel matrix filter (`stereo`, `dolphylogic`, `headphone`, `mono`). |
| `--dual-audio` | `false` | Generate dual audio streams (Track 1: AAC Stereo Normalized, Track 2: 5.1 Surround Passthrough). |
| `--audio-track <INDEX>` | *Auto* | Select specific audio stream track by 1-based index (e.g. `1` for Commentary, `2` for 5.1). |
| `--audio-lang <LANG>` | *Auto* | Preferred audio track language code (e.g. `eng`, `fre`, `spa`). |
| `--auto-audio-pref <LIST>`| *Auto* | Comma-separated ranked language preference list (e.g. `eng,fre,spa`). |
| `--subtitles` | `false` | Extract DVD subtitle tracks into output container. |
| `--sub-forced-only` | `false` | Extract forced-only subtitle streams (alien/foreign dialogue markers). |
| `--sub-external-srt` | `false` | Extract subtitle stream into a standalone external `.srt` sidecar file. |
| `--sub-default` | `false` | Flag extracted subtitle stream as default in media players (`-disposition:s:0 default`). |
| `--sub-lang <LANG>` | *Auto* | Preferred subtitle language code (e.g. `eng`, `fre`, `spa`). |
| `--sub-format <FMT>` | `dvdsub` | Subtitle stream format: `dvdsub` (bitmap) or `subrip` (`.srt` text format). |
| `--ocr` | `false` | Run Tesseract OCR on bitmap subtitles to output clean, direct-play SubRip `.srt` sidecars. |
| `--ocr-lang <LANG>` | `eng` | Target language code for Tesseract OCR subtitle transcription (e.g. `eng`, `fra`, `spa`). |
| `--tesseract <PATH>` | `tesseract` | Explicit filesystem path to Tesseract OCR executable binary. |
| `--fallback-out-dir <DIR>`| *None* | Secondary storage fallback directory path if primary storage lacks free space. |
| `--no-eject` | `false` | Do not eject optical disc tray upon successful rip completion (ideal for disc autoloaders). |
| `--eject-autoclose <SECS>`| *None* | Delay in seconds before automatically closing optical disc tray after ejection. |
| `--spindown` | `false` | Issue optical drive motor spindown signal upon rip completion to reduce platter noise & wear. |
| `--drive-pool <CSV>` | *Auto* | Comma-separated list of optical drive paths for multi-drive appliance pool monitoring. |
| `--auto-cleanup-days <N>` | *None* | Retention days threshold after which old rip records & temporary artifacts are purged. |
| `--label-regex-replace` | *None* | Custom regex replacement pattern (e.g. `PATTERN:REPLACEMENT`) for automated volume label cleaning. |
| `--nfo` | `false` | Generate Kodi/Jellyfin/Plex XML `.nfo` metadata sidecar file. |
| `--tags <CSV>` | *None* | Comma-separated custom metadata tags (e.g. `4K Remaster,Director Cut`) to embed in `.nfo` XML. |
| `--checksum` | `false` | Generate SHA-256 integrity verification sidecar file (`.sha256`) alongside converted media. |
| `--audit-log <PATH>`| *None* | Write structured JSON-Lines (`.jsonl`) audit event log file for enterprise logging (Splunk, Elastic). |
| `--chapters` | `true` | Extract and preserve DVD chapter timestamp markers into container metadata. |
| `--search <QUERY>` | *None* | Perform online IMDb/OMDb metadata candidate search. |
| `--imdb-id <ID>` | *None* | Explicitly select IMDb ID for metadata resolution (e.g. `tt0090605`). |

---

## 5. TV Series & Multi-Disc Box Set Options

| Flag / Option | Default | Description |
|---|---|---|
| `--tv` | `false` | Enable TV series disc ripping mode. |
| `--season <N>` | `1` | Season number for TV series metadata (e.g. `1` for Season 01). |
| `--start-episode <N>` | `1` | Starting episode number for the first detected episode on disc. |
| `--all-episodes` | `false` | Automatically rip all detected TV episode titles on the disc sequentially. |
| `--auto-boxset` | `false` | Automatically calculate cumulative episode numbering across multi-disc season box sets. |

---

## 6. Safeguards & Appliance Options

| Flag / Option | Default | Description |
|---|---|---|
| `--min-free-gb <GB>` | `10` | Minimum required free disk space threshold (GB) before initiating rip jobs. |
| `--no-overwrite` | `false` | Do not overwrite existing files (automatically append numeric suffix `_1`, `_2`). |
| `--webhook-url <URL>` | *None* | Webhook URL (Discord, Slack, Ntfy, Telegram) for HTTP status POST alerts. |
| `--mqtt-broker <ADDR>` | *None* | MQTT broker address (`mqtt://localhost:1883`) for Home Assistant auto-discovery. |
| `--api-key <KEY>` | *None* | Secret API key required for REST API endpoints. |
| `--auto-rip` | `false` | Enable unattended automated ripping upon optical disc insertion in daemon mode. |
| `--post-script <PATH>` | *None* | Executable script path to run upon rip completion. |

---

## 7. Command Usage Examples

### 1. Basic Movie Rip
```bash
dvd-ripper --input D:\ --search "Aliens" --transcode --codec h264
```

### 2. Multi-Disc TV Series Box Set Rip (Disc 2)
```bash
dvd-ripper --input D:\ --tv --season 1 --auto-boxset --all-episodes --deinterlace
```

### 3. Lossless Archival Copy to MKV
```bash
dvd-ripper --input /dev/sr0 --profile archival --mkv
```

### 4. Optical Drive Speed Benchmark
```bash
dvd-ripper --input D:\ --benchmark
```

### 5. Headless Appliance Daemon with Home Assistant MQTT
```bash
dvd-ripper --daemon --mqtt-broker mqtt://192.168.1.50:1883 --min-free-gb 20
```

### 6. 1:1 Bit-Exact Archival ISO Image Dump
```bash
dvd-ripper --input D:\ --iso --out-dir "E:\ISO_Backups"
```

### 7. Video Rip with Tesseract OCR Subtitles (.srt Sidecar)
```bash
dvd-ripper --input D:\ --search "The Matrix" --ocr --ocr-lang eng --sub-external-srt
```

### 8. Pure Rust Native IFO Extraction (Bypasses FFmpeg Demuxer Bugs)
```bash
dvd-ripper --cli D: --engine native --search "Gladiator"
```

### 9. Lossless MakeMKV Robot Mode Extraction (AACS/CSS Hardware Decryption)
```bash
dvd-ripper --cli D: --engine makemkv --out-dir "D:\Films"
```

### 10. Custom Media Taxonomy (Movies & Series Subfolders)
```bash
dvd-ripper --input D:\ --search "The Matrix" --out-dir "C:\Media" --films-dir "Movies" --tv-dir "Series"
```

For detailed engine architecture and benchmark comparisons, see:  
👉 **[Alternative Ripping Engines Guide](RIPPING_ENGINES.md)**, **[Custom Media Taxonomy Guide](CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md)**, and **[Copy Protection & Bad Sectors Guide](COPY_PROTECTION_AND_BAD_SECTORS.md)**.

---

## 8. Standalone Installer CLI (`dvd-ripper-installer`)

`dvd-ripper` provides a standalone cross-platform installer executable for setup, systemd service deployment, and udev auto-rip configuration.

```bash
dvd-ripper-installer [OPTIONS]
```

| Option | Flag | Description | Default |
|---|---|---|---|
| `--user` | | Install binary for current user | `true` |
| `--system` | | Install binary system-wide (requires Administrator/root) | `false` |
| `--dir <DIR>` | `-d` | Custom destination installation directory | *Auto* |
| `--service` | | Install Linux systemd daemon service and udev disc rules | `false` |
| `--uninstall` | `-u` | Uninstall DVD Ripper, clean binaries and PATH | `false` |
| `--yes` | `-y` | Non-interactive mode (automatically answer yes) | `false` |

