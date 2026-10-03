# Architectural Analysis & Concrete SOLID Refactor Plan for DVD Ripper

**Project**: DVD Ripper (Portable Multi-OS Movies & TV Series Backup Appliance)  
**Document Version**: 1.0.0  
**Status**: Ready for Implementation  
**Target Architecture**: Clean Architecture & SOLID Domain-Driven Rust Design  

---

## 1. Executive Summary & Codebase Audit

An architectural audit of the `dvd-ripper` codebase reveals a feature-rich, high-performance utility (supporting DVD decryption, ScreenPass/ARccOS copy-protection cluster heuristics, multi-audio ranking, OCR bitmap conversion, embedded web dashboard, SSE streaming, Prometheus metrics, and desktop GUI). 

However, rapid feature evolution has resulted in significant architectural coupling, oversized modules, and violations of the five SOLID principles.

### Key Architectural Metrics:
- **Total Rust Code**: ~8,000 lines across 15 files in `src/`.
- **Largest Files**:
  - `src/api.rs`: 2,027 lines (raw HTTP sockets, SSE, OpenAPI, HTML/JS SPA, pool management, ripping threads).
  - `src/ffmpeg.rs`: 1,913 lines (CLI construction, process pipes, duration heuristics, IFO binary parsing, cluster ranking).
  - `src/gui.rs`: 1,510 lines (eframe/egui layout, threading, network fetching, image decoding, OS explorer launching, config persistence).
  - `src/utils.rs`: 1,262 lines (15+ unrelated concerns from XML generation to disk guards).
- **Unit Test Suite**: 145 unit tests across main binary and installer (0 failures).

---

## 2. SOLID Principle Violations in Current Codebase

```
+-----------------------------------------------------------------------------------+
|                            CURRENT MONOLITHIC COUPLING                             |
+-----------------------------------------------------------------------------------+
|  [main.rs / gui.rs / api.rs / daemon.rs]                                          |
|         │                        │                     │                          |
|         ▼                        ▼                     ▼                          |
|   Fat Args Struct        Procedural Omdb         Procedural FFmpeg                |
|      (50+ fields)         (direct reqwest)       (child pipes, IFO, heuristics)   |
|         │                        │                     │                          |
|         └────────────────────────┼─────────────────────┘                          |
|                                  ▼                                                |
|                      Scattered utils.rs (1260 LOC)                                |
|             (XML, disk, webhooks, explorer, formatters)                           |
+-----------------------------------------------------------------------------------+
```

### 2.1 Single Responsibility Principle (SRP)
> *"A module should be responsible to one, and only one, actor or reason to change."*

| Module | Current Mixed Responsibilities | Proposed Decoupling |
| :--- | :--- | :--- |
| **[`src/ffmpeg.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/ffmpeg.rs)** | 1. Command-line builder<br>2. Process lifecycle & pipe I/O<br>3. Stderr progress string parser<br>4. Fast probe vs. binary IFO parser<br>5. Copy-protection clustering heuristics<br>6. Output path directory containment | Split into:<br>• `domain::transcoder`<br>• `infrastructure::ffmpeg_builder`<br>• `infrastructure::ffmpeg_runner`<br>• `domain::dvd_prober`<br>• `infrastructure::ifo_parser` |
| **[`src/gui.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/gui.rs)** | 1. Immediate-mode GUI layout<br>2. Thread orchestration<br>3. Network poster download<br>4. Image buffer decoding<br>5. OS folder dialog & Explorer spawning<br>6. Config & History persistence | Split into:<br>• `interfaces::gui::views`<br>• `interfaces::gui::state`<br>• `infrastructure::dialog_service`<br>• `infrastructure::system_explorer`<br>• `application::rip_orchestrator` |
| **[`src/api.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/api.rs)** | 1. Custom HTTP socket engine<br>2. Route pattern matching<br>3. SSE event pump<br>4. Prometheus renderer<br>5. OpenAPI JSON generator<br>6. Inline HTML/CSS/JS dashboard string<br>7. Appliance drive pooling | Split into:<br>• `interfaces::http::server`<br>• `interfaces::http::routes`<br>• `interfaces::http::sse`<br>• `interfaces::http::metrics`<br>• `interfaces::http::openapi`<br>• `interfaces::http::web_assets` |
| **[`src/utils.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/utils.rs)** | 1. Disk space guard<br>2. Atomic file write<br>3. Kodi NFO XML generator<br>4. Webhook HTTP POST sender<br>5. Media server triggers (Plex/Jellyfin/Emby)<br>6. Duration formatters | Split into:<br>• `domain::disk_service`<br>• `infrastructure::nfo_writer`<br>• `infrastructure::notifiers`<br>• `infrastructure::media_servers`<br>• `domain::formatters` |

