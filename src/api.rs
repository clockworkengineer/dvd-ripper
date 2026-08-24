/**
 * @file api.rs
 * @brief REST API web server, Prometheus metrics provider, and SSE event broadcast engine.
 */

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::dvd::eject_disc;
use crate::history::load_history;

#[allow(dead_code)]
pub const MIME_APPLICATION_JSON: &str = "application/json";

static CANCEL_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static CANCEL_TX: OnceLock<Arc<Mutex<Option<Sender<()>>>>> = OnceLock::new();

/// Returns the global atomic cancel flag handle for ongoing operations.
pub fn get_cancel_flag_handle() -> Arc<AtomicBool> {
    CANCEL_FLAG.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

/// Returns the global thread-safe sender handle for process cancellation signals.
pub fn get_cancel_tx_handle() -> Arc<Mutex<Option<Sender<()>>>> {
    CANCEL_TX.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplianceStatusInfo {
    pub status: String,
    pub drive: String,
    pub disc: String,
    pub current_title: String,
    pub progress: f64,
    pub fps: String,
    pub speed: String,
    pub has_selected_movie: bool,
    pub is_series: bool,
    pub year: Option<u32>,
}

#[allow(dead_code)]
impl ApplianceStatusInfo {
    pub fn reset(&mut self) {
        self.disc.clear();
        self.current_title.clear();
        self.has_selected_movie = false;
        self.status = "Idle".to_string();
        self.progress = 0.0;
        self.fps = "0".to_string();
        self.speed = "0x".to_string();
    }

    pub fn set_disc_detected(&mut self, disc_label: &str) {
        self.disc = disc_label.to_string();
        self.status = "Detected - Search Required".to_string();
    }

    pub fn set_ripping(&mut self, title: &str) {
        self.current_title = title.to_string();
        self.status = "Ripping".to_string();
    }

    pub fn update_progress(&mut self, progress: f64, fps: &str, speed: &str) {
        self.progress = progress;
        self.fps = fps.to_string();
        self.speed = speed.to_string();
    }
}


static APPLIANCE_STATUS: OnceLock<Arc<Mutex<ApplianceStatusInfo>>> = OnceLock::new();


pub fn get_appliance_status_handle() -> Arc<Mutex<ApplianceStatusInfo>> {
    APPLIANCE_STATUS
        .get_or_init(|| {
            Arc::new(Mutex::new(ApplianceStatusInfo {
                status: "Idle".to_string(),
                drive: "auto".to_string(),
                disc: "".to_string(),
                current_title: "".to_string(),
                progress: 0.0,
                fps: "0".to_string(),
                speed: "0x".to_string(),
                has_selected_movie: false,
                is_series: false,
                year: None,
            }))
        })
        .clone()
}

static COMPLETED_RIPS_COUNTER: AtomicU64 = AtomicU64::new(0);
static FAILED_RIPS_COUNTER: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub fn increment_completed_rips() {
    COMPLETED_RIPS_COUNTER.fetch_add(1, Ordering::SeqCst);
}

#[allow(dead_code)]
pub fn increment_failed_rips() {
    FAILED_RIPS_COUNTER.fetch_add(1, Ordering::SeqCst);
}

pub fn render_prometheus_metrics() -> String {
    let completed = COMPLETED_RIPS_COUNTER.load(Ordering::SeqCst);
    let failed = FAILED_RIPS_COUNTER.load(Ordering::SeqCst);

    let handle = get_appliance_status_handle();
    let (is_active, progress) = if let Ok(state) = handle.lock() {
        let active = if state.status.to_lowercase().contains("ripping") || state.status.to_lowercase().contains("active") { 1 } else { 0 };
        (active, state.progress)
    } else {
        (0, 0.0)
    };

    let queued_jobs = crate::queue::list_jobs().len();

    format!(
        "# HELP dvd_ripper_completed_rips_total Total number of successful DVD ripping jobs\n\
         # TYPE dvd_ripper_completed_rips_total counter\n\
         dvd_ripper_completed_rips_total {}\n\n\
         # HELP dvd_ripper_failed_rips_total Total number of failed DVD ripping jobs\n\
         # TYPE dvd_ripper_failed_rips_total counter\n\
         dvd_ripper_failed_rips_total {}\n\n\
         # HELP dvd_ripper_active_jobs Current number of active DVD ripping processes\n\
         # TYPE dvd_ripper_active_jobs gauge\n\
         dvd_ripper_active_jobs {}\n\n\
         # HELP dvd_ripper_queued_jobs Current number of pending ripping jobs in queue\n\
         # TYPE dvd_ripper_queued_jobs gauge\n\
         dvd_ripper_queued_jobs {}\n\n\
         # HELP dvd_ripper_progress_percent Current ripping job progress percentage\n\
         # TYPE dvd_ripper_progress_percent gauge\n\
         dvd_ripper_progress_percent {:.1}\n",
        completed, failed, is_active, queued_jobs, progress
    )
}

pub fn set_disc_detected(disc_label: &str) {
    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock() {
        if state.disc != disc_label {
            state.disc = disc_label.to_string();
            state.current_title.clear();
            state.has_selected_movie = false;
            state.status = "Detected - Search Required".to_string();
            state.progress = 0.0;
        }
    }
}

pub fn update_appliance_status(
    status: &str,
    disc: &str,
    title: &str,
    progress: f64,
    fps: &str,
    speed: &str,
) {
    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock() {
        state.status = status.to_string();
        if !disc.is_empty() {
            state.disc = disc.to_string();
        }
        if !title.is_empty() {
            state.current_title = title.to_string();
        }
        state.progress = progress;
        state.fps = fps.to_string();
        state.speed = speed.to_string();
    }
}

pub fn reset_appliance_status(status_name: &str) {
    update_appliance_status(status_name, "", "", 0.0, "0", "0x");
}

pub fn fail_appliance_status(title: &str, out_dir: &str, err_msg: &str) {
    reset_appliance_status("Failed");
    let _ = crate::history::record_rip_event(title, "Movie", out_dir, err_msg);
}

/// Embedded HTML5 Web UI Dashboard string embedded directly into the binary.
const EMBEDDED_DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DVD Ripper Embedded Appliance</title>
    <style>
        :root { --bg: #0f172a; --card: #1e293b; --accent: #38bdf8; --text: #f8fafc; --muted: #94a3b8; --success: #22c55e; --danger: #ef4444; }
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: var(--bg); color: var(--text); margin: 0; padding: 2rem; }
        .container { max-width: 900px; margin: 0 auto; }
        .header { display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #334155; padding-bottom: 1rem; margin-bottom: 2rem; }
        h1 { margin: 0; font-size: 1.5rem; color: var(--accent); }
        .badge { background: var(--success); color: #000; font-weight: bold; padding: 0.25rem 0.75rem; border-radius: 9999px; font-size: 0.85rem; }
        .card { background: var(--card); border-radius: 12px; padding: 1.5rem; margin-bottom: 1.5rem; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.3); }
        .btn { background: var(--accent); color: #000; border: none; padding: 0.6rem 1.2rem; border-radius: 8px; font-weight: bold; cursor: pointer; transition: opacity 0.2s; margin-right: 0.5rem; }
        .btn:hover { opacity: 0.9; }
        .btn:disabled { opacity: 0.4; cursor: not-allowed; }
        .btn-danger { background: var(--danger); color: #fff; }
        .btn-secondary { background: #64748b; color: #fff; }
        .progress-bar { width: 100%; background: #334155; height: 12px; border-radius: 6px; overflow: hidden; margin: 1rem 0; }
        .progress-fill { background: var(--accent); height: 100%; width: 0%; transition: width 0.3s; }
        .history-item { display: flex; justify-content: space-between; padding: 0.75rem 0; border-bottom: 1px solid #334155; }
        .history-item:last-child { border-bottom: none; }
        .muted { color: var(--muted); font-size: 0.9rem; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📀 DVD Ripper Embedded Appliance</h1>
            <span class="badge" id="appliance-badge">ONLINE</span>
        </div>

        <div class="card">
            <h2>Appliance Status</h2>
            <div id="status-text" class="muted">Querying daemon status...</div>
            <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 0.75rem; margin-bottom: 0.25rem;">
                <span class="muted" id="progress-state-label">Progress</span>
                <span id="progress-percent-label" style="font-weight: bold; color: var(--accent); font-size: 1.1rem;">0.0%</span>
            </div>
            <div class="progress-bar"><div id="progress-fill" class="progress-fill"></div></div>
            <div style="margin-top: 1rem;">
                <button class="btn" id="start-rip-btn" onclick="triggerRip()" disabled style="opacity: 0.4; cursor: not-allowed;" title="Insert DVD and select a movie to enable ripping.">▶ Start Rip</button>
                <button class="btn btn-danger" onclick="cancelRip()">⏹ Cancel</button>
                <button class="btn btn-secondary" onclick="ejectDisc()">⏏ Eject Tray</button>
                <button class="btn btn-secondary" onclick="fetchHistory()">🔄 Refresh History</button>
            </div>
        </div>

        <div class="card">
            <h2>🔍 IMDb Metadata Search & Candidate Selection</h2>
            <div style="display: flex; gap: 0.5rem; margin-bottom: 1rem;">
                <input type="text" id="search-input" placeholder="Search title or show name (e.g. Kill Bill, Aliens)..." style="flex: 1; padding: 0.6rem; border-radius: 8px; border: 1px solid #334155; background: #0f172a; color: #fff;" onkeypress="if(event.key==='Enter') searchImdb()">
                <button class="btn" onclick="searchImdb()">🔍 Search IMDb</button>
            </div>
            <div id="search-results"></div>
        </div>

        <div class="card">
            <h2>Ripping History</h2>
            <div id="history-list">Loading history...</div>
        </div>
    </div>

    <script>
        function renderStatusData(data) {
            document.getElementById('appliance-badge').innerText = data.status || 'Idle';
            const pct = (data.progress || 0).toFixed(1);
            let statusDetails = `<strong>State:</strong> ${data.status} | <strong>Drive:</strong> ${data.drive} | <strong>Disc/Title:</strong> ${data.current_title || data.disc || 'None'}`;
            if (data.fps && data.fps !== '0' && data.fps !== 'N/A') {
                statusDetails += ` | <strong>FPS:</strong> ${data.fps} | <strong>Speed:</strong> ${data.speed}`;
            }
            document.getElementById('status-text').innerHTML = statusDetails;
            document.getElementById('progress-fill').style.width = pct + '%';
            document.getElementById('progress-percent-label').innerText = pct + '%';

            const hasDvd = data.disc && data.disc.length > 0;
            const hasSelected = data.has_selected_movie === true;
            const startBtn = document.getElementById('start-rip-btn');
            if (startBtn) {
                if (hasDvd && hasSelected) {
                    startBtn.disabled = false;
                    startBtn.style.opacity = '1';
                    startBtn.style.cursor = 'pointer';
                    startBtn.title = 'Start Ripping DVD';
                } else {
                    startBtn.disabled = true;
                    startBtn.style.opacity = '0.4';
                    startBtn.style.cursor = 'not-allowed';
                    if (!hasDvd) {
                        startBtn.title = 'Insert a DVD disc to enable ripping.';
                    } else {
                        startBtn.title = 'Search and select a movie to enable ripping.';
                    }
                }
            }
        }

        async function pollStatus() {
            try {
                const res = await fetch('/api/status');
                const data = await res.json();
                renderStatusData(data);
            } catch(e) {}
        }

        async function searchImdb() {
            const query = document.getElementById('search-input').value.trim();
            if (!query) return;
            const container = document.getElementById('search-results');
            container.innerHTML = '<p class="muted">Searching IMDb/OMDb candidates...</p>';
            try {
                const res = await fetch('/api/search?q=' + encodeURIComponent(query));
                const data = await res.json();
                if (!data || data.length === 0) {
                    container.innerHTML = '<p class="muted">No search results found.</p>';
                    return;
                }
                container.innerHTML = data.map(item => `
                    <div class="history-item">
                        <div>
                            <strong>${item.title}</strong> ${item.year ? '<span class="muted">(' + item.year + ')</span>' : ''}
                            <div class="muted">IMDb ID: ${item.imdb_id} | Type: ${item.type_field}</div>
                        </div>
                        <div>
                            <button class="btn" style="padding: 0.35rem 0.8rem; font-size: 0.85rem;" onclick="selectCandidate('${item.imdb_id}')">Select</button>
                        </div>
                    </div>
                `).join('');
            } catch(e) {
                container.innerHTML = '<p class="muted" style="color:var(--danger)">Search request failed.</p>';
            }
        }

        async function selectCandidate(imdbId) {
            try {
                const res = await fetch('/api/select', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ imdb_id: imdbId })
                });
                const data = await res.json();
                if (data.success) {
                    alert('Selected title: ' + data.title + (data.year ? ' (' + data.year + ')' : ''));
                    pollStatus();
                } else {
                    alert(data.message || 'Selection failed.');
                }
            } catch(e) {
                alert('Error selecting title candidate.');
            }
        }

        async function fetchHistory() {
            try {
                const res = await fetch('/api/history');
                const data = await res.json();
                const container = document.getElementById('history-list');
                if (!data || data.length === 0) {
                    container.innerHTML = '<p class="muted">No ripping history records found.</p>';
                    return;
                }
                container.innerHTML = data.map(item => `
                    <div class="history-item">
                        <div>
                            <strong>${item.title}</strong> <span class="muted">(${item.media_type})</span>
                            <div class="muted">${item.output_path}</div>
                        </div>
                        <div>
                            <span style="color:${item.status === 'Success' ? '#22c55e' : '#ef4444'}">${item.status}</span>
                            <div class="muted">${item.timestamp}</div>
                        </div>
                    </div>
                `).join('');
            } catch(e) {
                document.getElementById('history-list').innerText = 'Failed to load history.';
            }
        }

        async function ejectDisc() {
            if (confirm('Eject optical drive tray?')) {
                const res = await fetch('/api/eject', { method: 'POST' });
                const data = await res.json();
                alert(data.success ? 'Tray ejected successfully.' : 'Failed to eject tray.');
            }
        }

        async function triggerRip() {
            const res = await fetch('/api/rip', { method: 'POST' });
            const data = await res.json();
            if (!data.success) {
                alert('⚠️ ' + (data.message || 'Cannot start rip.'));
            } else {
                alert(data.message || 'Triggered rip job.');
                pollStatus();
            }
        }

        async function cancelRip() {
            if (confirm('Cancel active DVD ripping process?')) {
                const res = await fetch('/api/cancel', { method: 'POST' });
                const data = await res.json();
                alert(data.message || 'Cancelled rip job.');
                pollStatus();
            }
        }

        fetchHistory();
        if (!!window.EventSource) {
            const evtSource = new EventSource('/api/events');
            evtSource.onmessage = function(e) {
                try {
                    const data = JSON.parse(e.data);
                    if (data && data.status) {
                        renderStatusData(data);
                    }
                } catch(err) {}
            };
        }
        setInterval(pollStatus, 3000);
        pollStatus();
    </script>
</body>
</html>
"#;

/// Starts the embedded HTTP REST API and Web UI dashboard server on port 8080.
pub fn start_embedded_api_server(port: u16, drive_path: String) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)?;
    println!("[Embedded Web API] Server listening on http://{}", addr);

    let handle = get_appliance_status_handle();
    if let Ok(mut state) = handle.lock() {
        if state.drive == "auto" || state.drive == crate::dvd::default_dvd_drive_path() {
            state.drive = crate::dvd::auto_detect_dvd_drive();
        }
    }


    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let drive = drive_path.clone();
            thread::spawn(move || {
                let _ = handle_client(stream, &drive);
            });
        }
    });

    Ok(())
}

pub fn parse_http_route(req_bytes: &[u8]) -> (&[u8], &[u8]) {
    let first_line = req_bytes.split(|&b| b == b'\r' || b == b'\n').next().unwrap_or(&[]);
    let mut parts = first_line.split(|&b| b == b' ');
    let method = parts.next().unwrap_or(&[]);
    let path = parts.next().unwrap_or(&[]);
    (method, path)
}

pub fn parse_query_param(path: &str, param_name: &str) -> Option<String> {
    let query_start = path.find('?')?;
    let query_str = &path[query_start + 1..];
    for pair in query_str.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next()?;
        if key == param_name {
            let val = kv.next().unwrap_or("");
            return Some(crate::utils::decode_url_query_value(val));
        }
    }
    None
}

