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
| `--out-dir <DIR>` | `Films` | Base output root directory for movies (`Films`) or TV series (`TV`). |
| `--title <NUM>` | `0` | DVD title track index to rip (`0` = auto-detect title with matching movie runtime or longest duration). |
| `--config <PATH>` | *Auto* | Path to custom TOML configuration file. |
| `--cli` | `false` | Force headless command-line execution (suppress GUI launch). |
| `--daemon` | `false` | Run in background appliance daemon mode with REST API server & Home Assistant integration. |
| `--check-protection` | `false` | Run copy protection (CSS/CPPM) and bad-sector diagnostic scan on disc. |
| `--benchmark` | `false` | Run 10-second optical sector read speed throughput test (MB/s) and exit. |

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
| `--denoise` | `false` | Apply 3D spatial/temporal denoising filter (`-vf hqdn3d`) to reduce film grain and analog noise. |
| `--sub-burnin` | `false` | Hard-burn subtitle text overlay directly onto video frames (`-vf subtitles=...`) for maximum compatibility. |
| `--mkv` | `false` | Force Matroska (`.mkv`) output container format instead of MP4 (`.mp4`). |

---

## 4. Audio, Subtitles & Metadata Options

| Flag / Option | Default | Description |
|---|---|---|
| `--all-audio` | `false` | Include all audio streams present on the DVD title track. |
| `--normalize-audio` | `false` | Apply EBU R128 loudness normalization (`-filter:a loudnorm=I=<TARGET>:TP=-1.5:LRA=11`). |
| `--norm-target <LUFS>` | `-16` | Configurable target LUFS loudness value (e.g. `-16`, `-14`, `-23 LUFS`). |
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
| `--fallback-out-dir <DIR>`| *None* | Secondary storage fallback directory path if primary storage lacks free space. |
| `--no-eject` | `false` | Do not eject optical disc tray upon successful rip completion (ideal for disc autoloaders). |
| `--eject-autoclose <SECS>`| *None* | Delay in seconds before automatically closing optical disc tray after ejection. |
| `--spindown` | `false` | Issue optical drive motor spindown signal upon rip completion to reduce platter noise & wear. |
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

