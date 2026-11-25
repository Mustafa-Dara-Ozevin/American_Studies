use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteGenConfig {
    pub site: SiteConfig,
    pub vault: VaultConfig,
}

impl SiteGenConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let cfg: SiteGenConfig = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse TOML in {}", path.display()))?;
        Ok(cfg)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteConfig {
    pub title: String,
    pub base_url: String,
    pub out_dir: PathBuf,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Amerika'nın Tarihi Yok Ki(!?)".to_string(),
            base_url: "mustafa-dara-ozevin.github.io/American_Studies".to_string(),
            out_dir: PathBuf::from("public"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultConfig {
    pub strip_prefixes: Vec<String>,
    pub ignore_dirs: Vec<String>,
    pub ignore_files: Vec<String>,
    pub asset_extensions: Vec<String>,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            strip_prefixes: vec!["content".to_string()],
            ignore_dirs: vec![
                ".git".to_string(),
                ".github".to_string(),
                ".obsidian".to_string(),
                ".trash".to_string(),
                ".quartz-cache".to_string(),
                ".nota".to_string(),
                "public".to_string(),
                "public-nota".to_string(),
                "site-gen".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                "private".to_string(),
                "reviews".to_string(),
                "templates".to_string(),
                "content/templates".to_string(),
                "Excalidraw".to_string(),
                "books".to_string(),
                "Math/_lib".to_string(),
                "Math/logic/cizelge".to_string(),
            ],
            ignore_files: vec![
                "README.md".to_string(),
                "CODE_OF_CONDUCT.md".to_string(),
                "LICENSE.txt".to_string(),
                "report.md".to_string(),
            ],
            asset_extensions: vec![
                "pdf".to_string(),
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "gif".to_string(),
                "webp".to_string(),
                "svg".to_string(),
                "mp3".to_string(),
                "wav".to_string(),
                "ogg".to_string(),
                "mp4".to_string(),
                "webm".to_string(),
            ],
        }
    }
}
