/**
 * @file config.rs
 * @brief Persistent TOML configuration file loader for dvd-ripper user defaults.
 */

use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::cli::Args;

/// Application configuration structure stored in dvd-ripper.toml or ~/.dvd-ripper/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub out_dir: Option<String>,
    pub codec: Option<String>,
    pub profile: Option<String>,
    pub preset: Option<String>,
    pub mqtt_broker: Option<String>,
    pub webhook_url: Option<String>,
    pub api_key: Option<String>,
    pub plex_url: Option<String>,
    pub plex_token: Option<String>,
    pub jellyfin_url: Option<String>,
    pub jellyfin_key: Option<String>,
    pub emby_url: Option<String>,
    pub emby_key: Option<String>,
    pub post_script: Option<String>,
    pub auto_boxset: Option<bool>,
    pub deinterlace: Option<bool>,
    pub deinterlace_algo: Option<String>,
    pub denoise: Option<bool>,
    pub min_free_gb: Option<u64>,
    pub no_eject: Option<bool>,
    pub sub_forced_only: Option<bool>,
    pub sub_external_srt: Option<bool>,
    pub audio_track: Option<u32>,
    pub audit_log: Option<String>,
    pub spindown: Option<bool>,
    pub sub_default: Option<bool>,
    pub tags: Option<String>,
    pub checksum: Option<bool>,
    pub fallback_out_dir: Option<String>,
    pub sub_burnin: Option<bool>,
    pub norm_target: Option<i32>,
    pub eject_autoclose: Option<u64>,
    pub label_regex_replace: Option<String>,
    pub audio_downmix: Option<String>,
    pub auto_cleanup_days: Option<u64>,
    pub drive_pool: Option<String>,
    pub tonemap: Option<String>,
}

fn try_load_config_file(path: &Path) -> Option<AppConfig> {
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(cfg) = parse_config_toml(&content) {
                println!("[Config] Loaded configuration from '{}'", path.display());
                return Some(cfg);
            }
        }
    }
    None
}

/// Loads configuration from custom path, ./dvd-ripper.toml, or ~/.dvd-ripper/config.toml
pub fn load_config(custom_path: Option<&str>) -> AppConfig {
    if let Some(path_str) = custom_path {
        if let Some(cfg) = try_load_config_file(Path::new(path_str)) {
            return cfg;
        }
    }

    if let Some(cfg) = try_load_config_file(Path::new("dvd-ripper.toml")) {
        return cfg;
    }

    let home_config = crate::utils::get_app_data_dir().join("config.toml");
    if let Some(cfg) = try_load_config_file(&home_config) {
        return cfg;
    }

    AppConfig::default()
}


/// Saves an AppConfig struct to TOML file using atomic file write.
#[allow(dead_code)]
pub fn save_config(config: &AppConfig, target_path: &Path) -> anyhow::Result<()> {
    let toml_str = toml::to_string_pretty(config)?;
    crate::utils::atomic_write_file(target_path, toml_str)?;
    Ok(())
}

/// Parses TOML content string into an AppConfig struct.
pub fn parse_config_toml(content: &str) -> anyhow::Result<AppConfig> {
    let cfg: AppConfig = toml::from_str(content)?;
    Ok(cfg)
}

