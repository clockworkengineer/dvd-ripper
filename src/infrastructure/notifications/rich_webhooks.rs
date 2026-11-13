//! Rich Notification Webhook Builders for Discord, Telegram, and Homelab Dashboards.
//!
//! Generates structured JSON payloads with visual embed cards, poster thumbnails,
//! duration, bitrate, and media server badges.

use serde_json::json;

/// Formats a Discord-specific Webhook payload with a colored rich embed card.
#[must_use]
pub fn build_discord_rich_embed(
    title: &str,
    status: &str,
    message: &str,
    poster_url: Option<&str>,
    duration_str: Option<&str>,
    file_size_str: Option<&str>,
    codec: Option<&str>,
) -> String {
    let (color, status_icon) = match status.to_lowercase().as_str() {
        "success" | "completed" => (0x2ecc71, "✅"), // Green
        "started" | "running" => (0x3498db, "⏳"),   // Blue
        "failed" | "error" => (0xe74c3c, "❌"),      // Red
        "cancelled" => (0x95a5a6, "🛑"),             // Gray
        _ => (0x9b59b6, "📀"),                       // Purple
    };

    let mut fields = vec![
        json!({
            "name": "Status",
            "value": format!("{} {}", status_icon, status),
            "inline": true
        }),
    ];

    if let Some(dur) = duration_str {
        fields.push(json!({
            "name": "Duration",
            "value": dur,
            "inline": true
        }));
    }

    if let Some(sz) = file_size_str {
        fields.push(json!({
            "name": "File Size",
            "value": sz,
            "inline": true
        }));
    }

    if let Some(c) = codec {
        fields.push(json!({
            "name": "Codec",
            "value": c,
            "inline": true
        }));
    }

    let mut embed = json!({
        "title": format!("📀 {}", title),
        "description": message,
        "color": color,
        "fields": fields,
        "footer": {
            "text": "DVD Ripper Homelab Appliance"
        },
        "timestamp": crate::utils::now_timestamp_str()
    });

    if let Some(poster) = poster_url
        && !poster.trim().is_empty()
    {
        embed["thumbnail"] = json!({ "url": poster });
    }

    let payload = json!({
        "username": "DVD Ripper",
        "avatar_url": "https://raw.githubusercontent.com/clockworkengineer/dvd-ripper/master/assets/icon.png",
        "embeds": [embed]
    });

    serde_json::to_string(&payload).unwrap_or_default()
}

/// Formats a Telegram Bot API HTML payload (`sendMessage`).
#[must_use]
pub fn build_telegram_html_payload(
    chat_id: &str,
    title: &str,
    status: &str,
    message: &str,
    duration_str: Option<&str>,
) -> String {
    let status_icon = match status.to_lowercase().as_str() {
        "success" | "completed" => "✅",
        "started" | "running" => "⏳",
        "failed" | "error" => "❌",
        _ => "📀",
    };

    let mut text = format!(
        "<b>{} DVD Ripper</b>\n<b>Title:</b> <code>{}</code>\n<b>Status:</b> {}\n<b>Message:</b> {}",
        status_icon, title, status, message
    );

    if let Some(dur) = duration_str {
        text.push_str(&format!("\n<b>Duration:</b> {}", dur));
    }

    let payload = json!({
        "chat_id": chat_id,
        "parse_mode": "HTML",
        "text": text,
        "disable_web_page_preview": false
    });

    serde_json::to_string(&payload).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_discord_rich_embed_success() {
        let payload = build_discord_rich_embed(
            "The Matrix (1999)",
            "Success",
            "Transcoded with HEVC 10-bit",
            Some("https://example.com/poster.jpg"),
            Some("2h 16m"),
            Some("4.5 GB"),
            Some("hevc"),
        );

        assert!(payload.contains("The Matrix (1999)"));
        assert!(payload.contains("https://example.com/poster.jpg"));
        assert!(payload.contains("4.5 GB"));
        assert!(payload.contains("3066993")); // 0x2ecc71 in decimal
    }

    #[test]
    fn test_build_telegram_html_payload() {
        let payload = build_telegram_html_payload(
            "123456789",
            "Pulp Fiction",
            "Started",
            "Beginning rip from drive D:",
            Some("2h 34m"),
        );

        assert!(payload.contains("123456789"));
        assert!(payload.contains("Pulp Fiction"));
        assert!(payload.contains("<b>Status:</b> Started"));
    }
}

