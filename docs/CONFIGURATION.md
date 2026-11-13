# Configuration Guide - `dvd-ripper`

`dvd-ripper` can be configured via command-line arguments, environment variables, or a persistent TOML configuration file (`dvd-ripper.toml` or `~/.dvd-ripper/config.toml`).

---

## 1. Configuration Priority Order

When determining application settings, `dvd-ripper` evaluates parameters in the following priority order:
1. **Command-Line Arguments (Highest Priority)**: Any flag passed on the command line overrides all configuration files.
2. **Custom TOML Config File**: File passed via `--config <PATH>`.
3. **Local TOML Config File**: `./dvd-ripper.toml` in the current working directory.
4. **User Home TOML Config File**: `~/.dvd-ripper/config.toml` in the user's home directory.
5. **Built-in Defaults (Lowest Priority)**: Hardcoded application default values.

---

## 2. TOML Configuration File (`dvd-ripper.toml`)

Save a `dvd-ripper.toml` file in your working directory or in `~/.dvd-ripper/config.toml`:

```toml
# ==============================================================================
# DVD-RIPPER CONFIGURATION SPECIFICATION (v0.3.0)
# ==============================================================================

# --- Base Directory & Output Architecture ---
# Base output directory (movies are routed to /Films and TV series to /TV)
# Supports sibling auto-resolution (e.g. C:\Media\Films routes TV shows to sibling C:\Media\TV)
out_dir = "Films"
# Secondary fallback directory if primary storage drops below min_free_gb
# fallback_out_dir = "E:\\BackupMedia"

# --- Transcoding & Remuxing Engines ---
# Transcoding engine selection: "auto", "ffmpeg", "native", "handbrake", or "makemkv"
engine = "auto"
# Single-pass dual output: concurrently creates archival remux (.mkv/.iso) and streaming target (.mp4)
dual_output = false
# Optical drive acoustic and spindle speed profile: "silent" (4x), "balanced" (8x), or "turbo" (16x+)
acoustic_profile = "balanced"
# Custom binary executable path for HandBrakeCLI
handbrake_path = "HandBrakeCLI"
# Custom binary executable path for MakeMKV (makemkvcon)
makemkv_path = "makemkvcon"

# --- Video Encoding & Containers ---
# Preferred video codec: "h264", "hevc", "av1", or "copy"
codec = "h264"
# Transcoding profile preset: "standard", "archival", "plex", or "mobile"
profile = "standard"
# FFmpeg CPU speed preset: "ultrafast", "superfast", "veryfast", "fast", "medium"
preset = "veryfast"
# 1:1 sector-by-sector raw ISO disc copy mode (overrides transcode)
iso = false
# Fault-tolerant demuxing for scratched discs & copy protection bad sectors
ignore_errors = false
# HDR-to-SDR tonemapping algorithm: "mobius", "hable", "reinhard"
# tonemap = "mobius"

# --- Video Filters & Deinterlacing ---
# Motion-adaptive video deinterlacing filter
deinterlace = true
# Algorithm: "bwdif" (best quality), "yadif", "w3fdif"
deinterlace_algo = "bwdif"
# 3D spatial/temporal denoising (hqdn3d)
denoise = true

# --- Audio Tracks & Loudness Normalization ---
# Specific 1-based audio stream index to extract
# audio_track = 1
# EBU R128 loudness normalization target LUFS (e.g. -16, -14, -23)
norm_target = -16
# Channel downmix mode: "stereo", "dolphylogic", "headphone", "mono"
# audio_downmix = "stereo"

# --- Subtitle & OCR Configuration ---
# Hard-burn subtitles directly onto video frames
sub_burnin = false
# Extract forced-only subtitle streams (dialogue in foreign languages)
sub_forced_only = false
# Save extracted subtitles as an external .srt sidecar file
sub_external_srt = true
# Mark subtitle track as default stream in media containers
sub_default = false
# On-the-fly Subtitle OCR using Tesseract (converts bitmap dvdsub into text .srt)
ocr = true
# Tesseract OCR language code (e.g. "eng", "fra", "deu", "spa")
ocr_lang = "eng"
# Custom binary path to Tesseract OCR executable
tesseract = "tesseract"

# --- Multi-Disc TV Box Sets ---
# Cumulative episode numbering across multi-disc box sets (~/.dvd-ripper/boxsets.json)
auto_boxset = true

# --- Multi-Drive Daemon Pool & Automation ---
# Comma-separated optical drive paths for daemon pooling
drive_pool = "D:,E:,F:"
# Unattended auto-ripping upon disc insertion
auto_rip = false

# --- Hardware & Optical Tray Control ---
# Minimum required free disk space threshold (GB) before initiating rip
min_free_gb = 10
# Suppress optical tray ejection upon job completion
no_eject = false
# Auto-close optical tray after ejection delay in seconds
# eject_autoclose = 15
# Issue motor spindown signal upon rip completion to reduce noise & wear
spindown = true

# --- Metadata, Sidecars & Verification ---
# Generate SHA-256 integrity hash sidecar (.sha256) alongside video
checksum = true
# Custom metadata tags to embed in .nfo XML (e.g. "Director Cut,4K Remaster")
# tags = "Archival,Remaster"
# Structured JSON-Lines audit log filepath
# audit_log = "rip_audit.jsonl"
# Regex replacement pattern for volume labels (PATTERN:REPLACEMENT)
# label_regex_replace = "_: "
# Retention threshold in days to clean up old rip records
auto_cleanup_days = 30

# --- Telemetry, Smart Home & Web Server ---
# Bearer token API key for HTTP REST endpoints
api_key = "secret_api_key_123"
# Smart Home MQTT broker (Home Assistant auto-discovery)
mqtt_broker = "192.168.1.50:1883"
# Webhook notification URL (Discord, Slack, Ntfy, Telegram, Gotify)
webhook_url = "https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN"

# --- Direct Media Server Library Refresh ---
plex_url = "http://192.168.1.100:32400"
plex_token = "YOUR_PLEX_TOKEN"
jellyfin_url = "http://192.168.1.100:8096"
jellyfin_key = "YOUR_JELLYFIN_KEY"
emby_url = "http://192.168.1.100:8096"
emby_key = "YOUR_EMBY_KEY"

# Post-processing script hook executable path
post_script = "/usr/local/bin/post_rip_handler.sh"
```