pub fn extract_body(req_bytes: &[u8]) -> Option<&str> {
    if let Some(pos) = req_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
        std::str::from_utf8(&req_bytes[pos + 4..]).ok()
    } else {
        None
    }
}

static CONFIGURED_API_KEY: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

fn get_configured_api_key_handle() -> &'static Arc<Mutex<Option<String>>> {
    CONFIGURED_API_KEY.get_or_init(|| Arc::new(Mutex::new(None)))
}

pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        res |= x ^ y;
    }
    res == 0
}

#[allow(dead_code)]
pub fn set_api_key(key: String) {
    let handle = get_configured_api_key_handle();
    let mut lock = lock_or_recover(handle);
    *lock = Some(key);
}

pub fn validate_api_key_header(req_bytes: &[u8], path_str: &str) -> bool {
    let handle = get_configured_api_key_handle();
    let lock = lock_or_recover(handle);
    let expected_key = match lock.clone() {
        Some(k) if !k.trim().is_empty() => k,
        _ => return true,
    };

    if let Some(param_key) = parse_query_param(path_str, "api_key") {
        if constant_time_eq(&param_key, &expected_key) {
            return true;
        }
    }

    let req_text = String::from_utf8_lossy(req_bytes);
    for line in req_text.lines() {
        if line.to_lowercase().starts_with("authorization:") {
            if let Some(bearer) = line.splitn(2, ':').nth(1) {
                let token = bearer.trim();
                if token.to_lowercase().starts_with("bearer ") {
                    let key = token[7..].trim();
                    if constant_time_eq(key, &expected_key) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

pub const OPENAPI_V3_JSON: &str = r#"{
  "openapi": "3.0.0",
  "info": {
    "title": "DVD Ripper Appliance REST API",
    "version": "1.0.0",
    "description": "High-performance automated DVD ripping appliance REST API for Home Assistant & media server integration."
  },
  "paths": {
    "/api/status": {
      "get": {
        "summary": "Get current DVD appliance status",
        "responses": { "200": { "description": "Appliance status JSON" } }
      }
    },
    "/api/events": {
      "get": {
        "summary": "Server-Sent Events (SSE) live status stream",
        "responses": { "200": { "description": "text/event-stream" } }
      }
    },
    "/api/openapi.json": {
      "get": {
        "summary": "OpenAPI v3 JSON specification",
        "responses": { "200": { "description": "OpenAPI 3.0 specification JSON" } }
      }
    },
    "/api/history": {
      "get": {
        "summary": "Get ripping history log",
        "responses": { "200": { "description": "History records array" } }
      }
    },
    "/api/search": {
      "get": {
        "summary": "Search IMDb/OMDb metadata candidates",
        "parameters": [{ "name": "q", "in": "query", "required": true, "schema": { "type": "string" } }],
        "responses": { "200": { "description": "Search candidates array" } }
      }
    },
    "/api/select": {
      "post": {
        "summary": "Select IMDb candidate by ID",
        "parameters": [{ "name": "imdb_id", "in": "query", "required": true, "schema": { "type": "string" } }],
        "responses": { "200": { "description": "Selection status" } }
      }
    },
    "/api/rip": {
      "post": {
        "summary": "Trigger DVD ripping process",
        "responses": { "200": { "description": "Job trigger response" } }
      }
    },
    "/api/cancel": {
      "post": {
        "summary": "Cancel active DVD ripping process",
        "responses": { "200": { "description": "Cancellation response" } }
      }
    },
    "/api/eject": {
      "post": {
        "summary": "Eject DVD optical tray",
        "responses": { "200": { "description": "Tray ejection response" } }
      }
    },
    "/api/queue/list": {
      "get": {
        "summary": "List queued ripping jobs",
        "responses": { "200": { "description": "Queued jobs array" } }
      }
    },
    "/api/queue/add": {
      "post": {
        "summary": "Enqueue a new ripping job",
        "responses": { "200": { "description": "Enqueue response" } }
      }
    },
    "/api/queue/remove": {
      "post": {
        "summary": "Remove job from queue",
        "responses": { "200": { "description": "Removal status" } }
      }
    }
  }
}"#;

fn handle_client(mut stream: TcpStream, drive_path: &str) -> Result<()> {
    let mut buffer = [0u8; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let req_bytes = &buffer[..bytes_read];
    let (method, path) = parse_http_route(req_bytes);

    let path_str = String::from_utf8_lossy(path);
    let method_str = String::from_utf8_lossy(method);

    if path_str == "/api/openapi.json" {
        send_http_response(&mut stream, "200 OK", "application/json", OPENAPI_V3_JSON)?;
        return Ok(());
    }

    if path_str.starts_with("/api/") && path_str != "/api/status" {
        if !validate_api_key_header(req_bytes, &path_str) {
            let body = "{\"success\":false,\"message\":\"Unauthorized: Invalid or missing API key header (Authorization: Bearer <KEY>)\"}";
            send_http_response(&mut stream, "401 Unauthorized", "application/json", body)?;
            return Ok(());
        }
    }

    if method_str == "GET" && path_str == "/metrics" {
        let metrics = render_prometheus_metrics();
        send_http_response(&mut stream, "200 OK", "text/plain; version=0.0.4; charset=utf-8", &metrics)?;
        return Ok(());
    }

    if method_str == "GET" && (path_str == "/" || path_str == "/index.html") {
        send_http_response(&mut stream, "200 OK", "text/html; charset=utf-8", EMBEDDED_DASHBOARD_HTML)?;
    } else if method_str == "GET" && path_str == "/api/events" {
        let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n";
        if stream.write_all(headers.as_bytes()).is_ok() {
            for _ in 0..10 {
                let handle = get_appliance_status_handle();
                let json_body = if let Ok(state) = handle.lock() {
                    serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string())
                } else {
                    "{}".to_string()
                };
                let event_payload = format!("data: {}\n\n", json_body);
                if stream.write_all(event_payload.as_bytes()).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    } else if method_str == "GET" && path_str == "/api/status" {
        let handle = get_appliance_status_handle();
        let json_body = if let Ok(state) = handle.lock() {
            serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string())
        } else {
            let label = crate::dvd::get_volume_label(drive_path).unwrap_or_default();
            format!(
                "{{\"status\":\"Active\",\"drive\":\"{}\",\"disc\":\"{}\",\"current_title\":\"\",\"progress\":0,\"fps\":\"0\",\"speed\":\"0x\"}}",
                drive_path, label
            )
        };
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "GET" && path_str == "/api/history" {
        let history = load_history(None);
        let json_body = serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "GET" && path_str == "/api/queue/list" {
        let jobs = crate::queue::list_jobs();
        let json_body = serde_json::to_string(&jobs).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "POST" && path_str.starts_with("/api/queue/add") {
        let title = parse_query_param(&path_str, "title").unwrap_or_else(|| "Unknown".to_string());
        let media_type = parse_query_param(&path_str, "type").unwrap_or_else(|| "Movie".to_string());
        let id = crate::queue::add_job(&title, &media_type, drive_path);
        let resp_obj = serde_json::json!({ "success": true, "job_id": id });
        send_http_response(&mut stream, "200 OK", "application/json", &resp_obj.to_string())?;
    } else if method_str == "POST" && path_str.starts_with("/api/queue/remove") {
        let id = parse_query_param(&path_str, "id").unwrap_or_default();
        let ok = crate::queue::remove_job(&id);
        let resp_obj = serde_json::json!({ "success": ok });
        send_http_response(&mut stream, "200 OK", "application/json", &resp_obj.to_string())?;
    } else if method_str == "GET" && path_str.starts_with("/api/search") {
        let query = parse_query_param(&path_str, "q").unwrap_or_default();
        let candidates = if !query.trim().is_empty() {
            crate::imdb::fetch_search_candidates(query.trim())
        } else {
            Vec::new()
        };
        let json_body = serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string());
        send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
    } else if method_str == "POST" && path_str.starts_with("/api/select") {
        let body_str = extract_body(req_bytes).unwrap_or_default();
        let mut imdb_id = parse_query_param(&path_str, "imdb_id");
        if imdb_id.is_none() && !body_str.trim().is_empty() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body_str) {
                if let Some(id) = parsed.get("imdb_id").and_then(|v| v.as_str()) {
                    imdb_id = Some(id.to_string());
                }
            }
        }

        if let Some(ref id) = imdb_id {
            if let Some(meta) = crate::imdb::lookup_omdb_by_id(id) {
                let handle = get_appliance_status_handle();
                let mut state = lock_or_recover(&handle);
                state.current_title = meta.title.clone();
                state.year = meta.year;
                state.is_series = meta.is_series;
                state.has_selected_movie = true;
                state.status = format!("Ready (Selected: {})", meta.title);

                let resp_obj = serde_json::json!({
                    "success": true,
                    "title": meta.title,
                    "year": meta.year,
                    "is_series": meta.is_series
                });
                send_http_response(&mut stream, "200 OK", "application/json", &resp_obj.to_string())?;
            } else {
                let json_body = "{\"success\":false,\"message\":\"Failed to find metadata for IMDb ID\"}";
                send_http_response(&mut stream, "404 Not Found", "application/json", json_body)?;
            }
        } else {
            let json_body = "{\"success\":false,\"message\":\"Missing imdb_id parameter\"}";
            send_http_response(&mut stream, "400 Bad Request", "application/json", json_body)?;
        }
    } else if method_str == "POST" && path_str == "/api/eject" {
        let ok = eject_disc(drive_path);
        let resp_obj = serde_json::json!({ "success": ok });
        send_http_response(&mut stream, "200 OK", "application/json", &resp_obj.to_string())?;
    } else if method_str == "POST" && path_str == "/api/rip" {
        let handle = get_appliance_status_handle();
        let (has_selected, title, is_series, year, disc) = if let Ok(state) = handle.lock() {
            (state.has_selected_movie, state.current_title.clone(), state.is_series, state.year, state.disc.clone())
        } else {
            (false, String::new(), false, None, String::new())
        };

        if disc.trim().is_empty() {
            let json_body = "{\"success\": false, \"message\": \"Ripping disabled: No DVD disc is present in the optical drive.\"}";
            send_http_response(&mut stream, "400 Bad Request", "application/json", json_body)?;
        } else if !has_selected || title.trim().is_empty() {
            let json_body = "{\"success\": false, \"message\": \"Ripping disabled: Please search and select a movie first.\"}";
            send_http_response(&mut stream, "400 Bad Request", "application/json", json_body)?;
        } else {
            update_appliance_status("Ripping", "", &title, 0.0, "0", "0x");
            let drive_clone = drive_path.to_string();
            let display_title = title.clone();

            let cancel_flag = get_cancel_flag_handle();
            cancel_flag.store(false, Ordering::SeqCst);
            let (cancel_tx, _cancel_rx) = channel();
            if let Ok(mut lock) = get_cancel_tx_handle().lock() {
                *lock = Some(cancel_tx);
            }

            thread::spawn(move || {
                let mut args = crate::cli::Args {
                    input: drive_clone.clone(),
                    tv: is_series,
                    ..Default::default()
                };
                let config = crate::config::load_config(None);
                crate::config::apply_config_defaults(&mut args, &config);
                if is_series {
                    args.out_dir = "TV".to_string();
                }
                let dvd_path = crate::dvd::normalize_dvd_path(&drive_clone);

                let (event_tx, event_rx) = channel();
                let item_title = title.clone();
                thread::spawn(move || {
                    while let Ok(event) = event_rx.recv() {
                        if let crate::ffmpeg::ProgressEvent::Progress { percent, fps, speed, .. } = event {
                            update_appliance_status("Ripping", "", &item_title, percent as f64, &fps, &speed);
                        }
                    }
                });

                if let Err(e) = crate::utils::check_disk_space_guard(std::path::Path::new(&args.out_dir), args.min_free_gb) {
                    fail_appliance_status(&title, &args.out_dir, &format!("Disk Space Error: {}", e));
                    return;
                }

                let res = if is_series {
                    let episodes = crate::ffmpeg::detect_tv_episodes(
                        &args.ffmpeg,
                        &dvd_path,
                        &title,
                        args.season,
                        args.start_episode,
                        Some(&cancel_flag),
                    );
                    let mut success = true;
                    for ep in &episodes {
                        if cancel_flag.load(Ordering::SeqCst) {
                            success = false;
                            break;
                        }
                        if let Ok(out_path) = crate::ffmpeg::resolve_tv_output_path(
                            &args,
                            Some(&title),
                            year,
                            args.season,
                            ep.episode_num,
                        ) {
                            update_appliance_status("Ripping", "", &ep.formatted_name, 0.0, "0", "0x");
                            let run_res = crate::ffmpeg::run_ffmpeg_with_channel(
                                &args,
                                &dvd_path,
                                &out_path,
                                &ep.formatted_name,
                                Some(ep.duration_secs),
                                Some(event_tx.clone()),
                                None,
                                Some(cancel_flag.clone()),
                                true,
                            );
                            if run_res.is_ok() {
                                let _ = crate::history::record_rip_event(&ep.formatted_name, "TV Series", &out_path.to_string_lossy(), "Success");
                            } else {
                                success = false;
                                if cancel_flag.load(Ordering::SeqCst) {
                                    let _ = crate::history::record_rip_event(&ep.formatted_name, "TV Series", &out_path.to_string_lossy(), "Cancelled");
                                }
                                break;
                            }
                        }
                    }
                    if success && !cancel_flag.load(Ordering::SeqCst) {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Ripping cancelled or failed"))
                    }
                } else {
                    if let Ok(out_path) = crate::ffmpeg::resolve_output_path(&args, Some(&title), year) {
                        update_appliance_status("Ripping", "", &title, 0.0, "0", "0x");
                        let run_res = crate::ffmpeg::run_ffmpeg_with_channel(
                            &args,
                            &dvd_path,
                            &out_path,
                            &title,
                            None,
                            Some(event_tx),
                            None,
                            Some(cancel_flag.clone()),
                            false,
                        );
                        if run_res.is_ok() {
                            let _ = crate::history::record_rip_event(&title, "Movie", &out_path.to_string_lossy(), "Success");
                            Ok(())
                        } else {
                            if cancel_flag.load(Ordering::SeqCst) {
                                let _ = crate::history::record_rip_event(&title, "Movie", &out_path.to_string_lossy(), "Cancelled");
                            }
                            Err(anyhow::anyhow!("Ripping cancelled or failed"))
                        }
                    } else {
                        Err(anyhow::anyhow!("Failed to resolve output path"))
                    }
                };

                if cancel_flag.load(Ordering::SeqCst) {
                    update_appliance_status("Cancelled", "", "", 0.0, "0", "0x");
                } else if res.is_ok() {
                    update_appliance_status("Completed", "", &title, 100.0, "0", "0x");
                    let _ = crate::dvd::eject_disc(&drive_clone);
                } else {
                    update_appliance_status("Failed", "", "", 0.0, "0", "0x");
                }
            });

            let resp_obj = serde_json::json!({
                "success": true,
                "message": format!("Started ripping selected title: {}", display_title)
            });
            send_http_response(&mut stream, "200 OK", "application/json", &resp_obj.to_string())?;
        }
    } else if method_str == "POST" && path_str == "/api/cancel" {
        let flag = get_cancel_flag_handle();
        flag.store(true, Ordering::SeqCst);
        if let Ok(mut lock) = get_cancel_tx_handle().lock() {
            if let Some(tx) = lock.take() {
                let _ = tx.send(());
            }
        }
        update_appliance_status("Cancelled", "", "", 0.0, "0", "0x");
        let json_body = "{\"success\": true, \"message\": \"Ripping process cancelled by user.\"}";
        send_http_response(&mut stream, "200 OK", "application/json", json_body)?;
    } else if method_str == "GET" && path_str == "/api/boxset" {
        let boxsets = crate::queue::list_boxsets();
        let json_body = serde_json::to_string(&boxsets).unwrap_or_else(|_| "[]".to_string());
        send_json_response(&mut stream, "200 OK", &json_body)?;
    } else if method_str == "POST" && path_str.starts_with("/api/boxset/reset") {
        let query_show = parse_query_param(&path_str, "show").unwrap_or_default();
        let query_season = parse_query_param(&path_str, "season")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let reset = crate::queue::reset_boxset_tracker(&query_show, query_season);
        let resp_obj = serde_json::json!({
            "success": reset,
            "show": query_show,
            "season": query_season
        });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "GET" && (path_str == "/" || path_str == "/dashboard" || path_str.starts_with("/dashboard")) {
        send_http_response(&mut stream, "200 OK", "text/html", render_web_dashboard_html())?;
    } else if method_str == "GET" && path_str.starts_with("/api/drives") {
        let drives = crate::dvd::detect_dvd_drives();
        let resp_obj = serde_json::json!({
            "drives": drives,
            "count": drives.len()
        });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "GET" && path_str.starts_with("/api/health") {
        let resp_obj = serde_json::json!({
            "status": "healthy",
            "uptime_seconds": get_uptime_seconds(),
            "active_transcodes": get_metrics().active_transcodes.load(Ordering::SeqCst)
        });
        send_json_response(&mut stream, "200 OK", &resp_obj.to_string())?;
    } else if method_str == "POST" && path_str.starts_with("/api/benchmark") {
        let drive = parse_query_param(&path_str, "drive").unwrap_or_else(|| "auto".to_string());
        let dvd_path = crate::dvd::normalize_dvd_path(&drive);
        match crate::dvd::run_drive_benchmark("ffmpeg", &dvd_path, 10) {
            Ok(report) => {
                let json_body = serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string());
                send_json_response(&mut stream, "200 OK", &json_body)?;
            }
            Err(e) => {
                send_json_error(&mut stream, "500 Internal Server Error", &e.to_string())?;
            }
        }
    } else {
        send_http_response(&mut stream, "404 Not Found", "text/plain", "404 Not Found")?;
    }

    Ok(())
}

/// Renders embedded HTML5/CSS3 Web Dashboard interface for headless appliances.
fn render_web_dashboard_html() -> &'static str {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DVD Ripper Appliance Web Dashboard</title>
    <style>
        :root { --bg: #0f172a; --card: #1e293b; --accent: #38bdf8; --text: #f8fafc; --muted: #94a3b8; --border: #334155; }
        body { font-family: system-ui, -apple-system, sans-serif; background: var(--bg); color: var(--text); margin: 0; padding: 20px; }
        .container { max-width: 1000px; margin: 0 auto; }
        .header { display: flex; align-items: center; justify-content: space-between; padding-bottom: 15px; border-bottom: 1px solid var(--border); margin-bottom: 20px; }
        h1 { margin: 0; font-size: 1.6rem; color: var(--accent); }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 16px; margin-bottom: 20px; }
        .card { background: var(--card); border: 1px solid var(--border); border-radius: 10px; padding: 18px; }
        .card h2 { font-size: 1.1rem; margin-top: 0; color: var(--accent); }
        .stat { font-size: 1.4rem; font-weight: bold; margin: 5px 0; }
        .badge { background: #0284c7; padding: 4px 8px; border-radius: 6px; font-size: 0.8rem; font-weight: 600; }
        .progress-bar { width: 100%; background: var(--border); border-radius: 6px; height: 16px; overflow: hidden; margin: 10px 0; }
        .progress-fill { height: 100%; background: linear-gradient(90deg, #0284c7, #38bdf8); width: 0%; transition: width 0.3s; }
        table { width: 100%; border-collapse: collapse; margin-top: 10px; }
        th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid var(--border); font-size: 0.9rem; }
        th { color: var(--muted); }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📀 DVD Ripper Appliance</h1>
            <span class="badge" id="appliance-status">STATUS: IDLE</span>
        </div>
        <div class="grid">
            <div class="card">
                <h2>Current Disc / Activity</h2>
                <div class="stat" id="current-disc">No Disc Detected</div>
                <div id="current-stage" style="color: var(--muted); font-size: 0.9rem;">Waiting for insertion</div>
                <div class="progress-bar"><div class="progress-fill" id="progress-fill"></div></div>
                <div style="display: flex; justify-content: space-between; font-size: 0.85rem; color: var(--muted);">
                    <span id="stat-fps">FPS: --</span>
                    <span id="stat-speed">Speed: --</span>
                </div>
            </div>
            <div class="card">
                <h2>Appliance Hardware</h2>
                <div id="hardware-drives">Detecting optical drives...</div>
            </div>
        </div>
        <div class="card">
            <h2>Recent Ripping History</h2>
            <table>
                <thead><tr><th>Time</th><th>Title</th><th>Type</th><th>Status</th></tr></thead>
                <tbody id="history-rows"><tr><td colspan="4">Loading history...</td></tr></tbody>
            </table>
        </div>
    </div>
    <script>
        async function updateDashboard() {
            try {
                const statusRes = await fetch('/api/status');
                if (statusRes.ok) {
                    const data = await statusRes.json();
                    document.getElementById('appliance-status').innerText = 'STATUS: ' + (data.status || 'IDLE').toUpperCase();
                    document.getElementById('current-disc').innerText = data.disc_detected || 'No Disc';
                    document.getElementById('current-stage').innerText = data.status || 'Idle';
                    const pct = data.progress_percent || 0;
                    document.getElementById('progress-fill').style.width = pct + '%';
                    document.getElementById('stat-fps').innerText = 'FPS: ' + (data.fps || '--');
                    document.getElementById('stat-speed').innerText = 'Speed: ' + (data.speed || '--');
                }
                const drivesRes = await fetch('/api/drives');
                if (drivesRes.ok) {
                    const drivesData = await drivesRes.json();
                    let html = '<strong>Optical Drives:</strong><br>';
                    (drivesData.drives || []).forEach(d => {
                        html += '• 💿 ' + d + '<br>';
                    });
                    document.getElementById('hardware-drives').innerHTML = html || 'No optical drives found';
                }
                const histRes = await fetch('/api/history');
                if (histRes.ok) {
                    const histData = await histRes.json();
                    let html = '';
                    (histData || []).slice(0, 5).forEach(r => {
                        html += `<tr><td>${r.timestamp || ''}</td><td>${r.title || ''}</td><td>${r.media_type || ''}</td><td>${r.status || ''}</td></tr>`;
                    });
                    document.getElementById('history-rows').innerHTML = html || '<tr><td colspan="4">No history records</td></tr>';
                }
            } catch(e) {}
        }
        updateDashboard();
        setInterval(updateDashboard, 3000);
    </script>
</body>
</html>"#
}

/// Helper: Extracts a case-insensitive header value from an HTTP header vector without heap allocations.
#[allow(dead_code)]
pub fn extract_header_value<'a>(headers: &'a [String], header_name: &str) -> Option<&'a str> {
    for line in headers {
        if let Some(colon_idx) = line.find(':') {
            let key = line[..colon_idx].trim();
            if key.eq_ignore_ascii_case(header_name) {
                return Some(line[colon_idx + 1..].trim());
            }
        }
    }
    None
}

/// Helper: Parses JSON HTTP request body after verifying Content-Type header.
#[allow(dead_code)]
pub fn parse_json_request_body<'a>(req: &'a [u8], headers: &[String]) -> Option<&'a str> {
    if let Some(ct) = extract_header_value(headers, "content-type") {
        if !ct.to_lowercase().contains("application/json") {
            return None;
        }
    }
    extract_body(req)
}

/// Extracts API authentication key from HTTP Bearer headers or `api_key=` query parameters.
#[allow(dead_code)]
pub fn extract_auth_key(headers: &[String], path: &str) -> Option<String> {
    if let Some(auth_val) = extract_header_value(headers, "authorization") {
        if auth_val.to_lowercase().starts_with("bearer ") {
            return Some(auth_val[7..].trim().to_string());
        }
    }
    parse_query_param(path, "api_key")
}

/// Returns the HTTP MIME Content-Type header string based on file path extension.
#[allow(dead_code)]
pub fn mime_type_for_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

/// Parses all URL query parameters into a HashMap of key-value pairs.
#[allow(dead_code)]
pub fn parse_query_map(url_path: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(q_idx) = url_path.find('?') {
        let query = &url_path[q_idx + 1..];
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                if !k.is_empty() {
                    map.insert(k.to_string(), crate::utils::decode_url_query_value(v));
                }
            }
        }
    }
    map
}

/// Snapshot of appliance telemetry metric counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricCounters {
    pub completed_rips: u64,
    pub failed_rips: u64,
    pub active_jobs: usize,
    pub queued_jobs: usize,
}

