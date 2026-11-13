use crate::cli::Args;
use serde::{Deserialize, Serialize};
/**
 * @file config.rs
 * @brief Persistent TOML configuration file loader for dvd-ripper user defaults.
 */
use std::fs;
use std::path::Path;

/// Application configuration structure stored in dvd-ripper.toml or ~/.dvd-ripper/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub out_dir: Option<String>,
    pub codec: Option<String>,
    pub profile: Option<String>,
    pub preset: Option<String>,
    pub engine: Option<String>,
    pub handbrake: Option<String>,
    pub makemkv: Option<String>,
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
    pub auto_rip: Option<bool>,
    pub tonemap: Option<String>,
    pub ocr: Option<bool>,
    pub ocr_lang: Option<String>,
    pub tesseract: Option<String>,
    pub ignore_errors: Option<bool>,
    pub iso: Option<bool>,
    #[serde(default)]
    pub auto_crop: Option<bool>,
    #[serde(default)]
    pub detelecine: Option<bool>,
    #[serde(default)]
    pub crf: Option<u8>,
    #[serde(default)]
    pub spool: Option<bool>,
    #[serde(default)]
    pub sector_retries: Option<u32>,
    #[serde(default)]
    pub acoustic_profile: Option<String>,
    #[serde(default)]
    pub dual_output: Option<bool>,
    #[serde(default)]
    pub window_pos_x: Option<f32>,
    #[serde(default)]
    pub window_pos_y: Option<f32>,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default)]
    pub window_maximized: Option<bool>,
    #[serde(default)]
    pub subwindow_rects: Option<std::collections::BTreeMap<String, [f32; 4]>>,
}

fn try_load_config_file(path: &Path) -> Option<AppConfig> {
    if path.exists()
        && let Ok(content) = fs::read_to_string(path)
        && let Ok(cfg) = parse_config_toml(&content)
    {
        println!("[Config] Loaded configuration from '{}'", path.display());
        return Some(cfg);
    }
    None
}

