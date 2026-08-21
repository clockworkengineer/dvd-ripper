# REST API & Telemetry Reference - `dvd-ripper`

The `dvd-ripper` embedded web server listens on port **8080** by default (http://localhost:8080). It provides HTTP REST endpoints, a Server-Sent Events (SSE) live progress stream, an OpenAPI 3.0 specification endpoint, and a Prometheus metrics exposition endpoint.

---

## 1. Authentication

When `--api-key <KEY>` is configured via CLI or `api_key = "..."` in `dvd-ripper.toml`, all administrative REST endpoints require authentication.

### Authentication Methods
1. **HTTP Bearer Token Header (Recommended)**:
   ```http
   Authorization: Bearer YOUR_API_KEY
   ```
2. **Query Parameter**:
   ```http
   GET /api/queue/list?api_key=YOUR_API_KEY
   ```

*Note: Public endpoints (`/`, `/index.html`, `/api/status`, `/api/openapi.json`, `/metrics`) remain accessible without an API key.*

---

## 2. API Endpoints

### 2.1 Appliance Status & Dashboard

#### `GET /api/status`
Returns the current status of the DVD appliance.

**Response `200 OK` (JSON)**:
```json
{
  "status": "Ready",
  "disc": "KILL_BILL_VOL1",
  "current_title": "Kill Bill: Vol. 1",
  "drive": "D:\\",
  "progress": 45.2,
  "fps": "28.5",
  "speed": "2.4x",
  "has_selected_movie": true,
  "is_series": false,
  "year": 2003
}
```

#### `GET /api/events`
Server-Sent Events (SSE) live progress stream broadcasting progress events every second (`Content-Type: text/event-stream`).

**Stream Event Output**:
```text
data: {"status":"Ripping","progress":52.4,"fps":"29.1","speed":"2.5x"}
```

---

### 2.2 Metadata Search & Selection

#### `GET /api/search?q={QUERY}`
Queries OMDb and IMDb online databases for movie and TV show candidate matches.

**Response `200 OK` (JSON)**:
```json
[
  {
    "imdb_id": "tt0266697",
    "title": "Kill Bill: Vol. 1",
    "year": 2003,
    "media_type": "movie",
    "poster_url": "https://m.media-amazon.com/images/M/..."
  }
]
```

#### `POST /api/select?imdb_id={IMDB_ID}`
Selects a candidate metadata entry by IMDb ID.

**Response `200 OK` (JSON)**:
```json
{
  "success": true,
  "title": "Kill Bill: Vol. 1",
  "year": 2003,
  "is_series": false
}
```

---

### 2.3 Ripping & Appliance Control

#### `POST /api/rip`
Triggers the DVD ripping process for the currently inserted disc.

**Response `200 OK` (JSON)**:
```json
{
  "success": true,
  "message": "Ripping process started"
}
```

#### `POST /api/cancel`
Cancels an active DVD ripping job.

**Response `200 OK` (JSON)**:
```json
{
  "success": true,
  "message": "Ripping process cancelled"
}
```

#### `POST /api/eject`
Ejects the optical tray.

**Response `200 OK` (JSON)**:
```json
{
  "success": true,
  "message": "Optical tray ejected"
}
```

---

### 2.4 Job Queue Management

#### `GET /api/queue/list`
Lists all queued ripping jobs.

**Response `200 OK` (JSON)**:
```json
[
  {
    "id": "job_a1b2c3d4",
    "title": "Aliens",
    "media_type": "Movie",
    "drive": "D:\\",
    "status": "Queued",
    "timestamp": "2026-08-17 16:30:00"
  }
]
```

#### `POST /api/queue/add?title={TITLE}&type={TYPE}`
Enqueues a new ripping job.

**Response `200 OK` (JSON)**:
```json
{
  "success": true,
  "job_id": "job_a1b2c3d4"
}
```

#### `POST /api/queue/remove?id={JOB_ID}`
Removes a queued job by ID.

**Response `200 OK` (JSON)**:
```json
{
  "success": true
}
```

---

### 2.5 Multi-Disc Box Set & Drive Diagnostics

#### `GET /api/boxset`
Returns a JSON array of active multi-disc TV show season box set tracking records (`boxsets.json`).

**Response `200 OK` (JSON)**:
```json
[
  {
    "show_name": "The Office",
    "season": 1,
    "last_episode": 8,
    "total_discs_ripped": 2,
    "updated_at": "2026-08-18 15:45:10"
  }
]
```

#### `POST /api/boxset/reset?show={SHOW}&season={N}`
Resets cumulative episode tracking counter for a specific show and season.

**Response `200 OK` (JSON)**:
```json
{
  "success": true,
  "show": "The Office",
  "season": 1
}
```

#### `POST /api/benchmark?drive={DRIVE}`
Executes a 10-second optical sector read speed test and returns throughput metrics and drive health rating.

**Response `200 OK` (JSON)**:
```json
{
  "drive_path": "D:\\",
  "test_duration_secs": 10,
  "read_bytes": 125829120,
  "read_speed_mbps": 12.58,
  "demux_speed_mbps": 18.20,
  "fps": "45.0",
  "rating_summary": "Standard DVD Read Speed (8x Speed)"
}
```

---

### 2.6 OpenAPI v3 Specification & Prometheus Metrics

#### `GET /api/openapi.json`
Returns the complete OpenAPI 3.0 specification JSON document describing all REST API routes and schemas.

#### `GET /metrics`
Returns standard Prometheus text exposition format metrics (`Content-Type: text/plain; version=0.0.4; charset=utf-8`):

```text
# HELP dvd_ripper_completed_rips_total Total number of successful DVD ripping jobs
# TYPE dvd_ripper_completed_rips_total counter
dvd_ripper_completed_rips_total 12

# HELP dvd_ripper_failed_rips_total Total number of failed DVD ripping jobs
# TYPE dvd_ripper_failed_rips_total counter
dvd_ripper_failed_rips_total 0

# HELP dvd_ripper_active_jobs Current number of active DVD ripping processes
# TYPE dvd_ripper_active_jobs gauge
dvd_ripper_active_jobs 1

# HELP dvd_ripper_queued_jobs Current number of pending ripping jobs in queue
# TYPE dvd_ripper_queued_jobs gauge
dvd_ripper_queued_jobs 2

# HELP dvd_ripper_progress_percent Current ripping job progress percentage
# TYPE dvd_ripper_progress_percent gauge
dvd_ripper_progress_percent 64.5
```

---

## 3. Appliance State Machine Transitions

When running in daemon mode, `GET /api/status` transitions through the following status strings:

- `"Idle"`: Drive is empty or awaiting disc insertion.
- `"Detected - Search Required"`: Disc inserted. Auto-ripping is paused awaiting title selection via `/api/select`.
- `"Ripping"`: Active extraction job running.
- `"Completed"`: Job finished successfully.
- `"Cancelled"`: Ripping process terminated by user via `/api/cancel`.

For end-to-end appliance architecture details, consult [`DAEMON_APPLIANCE_MODE.md`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/docs/DAEMON_APPLIANCE_MODE.md).
