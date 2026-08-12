/**
 * @file api.rs
 * @brief Zero-dependency embedded HTTP REST API and HTML5 Web UI management dashboard.
 */

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use anyhow::Result;

use crate::dvd::eject_disc;
use crate::history::load_history;

/// Embedded HTML5 Web UI Dashboard string embedded directly into the binary.
const EMBEDDED_DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DVD Ripper Embedded Appliance</title>
    <style>
        :root { --bg: #0f172a; --card: #1e293b; --accent: #38bdf8; --text: #f8fafc; --muted: #94a3b8; --success: #22c55e; }
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: var(--bg); color: var(--text); margin: 0; padding: 2rem; }
        .container { max-width: 900px; margin: 0 auto; }
        .header { display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid #334155; padding-bottom: 1rem; margin-bottom: 2rem; }
        h1 { margin: 0; font-size: 1.5rem; color: var(--accent); }
        .badge { background: var(--success); color: #000; font-weight: bold; padding: 0.25rem 0.75rem; border-radius: 9999px; font-size: 0.85rem; }
        .card { background: var(--card); border-radius: 12px; padding: 1.5rem; margin-bottom: 1.5rem; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.3); }
        .btn { background: var(--accent); color: #000; border: none; padding: 0.6rem 1.2rem; border-radius: 8px; font-weight: bold; cursor: pointer; transition: opacity 0.2s; }
        .btn:hover { opacity: 0.9; }
        .history-item { display: flex; justify-content: space-between; padding: 0.75rem 0; border-bottom: 1px solid #334155; }
        .history-item:last-child { border-bottom: none; }
        .muted { color: var(--muted); font-size: 0.9rem; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>📀 DVD Ripper Embedded Appliance</h1>
            <span class="badge">ONLINE</span>
        </div>

        <div class="card">
            <h2>Appliance Status</h2>
            <p class="muted">Embedded Daemon Service Active</p>
            <div style="margin-top: 1rem;">
                <button class="btn" onclick="ejectDisc()">⏏ Eject Optical Tray</button>
                <button class="btn" style="background:#64748b; color:#fff;" onclick="fetchHistory()">🔄 Refresh History</button>
            </div>
        </div>

        <div class="card">
            <h2>Ripping History</h2>
            <div id="history-list">Loading history...</div>
        </div>
    </div>

    <script>
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

        fetchHistory();
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

fn handle_client(mut stream: TcpStream, drive_path: &str) -> Result<()> {
    let mut buffer = [0u8; 1024];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                EMBEDDED_DASHBOARD_HTML.len(),
                EMBEDDED_DASHBOARD_HTML
            );
            stream.write_all(response.as_bytes())?;
        }
        ("GET", "/api/history") => {
            let history = load_history(None);
            let json_body = serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string());
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json_body.len(),
                json_body
            );
            stream.write_all(response.as_bytes())?;
        }
        ("POST", "/api/eject") => {
            let ok = eject_disc(drive_path);
            let json_body = format!("{{\"success\": {}}}", ok);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                json_body.len(),
                json_body
            );
            stream.write_all(response.as_bytes())?;
        }
        _ => {
            let body = "404 Not Found";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes())?;
        }
    }

    Ok(())
}
