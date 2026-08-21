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
# Base output directory (default: "Films")
out_dir = "Films"

# Preferred video codec: "h264", "hevc", "av1", or "copy"
codec = "h264"

# Transcoding profile preset: "standard", "archival", "plex", or "mobile"
profile = "standard"

# FFmpeg CPU speed preset: "ultrafast", "superfast", "veryfast", "fast", "medium"
preset = "veryfast"

# Smart Home MQTT broker (Home Assistant auto-discovery)
mqtt_broker = "192.168.1.50:1883"

# Webhook notification URL (Discord, Ntfy, Telegram, Gotify, Slack)
webhook_url = "https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN"

# REST API Bearer Token Key
api_key = "secret_api_key_123"

# Direct Media Server Scan Triggers
plex_url = "http://192.168.1.100:32400"
plex_token = "YOUR_PLEX_TOKEN"

jellyfin_url = "http://192.168.1.100:8096"
jellyfin_key = "YOUR_JELLYFIN_KEY"

emby_url = "http://192.168.1.100:8096"
emby_key = "YOUR_EMBY_KEY"

# Post-processing script hook executable path
post_script = "/usr/local/bin/post_rip_handler.sh"

# Multi-Disc TV Series Box Set Auto-Stitching Mode
auto_boxset = true

# Video Deinterlacing & Denoising Quality Suite
deinterlace = true
deinterlace_algo = "bwdif"
denoise = true

# Minimum required free disk space threshold (GB) before ripping
min_free_gb = 10
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

