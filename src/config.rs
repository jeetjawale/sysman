use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_refresh_rate")]
    pub refresh_rate_ms: u64,
    #[serde(default)]
    pub thresholds: ThresholdConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
}

fn default_refresh_rate() -> u64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    #[serde(default = "default_cpu_high")]
    pub cpu_high: f32,
    #[serde(default = "default_mem_high")]
    pub mem_high: f32,
    #[serde(default = "default_disk_high")]
    pub disk_high: f32,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            cpu_high: default_cpu_high(),
            mem_high: default_mem_high(),
            disk_high: default_disk_high(),
        }
    }
}

fn default_cpu_high() -> f32 {
    85.0
}
fn default_mem_high() -> f32 {
    90.0
}
fn default_disk_high() -> f32 {
    95.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    pub brand_color: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(config) = toml::from_str(&content)
        {
            return config;
        }

        let default_config = Config {
            refresh_rate_ms: 1000,
            thresholds: ThresholdConfig::default(),
            theme: ThemeConfig::default(),
        };

        // Try to save default if it doesn't exist
        let _ = fs::create_dir_all(path.parent().unwrap());
        if let Ok(content) = toml::to_string_pretty(&default_config) {
            let _ = fs::write(&path, content);
        }

        default_config
    }
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("sysman");
    path.push("config.toml");
    path
}

mod dirs {
    use std::path::PathBuf;
    pub fn config_dir() -> Option<PathBuf> {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                #[cfg(target_os = "linux")]
                {
                    std::env::var_os("HOME").map(|home| {
                        let mut p = PathBuf::from(home);
                        p.push(".config");
                        p
                    })
                }
                #[cfg(not(target_os = "linux"))]
                {
                    None
                }
            })
    }
}