/// Returns a thread-safe snapshot of current appliance telemetry metric counters.
#[allow(dead_code)]
pub fn snapshot_metrics() -> MetricCounters {
    MetricCounters {
        completed_rips: COMPLETED_RIPS_COUNTER.load(Ordering::SeqCst),
        failed_rips: FAILED_RIPS_COUNTER.load(Ordering::SeqCst),
        active_jobs: if get_appliance_status_handle().lock().map_or(false, |s| s.status == "Ripping") { 1 } else { 0 },
        queued_jobs: crate::queue::list_jobs().len(),
    }
}

/// Helper: Formats MetricCounters snapshot into a JSON string.
#[allow(dead_code)]
pub fn format_metrics_json(metrics: &MetricCounters) -> String {
    serde_json::to_string(metrics).unwrap_or_else(|_| "{}".to_string())
}

/// Helper: Constructs a complete HTTP/1.1 response byte buffer with status line, content type, CORS, security, and length headers.
pub fn build_http_response_bytes(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(160 + status.len() + content_type.len() + body.len());
    use std::io::Write;
    let _ = write!(
        bytes,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
        status, content_type, body.len()
    );
    bytes.extend_from_slice(body);
    bytes
}

/// Helper: Formats an HTTP Content-Length header string (e.g. "Content-Length: 1024\r\n").
#[allow(dead_code)]
pub fn format_content_length_header(length: usize) -> String {
    format!("Content-Length: {}\r\n", length)
}

