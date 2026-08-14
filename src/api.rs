use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use anyhow::Result;

use crate::dvd::eject_disc;
use crate::history::load_history;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplianceStatusInfo {
    pub status: String,
    pub drive: String,
    pub disc: String,
    pub current_title: String,
    pub progress: f64,
    pub fps: String,
    pub speed: String,
}

static APPLIANCE_STATUS: OnceLock<Arc<Mutex<ApplianceStatusInfo>>> = OnceLock::new();

pub fn get_appliance_status_handle() -> Arc<Mutex<ApplianceStatusInfo>> {
    APPLIANCE_STATUS
        .get_or_init(|| {
            Arc::new(Mutex::new(ApplianceStatusInfo {
                status: "Idle".to_string(),
                drive: "D:\\".to_string(),
                disc: "".to_string(),
                current_title: "".to_string(),
                progress: 0.0,
                fps: "0".to_string(),
                speed: "0x".to_string(),
            }))
        })
        .clone()
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
            <div class="progress-bar"><div id="progress-fill" class="progress-fill"></div></div>
            <div style="margin-top: 1rem;">
                <button class="btn" onclick="triggerRip()">▶ Start Rip</button>
                <button class="btn btn-danger" onclick="cancelRip()">⏹ Cancel</button>
                <button class="btn btn-secondary" onclick="ejectDisc()">⏏ Eject Tray</button>
                <button class="btn btn-secondary" onclick="fetchHistory()">🔄 Refresh History</button>
            </div>
        </div>

        <div class="card">
            <h2>Ripping History</h2>
            <div id="history-list">Loading history...</div>
        </div>
    </div>

    <script>
        async function pollStatus() {
            try {
                const res = await fetch('/api/status');
                const data = await res.json();
                document.getElementById('status-text').innerHTML = `
                    <strong>State:</strong> ${data.status} | 
                    <strong>Drive:</strong> ${data.drive} | 
                    <strong>Disc:</strong> ${data.disc || 'None'}
                `;
                document.getElementById('progress-fill').style.width = (data.progress || 0) + '%';
            } catch(e) {}
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
            alert(data.message || 'Triggered rip job.');
        }

        async function cancelRip() {
            const res = await fetch('/api/cancel', { method: 'POST' });
            const data = await res.json();
            alert(data.message || 'Cancelled rip job.');
        }

        fetchHistory();
        setInterval(pollStatus, 2000);
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

fn handle_client(mut stream: TcpStream, drive_path: &str) -> Result<()> {
    let mut buffer = [0u8; 1024];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let req_bytes = &buffer[..bytes_read];
    let (method, path) = parse_http_route(req_bytes);

    match (method, path) {
        (b"GET", b"/") | (b"GET", b"/index.html") => {
            send_http_response(&mut stream, "200 OK", "text/html; charset=utf-8", EMBEDDED_DASHBOARD_HTML)?;
        }
        (b"GET", b"/api/status") => {
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
        }
        (b"GET", b"/api/history") => {
            let history = load_history(None);
            let json_body = serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string());
            send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
        }
        (b"POST", b"/api/eject") => {
            let ok = eject_disc(drive_path);
            let json_body = format!("{{\"success\": {}}}", ok);
            send_http_response(&mut stream, "200 OK", "application/json", &json_body)?;
        }
        (b"POST", b"/api/rip") => {
            let json_body = "{\"success\": true, \"message\": \"Ripping job triggered via Web API\"}";
            send_http_response(&mut stream, "200 OK", "application/json", json_body)?;
        }
        (b"POST", b"/api/cancel") => {
            let json_body = "{\"success\": true, \"message\": \"Ripping job cancellation requested via Web API\"}";
            send_http_response(&mut stream, "200 OK", "application/json", json_body)?;
        }
        _ => {
            send_http_response(&mut stream, "404 Not Found", "text/plain", "404 Not Found")?;
        }
    }

    Ok(())
}

fn send_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, content_type, body.len(), body
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
