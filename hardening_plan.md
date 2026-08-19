# DVD Ripper Code Hardening & Security Audit Plan

## 1. Executive Summary

This document provides a comprehensive security and robustness audit of the `dvd-ripper` Rust codebase and outlines concrete implementation steps to harden the software against command injection, JSON payload injection, path traversal, timing attacks, mutex poisoning, zombie subprocesses, and un-sanitized input edge cases.

---

## 2. Identified Vulnerabilities & Hardening Target Areas

### Category A: Command Injection & Subprocess Hardening
1. **Unsafe PowerShell Eject Command** (`src/dvd.rs`):
   - *Risk*: `eject_disc()` constructs PowerShell invocation strings using `root_path.chars().next()`. If an invalid or malicious drive identifier is supplied, unescaped string interpolation could allow command injection.
   - *Fix*: Strictly validate `drive_letter` to ASCII alphabetic characters (`A-Z` / `a-z`) or switch to native Win32 `IOCTL_STORAGE_EJECT_MEDIA` / safe API handles.
2. **CLI Argument Option Injection** (`src/ffmpeg.rs`, `src/utils.rs`):
   - *Risk*: Path parameters passed to external utilities (`ffmpeg`, post-processing scripts) could start with `-` (e.g. `-vf`), triggering unwanted CLI option flags.
   - *Fix*: Pass `sanitize_cli_path_arg` output or explicit `--` delimiters for positional process arguments.

### Category B: JSON Injection & Telemetry Payload Safety
1. **Raw String Format JSON Injections** (`src/mqtt.rs`, `src/api.rs`):
   - *Risk*: Telemetry payloads (`publish_mqtt_status`, `build_webhook_payload`) and REST API responses (`/api/select`, `/api/status`, `/api/queue/*`) construct JSON via manual string formatting (`format!("{\"disc\":\"{}\"}", disc_name)`). If a movie title or disc label contains quotes (`"`), newlines (`\n`), backslashes (`\`), or control characters, the payload breaks or allows JSON injection.
   - *Fix*: Replace all manual string JSON formatting with `serde_json::to_string()` or `serde_json::json!`.

### Category C: Path Traversal & File System Boundaries
1. **Output Directory Path Traversal** (`src/ffmpeg.rs`, `src/utils.rs`):
   - *Risk*: Volume labels or movie titles fetched from external APIs could contain `..` or relative directory sequences, resolving outside `out_dir`.
   - *Fix*: Validate canonical paths using path containment checks (`path.starts_with(base_out_dir)`) and enforce `is_safe_filename` validation.
2. **Non-Atomic Database Cache Writes** (`src/imdb.rs`):
   - *Risk*: `save_fingerprint_cache()` uses direct `fs::write()` instead of atomic file replacement, risking corrupted cache files if interrupted.
   - *Fix*: Use `utils::atomic_write_file()` for fingerprint cache updates.

### Category D: Network & Web API Security
1. **Timing Side-Channel in API Key Validation** (`src/api.rs`):
   - *Risk*: `validate_api_key_header()` uses standard `==` string equality comparison, which is susceptible to timing side-channel attacks.
   - *Fix*: Use constant-time byte array comparison (`subtle::ConstantTimeEq` or constant-time loop).
2. **Hardcoded API Keys** (`src/imdb.rs`):
   - *Risk*: Third-party API keys (TMDB, OMDb trilogy demo key) are hardcoded in source.
   - *Fix*: Make third-party API keys configurable via environment variables (`TMDB_API_KEY`, `OMDB_API_KEY`) and `AppConfig`.
3. **HTTP Security Headers** (`src/api.rs`):
   - *Risk*: Embedded API responses lack standard HTTP security headers.
   - *Fix*: Add `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, and `Content-Security-Policy`.

### Category E: Concurrency & Process Lifecycle
1. **Mutex Poison Propagation** (`src/api.rs`, `src/queue.rs`):
   - *Risk*: If a thread panics while holding a global state mutex (`APPLIANCE_STATUS`, `JOB_QUEUE`, `BOXSET_MANAGER`), subsequent calls to `.lock().unwrap()` panic across the application.
   - *Fix*: Wrap lock acquisitions with poison recovery (`match lock { Ok(g) => g, Err(p) => p.into_inner() }`).
2. **Orphaned FFmpeg Subprocesses** (`src/ffmpeg.rs`):
   - *Risk*: Cancellation or error paths during ripping might leave spawned FFmpeg processes running in the background.
   - *Fix*: Ensure explicit `child.kill()` and `child.wait()` cleanup in all failure and drop execution paths.

---

## 3. Concrete Hardening Implementation Plan

### Phase 1: Input Sanitization & Command Injection Prevention
- Refactor `eject_disc` in `src/dvd.rs` with strict drive letter verification (`c.is_ascii_alphabetic()`).
- Add `--` positional argument boundary handling in `src/ffmpeg.rs` process spawning.
- Enhance `is_safe_filename` in `src/utils.rs` to detect relative path traversal elements (`..`, leading slashes).

### Phase 2: JSON Payload Serialization Refactoring
- Refactor `publish_mqtt_status` and `build_webhook_payload` in `src/mqtt.rs` using `serde_json::json!`.
- Refactor all `/api/*` endpoint JSON responses in `src/api.rs` to use `serde_json::to_string()`.

### Phase 3: Path Containment & Atomic Persistence
- Implement `ensure_path_contained(base: &Path, target: &Path)` in `src/utils.rs` and apply to `resolve_output_path` and `resolve_tv_output_path`.
- Refactor `save_fingerprint_cache` in `src/imdb.rs` to use `atomic_write_file`.

### Phase 4: API Security, Key Management & Mutex Hardening
- Implement constant-time string equality check for API key verification in `src/api.rs`.
- Inject security headers in `send_http_response`.
- Make TMDB API keys configurable in `src/imdb.rs`.
- Implement robust lock helpers to handle mutex poison recovery across `src/api.rs` and `src/queue.rs`.

---

## 4. Verification & Testing Strategy

1. **Automated Unit Tests**:
   - Test `is_safe_filename` with path traversal attempts (`../`, `..\`, `CON`, `NUL`).
   - Test `ensure_path_contained` boundary checks.
   - Test JSON escaping & serialization with special characters (`"`, `\n`, `\t`, `\`).
   - Test constant-time API key verification logic.
2. **Build & Runtime Checks**:
   - Run `cargo check` and `cargo test` to ensure 100% clean compilation and test execution.