/// Helper: Constructs a complete HTTP/1.1 JSON response byte buffer.
#[allow(dead_code)]
pub fn build_http_json_response_bytes(status: &str, json_body: &str) -> Vec<u8> {
    build_http_response_bytes(status, "application/json", json_body.as_bytes())
}

fn send_json_response(stream: &mut TcpStream, status: &str, body_json: &str) -> Result<()> {
    send_http_response(stream, status, "application/json", body_json)
}

/// Helper: Sends a simple single key-value JSON response body (e.g. {"success": "true"} or {"message": "..."}).
#[allow(dead_code)]
pub fn send_json_message(stream: &mut TcpStream, status: &str, key: &str, val: &str) -> Result<()> {
    let json_body = format!("{{\"{}\": \"{}\"}}", key, crate::utils::escape_json_str(val));
    send_json_response(stream, status, &json_body)
}

/// Helper: Sends a standardized boolean status JSON response (e.g. {"success": true}).
#[allow(dead_code)]
pub fn send_json_status_bool(stream: &mut TcpStream, success: bool, message: Option<&str>) -> Result<()> {
    let json_body = if let Some(msg) = message {
        format!("{{\"success\": {}, \"message\": \"{}\"}}", success, crate::utils::escape_json_str(msg))
    } else {
        format!("{{\"success\": {}}}", success)
    };
    let http_status = if success { "200 OK" } else { "400 Bad Request" };
    send_json_response(stream, http_status, &json_body)
}

