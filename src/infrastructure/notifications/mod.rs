pub mod composite;
pub mod media_servers;
pub mod mqtt;
pub mod rich_webhooks;
pub mod webhook;

pub use composite::CompositeNotifier;
pub use media_servers::MediaServerNotifier;
pub use mqtt::MqttNotifier;
pub use rich_webhooks::{build_discord_rich_embed, build_telegram_html_payload};
pub use webhook::WebhookNotifier;
