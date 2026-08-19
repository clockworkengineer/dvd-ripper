/**
 * @file mqtt.rs
 * @brief Binary MQTT 3.1.1 Home Assistant telemetry and multi-service webhook notification engine.
 */

use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;
use anyhow::{anyhow, Result};

/// Validates and normalizes network host and port addresses.
pub fn sanitize_network_address(host: &str, default_port: u16) -> String {
    let trimmed = host.trim();
    if let Some((h, p)) = trimmed.split_once(':') {
        let clean_h = h.trim();
        if let Ok(port) = p.trim().parse::<u16>() {
            if (1..=65535).contains(&port) {
                return format!("{}:{}", clean_h, port);
            }
        }
        format!("{}:{}", clean_h, default_port)
    } else {
        format!("{}:{}", trimmed, default_port)
    }
}

/// Formats and resolves broker address with default port 1883.
fn format_broker_address(broker: &str) -> String {
    sanitize_network_address(broker, 1883)
}

/// Normalizes an MQTT topic prefix string by trimming leading and trailing slashes.
pub fn normalize_mqtt_prefix(prefix: &str) -> String {
    prefix.trim_matches('/').to_string()
}

/// Formats a clean MQTT topic string (e.g. "dvd-ripper/appliance/status").
pub fn format_mqtt_topic(prefix: &str, subtopic: &str) -> String {
    let clean_prefix = normalize_mqtt_prefix(prefix);
    let clean_sub = subtopic.trim_matches('/');
    if clean_prefix.is_empty() {
        clean_sub.to_string()
    } else {
        format!("{}/{}", clean_prefix, clean_sub)
    }
}

/// Encodes a string into MQTT 3.1.1 length-prefixed UTF-8 byte array.
pub fn encode_mqtt_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u16;
    let mut buf = Vec::with_capacity(2 + bytes.len());
    buf.push((len >> 8) as u8);
    buf.push((len & 0xFF) as u8);
    buf.extend_from_slice(bytes);
    buf
}

/// Encodes MQTT variable remaining length bytes.
pub fn encode_mqtt_remaining_length(mut len: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    loop {
        let mut byte = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if len == 0 {
            break;
        }
    }
    buf
}

/// Builds a binary MQTT 3.1.1 CONNECT control frame.
pub fn build_mqtt_connect_packet(client_id: &str) -> Vec<u8> {
    let mut var_header_payload = Vec::new();
    // Protocol Name ("MQTT")
    var_header_payload.extend_from_slice(&encode_mqtt_string("MQTT"));
    // Protocol Level (4 = v3.1.1)
    var_header_payload.push(0x04);
    // Connect Flags (0x02 = Clean Session)
    var_header_payload.push(0x02);
    // Keep Alive (60 seconds)
    var_header_payload.extend_from_slice(&[0x00, 0x3C]);
    // Client Identifier
    var_header_payload.extend_from_slice(&encode_mqtt_string(client_id));

    let mut frame = Vec::new();
    frame.push(0x10); // CONNECT packet type
    frame.extend_from_slice(&encode_mqtt_remaining_length(var_header_payload.len()));
    frame.extend_from_slice(&var_header_payload);
    frame
}

/// Builds a binary MQTT 3.1.1 PUBLISH control frame (QoS 0).
pub fn build_mqtt_publish_packet(topic: &str, payload: &str) -> Vec<u8> {
    let mut var_header_payload = Vec::new();
    var_header_payload.extend_from_slice(&encode_mqtt_string(topic));
    var_header_payload.extend_from_slice(payload.as_bytes());

    let mut frame = Vec::new();
    frame.push(0x30); // PUBLISH packet type (QoS 0)
    frame.extend_from_slice(&encode_mqtt_remaining_length(var_header_payload.len()));
    frame.extend_from_slice(&var_header_payload);
    frame
}

/// Sends Home Assistant MQTT Auto-Discovery configuration payloads.
pub fn publish_ha_discovery(socket: &mut TcpStream) -> Result<()> {
    let status_config = r#"{"name":"DVD Ripper Status","unique_id":"dvd_ripper_status","state_topic":"dvd_ripper/state","value_template":"{{ value_json.status }}"}"#;
    let progress_config = r#"{"name":"DVD Ripper Progress","unique_id":"dvd_ripper_progress","state_topic":"dvd_ripper/progress","unit_of_measurement":"%","value_template":"{{ value_json.progress }}"}"#;

    let status_packet = build_mqtt_publish_packet("homeassistant/sensor/dvd_ripper_status/config", status_config);
    let progress_packet = build_mqtt_publish_packet("homeassistant/sensor/dvd_ripper_progress/config", progress_config);

    socket.write_all(&status_packet)?;
    socket.write_all(&progress_packet)?;
    Ok(())
}

