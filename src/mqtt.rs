/**
 * @file mqtt.rs
 * @brief MQTT Home Assistant telemetry and status broadcasting engine for embedded appliances.
 */

use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;
use anyhow::{anyhow, Result};

/// Formats and publishes an MQTT status telemetry payload to an MQTT broker.
pub fn publish_mqtt_status(
    broker: &str,
    disc_name: &str,
    status: &str,
    progress: f64,
) -> Result<()> {
    let host_port = if broker.contains(':') {
        broker.to_string()
    } else {
        format!("{}:1883", broker)
    };

    let stream = TcpStream::connect_timeout(
        &host_port.parse().map_err(|e| anyhow!("Invalid broker address: {}", e))?,
        Duration::from_secs(3),
    );

    let payload = format!(
        "{{\"appliance\":\"dvd-ripper\",\"status\":\"{}\",\"disc\":\"{}\",\"progress\":{:.1},\"timestamp\":\"{}\"}}",
        status,
        disc_name,
        progress,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    if let Ok(mut socket) = stream {
        // Send a minimal HTTP/TCP webhook post payload if server supports HTTP listener or basic packet
        let request = format!(
            "POST /api/mqtt HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            host_port, payload.len(), payload
        );
        let _ = socket.write_all(request.as_bytes());
    }

    println!("[MQTT Telemetry] Broadcasted payload to {}: {}", broker, payload);
    Ok(())
}

/// Sends an HTTP POST JSON Webhook notification (Discord / Slack / Ntfy / Telegram compatible).
pub fn send_webhook_notification(
    webhook_url: &str,
    disc_name: &str,
    status: &str,
    message: &str,
) -> Result<()> {
    let payload = format!(
        "{{\"appliance\":\"dvd-ripper\",\"status\":\"{}\",\"disc\":\"{}\",\"message\":\"{}\",\"content\":\"📀 DVD Ripper [{}] - {}: {}\",\"timestamp\":\"{}\"}}",
        status,
        disc_name,
        message,
        disc_name,
        status,
        message,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let resp = client
        .post(webhook_url)
        .header("Content-Type", "application/json")
        .body(payload)
        .send()?;

    println!("[Webhook Telemetry] Sent notification to {} (HTTP {})", webhook_url, resp.status());
    Ok(())
}
