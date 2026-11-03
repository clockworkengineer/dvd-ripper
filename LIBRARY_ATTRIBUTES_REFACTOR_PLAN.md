# Software Library Attributes Refactor Plan
**Target Codebase:** `dvd-ripper`  
**Reference Document:** `notes/ATTRIBUTES.md`  
**Date:** November 2026  
**Status:** Approved for Implementation  

---

## 1. Executive Summary

According to [`notes/ATTRIBUTES.md`](notes/ATTRIBUTES.md):
> *"A well-written software library is more than just functional code; it is an artifact designed for others to use, understand, and rely upon."*

While `dvd-ripper` currently functions as a versatile CLI, GUI, and Web API application with a Clean Architecture domain model, it is fundamentally packaged as a monolithic **application binary** rather than a **reusable software library**. Consumers cannot integrate its optical probing, transcoding pipelines, TV episode heuristics, or ISO archiving capabilities into external Rust applications without copy-pasting code or executing it as a child subprocess.

This refactor plan provides a concrete, multi-phase roadmap to transform `dvd-ripper` into a world-class, production-grade Rust library and modular application suite, systematically fulfilling all **10 Software Library Attributes**.

---

## 2. Codebase Audit Against the 10 Library Attributes

| Attribute | Current Codebase Status | Key Deficiencies | Target Quality State |
| :--- | :--- | :--- | :--- |
| **1. Intuitive API Design** | ⚠️ Partial | Crate is a binary without `src/lib.rs`. Functions require monolithic `Args` struct (45+ fields). | Public `lib.rs` with fluent builders (`RipJobBuilder`, `TranscodePipelineBuilder`), typed configs, and clean prelude. |
| **2. Comprehensive Documentation** | ⚠️ Partial | Markdown guides exist in `docs/`, but rustdoc is sparse; `# Examples`, `# Errors`, and doctests are missing. | 100% rustdoc coverage on public items; interactive doctests verified via `cargo test --doc`. |
| **3. High Reliability** | ⚠️ Partial | Mutex `.unwrap()` calls present; hardcoded temp file paths in `std::env::temp_dir()`; raw subprocess calls. | Poison-resilient synchronization, RAII temporary resources (`TempPath`), and robust child process tree termination. |
| **4. Performance & Efficiency** | ⚠️ Moderate | Fixed 64KB I/O buffer for ISO copy; progress parsing allocates `String`s per line; redundant `ffprobe` passes. | Configurable 1MB–2MB aligned DMA buffers; zero-copy byte slice parser (`&[u8]`); session probe caching. |
| **5. Maintainability** | ⚠️ Moderate | `src/ffmpeg.rs` is ~2500 lines; `src/gui.rs` is ~1900 lines; `src/utils.rs` is ~1400 lines. | Modular decomposition into cohesive sub-crates/modules (`ffmpeg/probe.rs`, `gui/components/`, `utils/fs.rs`). |
| **6. Flexibility & Customization** | ⚠️ Partial | Hardcoded FFmpeg filter flags and encoder arguments; fixed notification targets. | Strategy-based filter graph builder; pluggable notification and metadata provider traits. |
| **7. Strong Security** | ⚠️ Moderate | HTTP API lacks payload size limits; path traversals checked late; some PowerShell string interpolation. | Strict 1MB HTTP payload limits; upfront path containment; pure argument-vector process execution. |
| **8. High Testability** | ✅ Strong (205 tests) | Strong unit & CLI tests, but lacks property-based tests and automated microbenchmarks. | `criterion` benchmark suite for ISO dumping & parser; `proptest` for string and path sanitization. |
| **9. Compatibility & Portability** | ⚠️ Moderate | Windows-centric defaults (`D:\`, PowerShell COM) alongside Unix conditionals. | Segregated platform abstractions (`platform/windows.rs`, `platform/unix.rs`, `platform/macos.rs`). |
| **10. Low Dependency Footprint** | ⚠️ Partial | Default compilation pulls `eframe`, `rfd`, and `image` even when only library core is needed. | Modular Cargo feature flags: `core` (minimal dependencies), `cli`, `gui`, `api`, `full`. |

---

## 3. Target Architecture & Package Layout

```mermaid
graph TD
    subgraph External Consumers
        App[External Application / Media Server]
        CLI[dvd-ripper CLI Binary]
        GUI[dvd-ripper GUI Binary]
        Daemon[dvd-ripper Daemon Appliance]
    end

    subgraph dvd-ripper Library (src/lib.rs)
        Prelude[dvd_ripper::prelude]
        
        subgraph Domain Layer [Zero External Dependencies]
            Entities[Entities: Disc, Job, Media, Error]
            Heuristics[Heuristics: TVDetector, Speedup, ClusterDetector]
            Ports[Ports: Transcoder, OpticalDrive, Storage, Metadata, Notification]
        end

        subgraph Application Layer [Use Cases & Orchestration]
            RipSingle[RipSingleTitleUseCase]
            RipTv[RipTvBatchUseCase]
            DumpIso[DumpIsoUseCase]
            ProbeDisc[ProbeDiscUseCase]
            DrivePool[DrivePool Service]
            Container[AppContainer]
        end

        subgraph Infrastructure Layer [Implementations & Drivers]
            FfmpegMod[infrastructure::ffmpeg: Transcoder, Builder, Parser]
            OpticalMod[infrastructure::optical: Platform Drivers]
            StorageMod[infrastructure::storage: History, DiskGuard]
            MetaMod[infrastructure::metadata: OMDb, TVMaze, DiscCache]
        end
    end

    App --> Prelude
    CLI --> Container
    GUI --> Container
    Daemon --> Container
    
    Container --> Application Layer
    Application Layer --> Domain Layer
    Infrastructure Layer -.-> Ports
```

### Feature Flag Matrix (`Cargo.toml`)
- **`default = ["cli", "gui"]`**
- **`cli = ["clap"]`**: Enables CLI argument parsing and binary entry point.
- **`gui = ["dep:eframe", "dep:image", "dep:rfd"]`**: Enables desktop graphical user interface.
- **`api = ["dep:serde_json"]`**: Enables embedded REST API, Prometheus metrics, and SSE broadcast engine.
- **`full = ["cli", "gui", "api"]`**: All features enabled.
- **Core Library (`--no-default-features`)**: Zero GUI dependencies (`eframe`, `rfd`, `image` completely stripped), compiling in <3 seconds with minimal footprint.

---

## 4. Implementation Roadmap

### Phase 1: Dual Crate / Library Architecture & Feature Flag Segregation
1. **Expose `src/lib.rs`**:
   - Establish `dvd-ripper` as both a library (`[lib]`) and binary (`[[bin]]`).
   - Define clean public API modules:
     - `pub mod domain;`
     - `pub mod application;`
     - `pub mod infrastructure;`
     - `pub mod prelude;`
   - Create `src/prelude.rs` re-exporting key traits and builders:
     - `OpticalDrive`, `DiscInspector`, `TrayController`
     - `Transcoder`, `ProgressObserver`
     - `HistoryRepository`, `DiskSpaceGuard`
     - `RipSingleTitleUseCase`, `RipTvBatchUseCase`, `DumpIsoUseCase`, `ProbeDiscUseCase`
     - `AppContainer`
2. **Modularize `Cargo.toml` Features**:
   - Refactor `Cargo.toml` to declare `[lib] name = "dvd_ripper" path = "src/lib.rs"`.
   - Gate GUI dependencies strictly under `feature = "gui"`.
   - Gate CLI command parser dependencies strictly under `feature = "cli"`.

---

### Phase 2: Intuitive API Design & Builder Patterns
1. **Eliminate Monolithic `Args` Parameter Leakage**:
   - Create typed, focused option structures:
     - `TranscodeOptions`: Codec, profile, preset, deinterlace, audio tracks, subtitles.
     - `DiscOptions`: Min free space, no-eject, auto-spindown.
   - Refactor `FfmpegTranscoder::new(...)` to accept `TranscodeOptions` rather than `cli::Args`.
2. **Implement Fluent Builders**:
   - Implement `RipSingleTitleRequestBuilder`:
     ```rust
     let request = RipSingleTitleRequest::builder(dvd_path, title_num, out_path)
         .display_title("The Matrix")
         .expected_duration(8160.0)
         .min_free_gb(15)
         .build()?;
     ```
   - Implement `DumpIsoRequestBuilder`:
     ```rust
     let request = DumpIsoRequest::builder(dvd_path, out_path)
         .volume_label("MATRIX_DISC1")
         .min_free_gb(10)
         .build()?;
     ```
   - Implement `RipTvBatchRequestBuilder`:
     ```rust
     let request = RipTvBatchRequest::builder(dvd_path, "Doctor Who", out_dir)
         .season(1)
         .start_episode(1)
         .lookup_episode_titles(true)
         .track_boxset(true)
         .build()?;
     ```

---

### Phase 3: Comprehensive Rustdoc Documentation & Doctests
1. **Module-Level & Crate-Level Documentation**:
   - Add rich top-level `//!` documentation in `src/lib.rs` with architecture overview, feature flags, and quickstart example.
   - Add module documentation for `domain`, `application`, `infrastructure`, and `prelude`.
2. **Standardized Method Documentation**:
   - Ensure every public trait, struct, enum, and method includes:
     - Brief summary line.
     - Detailed explanation.
     - `# Errors` section explaining failure conditions.
     - `# Panics` section (or guarantee `# Panics: Never`).
     - `# Example` code snippet.
3. **Automated Doctests**:
   - Enable and verify `cargo test --doc` across all library modules.

---

### Phase 4: Reliability, RAII Resources & Process Safety
1. **Poison-Resilient Concurrency**:
   - Replace all unhandled `mutex.lock().unwrap()` across `src/api.rs`, `src/gui.rs`, and application services with safe lock acquisition (`mutex.lock().unwrap_or_else(|e| e.into_inner())`).
2. **RAII Temporary Resource Management**:
   - Introduce `infrastructure::storage::TempResource`:
     - Creates unique, collision-proof temporary files (using process ID and timestamp/UUID).
     - Automatically deletes files on `Drop`.
     - Replaces hardcoded `std::env::temp_dir().join("dvd_thumb_title_1.jpg")` in thumbnail extraction.
3. **Subprocess Process Tree Supervision**:
   - In `FfmpegTranscoder` and `SystemDiscInspector`, wrap spawned child processes with cooperative termination tokens to prevent orphaned processes when tasks are cancelled or dropped.

---

### Phase 5: High Performance, Zero-Allocation Streaming & Microbenchmarks
1. **Dynamic DMA Buffer Optimization in ISO Archiving**:
   - Upgrade `iso::copy_sectors` to dynamically scale buffer sizes from 64KB up to 2MB based on drive throughput.
   - Align memory buffers to optical drive sector boundaries (2048 bytes).
2. **Zero-Copy Progress Parsing**:
   - Refactor `ProgressParser` in `src/infrastructure/ffmpeg/progress_parser.rs` to process byte slices (`&[u8]`) directly without allocating heap `String`s for each tick line.
3. **Session-Level Disc Metadata Caching**:
   - Introduce `SessionDiscCache` in `ProbeDiscUseCase` to eliminate duplicate `ffprobe` / `ffmpeg` subtitle and chapter scanning within the same session.
4. **Criterion Benchmarks**:
   - Create `benches/parser_benchmarks.rs` and `benches/iso_benchmarks.rs` to benchmark:
     - Sector throughput copy rate.
     - Progress line parsing speed.
     - Title duration median calculation in `tv_detector`.

---

### Phase 6: Strong Security, Strict Path Containment & DoS Protection
1. **HTTP DoS Hardening (`src/api.rs`)**:
   - Enforce maximum request body limit (1MB) on all incoming POST requests.
   - Reject malformed or oversized payloads with HTTP `413 Payload Too Large`.
2. **Upfront Path Containment & Sanitization**:
   - Ensure all output paths resolve strictly within designated destination directories using `canonicalize` and prefix validation.
   - Reject paths containing directory traversal tokens (`..`), path separators in title names, or non-printable control characters.
3. **Injection Elimination**:
   - Audit all `std::process::Command` invocations to verify 100% vector-based argument passing with zero string interpolation into command shells.

---

### Phase 7: Cross-Platform Abstraction & Low-Footprint Verification
1. **Platform Optical Abstraction Layer**:
   - Create `src/infrastructure/optical/platform/`:
     - `windows.rs`: PowerShell / Win32 COM tray operations.
     - `unix.rs`: POSIX `/dev/sr*`, `ioctl(CDROMEJECT)`, `eject -i`.
     - `macos.rs`: `drutil tray eject`.
   - Provide clean unified driver selection at compile time without polluting domain ports.
2. **Zero-Dependency Domain Invariant Verification**:
   - Ensure `src/domain/` has **zero** non-standard dependencies (only `anyhow`, standard library, and pure domain algorithms).
3. **End-to-End Verification Gate**:
   - `cargo test --all-targets` (all unit and integration tests passing).
   - `cargo test --doc` (all documentation tests passing).
   - `cargo clippy --all-targets -- -D warnings` (clean).
   - `cargo fmt --check` (clean).
   - `cargo check --no-default-features` (minimal footprint build verified).

---

## 5. Architectural Quality Standards & Verification Gates

```
[Quality Gate 1: Zero-Warning Build]
    ├── cargo clippy --all-targets -- -D warnings
    └── cargo fmt --check

[Quality Gate 2: Test Suite Completeness]
    ├── cargo test --all-targets (Target: >210 tests passed)
    └── cargo test --doc (Target: 100% doctests passed)

[Quality Gate 3: Footprint & Portability]
    ├── cargo check --no-default-features (Minimal Core Crate)
    ├── cargo build --release (Optimized Binary)
    └── Zero unwrap() panics in library code paths
```