---

### 2.2 Open/Closed Principle (OCP)
> *"Software entities should be open for extension, but closed for modification."*

- **Metadata Providers**: Currently, only OMDb/IMDb is supported procedural logic in [`src/imdb.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/imdb.rs). Adding TheMovieDb (TMDb), TVDb, or local metadata sidecars requires modifying `lookup_film_details` and `fetch_search_candidates`.
- **Notification Services**: Notifications are dispatched through scattered procedural functions (Discord/Slack webhook in `utils.rs`, MQTT in `mqtt.rs`, Plex/Jellyfin in `utils.rs`). Adding Matrix, Telegram, Email, or Gotify requires editing caller match expressions.
- **Transcode Engines**: Hard-coded directly to FFmpeg CLI subprocesses. Testing or swapping with a native library or mock requires modifying `run_ffmpeg_with_channel`.

---

### 2.3 Liskov Substitution Principle (LSP)
> *"Subtypes must be substitutable for their base types without altering program correctness."*

- Trait abstractions like `ConfigRepository` or `PathResolver` in [`src/config.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/config.rs) and [`src/main.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/main.rs) are partial stubs and not used throughout core paths.
- All domain abstractions must have strictly interchangeable implementations:
  - `MockTranscoder` can replace `FfmpegTranscoder` in all unit and integration tests without process spawning.
  - `MockMetadataProvider` can replace `OmdbMetadataProvider` without outbound network calls.
  - `VirtualDiscDrive` can substitute `PhysicalOpticalDrive` for testing protected ISOs or virtual drives.

---

### 2.4 Interface Segregation Principle (ISP)
> *"Clients should not be forced to depend on interfaces or data structures they do not use."*

- **The Fat `Args` Structure**: The [`Args`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/cli.rs) struct contains 54 CLI flags. Functions like `resolve_output_path` or `probe_dvd_titles_fast` take `&Args` or require assembling a mock `Args` with 50 unused fields.
- **Segregated Configuration Interfaces**:
  - `TranscodeConfig`: Codec, profile, preset, hwaccel, deinterlace, denoise.
  - `AudioSubtitleConfig`: Languages, normalizations, downmix, OCR, subtitles.
  - `StorageConfig`: Output root dir, fallback dir, overwrite policy, min free space.
  - `IntegrationConfig`: Plex, Jellyfin, Emby, Webhooks, MQTT.

---

### 2.5 Dependency Inversion Principle (DIP)
> *"High-level modules should not depend on low-level modules. Both should depend on abstractions."*

- Currently, high-level UI controllers (`DvdRipperApp`) and HTTP controllers depend directly on concrete subprocess spawners, concrete network sockets, and concrete file system paths.
- In the refactored architecture, all high-level use cases depend solely on domain interfaces (`MetadataProvider`, `Transcoder`, `DiscDrive`, `ConfigStore`, `RipHistoryStore`), which are injected at application bootstrap.

---

## 3. Target SOLID Architecture (Clean / Hexagonal)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             PRESENTATION LAYER                              │
│  [Desktop GUI (eframe)]    [CLI Command Engine]    [Web Appliance & API]    │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              APPLICATION LAYER                              │
│  [RipMediaUseCase]   [BatchTvRipUseCase]   [DiscProbeUseCase]   [DrivePool] │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                DOMAIN LAYER                                 │
│  Entities:     MediaItem, DiscInfo, TitleTrack, RipJob, RipProgress         │
│  Traits (DIP): Transcoder, MetadataProvider, DiscDrive, Notifier,          │
│                ConfigRepository, HistoryRepository, FileDialogService       │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            INFRASTRUCTURE LAYER                             │
│  Transcoder:        FfmpegTranscoder (Builder, StderrParser, ChildRunner)  │
│  Metadata:          OmdbProvider, TmdbProvider, OfflineCacheProvider        │
│  Hardware/IO:       WindowsOpticalDrive, LinuxOpticalDrive, SystemExplorer  │
│  Notifications:     WebhookNotifier, MqttNotifier, MediaServerNotifier      │
│  Storage:           TomlConfigStore, JsonHistoryStore, AtomicFileWriter     │
│  Subtitles:         TesseractOcrEngine                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Module Decomposition & Proposed Directory Layout

```
src/
├── domain/                      # Pure Business Rules (No external dependencies)
│   ├── mod.rs
│   ├── entities/
│   │   ├── mod.rs
│   │   ├── media.rs             # MediaMetadata, MediaType, SearchCandidate
│   │   ├── disc.rs              # DiscInfo, TitleTrack, Chapter, CopyProtectionReport
│   │   └── job.rs               # RipJob, JobStatus, ProgressState, EncodingProfile
│   ├── ports/                   # Inversion Interfaces (Traits)
│   │   ├── mod.rs
│   │   ├── transcoder.rs        # Transcoder, ProgressObserver, CommandBuilder
│   │   ├── metadata.rs          # MetadataProvider, CacheRepository
│   │   ├── optical_drive.rs     # OpticalDrive, DriveDetector
│   │   ├── notification.rs      # Notifier, EventPublisher
│   │   ├── storage.rs           # ConfigRepository, HistoryRepository, DiskGuard
│   │   └── dialogs.rs           # FolderPicker, MessageDialog
│   └── heuristics/              # Domain Algorithms
│       ├── mod.rs
│       ├── cluster_detector.rs  # Title cluster ranking & bad-sector decoy bypass
│       └── speedup.rs           # PAL 25fps vs NTSC 24fps duration math
│
├── application/                 # Use Cases & Orchestration
│   ├── mod.rs
│   ├── rip_single_title.rs      # Single Movie / Episode Ripping Pipeline
│   ├── rip_tv_batch.rs          # Sequential Box Set Multi-Episode Pipeline
│   ├── probe_disc.rs            # Probing titles, IFO parsing, metadata matching
│   ├── benchmark_drive.rs       # Optical throughput testing
│   └── drive_pool.rs            # Appliance multi-drive coordination & cancel flags
│
├── infrastructure/              # Concrete Implementations of Domain Ports
│   ├── mod.rs
│   ├── ffmpeg/
│   │   ├── mod.rs
│   │   ├── builder.rs           # Pure FFmpeg argument construction
│   │   ├── process_runner.rs    # Process spawning, stderr streaming, cancellation
│   │   ├── progress_parser.rs   # Regex/token parsing of time, fps, speed
│   │   └── ifo_parser.rs        # Direct binary IFO header reading
│   ├── metadata/
│   │   ├── mod.rs
│   │   ├── omdb.rs              # OMDb HTTP API integration
│   │   └── cache.rs             # Fingerprint disk cache implementation
│   ├── optical/
│   │   ├── mod.rs
│   │   ├── windows.rs           # Win32 drive detection, IOCTL eject
│   │   └── unix.rs              # Linux sysfs/udev drive detection
│   ├── notifications/
│   │   ├── mod.rs
│   │   ├── webhook.rs           # Discord / Slack / Ntfy JSON POST
│   │   ├── mqtt.rs              # MQTT 3.1.1 byte packet publisher
│   │   └── media_servers.rs     # Plex / Jellyfin / Emby refresh hooks
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── toml_config.rs       # ~/.dvd-ripper/config.toml
│   │   ├── json_history.rs      # ~/.dvd-ripper/history.json
│   │   ├── disk_guard.rs        # Free space checks & fallback directory
│   │   └── nfo_writer.rs        # Kodi/Jellyfin XML sidecar writer
│   ├── ocr/
│   │   ├── mod.rs
│   │   └── tesseract.rs         # Bitmap to SRT OCR conversion
│   └── system/
│       ├── mod.rs
│       ├── file_dialog.rs       # rfd native folder dialog
│       └── explorer.rs          # Native file manager opener
│
├── interfaces/                  # Entry Points & User Presentations
│   ├── mod.rs
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── args.rs              # Clap CLI definitions (segregated slices)
│   │   └── runner.rs            # CLI execution flow
│   ├── gui/
│   │   ├── mod.rs
│   │   ├── app.rs               # eframe::App lifecycle
│   │   ├── components/          # Reusable egui widgets
│   │   │   ├── mod.rs
│   │   │   ├── drive_selector.rs
│   │   │   ├── metadata_card.rs
│   │   │   ├── progress_panel.rs
│   │   │   └── settings_grid.rs
│   │   └── state.rs             # GUI Reactive State Machine
│   ├── http/
│   │   ├── mod.rs
│   │   ├── server.rs            # HTTP Socket & Connection Listener
│   │   ├── router.rs            # Route dispatcher
│   │   ├── controllers.rs       # REST API endpoints
│   │   ├── sse.rs               # Server-Sent Events stream
│   │   ├── metrics.rs           # Prometheus metrics renderer
│   │   └── assets.rs            # Embedded HTML/CSS/JS web dashboard
│   └── daemon/
│       ├── mod.rs
│       └── watcher.rs           # Optical disc auto-insertion watcher loop
│
└── main.rs                      # Composition Root (Dependency Injection Bootstrap)
```

---

## 5. Core Domain Traits (The Dependency Inversion Contracts)

### 5.1 Transcoder Port (`domain/ports/transcoder.rs`)
```rust
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use anyhow::Result;

pub struct TranscodeJob<'a> {
    pub source_dvd: &'a Path,
    pub title_number: u32,
    pub output_path: &'a Path,
    pub display_title: &'a str,
    pub expected_duration_secs: Option<f64>,
    pub encoding_options: &'a crate::domain::entities::job::EncodingOptions,
}

