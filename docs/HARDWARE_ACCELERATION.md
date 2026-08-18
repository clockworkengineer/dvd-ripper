# Hardware Video Encoding & GPU Acceleration Tuning Guide

`dvd-ripper` supports hardware-accelerated video encoding across NVIDIA GPUs, Intel/AMD VAAPI Linux drivers, Intel QuickSync, and ARM Raspberry Pi platforms using the `--hwaccel` option.

---

## 1. Overview & Supported Acceleration Modes

| `--hwaccel` Value | Platform / GPU Target | FFmpeg Video Codec | Filter Pipeline Integration |
|---|---|---|---|
| `copy` (Default) | CPU / Software Encoding | `libx264`, `libx265`, `libsvtav1` | Software (`-vf scale=-2:720,bwdif,hqdn3d`) |
| `nvenc` | NVIDIA GeForce / Quadro / Tesla GPUs | `h264_nvenc`, `hevc_nvenc` | CUDA / NVENC pipeline |
| `vaapi` | Intel / AMD GPUs (Linux `/dev/dri/renderD128`) | `h264_vaapi` | Hardware upload (`-vf format=nv12,hwupload`) |
| `qsv` | Intel QuickSync Video (Windows / Linux) | `h264_qsv` | QuickSync MFX pipeline |
| `v4l2` / `v4l2m2m` | Raspberry Pi 4/5 & ARM SBCs | `h264_v4l2m2m` | V4L2 memory-to-memory hardware block |

---

## 2. Command Usage & Driver Setup

### 2.1 NVIDIA NVENC
Requires NVIDIA graphics drivers with NVENC support:

```bash
dvd-ripper --input D:\ --search "Aliens" --transcode --hwaccel nvenc --codec h264
```

### 2.2 Intel / AMD VAAPI (Linux)
Requires VAAPI drivers installed (`intel-media-driver` or `mesa-va-drivers`) and DRI render device permissions (`/dev/dri/renderD128`):

```bash
dvd-ripper --input /dev/sr0 --search "Aliens" --transcode --hwaccel vaapi
```

### 2.3 Raspberry Pi V4L2 M2M
Requires Raspberry Pi OS with H.264 hardware encoding enabled:

```bash
dvd-ripper --input /dev/sr0 --search "Aliens" --transcode --hwaccel v4l2
```

---

## 3. Hardware Video Filter Adaptation

When video deinterlacing (`--deinterlace`) or denoising (`--denoise`) filters are enabled in conjunction with hardware acceleration, `dvd-ripper` automatically structures the FFmpeg filter graph so software filter nodes precede hardware memory uploads (`format=nv12,hwupload`), preventing pipeline crashes.
