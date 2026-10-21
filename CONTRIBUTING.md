# Contributing to DVD Ripper (`dvd-ripper`)

Thank you for your interest in contributing to `dvd-ripper`! This document outlines the project's development workflow, coding conventions, architectural guidelines, and pull request procedures.

---

## 1. Code of Conduct

We are committed to providing a welcoming, inclusive, and harassment-free environment for all contributors. Please treat others with respect and professionalism.

---

## 2. Development Environment Setup

### 2.1 Prerequisites
- **Rust Toolchain**: Rust 2024 edition (`cargo` & `rustc`). Install via [rustup.rs](https://rustup.rs/).
- **FFmpeg**: Must be available in system `PATH` built with `dvdvideo` demuxer enabled.
- **libdvdcss**: Required for encrypted commercial DVD playback/decryption. See [`docs/DEPENDENCIES_INSTALLATION.md`](docs/DEPENDENCIES_INSTALLATION.md).
- **Tesseract OCR (Optional)**: For testing subtitle bitmap-to-text extraction.

### 2.2 Building the Project
```bash
# Clone the repository
git clone https://github.com/clockworkengineer/dvd-ripper.git
cd dvd-ripper

# Build in debug mode
cargo build

# Run desktop GUI
cargo run

# Run headless CLI
cargo run -- --cli -i auto
```

---

## 3. Architectural Boundaries (Clean & SOLID)

`dvd-ripper` strictly enforces **Clean Architecture** and the **5 SOLID Principles**. Before introducing new code, consult:
👉 **[`docs/SOLID_ARCHITECTURE.md`](docs/SOLID_ARCHITECTURE.md)**

### Key Rules:
1. **Domain Independence**:
   - Code inside `src/domain/` must remain pure Rust.
   - Never import external I/O crates, process runners, file writers, or network clients into `src/domain/`.
2. **Ports & Adapters**:
   - Define abstract trait interfaces in `src/domain/ports/`.
   - Implement concrete I/O adapters in `src/infrastructure/`.
3. **Application Use Cases**:
   - Business workflows belong in `src/application/`.
   - Use cases must depend on traits (`Arc<dyn Port>`), never on concrete infrastructure structs.
4. **Composition Root**:
   - Service wiring occurs in `src/main.rs: AppContainer::production()`.

---

## 4. Coding Standards & Lints

1. **Formatting**:
   Always format code prior to submitting pull requests:
   ```bash
   cargo fmt --all -- --check
   ```
2. **Clippy Lints**:
   Code must pass clippy with zero errors:
   ```bash
   cargo clippy --all-targets --all-features
   ```
3. **Documentation**:
   - Document all public items with doc comments (`///`).
   - Add module-level documentation (`//!`) to module entrypoints.
   - Maintain clickable file links (`[filename](file:///path/to/file)`) in documentation.

---

## 5. Testing Requirements

All contributions must include comprehensive automated tests.

1. **Unit Tests**:
   - Test domain heuristics (`speedup`, `cluster_detector`, `tv_detector`) for edge cases.
   - Use mock adapters in `src/application/tests.rs` to verify new application use cases without launching external processes.
2. **Deterministic Execution**:
   - Tests must run independently and leave zero temporary files on disk (use `TestTempDir` guards).
3. **Test Suite Verification**:
   Ensure all 163+ tests pass before submitting your PR:
   ```bash
   cargo test
   ```

---

## 6. Pull Request Checklist

When submitting a PR:
- [ ] Code compiles without warnings (`cargo check`).
- [ ] Code is formatted with `cargo fmt`.
- [ ] All automated tests pass (`cargo test`).
- [ ] Relevant documentation in `docs/` is updated.
- [ ] Commit messages follow concise, imperative phrasing (e.g. `feat: add AV1 hardware encoding profile for VAAPI`).