pub trait ProgressObserver: Send + Sync {
    fn on_progress(&self, percent: f64, fps: &str, speed: &str);
    fn on_log(&self, line: &str);
}

pub trait Transcoder: Send + Sync {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()>;
}
```

### 5.2 Metadata Port (`domain/ports/metadata.rs`)
```rust
use anyhow::Result;
use crate::domain::entities::media::{FilmMetadata, SearchCandidate};

pub trait MetadataProvider: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<SearchCandidate>>;
    fn get_details_by_id(&self, id: &str) -> Result<Option<FilmMetadata>>;
    fn get_details_by_title_and_year(&self, title: &str, year: Option<u32>) -> Result<Option<FilmMetadata>>;
}
```

### 5.3 Optical Drive Port (`domain/ports/optical_drive.rs`)
```rust
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::domain::entities::disc::{DiscCopyProtectionReport, DvdTitleInfo};

pub trait OpticalDrive: Send + Sync {
    fn get_path(&self) -> &Path;
    fn read_volume_label(&self) -> Option<String>;
    fn probe_titles(&self) -> Vec<DvdTitleInfo>;
    fn inspect_protection(&self) -> DiscCopyProtectionReport;
    fn eject(&self) -> Result<()>;
}

pub trait DriveDetector: Send + Sync {
    fn detect_available_drives(&self) -> Vec<PathBuf>;
    fn auto_detect_primary(&self) -> Option<PathBuf>;
}
```

### 5.4 Notification Port (`domain/ports/notification.rs`)
```rust
use anyhow::Result;