/// Formats and publishes binary MQTT status telemetry and Home Assistant discovery payloads.
pub fn publish_mqtt_status(
    broker: &str,
    disc_name: &str,
    status: &str,
    progress: f64,
) -> Result<()> {
    let host_port = format_broker_address(broker);

    let stream = TcpStream::connect_timeout(
        &host_port.parse().map_err(|e| anyhow!("Invalid broker address: {}", e))?,
        Duration::from_secs(3),
    );

    let state_payload = format!(
        "{{\"appliance\":\"dvd-ripper\",\"status\":\"{}\",\"disc\":\"{}\",\"progress\":{:.1},\"timestamp\":\"{}\"}}",
        status,
        disc_name,
        progress,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    let progress_payload = format!("{{\"progress\":{:.1}}}", progress);

    if let Ok(mut socket) = stream {
        // Send MQTT CONNECT packet
        let connect_pkt = build_mqtt_connect_packet("dvd-ripper-appliance");
        let _ = socket.write_all(&connect_pkt);

        // Send Home Assistant Auto-Discovery configs
        let _ = publish_ha_discovery(&mut socket);

        // Send State and Progress PUBLISH packets
        let state_pkt = build_mqtt_publish_packet("dvd_ripper/state", &state_payload);
        let progress_pkt = build_mqtt_publish_packet("dvd_ripper/progress", &progress_payload);

        let _ = socket.write_all(&state_pkt);
        let _ = socket.write_all(&progress_pkt);
    }

    println!("[MQTT Telemetry] Broadcasted payload to {}: {}", broker, state_payload);
    Ok(())
}

/// Sends a multi-service HTTP POST JSON Webhook notification (Discord / Slack / Ntfy / Telegram / Gotify).
/// Helper: Builds a unified JSON webhook payload compatible with Discord, Slack, Ntfy, Telegram, and Gotify.
pub fn build_webhook_payload(disc_name: &str, status: &str, message: &str) -> String {
    let text_message = format!("📀 DVD Ripper [{}] - {}: {}", disc_name, status, message);
    format!(
        "{{\"appliance\":\"dvd-ripper\",\"status\":\"{}\",\"disc\":\"{}\",\"message\":\"{}\",\"content\":\"{}\",\"text\":\"{}\",\"title\":\"DVD Ripper Alert\",\"timestamp\":\"{}\"}}",
        status,
        disc_name,
        message,
        text_message,
        text_message,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

pub fn send_webhook_notification(
    webhook_url: &str,
    disc_name: &str,
    status: &str,
    message: &str,
) -> Result<()> {
    let payload = build_webhook_payload(disc_name, status, message);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_broker_address() {
        assert_eq!(format_broker_address("192.168.1.50"), "192.168.1.50:1883");
        assert_eq!(format_broker_address("192.168.1.50:18833"), "192.168.1.50:18833");
        assert_eq!(format_broker_address("mqtt.local:8883"), "mqtt.local:8883");
    }

    #[test]
    fn test_sanitize_network_address() {
        assert_eq!(sanitize_network_address(" 192.168.1.100 ", 8080), "192.168.1.100:8080");
        assert_eq!(sanitize_network_address("localhost:9090", 8080), "localhost:9090");
        assert_eq!(sanitize_network_address("192.168.1.1:999999", 8080), "192.168.1.1:8080");
    }

    #[test]
    fn test_encode_mqtt_string() {
        let encoded = encode_mqtt_string("MQTT");
        assert_eq!(encoded, vec![0x00, 0x04, b'M', b'Q', b'T', b'T']);
    }

    #[test]
    fn test_build_mqtt_connect_packet() {
        let pkt = build_mqtt_connect_packet("test-client");
        assert_eq!(pkt[0], 0x10); // CONNECT packet type
        assert!(pkt.len() > 10);
    }

    #[test]
    fn test_build_mqtt_publish_packet() {
        let pkt = build_mqtt_publish_packet("dvd_ripper/state", "{\"status\":\"Idle\"}");
        assert_eq!(pkt[0], 0x30); // PUBLISH packet type
        assert!(pkt.len() > 15);
    }

    #[test]
    fn test_build_webhook_payload() {
        let payload = build_webhook_payload("ALIENS", "Success", "Completed rip");
        assert!(payload.contains("\"appliance\":\"dvd-ripper\""));
        assert!(payload.contains("\"disc\":\"ALIENS\""));
    }

    #[test]
    fn test_format_mqtt_topic() {
        assert_eq!(format_mqtt_topic("dvd-ripper", "status"), "dvd-ripper/status");
        assert_eq!(format_mqtt_topic("/dvd-ripper/", "/status/"), "dvd-ripper/status");
    }

    #[test]
    fn test_normalize_mqtt_prefix() {
        assert_eq!(normalize_mqtt_prefix("/dvd-ripper/"), "dvd-ripper");
        assert_eq!(normalize_mqtt_prefix("dvd-ripper"), "dvd-ripper");
    }
}
