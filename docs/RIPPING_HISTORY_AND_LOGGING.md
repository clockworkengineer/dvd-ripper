# Persistent Ripping History & Logging Guide

`dvd-ripper` maintains a persistent database log of all backup jobs in `ripping_history.json` via [`src/history.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/history.rs).

This document details the structure, schema, management tools, and library auditing capabilities enabled by the ripping history database.

---

## 1. Storage Location & Resolution

The history log is stored as a JSON array file located in:
- **User Home Directory (Default)**: `~/.dvd-ripper/ripping_history.json`
- **Local Working Directory (Fallback)**: `./ripping_history.json`

---

## 2. JSON Schema Specification

Each backup operation appends a structured record (`RipRecord`) to `ripping_history.json`.

```json
[
  {
    "timestamp": "2026-08-21 16:04:12",
    "disc_label": "KILL_BILL_VOL1",
    "title_name": "Kill Bill: Vol. 1",
    "year": 2003,
    "media_type": "Movie",
    "output_path": "Films/Kill Bill Vol. 1 (2003)/Kill Bill Vol. 1 (2003).mpg",
    "duration_secs": 6660.0,
    "file_size_bytes": 4831838208,
    "status": "Completed",
    "video_codec": "copy",
    "audio_lang": "eng",
    "subtitles_enabled": true
  }
]
```

### Field Definitions

| Field Name | Type | Description |
|---|---|---|
| `timestamp` | String | ISO-8601 formatted date and time of job execution. |
| `disc_label` | String | Optical volume label read from optical drive sector descriptors. |
| `title_name` | String | Resolved movie or TV show title name. |
| `year` | Integer | Release year resolved via OMDb/IMDb APIs. |
| `media_type` | String | Classification: `"Movie"` or `"TV Series"`. |
| `output_path` | String | Relative or absolute path to generated output file. |
| `duration_secs` | Float | Total running time of extracted video stream in seconds. |
| `file_size_bytes` | Integer | Final disk size of output file in bytes. |
| `status` | String | Final job status: `"Completed"`, `"Cancelled"`, or `"Failed"`. |
| `video_codec` | String | Video codec choice (`"copy"`, `"h264"`, `"hevc"`, `"av1"`). |
| `audio_lang` | String | Preferred audio language code (e.g. `"eng"`). |
| `subtitles_enabled` | Boolean | `true` if subtitle streams were extracted into container. |

---

## 3. Accessing & Managing History

### 3.1 Via Desktop GUI (History Tab)
1. Open `dvd-ripper` GUI.
2. Select the **📜 History Log** tab.
3. Browse past rip operations sorted chronologically.
4. Click **Clear History Log** to reset history records.

### 3.2 Via REST API
- **List Ripping History**: `GET /api/history`
- **Clear History Log**: `POST /api/history/clear`

### 3.3 Via Command-Line JSON Parsers (`jq`)
Search history database from terminal:

```bash
# View total number of completed rips
jq 'map(select(.status == "Completed")) | length' ~/.dvd-ripper/ripping_history.json

# Calculate total gigabytes extracted
jq '[.[].file_size_bytes] | add / 1073741824' ~/.dvd-ripper/ripping_history.json
```

---

## 4. Audit Trail & Duplicate Detection

The history database serves as an automated safeguard:
- Prevents re-ripping previously processed optical discs when `--no-overwrite` is active.
- Helps identify corrupted or incomplete rips (`"status": "Failed"`) for re-processing.
- Provides compliance logging for media server administration.
