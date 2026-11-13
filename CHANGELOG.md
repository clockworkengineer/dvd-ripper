# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-11-13

### Added
- **Configurable Media Taxonomies & Custom Subfolder Routing**:
  - Made Films and TV destination subfolders fully configurable via `--films-dir` and `--tv-dir` CLI flags, `films_dir` and `tv_dir` configuration settings, and the Desktop GUI Settings tab.
  - Sibling directory auto-resolution dynamically recognizes custom folder names (e.g. `Movies` and `Series`) without creating nested duplicate directories.
  - Added live dynamic destination path preview badge to Desktop GUI Rip tab.
  - Elevated path resolution and hierarchy formatting to pure domain logic (`src/domain/naming.rs`) conforming to Single Responsibility Principle.
- **7 Architectural Supremacy Pillars**:
  - **Optical Drive Acoustic & Thermal Supervisor**: Active SCSI speed negotiation (`IOCTL_CDROM_SET_SPEED`) with 3 profile tiers (`Silent` 4x whisper-quiet, `Balanced` 8x standard, `Turbo` 16x+ maximum) and laser focus-coil thermal rest intervals to prevent optical pickup burnout on damaged discs.
  - **Single-Pass Dual-Output Engine (`StreamTee`)**: Concurrent stream multiplexer writing both a bit-for-bit archival master (`.mkv`/`.iso`) and a lightweight transcoded streaming file (`.mp4`) simultaneously in a single read pass, cutting optical drive wear and laser exposure by 50%.
  - **Cadence-Aware 3:2 Pulldown IVTC Pipeline**: Algorithmic video cadence classification inspecting MPEG-2 Repeat First Field (`RFF`), Top Field First (`TFF`), and progressive sequence flags to perform lossless inverse telecine (`fieldmatch + decimate`), restoring native 23.976p film without cadence judder.
  - **Disc Rosetta Stone Registry**: Deterministic 128-bit optical volume fingerprinting (`compute_optical_rosetta_hash`) and offline canonical metadata store, instantly bypassing multi-hundred playlist decoy traps (Lionsgate, Disney) in 0.001 seconds without scanning.
  - **Autonomous Robotic Homelab Appliance**: Autonomous optical media arrival monitor (`PollingMediaMonitor`) detecting tray load transitions, triggering end-to-end rip/eject lifecycles, and dispatching rich Discord embed cards and Telegram HTML alerts with poster art and duration stats.
  - **Smart Subtitle Palette (CLUT) Normalizer & Text Cleaner**: Automatically detects corrupted, transparent, or low-contrast IFO Color Lookup Tables and normalizes them to broadcast-standard high contrast (`#FFFFFF` text, black border), while cleaning OCR scanning ambiguities (quotes, em-dashes, pronoun misreads).
  - **Universal Embedded Web Dashboard & Live Remote Controller**: Zero-dependency responsive mobile/desktop web console (`http://localhost:8080/dashboard`) with live Server-Sent Events (SSE) telemetry, one-click acoustic profile switching (`POST /api/acoustic`), tray ejection, and title candidate selection.
- **Full DRY Architectural Refactoring**:
  - Unified rip lifecycle recording and notification dispatch via extracted `record_and_notify_outcome`.
  - Consolidated duplicate traits (`HistoryStorage`, `MediaServerNotifierPort`) and standardized NFO generation across use cases.
- **Engineering Documentation & Theoretical Foundations**:
  - Published 4 new comprehensive engineering guides: `CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md`, `MENCODER_AND_LEGACY_CODEC_STUDY.md`, `SECURITY_AND_PATH_TRAVERSAL.md`, and `CROSS_PLATFORM_OPTICAL_IO.md`.
  - Added comprehensive Settings Tab documentation to `GUI_GUIDE.md`.
- **Enterprise Test Expansion**:
  - Expanded test suite to **409 unit tests + 20 doc-tests** with 0 warnings under strict `-D warnings`.

## [0.3.0] - 2026-11-03

### Added
- **Multi-Drive Concurrent Watcher Pool**:
  - Concurrent multi-drive appliance monitoring and simultaneous ripping (`--drive-pool "D:,E:,F:"` or `/dev/sr0,/dev/sr1`).
  - Thread-safe `DrivePoolState` registry with independent per-drive progress tracking, cancellation tokens, and automated tray ejection.
  - Dedicated pool management REST endpoints (`GET /api/pool/status`, `GET /api/drives`, `POST /api/trigger`).
- **Prometheus Telemetry & Health Monitoring**:
  - Standard Prometheus exposition format endpoint (`GET /metrics`) exposing `dvd_ripper_completed_rips_total`, `dvd_ripper_failed_rips_total`, `dvd_ripper_active_jobs`, `dvd_ripper_queued_jobs`, and `dvd_ripper_progress_percent`.
  - Lightweight appliance health probe endpoint (`GET /api/health`).
- **Tesseract OCR Subtitle Extraction**:
  - Optical character recognition engine converting DVD `dvdsub` bitmap overlays into SubRip (`.srt`) sidecars (`--ocr`, `--ocr-lang`, `--tesseract`).
  - Prevents server-side video transcoding on Plex, Jellyfin, and Emby by delivering universal text subtitle streams.