/// Merges loaded TOML config options into CLI Args if CLI flags were omitted.
pub fn apply_config_defaults(args: &mut Args, config: &AppConfig) {
    if let Some(ref val) = config.out_dir {
        if args.out_dir == "Films" {
            args.out_dir = val.clone();
        }
    }
    if let Some(ref val) = config.codec {
        if args.codec == "h264" {
            args.codec = val.clone();
        }
    }
    if let Some(ref val) = config.profile {
        if args.profile == "standard" {
            args.profile = val.clone();
        }
    }
    if let Some(ref val) = config.preset {
        if args.preset == "veryfast" {
            args.preset = val.clone();
        }
    }
    if args.mqtt_broker.is_none() {
        args.mqtt_broker = config.mqtt_broker.clone();
    }
    if args.webhook_url.is_none() {
        args.webhook_url = config.webhook_url.clone();
    }
    if args.api_key.is_none() {
        args.api_key = config.api_key.clone();
    }
    if args.plex_url.is_none() {
        args.plex_url = config.plex_url.clone();
    }
    if args.plex_token.is_none() {
        args.plex_token = config.plex_token.clone();
    }
    if args.jellyfin_url.is_none() {
        args.jellyfin_url = config.jellyfin_url.clone();
    }
    if args.jellyfin_key.is_none() {
        args.jellyfin_key = config.jellyfin_key.clone();
    }
    if args.emby_url.is_none() {
        args.emby_url = config.emby_url.clone();
    }
    if args.emby_key.is_none() {
        args.emby_key = config.emby_key.clone();
    }
    if args.post_script.is_none() {
        args.post_script = config.post_script.clone();
    }
    if let Some(val) = config.auto_boxset {
        if !args.auto_boxset {
            args.auto_boxset = val;
        }
    }
    if let Some(val) = config.deinterlace {
        if !args.deinterlace {
            args.deinterlace = val;
        }
    }
    if args.deinterlace_algo.is_none() {
        args.deinterlace_algo = config.deinterlace_algo.clone();
    }
    if let Some(val) = config.denoise {
        if !args.denoise {
            args.denoise = val;
        }
    }
    if let Some(val) = config.min_free_gb {
        if args.min_free_gb == 10 {
            args.min_free_gb = val;
        }
    }
    if let Some(val) = config.no_eject {
        if !args.no_eject {
            args.no_eject = val;
        }
    }
    if let Some(val) = config.sub_forced_only {
        if !args.sub_forced_only {
            args.sub_forced_only = val;
        }
    }
    if let Some(val) = config.sub_external_srt {
        if !args.sub_external_srt {
            args.sub_external_srt = val;
        }
    }
    if args.audio_track.is_none() {
        args.audio_track = config.audio_track;
    }
    if args.audit_log.is_none() {
        args.audit_log = config.audit_log.clone();
    }
    if let Some(val) = config.spindown {
        if !args.spindown {
            args.spindown = val;
        }
    }
    if let Some(val) = config.sub_default {
        if !args.sub_default {
            args.sub_default = val;
        }
    }
    if args.tags.is_none() {
        args.tags = config.tags.clone();
    }
    if let Some(val) = config.checksum {
        if !args.checksum {
            args.checksum = val;
        }
    }
    if args.fallback_out_dir.is_none() {
        args.fallback_out_dir = config.fallback_out_dir.clone();
    }
    if let Some(val) = config.sub_burnin {
        if !args.sub_burnin {
            args.sub_burnin = val;
        }
    }
    if args.norm_target.is_none() {
        args.norm_target = config.norm_target;
    }
    if args.eject_autoclose.is_none() {
        args.eject_autoclose = config.eject_autoclose;
    }
    if args.label_regex_replace.is_none() {
        args.label_regex_replace = config.label_regex_replace.clone();
    }
    if args.audio_downmix.is_none() {
        args.audio_downmix = config.audio_downmix.clone();
    }
    if args.auto_cleanup_days.is_none() {
        args.auto_cleanup_days = config.auto_cleanup_days;
    }
    if args.drive_pool.is_none() {
        args.drive_pool = config.drive_pool.clone();
    }
    if args.tonemap.is_none() {
        args.tonemap = config.tonemap.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_toml() {
        let toml_str = r#"
            out_dir = "Movies"
            codec = "hevc"
            profile = "plex"
            mqtt_broker = "192.168.1.50:1883"
            api_key = "secret_key"
        "#;

        let cfg = parse_config_toml(toml_str).unwrap();
        assert_eq!(cfg.out_dir, Some("Movies".to_string()));
        assert_eq!(cfg.codec, Some("hevc".to_string()));
        assert_eq!(cfg.profile, Some("plex".to_string()));
        assert_eq!(cfg.mqtt_broker, Some("192.168.1.50:1883".to_string()));
        assert_eq!(cfg.api_key, Some("secret_key".to_string()));
    }

    #[test]
    fn test_apply_config_defaults() {
        let mut args = Args::default();
        let cfg = AppConfig {
            out_dir: Some("CustomMedia".to_string()),
            codec: Some("av1".to_string()),
            api_key: Some("test_api_key".to_string()),
            ..Default::default()
        };

        apply_config_defaults(&mut args, &cfg);
        assert_eq!(args.out_dir, "CustomMedia");
        assert_eq!(args.codec, "av1");
        assert_eq!(args.api_key, Some("test_api_key".to_string()));
    }
}