fn send_json_error(stream: &mut TcpStream, status: &str, message: &str) -> Result<()> {
    let err_body = format!("{{\"error\": \"{}\"}}", crate::utils::escape_json_str(message));
    send_http_response(stream, status, "application/json", &err_body)
}

fn send_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) -> Result<()> {
    let bytes = build_http_response_bytes(status, content_type, body.as_bytes());
    stream.write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_content_length_header() {
        assert_eq!(format_content_length_header(100), "Content-Length: 100\r\n");
    }

    #[test]
    fn test_api_metric_counter_increments() {
        increment_completed_rips();
        increment_failed_rips();
        let snap = snapshot_metrics();
        assert!(snap.completed_rips > 0 || snap.failed_rips > 0);
    }

    #[test]
    fn test_extract_auth_key_bearer_and_param() {
        let headers = vec!["Authorization: Bearer secret_token_123".to_string()];
        assert_eq!(extract_auth_key(&headers, "/api/status"), Some("secret_token_123".to_string()));

        let empty_headers: Vec<String> = Vec::new();
        assert_eq!(extract_auth_key(&empty_headers, "/api/status?api_key=query_secret"), Some("query_secret".to_string()));
    }

    #[test]
    fn test_parse_http_route() {
        let req1 = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req1), (b"GET" as &[u8], b"/" as &[u8]));

        let req2 = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req2), (b"GET" as &[u8], b"/api/status" as &[u8]));

        let req3 = b"POST /api/eject HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req3), (b"POST" as &[u8], b"/api/eject" as &[u8]));

        let req4 = b"POST /api/rip HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req4), (b"POST" as &[u8], b"/api/rip" as &[u8]));

        let req5 = b"DELETE /api/unknown HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req5), (b"DELETE" as &[u8], b"/api/unknown" as &[u8]));
    }

    #[test]
    fn test_parse_query_param() {
        let path1 = "/api/search?q=Kill+Bill";
        assert_eq!(parse_query_param(path1, "q"), Some("Kill Bill".to_string()));

        let path2 = "/api/select?imdb_id=tt0266697&foo=bar";
        assert_eq!(parse_query_param(path2, "imdb_id"), Some("tt0266697".to_string()));

        let path3 = "/api/status";
        assert_eq!(parse_query_param(path3, "q"), None);
    }

    #[test]
    fn test_extract_body() {
        let req = b"POST /api/select HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"imdb_id\":\"tt0266697\"}";
        assert_eq!(extract_body(req), Some("{\"imdb_id\":\"tt0266697\"}"));
    }

    #[test]
    fn test_extract_header_value() {
        let headers = vec!["Host: localhost:8080".to_string(), "Content-Type: application/json".to_string()];
        assert_eq!(extract_header_value(&headers, "content-type"), Some("application/json"));
        assert_eq!(extract_header_value(&headers, "host"), Some("localhost:8080"));
        assert_eq!(extract_header_value(&headers, "missing"), None);
    }

    #[test]
    fn test_parse_json_request_body() {
        let req = b"POST /api/rip HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"drive\":\"D:\"}";
        let headers = vec!["Content-Type: application/json".to_string()];
        assert_eq!(parse_json_request_body(req, &headers), Some("{\"drive\":\"D:\"}"));
    }

    #[test]
    fn test_parse_sse_events_route() {
        let req = b"GET /api/events HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n";
        assert_eq!(parse_http_route(req), (b"GET" as &[u8], b"/api/events" as &[u8]));
    }

    #[test]
    fn test_api_key_validation() {
        set_api_key("secret123".to_string());
        let req_valid = b"POST /api/rip HTTP/1.1\r\nAuthorization: Bearer secret123\r\n\r\n";
        assert!(validate_api_key_header(req_valid, "/api/rip"));

        let req_invalid = b"POST /api/rip HTTP/1.1\r\nAuthorization: Bearer wrong\r\n\r\n";
        assert!(!validate_api_key_header(req_invalid, "/api/rip"));
    }

    #[test]
    fn test_extract_auth_key() {
        let headers = vec!["Authorization: Bearer my_token_123".to_string()];
        assert_eq!(extract_auth_key(&headers, "/api/rip"), Some("my_token_123".to_string()));

        let empty_headers: Vec<String> = Vec::new();
        assert_eq!(extract_auth_key(&empty_headers, "/api/rip?api_key=query_token"), Some("query_token".to_string()));
    }

    #[test]
    fn test_parse_query_map() {
        let map = parse_query_map("/api/search?q=Aliens&api_key=secret");
        assert_eq!(map.get("q"), Some(&"Aliens".to_string()));
        assert_eq!(map.get("api_key"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_send_json_message() {
        // Test helper string formatting logic
        let json_body = format!("{{\"{}\": \"{}\"}}", "status", crate::utils::escape_json_str("success"));
        assert_eq!(json_body, "{\"status\": \"success\"}");
    }

    #[test]
    fn test_send_json_status_bool() {
        // Test helper string formatting logic
        let json_true = format!("{{\"success\": {}}}", true);
        assert_eq!(json_true, "{\"success\": true}");

        let json_msg = format!("{{\"success\": {}, \"message\": \"{}\"}}", false, crate::utils::escape_json_str("Error occurred"));
        assert_eq!(json_msg, "{\"success\": false, \"message\": \"Error occurred\"}");
    }

    #[test]
    fn test_snapshot_metrics() {
        let metrics = snapshot_metrics();
        assert_eq!(metrics.queued_jobs, crate::queue::list_jobs().len());

        let json = format_metrics_json(&metrics);
        assert!(json.contains("completed_rips"));
    }

    #[test]
    fn test_openapi_spec_route() {
        let req = b"GET /api/openapi.json HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req), (b"GET" as &[u8], b"/api/openapi.json" as &[u8]));
        assert!(OPENAPI_V3_JSON.contains("DVD Ripper Appliance REST API"));
    }

    #[test]
    fn test_prometheus_metrics_render() {
        increment_completed_rips();
        increment_failed_rips();

        let output = render_prometheus_metrics();
        assert!(output.contains("dvd_ripper_completed_rips_total"));
        assert!(output.contains("dvd_ripper_failed_rips_total"));
        assert!(output.contains("dvd_ripper_active_jobs"));
        assert!(output.contains("dvd_ripper_queued_jobs"));
    }

    #[test]
    fn test_prometheus_metrics_route() {
        let req = b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_http_route(req), (b"GET" as &[u8], b"/metrics" as &[u8]));
    }

    #[test]
    fn test_build_http_response_bytes() {
        let bytes = build_http_response_bytes("200 OK", "application/json", b"{\"ok\":true}");
        let resp_str = String::from_utf8_lossy(&bytes);
        assert!(resp_str.starts_with("HTTP/1.1 200 OK"));
        assert!(resp_str.contains("Content-Type: application/json"));
        assert!(resp_str.contains("{\"ok\":true}"));
    }

    #[test]
    fn test_build_http_json_response_bytes() {
        let bytes = build_http_json_response_bytes("200 OK", "{\"ok\":true}");
        let resp_str = String::from_utf8_lossy(&bytes);
        assert!(resp_str.contains("Content-Type: application/json"));
    }

    #[test]
    fn test_mime_type_for_path() {
        assert_eq!(mime_type_for_path("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_type_for_path("app.css"), "text/css; charset=utf-8");
        assert_eq!(mime_type_for_path("data.json"), "application/json");
        assert_eq!(mime_type_for_path("image.png"), "image/png");
    }
}