---

## 3. Command-Line Reference

| Flag | Short | Description | Default |
|---|---|---|---|
| `--input <PATH>` | `-i` | Input optical DVD drive path or ISO directory | `"auto"` / `"D:\"` |
| `--output <PATH>` | `-o` | Custom destination file path | `None` |
| `--out-dir <PATH>` | `-d` | Custom output folder directory | `"Films"` / `"TV"` |
| `--config <PATH>` | `-c` | Custom TOML configuration file path | `dvd-ripper.toml` |
| `--codec <CODEC>` | | Video codec choice (`h264`, `hevc`, `av1`, `copy`) | `"h264"` |
| `--profile <PRESET>` | | Transcoding profile (`standard`, `archival`, `plex`, `mobile`) | `"standard"` |
| `--preset <PRESET>` | | FFmpeg CPU speed preset (`veryfast`, `fast`, `medium`) | `"veryfast"` |
| `--deinterlace` | | Motion-adaptive video deinterlacing filter | `false` |
| `--deinterlace-algo <ALGO>` | | Deinterlacing algorithm (`bwdif`, `yadif`, `w3fdif`) | `"bwdif"` |
| `--denoise` | | 3D spatial/temporal noise reduction filter (`hqdn3d`) | `false` |
| `--min-free-gb <GB>` | | Minimum free disk space threshold (GB) | `10` |
| `--auto-boxset` | | Auto-calculate episode numbering across multi-disc box sets | `false` |
| `--benchmark` | | Run 10-second optical sector read speed throughput test | `false` |
| `--sub-format <FMT>` | | Subtitle stream codec format (`dvdsub` or `subrip`/`srt`) | `"dvdsub"` |
| `--sub-external-srt` | | Save extracted subtitle stream as a standalone external `.srt` sidecar | `false` |
| `--ocr` | | Convert DVD bitmap subtitles into clean text `.srt` using Tesseract OCR | `false` |
| `--ocr-lang <LANG>` | | Target language code for subtitle OCR extraction | `"eng"` |
| `--tesseract <PATH>` | | Custom executable path to Tesseract OCR binary | `"tesseract"` |
| `--drive-pool <DRIVES>` | | Comma-separated list of drives for concurrent daemon monitoring | `None` |
| `--auto-rip` | | Unattended automated ripping upon optical disc insertion | `false` |
| `--normalize-audio` | | EBU R128 audio loudness normalization filter | `false` |
| `--dual-audio` | | Dual-track output (Stereo AAC + 5.1 Passthrough) | `false` |
| `--daemon` | | Launch headless multi-drive monitoring daemon | `false` |
| `--cli` | | Force command-line terminal mode instead of native GUI | `false` |
| `--mqtt-broker <ADDR>` | | MQTT broker address for Home Assistant telemetry | `None` |
| `--webhook-url <URL>` | | HTTP JSON webhook notification URL | `None` |
| `--api-key <KEY>` | | Bearer token API key for HTTP REST endpoints | `None` |
| `--post-script <PATH>` | | Post-processing script hook binary executable path | `None` |
| `--plex-url <URL>` | | Plex server base URL for library scan refresh | `None` |
| `--plex-token <TOKEN>` | | Plex authentication token | `None` |

---

## 4. Post-Processing Script Hooks

When `--post-script <PATH>` or `post_script = "..."` is configured, `dvd-ripper` invokes the external binary/script upon rip completion, injecting environment variables:

- `DVD_OUTPUT_PATH`: Absolute filepath of the newly ripped media file (e.g. `/Films/Aliens (1986)/Aliens (1986).mkv`).
- `DVD_TITLE`: Title name of the media entry (e.g. `Aliens`).
- `DVD_MEDIA_TYPE`: Media classification (`Movie` or `TV Series`).
- `DVD_YEAR`: Release year (e.g. `1986`).

### Example Shell Hook Script (`post_rip_handler.sh`)
```bash
#!/usr/bin/env bash
echo "New backup created: $DVD_TITLE ($DVD_YEAR)"
echo "File path: $DVD_OUTPUT_PATH"

# Send desktop notification
notify-send "DVD Ripper" "Backup completed for $DVD_TITLE"
```

---

## 5. Daemon Appliance & Home Automation Setup

For complete instructions on configuring `dvd-ripper` in background appliance daemon mode, Home Assistant MQTT auto-discovery, HTTP Webhooks, and systemd service deployment, consult the [`DAEMON_APPLIANCE_MODE.md`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/docs/DAEMON_APPLIANCE_MODE.md) guide.