pub enum NotificationEvent<'a> {
    RipStarted { title: &'a str, drive: &'a str },
    RipProgress { title: &'a str, percent: f64 },
    RipSuccess { title: &'a str, output_file: &'a str, duration_secs: f64 },
    RipFailed { title: &'a str, error_message: &'a str },
    RipCancelled { title: &'a str },
}

pub trait Notifier: Send + Sync {
    fn send(&self, event: &NotificationEvent) -> Result<()>;
}
```

### 5.5 Configuration & Dialog Ports (`domain/ports/storage.rs`, `dialogs.rs`)
```rust
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::domain::entities::job::AppConfig;

pub trait ConfigRepository: Send + Sync {
    fn load_config(&self, custom_path: Option<&Path>) -> AppConfig;
    fn save_config(&self, config: &AppConfig) -> Result<()>;
}

pub trait FileDialogService: Send + Sync {
    fn pick_folder(&self, starting_dir: Option<&Path>, title: &str) -> Option<PathBuf>;
    fn open_in_explorer(&self, directory: &Path);
}
```

---

## 6. Composition Root & Dependency Injection (`src/main.rs`)

In a clean SOLID architecture, all concrete dependencies are instantiated and wired together in the **Composition Root** (entry point) and injected into high-level controllers:

```rust
pub struct AppContainer {
    pub transcoder: Arc<dyn Transcoder>,
    pub metadata_provider: Arc<dyn MetadataProvider>,
    pub drive_detector: Arc<dyn DriveDetector>,
    pub notifier: Arc<dyn Notifier>,
    pub config_repo: Arc<dyn ConfigRepository>,
    pub dialog_service: Arc<dyn FileDialogService>,
}

impl AppContainer {
    pub fn build_production() -> Self {
        let config_repo = Arc::new(TomlConfigStore::default());
        let notifier = Arc::new(CompositeNotifier::new(vec![
            Box::new(WebhookNotifier::default()),
            Box::new(MqttNotifier::default()),
            Box::new(MediaServerNotifier::default()),
        ]));
        
        Self {
            transcoder: Arc::new(FfmpegTranscoder::new("ffmpeg")),
            metadata_provider: Arc::new(OmdbMetadataProvider::default()),
            drive_detector: Arc::new(SystemDriveDetector::default()),
            notifier,
            config_repo,
            dialog_service: Arc::new(NativeDialogService::default()),
        }
    }
}
```

---

## 7. Phased Implementation Roadmap

To maintain zero downtime, guarantee 100% test passing at every commit, and preserve existing CLI/GUI interfaces, the refactoring will execute in four sequential phases:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ PHASE 1: Domain Extraction & Trait Definitions (Zero Behavior Changes)      │
│ • Create domain module with pure entities & ports.                          │
│ • Extract pure clustering/heuristics out of ffmpeg.rs into domain::heuristics│
│ • Tests: 145/145 passing.                                                   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ PHASE 2: Infrastructure Layer Segregation                                   │
│ • Split ffmpeg.rs into builder, runner, parser, ifo_parser.                 │
│ • Split utils.rs into notifiers, nfo_writer, disk_guard, explorer.          │
│ • Wrap Tesseract and OMDb inside domain trait implementations.              │
│ • Tests: 145/145 passing.                                                   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ PHASE 3: Application Use Cases & Interface Segregation (ISP)                │
│ • Split Args into focused configuration structs.                            │
│ • Implement RipMediaUseCase and BatchTvRipUseCase.                          │
│ • Decouple api.rs into dedicated route handlers, sse, openapi, and metrics. │
│ • Tests: 145/145 passing.                                                   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ PHASE 4: UI & Delivery Layer Clean-Up (DIP & Composition Root)              │
│ • Reorganize gui.rs into components and state store using injected services.│
│ • Inject AppContainer at main.rs bootstrap.                                 │
│ • Add comprehensive Mock unit tests for all domain use cases.               │
│ • Final Verification & Release Compilation.                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Risk Management & Backward Compatibility Guarantees

1. **CLI Flag Stability**: All 54 clap flags (`--out-dir`, `--codec`, `--tv`, `--deinterlace`, `--ocr`, etc.) remain identical and continue to serialize seamlessly into the new configuration slices.
2. **Configuration Persistence**: The schema in `~/.dvd-ripper/config.toml` and `dvd-ripper.toml` remains 100% compatible with existing user settings.
3. **Decoy & Copy-Protection Reliability**: The ScreenPass bad-sector detection heuristics and PAL speedup ranking algorithms will reside in isolated, pure unit-tested domain functions, preventing regression.
4. **Installer Integrity**: Standalone `dvd-ripper-installer.exe` binary build remains untouched and continues to install to `%LOCALAPPDATA%\dvd-ripper\bin`.

---

*Authored by Antigravity Senior Systems & Software Architecture Pair-Programming Agent.*
