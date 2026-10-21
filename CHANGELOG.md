# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
