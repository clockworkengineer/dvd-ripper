# DVD Ripper Troubleshooting & Diagnostic Guide

This guide details common diagnostic workflows, hardware throughput bottlenecks, copy protection errors, disk space safeguards, and video quality tuning for `dvd-ripper`.

---

## 1. Optical Drive Throughput & RipLock Diagnostics

### Symptom: Slow Ripping Read Speed (< 4 MB/s or < 2x Speed)
Commercial DVD drives often ship with firmware **RipLock** enabled, restricting optical disc read speeds to 2x (approx. 2.7 MB/s) during video playback.

### Diagnostic Command
Run the built-in Optical Drive Read Speed Benchmark:

```bash
dvd-ripper --input D:\ --benchmark
```

### Expected Output
- **High Performance (16x+ Speed)**: `>= 15.0 MB/s` (Ideal for fast batch ripping).
- **Standard Read Speed (8x Speed)**: `8.0 – 15.0 MB/s`.
- **RipLocked / Bottlenecked (< 4x Speed)**: `< 4.0 MB/s`.

### Solutions
1. **Flash Unlocked Firmware**: Flash your optical drive with LibreDrive or MCSE unlocked firmware to remove RipLock read limits.
2. **Check USB/SATA Cables**: Ensure external USB drives use USB 3.0 ports (blue ports) rather than USB 2.0 hubs.

---

## 2. Copy Protection (CSS/CPPM) & libdvdcss Errors

### Symptom: FFmpeg cannot rip encrypted DVD ("CSS support is unavailable")
Commercial encrypted DVDs require `libdvdcss` to decrypt scrambled sector keys during extraction.

### Solution:
Install `libdvdcss` using your operating system package manager:
- **macOS**: `brew install libdvdcss`
- **Linux (Debian/Ubuntu)**: `sudo apt install libdvd-pkg && sudo dpkg-reconfigure libdvd-pkg`
- **Windows**: `choco install libdvdcss` or place `libdvdcss-2.dll` in your system `PATH`.

For full cross-platform instructions, see **[FFmpeg & libdvdcss Installation Guide](DEPENDENCIES_INSTALLATION.md)**.

---

### Symptom: FFmpeg exits with `Invalid IFO for title 0 (VIDEO_TS.IFO)`
On commercial discs with Sony ARccOS or corrupted root tables, FFmpeg's `libdvdread` demuxer aborts with:
```text
[in#0] libdvdread: Invalid IFO for title 0 (VIDEO_TS.IFO).
[in#0] libdvdread: Invalid IFO for title 0 (VIDEO_TS.BUP).
[in#0] Error opening input: Generic error in an external library
```

### Solutions:
1. **Rely on Auto-Fallback (`--engine auto`)**:
   `dvd-ripper` automatically catches FFmpeg demuxer errors and triggers fallback to the pure Rust **Native IFO Extractor** (`native_ifo`), which reads `.BUP` backup tables and directly parses title sets without touching the corrupted root table:
   ```text
   [Auto Engine Fallback] Primary engine encountered an error. Falling back to engine 'native_ifo' (2/2)...
   ```
2. **Force Native IFO Extractor via CLI or GUI**:
   ```bash
   dvd-ripper --cli D: --engine native --search "Movie Title"
   ```
3. **Use MakeMKV for Heavily Encrypted Discs**:
   ```bash
   dvd-ripper --cli D: --engine makemkv
   ```

---

### Symptom: Ripping hangs or produces I/O Errors at specific chapters (e.g. stops at 62.8%)
Commercial DVDs use deliberate bad sector structures (Sony ARccOS / Macrovision RipGuard) positioned in unreferenced gaps between chapters to cause digital read head locks.

### Solutions:
1. **Switch to Native IFO Engine (`--engine native`)**:
   The native extractor only extracts logical sectors explicitly referenced by the Program Chain (`VTS_PGCITI`), completely jumping over unreferenced bad sector traps.
2. **Enable Scratched Disc Fault Tolerance**:
   ```bash
   dvd-ripper --cli D: --ignore-errors
   ```
3. See **[Copy Protection, Bad Sectors, and Disc Recovery Guide](COPY_PROTECTION_AND_BAD_SECTORS.md)** for in-depth diagnostic details.

---

## 3. Enterprise Disk Space Guard Alerts

### Symptom: Ripping aborts with `Disk Space Guard Safeguard Triggered`
Before initiating ripping jobs, `dvd-ripper` inspects available free disk space on the target destination partition.

### Error Message
```text
Disk Space Guard Safeguard Triggered: Only 4.50 GB free space available in 'C:\Films', but minimum threshold is 10 GB.
```

### Solutions
1. **Adjust Minimum Threshold**: Lower `--min-free-gb` if operating on small partitions:
   ```bash
   dvd-ripper --input D:\ --search "Aliens" --min-free-gb 5
   ```
2. **Specify External Output Directory**: Point `--out-dir` to an external hard drive or NAS share with ample free space:
   ```bash
   dvd-ripper --input D:\ --search "Aliens" --out-dir "E:\Films"
   ```

---

## 4. Video Interlacing Comb Artifacts

### Symptom: Horizontal jagged lines or comb artifacts on fast motion
Standard-definition DVDs (NTSC 480i / PAL 576i) store video using interlaced fields.

### Solution
Enable motion-adaptive deinterlacing (`--deinterlace`) using the `bwdif` algorithm:

```bash
dvd-ripper --input D:\ --search "Dr Who" --transcode --deinterlace --deinterlace-algo bwdif
```

---

## 5. Network Notifications & Webhook Failures

### Discord / Slack / Webhook HTTP 400 Errors
- Ensure `--webhook-url` contains the full HTTPS webhook endpoint URL (e.g. `https://discord.com/api/webhooks/...`).

### Home Assistant MQTT Disconnections
- Ensure your MQTT broker URI includes the protocol scheme: `--mqtt-broker mqtt://192.168.1.50:1883`.
