# Developer Onboarding & Architecture Guide - `dvd-ripper`

Welcome to the `dvd-ripper` developer documentation. This guide details repository layout, compilation workflows, module boundaries, adding new features, writing unit tests, and coding conventions.

---

## 1. Repository Layout & Crate Structure

```text
dvd-ripper/
├── Cargo.toml               # Crate dependencies & build targets
├── src/
│   ├── main.rs              # Application entry point & CLI orchestration
│   ├── cli.rs               # Args CLI schema & EncodingOptions builder
│   ├── config.rs            # TOML config parser & default application
│   ├── dvd.rs               # Drive detection, ISO-9660 reader, protection & benchmark engine
│   ├── ffmpeg.rs            # FFmpeg command assembly, video filter graphs & progress parser
│   ├── gui.rs               # Desktop immediate-mode GUI (eframe/egui)
│   ├── api.rs               # Embedded REST API, SSE streaming, & OpenAPI spec
│   ├── queue.rs             # Job queue & TV BoxSet manager (boxsets.json)
│   ├── daemon.rs            # Multi-drive monitoring watcher daemon
│   ├── imdb.rs              # TMDB/OMDb API client & fingerprint cache
│   ├── mqtt.rs              # Binary MQTT 3.1.1 encoder & webhook notifications
│   ├── history.rs           # Rip history database (ripping_history.json)
│   ├── utils.rs             # Formatting, disk space guard, & media triggers
│   └── bin/
│       └── installer.rs     # Cross-platform installer binary (dvd-ripper-installer)
├── docs/                    # Technical & user documentation manuals (13 guides)
│   ├── API_REFERENCE.md
│   ├── ARCHITECTURE.md
│   ├── CLI_GUIDE.md
│   ├── CONFIGURATION.md
│   ├── DAEMON_APPLIANCE_MODE.md
│   ├── DEVELOPER_GUIDE.md
│   ├── GUI_GUIDE.md
│   ├── HARDWARE_ACCELERATION.md
│   ├── INSTALLER_AND_SERVICE_GUIDE.md
│   ├── MEDIA_SERVER_INTEGRATION.md
│   ├── RIPPING_HISTORY_AND_LOGGING.md
│   ├── TROUBLESHOOTING.md
│   └── TV_BOXSET_GUIDE.md
└── contrib/                 # Systemd service unit & udev rules
```

---

## 2. Building & Running

### Build Application Binaries
```bash
# Debug build (dvd-ripper binary)
cargo build

# Release build with binary size optimizations
cargo build --release

# Build standalone cross-platform installer binary
cargo build --release --bin dvd-ripper-installer
```

### Run Unit Test Suite
```bash
cargo test
```

---

## 3. Extending Functionality

### Adding a New FFmpeg Video Filter
1. Update `Args` in [`src/cli.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/cli.rs) with new CLI argument flags.
2. Update `AppConfig` in [`src/config.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/config.rs) for TOML config support.
3. Update `build_ffmpeg_command` in [`src/ffmpeg.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/ffmpeg.rs) to append the filter string to `vf_filters`.
4. Add a unit test in `ffmpeg::tests`.

### Adding a New REST API Route
1. Open [`src/api.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/api.rs).
2. Add route matching logic inside `handle_http_connection()`.
3. Use `send_json_response()` or `send_json_error()` to format outputs.
4. Update `build_openapi_spec()` JSON string to keep the OpenAPI specification synchronized.

### Adding a New Desktop GUI Control
1. Open [`src/gui.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/gui.rs).
2. Update state struct `DvdRipperApp` with new parameters.
3. Implement `egui` UI widget controls inside `update()`.
4. Update `GUI_GUIDE.md` manual.

