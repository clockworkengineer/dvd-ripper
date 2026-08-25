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

### Symptom: Ripping hangs or produces I/O Errors at specific chapters
Commercial DVDs use Content Scramble System (CSS), CPPM, or deliberate bad sector structure (ArccOS / Macrovision) to block digital extraction.

### Diagnostic Command
Run structural copy protection analysis:

```bash
dvd-ripper --input D:\ --check-protection
```

### Diagnostic Output Example
```text
--- [Disc Copy Protection Diagnostic Report] ---
  • Total VOB Video Objects on disc: 12
  • Total IFO Metadata Files on disc: 4
  • Primary DVD Volume Label: KILL_BILL_VOL2
  • Disc Structural Health: 12 VOBs accessible, 4 IFO headers verified.
------------------------------------------------
```

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