- **REST API & Telemetry Expansion**:
  - Added REST endpoints: `/api/history` (structured ripping history), `/api/history/clear` (history reset), `/api/disc-cache` (inspect cached disc fingerprints), `/api/queue/batch` (batch queue operations), `/api/thumbnail` (frame preview extraction), and `/api/ws` (WebSocket live progress stream).
  - Extended `POST /api/rip` with sector ISO formatting options (`format: "iso"`), audio track selection, and custom transcoding profiles.
- **Enterprise Test Suite & Code Coverage**:
  - Expanded unit and integration test suite to **321 passing tests (0 failures)**.
  - Elevated overall workspace line coverage from **49.51% to 83.15%** (15,915 lines covered).
  - Added fast in-process HTTP mock socket responders, isolated test mutexes, and non-blocking process execution patterns.
- **Modular GUI Architecture**:
  - Refactored Desktop GUI into dedicated component and tab modules under `src/gui/` (`controller.rs`, `state.rs`, `theme.rs`, `components/`, `tabs/`).

### Fixed
- **Optical Drive Fingerprint Null Hash Collision**: Guarded empty/unmounted optical drive hash (`disc_08c643c98b44bb79`) against cache pollution across empty drive lookups.
- **Windows Process Execution Safety**: Eliminated interactive terminal hangs in background script execution by utilizing explicit batch script terminations.
- **Non-blocking Device Probing**: Replaced slow OS optical device timeouts on missing media paths with instantaneous mock validation.

## [0.2.0] - 2026-10-21

### Added
- **1:1 Archival ISO Mode**: Sector-by-sector raw optical disc imaging (`--iso`) across CLI, Desktop GUI, and Web REST API (`POST /api/rip` with `format: "iso"`), featuring real-time sector copy progress and cancellation support.
- **Clean Architecture & SOLID Unification**:
  - Domain entities (`FilmMetadata`, `RipRecord`, `DvdError`, `RipJob`, `OpticalMedia`) isolated in `src/domain/`.
  - Formal Ports & Adapters interfaces (`MetadataProvider`, `DriveDetector`, `Transcoder`, `Notifier`, `HistoryRepository`, `DiskSpaceGuard`).
  - Dynamic dependency injection container `AppContainer` wiring use case coordinators (`RipSingleTitleUseCase`, `RipTvBatchUseCase`, `ProbeDiscUseCase`) directly into CLI and GUI runtimes.
- **Automated Multi-OS CI/CD Pipeline**:
  - Matrix testing on Windows (`windows-latest`), Linux (`ubuntu-latest`), and macOS (`macos-latest`).
  - Enforcement of `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, headless `cargo clippy --no-default-features`, and `cargo test`.
  - Automated release pipeline cross-compiling release binaries for `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, and `aarch64-apple-darwin` with `SHA256SUMS.txt` cryptographic digests.
- **Integration Test Suite**: `tests/cli_tests.rs` covering `--help`, `--version`, and graceful error handling for missing drives across binaries.
- **Container Appliance & NAS Deployment**:
  - Multi-stage `Dockerfile` producing minimal Debian Bookworm runtime image for automated headless ripping on Unraid, TrueNAS, and Synology.
  - `docker-compose.yml` with `/dev/sr0` optical drive passthrough and persistent volume mappings.
- **Installer & PATH Scoping**:
  - Enhanced `dvd-ripper-installer` with system vs. user PATH scope selection on Windows (`Machine` registry scope when `--system` is specified).
  - Standalone Unix quick-installer script in `scripts/install.sh` for one-line deployment.
  - Debian package configuration (`cargo-deb`) in `Cargo.toml`.

### Fixed
- **Headless Build Orthogonality**: Gated GUI dependencies (`rfd::FileDialog`, `eframe`, `image`) behind `cfg(feature = "gui")` to guarantee clean compilation with `--no-default-features`.
- **PowerShell COM Quote Escaping**: Escaped single quotes in optical volume labels during Windows WMI/COM disc queries.
- **Linter & Warning Cleanliness**: Resolved all clippy warnings across all compilation targets and feature flags under `-D warnings`.
- **Repository Metadata**: Corrected repository and documentation URLs to `https://github.com/clockworkengineer/dvd-ripper`.

---

## [0.1.0] - 2026-10-14

### Added
- Native portable Desktop GUI built with `eframe` / `egui`.
- CLI engine utilizing FFmpeg `dvdvideo` demuxer.
- Automated OMDb and IMDb metadata fetching and cover artwork caching.
- Multi-episode TV series detection and TVMaze auto-naming.
- TV box set multi-disc tracking and cumulative episode numbering.
- Video deinterlacing (`bwdif`, `yadif`, `w3fdif`) and 3D spatial/temporal denoising (`hqdn3d`).
- Hardware accelerated transcoding (NVENC, QSV, VAAPI) for H.264, HEVC, and AV1.
- Embedded HTTP REST API and HTML5 live dashboard on port 8080.
- Telemetry notifiers: Webhook (JSON) and Home Assistant MQTT.
- On-the-fly subtitle OCR generating external SubRip (`.srt`) sidecars.
