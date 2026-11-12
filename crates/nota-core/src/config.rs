use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotaConfig {
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub logic: LogicConfig,
    #[serde(default)]
    pub srs: SrsConfig,
    #[serde(default)]
    pub site: SiteConfig,
}

impl Default for NotaConfig {
    fn default() -> Self {
        Self {
            vault: VaultConfig::default(),
            logic: LogicConfig::default(),
            srs: SrsConfig::default(),
            site: SiteConfig::default(),
        }
    }
}

impl NotaConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;
        let cfg: NotaConfig = toml::from_str(&raw)
            .with_context(|| format!("Failed to parse TOML in {}", path.display()))?;
        Ok(cfg)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    pub assets_dir: String,
    pub books_dirs: Vec<PathBuf>,
    pub ignore_paths: Vec<String>,
    pub templates_dir: PathBuf,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("content"),
            assets_dir: "assets".to_string(),
            books_dirs: vec![
                PathBuf::from("books"),
                PathBuf::from("content/assets/books"),
            ],
            ignore_paths: vec![
                ".obsidian".to_string(),
                ".trash".to_string(),
                "templates".to_string(),
                "private".to_string(),
                "Excalidraw".to_string(),
            ],
            templates_dir: PathBuf::from("content/templates"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicConfig {
    pub true_symbol: String,
    pub false_symbol: String,
    pub packed_variable_columns: bool,
    pub alphabetical_atoms: bool,
    pub alpha_first_trees: bool,
}

impl Default for LogicConfig {
    fn default() -> Self {
        Self {
            true_symbol: "D".to_string(),
            false_symbol: "Y".to_string(),
            packed_variable_columns: true,
            alphabetical_atoms: true,
            alpha_first_trees: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrsConfig {
    pub revlog_path: PathBuf,
    pub schedule_path: PathBuf,
    pub desired_retention: f64,
    pub new_cards_per_day: usize,
    pub max_reviews_per_day: usize,
    pub day_boundary_hour: u32,
    pub leech_threshold: u32,
    pub bury_siblings: bool,
}

impl Default for SrsConfig {
    fn default() -> Self {
        Self {
            revlog_path: PathBuf::from("reviews/revlog.jsonl"),
            schedule_path: PathBuf::from("reviews/schedule.json"),
            desired_retention: 0.90,
            new_cards_per_day: 20,
            max_reviews_per_day: 200,
            day_boundary_hour: 4,
            leech_threshold: 8,
            bury_siblings: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub title: String,
    pub base_url: String,
    pub out_dir: PathBuf,
    pub publish_epubs: bool,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Amerika'nın Tarihi Yok Ki(!?)".to_string(),
            base_url: "mustafa-dara-ozevin.github.io/American_Studies".to_string(),
            out_dir: PathBuf::from("public-nota"),
            publish_epubs: false,
        }
    }
}