/// Loads configuration from custom path, ./dvd-ripper.toml, or ~/.dvd-ripper/config.toml
pub fn load_config(custom_path: Option<&str>) -> AppConfig {
    if let Some(path_str) = custom_path
        && let Some(cfg) = try_load_config_file(Path::new(path_str))
    {
        return cfg;
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

/// Returns the default global configuration file path (~/.dvd-ripper/config.toml).
#[allow(dead_code)]
pub fn get_default_config_path() -> std::path::PathBuf {
    crate::utils::get_app_data_dir().join("config.toml")
}

/// Saves an AppConfig struct to the default configuration file (~/.dvd-ripper/config.toml).
#[allow(dead_code)]
pub fn save_default_config(config: &AppConfig) -> anyhow::Result<()> {
    let target = get_default_config_path();
    save_config(config, &target)
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

macro_rules! merge_str_default {
    ($args:expr, $config:expr, $field:ident, $default:expr) => {
        if let Some(ref val) = $config.$field
            && $args.$field == $default
        {
            $args.$field = val.clone();
        }
    };
}

macro_rules! merge_opt {
    ($args:expr, $config:expr, $field:ident) => {
        if $args.$field.is_none() && $config.$field.is_some() {
            $args.$field = $config.$field.clone();
        }
    };
}

macro_rules! merge_bool {
    ($args:expr, $config:expr, $field:ident) => {
        if let Some(val) = $config.$field
            && !$args.$field
        {
            $args.$field = val;
        }
    };
}

macro_rules! merge_num_default {
    ($args:expr, $config:expr, $field:ident, $default:expr) => {
        if let Some(val) = $config.$field
            && $args.$field == $default
        {
            $args.$field = val;
        }
    };
}

/// Merges loaded TOML config options into CLI Args if CLI flags were omitted.
pub fn apply_config_defaults(args: &mut Args, config: &AppConfig) {
    merge_str_default!(args, config, out_dir, "Films");
    merge_str_default!(args, config, codec, "h264");
    merge_str_default!(args, config, profile, "standard");
    merge_str_default!(args, config, preset, "veryfast");
    merge_str_default!(args, config, engine, "auto");
    merge_str_default!(args, config, handbrake, "HandBrakeCLI");
    merge_str_default!(args, config, makemkv, "makemkvcon");
    merge_str_default!(args, config, ocr_lang, "eng");
    merge_str_default!(args, config, tesseract, "tesseract");
    merge_str_default!(args, config, acoustic_profile, "balanced");

    merge_opt!(args, config, mqtt_broker);
    merge_opt!(args, config, webhook_url);
    merge_opt!(args, config, api_key);
    merge_opt!(args, config, plex_url);
    merge_opt!(args, config, plex_token);
    merge_opt!(args, config, jellyfin_url);
    merge_opt!(args, config, jellyfin_key);
    merge_opt!(args, config, emby_url);
    merge_opt!(args, config, emby_key);
    merge_opt!(args, config, post_script);
    merge_opt!(args, config, deinterlace_algo);
    merge_opt!(args, config, audio_track);
    merge_opt!(args, config, audit_log);
    merge_opt!(args, config, tags);
    merge_opt!(args, config, fallback_out_dir);
    merge_opt!(args, config, norm_target);
    merge_opt!(args, config, eject_autoclose);
    merge_opt!(args, config, label_regex_replace);
    merge_opt!(args, config, audio_downmix);
    merge_opt!(args, config, auto_cleanup_days);
    merge_opt!(args, config, drive_pool);
    merge_opt!(args, config, tonemap);
    merge_opt!(args, config, crf);

    merge_bool!(args, config, auto_boxset);
    merge_bool!(args, config, deinterlace);
    merge_bool!(args, config, denoise);
    merge_bool!(args, config, no_eject);
    merge_bool!(args, config, sub_forced_only);
    merge_bool!(args, config, sub_external_srt);
    merge_bool!(args, config, spindown);
    merge_bool!(args, config, sub_default);
    merge_bool!(args, config, checksum);
    merge_bool!(args, config, sub_burnin);
    merge_bool!(args, config, auto_rip);
    merge_bool!(args, config, ocr);
    merge_bool!(args, config, iso);
    merge_bool!(args, config, auto_crop);
    merge_bool!(args, config, detelecine);
    merge_bool!(args, config, spool);
    merge_bool!(args, config, dual_output);

    merge_num_default!(args, config, min_free_gb, 10);
    merge_num_default!(args, config, sector_retries, 2);

    if let Some(val) = config.ignore_errors {
        args.ignore_errors = val;
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
            ocr: Some(true),
            ocr_lang: Some("fra".to_string()),
            tesseract: Some("tesseract-ocr".to_string()),
            ..Default::default()
        };

        apply_config_defaults(&mut args, &cfg);
        assert_eq!(args.out_dir, "CustomMedia");
        assert_eq!(args.codec, "av1");
        assert_eq!(args.api_key, Some("test_api_key".to_string()));
        assert!(args.ocr);
        assert_eq!(args.ocr_lang, "fra");
        assert_eq!(args.tesseract, "tesseract-ocr");
    }

    #[test]
    fn test_apply_config_all_branches() {
        let mut args = Args::default();
        let cfg = AppConfig {
            profile: Some("jellyfin".to_string()),
            webhook_url: Some("http://webhook.local".to_string()),
            plex_url: Some("http://plex.local".to_string()),
            jellyfin_url: Some("http://jf.local".to_string()),
            emby_url: Some("http://emby.local".to_string()),
            deinterlace: Some(true),
            deinterlace_algo: Some("bwdif".to_string()),
            denoise: Some(true),
            min_free_gb: Some(25),
            no_eject: Some(true),
            sub_forced_only: Some(true),
            sub_external_srt: Some(true),
            audio_track: Some(2),
            audit_log: Some("audit.log".to_string()),
            spindown: Some(true),
            sub_default: Some(true),
            tags: Some("Action".to_string()),
            checksum: Some(true),
            fallback_out_dir: Some("Fallback".to_string()),
            sub_burnin: Some(true),
            norm_target: Some(-16),
            eject_autoclose: Some(15),
            label_regex_replace: Some("s/foo/bar/".to_string()),
            audio_downmix: Some("stereo".to_string()),
            auto_cleanup_days: Some(30),
            drive_pool: Some("D:,E:".to_string()),
            auto_rip: Some(true),
            tonemap: Some("mobius".to_string()),
            ignore_errors: Some(true),
            iso: Some(true),
            ..Default::default()
        };

        apply_config_defaults(&mut args, &cfg);
        assert_eq!(args.profile, "jellyfin");
        assert_eq!(args.webhook_url, Some("http://webhook.local".to_string()));
        assert!(args.deinterlace);
        assert_eq!(args.deinterlace_algo.as_deref(), Some("bwdif"));
        assert!(args.denoise);
        assert_eq!(args.min_free_gb, 25);
        assert!(args.no_eject);
        assert!(args.sub_forced_only);
        assert!(args.sub_external_srt);
        assert_eq!(args.audio_track, Some(2));
        assert!(args.spindown);
        assert!(args.sub_default);
        assert!(args.checksum);
        assert!(args.sub_burnin);
        assert_eq!(args.norm_target, Some(-16));
        assert_eq!(args.eject_autoclose, Some(15));
        assert_eq!(args.audio_downmix, Some("stereo".to_string()));
        assert_eq!(args.auto_cleanup_days, Some(30));
        assert_eq!(args.drive_pool, Some("D:,E:".to_string()));
        assert!(args.auto_rip);
        assert!(args.ignore_errors);
        assert!(args.iso);
    }

    #[test]
    fn test_config_file_persistence_and_repo() {
        let temp = std::env::temp_dir().join(format!(
            "test_config_{}.toml",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cfg = AppConfig {
            out_dir: Some("C:\\Saved".to_string()),
            codec: Some("hevc".to_string()),
            ..Default::default()
        };

        let res = save_config(&cfg, &temp);
        assert!(res.is_ok());

        let loaded = load_config(Some(&temp.to_string_lossy()));
        assert_eq!(loaded.out_dir, Some("C:\\Saved".to_string()));
        assert_eq!(loaded.codec, Some("hevc".to_string()));

        let _ = get_default_config_path();
        let _ = std::fs::remove_file(&temp);
    }

    #[test]
    fn test_config_window_geometry_and_subwindow_rects_persistence() {
        let mut sub_rects = std::collections::BTreeMap::new();
        sub_rects.insert("help_modal".to_string(), [120.0, 80.0, 620.0, 480.0]);
        let cfg = AppConfig {
            window_pos_x: Some(150.0),
            window_pos_y: Some(90.0),
            window_width: Some(1024.0),
            window_height: Some(960.0),
            window_maximized: Some(false),
            subwindow_rects: Some(sub_rects),
            ..Default::default()
        };

        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed = parse_config_toml(&toml_str).unwrap();
        assert_eq!(parsed.window_pos_x, Some(150.0));
        assert_eq!(parsed.window_pos_y, Some(90.0));
        assert_eq!(parsed.window_width, Some(1024.0));
        assert_eq!(parsed.window_height, Some(960.0));
        assert_eq!(parsed.window_maximized, Some(false));
        assert_eq!(
            parsed
                .subwindow_rects
                .as_ref()
                .and_then(|m| m.get("help_modal"))
                .copied(),
            Some([120.0, 80.0, 620.0, 480.0])
        );
    }
}
