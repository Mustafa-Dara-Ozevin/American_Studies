mod config;
mod frontmatter;
mod parser;
mod render;
mod slug;
mod vault;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::SiteGenConfig;
use render::build_static_site;
use std::path::PathBuf;
use vault::VaultIndex;

#[derive(Parser, Debug)]
#[command(
    name = "nota-site-gen",
    version,
    about = "Lightweight static site publisher for Obsidian vaults and Typst PDF outputs"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build the static site from the Obsidian vault
    Build {
        /// Path to the Obsidian vault root
        #[arg(long, default_value = ".")]
        vault: PathBuf,
        /// Output directory (overrides site.out_dir in config)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Path to site.toml configuration file
        #[arg(long, default_value = "site.toml")]
        config: PathBuf,
        /// Base URL (overrides site.base_url in config)
        #[arg(long)]
        base_url: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            vault,
            out,
            config,
            base_url,
        } => {
            let mut cfg = SiteGenConfig::load_from_path(&config)?;
            if let Some(out_dir) = out {
                cfg.site.out_dir = out_dir;
            }
            if let Some(url) = base_url {
                cfg.site.base_url = url;
            }

            let index = VaultIndex::scan(&vault, &cfg.vault)?;
            let summary = build_static_site(&index, &cfg.site, &cfg.site.out_dir)?;

            println!(
                "Built static site in {}:\n  notes: {}\n  pdf viewers: {}\n  assets copied: {}\n  tag pages: {}\n  alias redirects: {}\n  unresolved links: {}",
                summary.out_dir.display(),
                summary.pages_built,
                summary.pdf_viewers_built,
                summary.assets_copied,
                summary.tag_pages_built,
                summary.alias_redirects_built,
                summary.unresolved_links,
            );
        }
    }

    Ok(())
}
