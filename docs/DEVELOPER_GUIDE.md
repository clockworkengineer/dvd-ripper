# Developer Onboarding & Architecture Guide - `dvd-ripper`

Welcome to the `dvd-ripper` developer documentation. This guide details repository layout, compilation workflows, Clean Architecture boundaries, extending ports and adapters, writing mock unit tests, and coding conventions.

---

## 1. Repository Layout & Crate Structure

The repository follows **Clean / Hexagonal Architecture** with distinct concentric layers:

```text
dvd-ripper/
├── Cargo.toml               # Crate dependencies & build targets
├── src/
│   ├── main.rs              # Application entry point, CLI orchestration, & AppContainer Composition Root
│   ├── cli.rs               # Args CLI schema & segregated configuration traits
│   ├── config.rs            # TOML config parser & default application
│   ├── gui.rs               # Desktop immediate-mode GUI (eframe/egui)
│   ├── api.rs               # Embedded REST API, SSE streaming, & OpenAPI spec
│   ├── daemon.rs            # Multi-drive monitoring watcher daemon
│   │
│   ├── domain/              # PURE DOMAIN LAYER (Zero external I/O or library dependencies)
│   │   ├── entities/        # Domain models (Disc, Metadata, Rip, Drive)
│   │   ├── heuristics/      # Business algorithms (PAL speedup, cluster ranking, TV detection)
│   │   └── ports/           # Abstract port interfaces (DIP traits: Transcoder, Optical, Storage, etc.)
│   │
│   ├── application/         # APPLICATION LAYER (Orchestrates domain entities & ports)
│   │   ├── probe_disc.rs    # ProbeDiscUseCase
│   │   ├── rip_single_title.rs # RipSingleTitleUseCase
│   │   ├── rip_tv_batch.rs  # RipTvBatchUseCase
│   │   ├── drive_pool.rs    # DrivePool multi-drive concurrency
│   │   └── tests.rs         # In-memory mock tests for all use cases
│   │
│   ├── infrastructure/      # INFRASTRUCTURE ADAPTERS (Implements domain ports)
│   │   ├── ffmpeg/          # FFmpeg Command Builder, Process Runner, & Progress Parser
│   │   ├── optical/         # Win32 & Linux ISO-9660 drive readers & tray ejectors
│   │   ├── metadata/        # OMDb / TMDb HTTP clients & local fingerprint cache
│   │   ├── storage/         # Disk space guard & JSON history persistence
│   │   ├── notifications/   # MQTT 3.1.1 packet generator & HTTP webhook alerts
│   │   └── ocr/             # Tesseract OCR runner for SubRip sidecar generation
│   │
│   ├── ffmpeg.rs            # Output path resolution, formatters, & backward compatibility facades
│   ├── dvd.rs               # Optical sector benchmarking & copy protection inspection
│   ├── queue.rs             # Job queue & TV BoxSet manager (boxsets.json)
│   ├── history.rs           # History database serialization
│   ├── utils.rs             # String formatting, disk space guard checks, & media server hooks
│   └── bin/
│       └── installer.rs     # Standalone cross-platform installer binary
├── docs/                    # Technical & user documentation manuals (14 guides)
└── contrib/                 # Systemd service unit & udev rules
```

---

## 2. Building & Running

### 2.1 Build Application Binaries
```bash
# Debug build (dvd-ripper binary)
cargo build

# Release build with binary size optimizations
cargo build --release

# Build standalone cross-platform installer binary
cargo build --release --bin dvd-ripper-installer
```

### 2.2 Run Unit Test Suite
The test suite consists of **163 automated unit and integration tests** that run deterministically in under one second:

```bash
cargo test
```

To run a specific module's tests:
```bash
cargo test domain::heuristics
cargo test application::tests
cargo test ffmpeg::tests
```

---

## 3. Extending Functionality (Clean Architecture Workflow)

### 3.1 Adding a New Domain Heuristic
1. Add the pure heuristic algorithm in `src/domain/heuristics/<heuristic_name>.rs`.
2. Do not introduce network, file, or process dependencies in domain heuristics.
3. Write thorough unit tests validating boundary conditions (e.g. edge runtimes, floating point tolerances).
4. Re-export the heuristic in `src/domain/heuristics/mod.rs`.

### 3.2 Adding a New Port (DIP Interface)
1. Define the trait contract inside `src/domain/ports/<port_name>.rs`.
2. Ensure the trait bounds include `Send + Sync`.
3. Keep interfaces narrow according to the Interface Segregation Principle (ISP).
4. Export the trait in `src/domain/ports/mod.rs`.

### 3.3 Adding a New Infrastructure Adapter
1. Create a concrete implementation in `src/infrastructure/<subsystem>/<adapter_name>.rs`.
2. Implement the corresponding domain port trait.
3. Handle platform-specific or I/O error mapping cleanly into domain errors.
4. Wire the concrete adapter into `AppContainer::production()` in `src/main.rs`.

### 3.4 Adding a New Application Use Case
1. Create `src/application/<use_case_name>.rs`.
2. Define a typed request struct (`struct MyUseCaseRequest`).
3. Inject required port traits via `Arc<dyn MyPort>`.
4. Implement `pub fn execute(&self, req: MyUseCaseRequest) -> Result<MyUseCaseResponse>`.
5. Add unit tests in `src/application/tests.rs` using mock trait implementations.

---

## 4. Writing In-Memory Mock Tests

Because high-level application use cases depend strictly on domain ports, you can test complex ripping, cancellation, and error-handling workflows without launching FFmpeg or needing a physical DVD drive.

### Example Mock Test:

```rust
use std::sync::Arc;
use crate::application::rip_single_title::{RipSingleTitleRequest, RipSingleTitleUseCase};
use crate::domain::ports::transcoder::MockTranscoder;
use crate::domain::ports::storage::MockHistoryRepository;

#[test]
fn test_rip_single_title_success() {
    let mock_transcoder = Arc::new(MockTranscoder::default());
    let mock_history = Arc::new(MockHistoryRepository::default());

    let use_case = RipSingleTitleUseCase::new(
        mock_transcoder.clone(),
        mock_history.clone(),
    );

    let req = RipSingleTitleRequest {
        drive_path: "D:\\".into(),
        output_file: "Films/Test/Test.mp4".into(),
        title_num: 1,
        film_name: "Test Movie".into(),
        duration_secs: Some(7200.0),
        cancel_token: None,
    };

    let result = use_case.execute(req);
    assert!(result.is_ok());
    assert_eq!(mock_history.records().len(), 1);
}
```

---

## 5. Coding Standards & Conventions

1. **SOLID Adherence**:
   - Keep structs focused on single responsibilities.
   - Use traits for abstract contracts; never bind use cases directly to low-level APIs.
2. **Formatting & Lints**:
   - Run `cargo fmt --check` before committing.
   - Run `cargo clippy --all-targets --all-features` and address all warnings.
3. **Documentation Integrity**:
   - Maintain clickable markdown file links (`[filename](file:///path/to/file)`) in documentation.
   - Use rustdoc comments (`///` for public symbols, `//!` for module roots).
