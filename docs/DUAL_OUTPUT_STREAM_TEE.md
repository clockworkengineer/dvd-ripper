# Single-Pass Dual-Output Engine (StreamTee) Guide - `dvd-ripper`

`dvd-ripper` introduces a **Single-Pass Dual-Output Pipeline** (`src/infrastructure/pipeline/stream_tee.rs` and `src/application/dual_output_rip.rs`), allowing users to simultaneously produce both an uncompressed, bit-for-bit archival master (`.mkv` or `.iso`) and a lightweight, compressed streaming target (`.mp4`) in a **single optical reading pass**.

---

## 1. The Conventional Double-Read Dilemma

In traditional DVD archival workflows (such as running MakeMKV for archival followed by HandBrake for Plex streaming):
1. **Double Laser Wear**: The optical drive must spin up and read all 4.7 GB – 8.5 GB of data twice from disc.
2. **Double Ingest Time**: Ripping takes twice as long (e.g. 15 minutes for MakeMKV remux + 15 minutes for HandBrake transcode).
3. **Drive Spindle Stress**: Continuous re-reading increases the risk of scratching delicate media and causing mechanical drive failure.

```mermaid
flowchart LR
    subgraph Conventional ["Conventional Workflow (2 Full Disc Reads)"]
        D1[Optical Disc] -->|Read 1 (100% Disc)| MKV[MakeMKV Archival MKV]
        D1 -->|Read 2 (100% Disc)| HB[HandBrake Transcoded MP4]
    end
```

---

## 2. The Single-Pass StreamTee Solution

`dvd-ripper` solves this with the **StreamTee** architecture:

```mermaid
flowchart LR
    subgraph StreamTee_Flow ["StreamTee Single-Pass Architecture"]
        Disc[Optical Disc] -->|Single Read Pass| Tee["StreamTee Buffer (128 KB Blocks)"]
        Tee -->|Sink A (Synchronous File Writer)| Archival["Archival Master (.mkv / .iso)"]
        Tee -->|Sink B (Named Pipe / Transcode Ingest)| Transcoder["FFmpeg Transcoder (.mp4)"]
    end
```

### Key Technical Characteristics
- **128 KB Sector Block Replication**: Data is ingested in aligned 128 KB blocks (`64 * 2048` optical sector boundaries) into memory.
- **Concurrent Tee Dispatching**: Each block is dispatched synchronously to the Archival Master file writer and asynchronously to the streaming transcode sink.
- **Bounded Memory Profile**: Memory consumption is capped to the active 128 KB chunk buffer plus a small ring buffer, ensuring <32 MB peak RAM footprint even on low-spec Raspberry Pi or NAS hardware.
- **50% Optical Wear Reduction**: Laser exposure and spindle spin time are reduced by exactly 50%.

---

## 3. Application Pipeline Execution (`DualOutputRipUseCase`)

The `DualOutputRipUseCase` (`src/application/dual_output_rip.rs`) orchestrates the dual-output lifecycle:

1. **Destination Verification**: Ensures directory paths exist for both the archival output path (`archival_output_path`) and transcode output path (`transcode_output_path`).
2. **Disk Space Verification**: Checks target partitions via `DiskSpaceGuard` for sufficient capacity to store both media artifacts simultaneously.
3. **Tee Execution**: Pipes raw MPEG-2 program stream chunks to the archival sink while feeding FFmpeg's video filter and encoding pipeline.
4. **Sidecar Generation**: Generates NFO XML metadata sidecars for media library indexing.
5. **Unified Outcome Notification**: Dispatches completion telemetry, logs historical records in `ripping_history.json`, and triggers Plex/Jellyfin library scans.

---

## 4. Usage & Configuration

### CLI
Pass the `--dual-output` flag when specifying an output destination:
```bash
dvd-ripper --input D:\ --dual-output --out-dir "C:\Media" --codec hevc
```
**Resulting Library Artifacts**:
```text
C:\Media\Films\The Matrix (1999)\
├── The Matrix (1999) - Archival Master.mkv    <-- Bit-exact uncompressed MPEG-2 remux
├── The Matrix (1999).mp4                      <-- Lightweight HEVC streaming version
└── The Matrix (1999).nfo                      <-- Kodi / Plex metadata sidecar
```

### TOML Configuration (`dvd-ripper.toml`)
```toml
# Enable single-pass dual output for all rip jobs
dual_output = true
```

### Desktop GUI
In the **Main Rip Panel**, check the **Single-Pass Dual Output** checkbox next to the Transcode toggle.
