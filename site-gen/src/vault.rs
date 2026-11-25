use crate::config::VaultConfig;
use crate::frontmatter::{parse_frontmatter, NoteFrontmatter};
use crate::parser::{parse_note_body, ParsedNoteBody, WikiLink};
use crate::slug::{normalize_lookup_key, slugify_file_path, strip_prefix_path};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedNote {
    pub rel_path: String,
    pub public_path: String,
    pub file_name: String,
    pub file_stem: String,
    pub slug: String,
    pub is_moc: bool,
    pub frontmatter: NoteFrontmatter,
    pub body: ParsedNoteBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedAsset {
    pub rel_path: String,
    pub public_path: String,
    pub file_name: String,
    pub file_stem: String,
    pub is_pdf: bool,
    pub viewer_slug: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct VaultIndex {
    pub vault_root: PathBuf,
    pub notes: BTreeMap<String, IndexedNote>,
    pub assets: BTreeMap<String, IndexedAsset>,
    pub note_lookup: HashMap<String, String>,
    pub asset_lookup: HashMap<String, String>,
    pub incoming_links: BTreeMap<String, BTreeSet<String>>,
    pub outgoing_links: BTreeMap<String, BTreeSet<String>>,
    pub unresolved_count: usize,
}

impl VaultIndex {
    pub fn scan(vault_root: impl AsRef<Path>, config: &VaultConfig) -> Result<Self> {
        let vault_root = vault_root.as_ref().to_path_buf();
        let mut notes = BTreeMap::new();
        let mut assets = BTreeMap::new();

        let allowed_exts: BTreeSet<String> = config
            .asset_extensions
            .iter()
            .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
            .collect();

        for entry in WalkDir::new(&vault_root).into_iter().filter_entry(|e| {
            if e.path() == vault_root {
                return true;
            }
            let rel_buf = e.path().strip_prefix(&vault_root).unwrap_or(e.path());
            let rel = rel_buf
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let name = e.file_name().to_string_lossy();

            if e.file_type().is_dir() {
                return !is_dir_ignored(&rel, &name, &config.ignore_dirs);
            }
            true
        }) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs_path = entry.path();
            let rel_buf = abs_path.strip_prefix(&vault_root).unwrap_or(abs_path);
            let rel_path = rel_buf
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");

            let file_name = entry.file_name().to_string_lossy().to_string();
            let lower_name = file_name.to_ascii_lowercase();

            if is_file_ignored(&rel_path, &file_name, &config.ignore_files) {
                continue;
            }

            if lower_name.ends_with(".excalidraw.md")
                || lower_name.ends_with(".canvas")
                || lower_name.ends_with(".base")
            {
                continue;
            }

            if lower_name.ends_with(".md") {
                let raw_markdown = fs::read_to_string(abs_path)
                    .with_context(|| format!("Failed to read note {}", abs_path.display()))?;
                let frontmatter = parse_frontmatter(&raw_markdown);
                if frontmatter.draft {
                    continue;
                }
                let body = parse_note_body(
                    &raw_markdown,
                    frontmatter.body_byte_offset,
                    frontmatter.body_start_line,
                );
                let file_stem = file_name
                    .strip_suffix(".md")
                    .or_else(|| file_name.strip_suffix(".MD"))
                    .unwrap_or(&file_name)
                    .to_string();
                let public_path = strip_prefix_path(&rel_path, &config.strip_prefixes);
                let slug = slugify_file_path(&public_path);
                let is_moc = file_stem.to_ascii_lowercase().contains("moc");

                notes.insert(
                    rel_path.clone(),
                    IndexedNote {
                        rel_path,
                        public_path,
                        file_name,
                        file_stem,
                        slug,
                        is_moc,
                        frontmatter,
                        body,
                    },
                );
            } else if let Some(ext) = abs_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
            {
                if !allowed_exts.contains(&ext) {
                    continue;
                }
                let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let public_path = strip_prefix_path(&rel_path, &config.strip_prefixes);
                let file_stem = abs_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&file_name)
                    .to_string();
                let is_pdf = ext == "pdf";
                let viewer_slug = if is_pdf {
                    Some(slugify_file_path(&public_path))
                } else {
                    None
                };

                assets.insert(
                    rel_path.clone(),
                    IndexedAsset {
                        rel_path,
                        public_path,
                        file_name,
                        file_stem,
                        is_pdf,
                        viewer_slug,
                        size_bytes,
                    },
                );
            }
        }

        // Build note lookup: exact paths first, then shallower stems/filenames, then aliases.
        let mut note_lookup: HashMap<String, String> = HashMap::new();
        for (rel_path, note) in &notes {
            note_lookup.insert(normalize_lookup_key(rel_path), rel_path.clone());
            note_lookup.insert(normalize_lookup_key(&note.public_path), rel_path.clone());
            let pub_no_ext = note
                .public_path
                .strip_suffix(".md")
                .or_else(|| note.public_path.strip_suffix(".MD"))
                .unwrap_or(&note.public_path);
            note_lookup.insert(normalize_lookup_key(pub_no_ext), rel_path.clone());
        }

        let mut sorted_notes: Vec<&IndexedNote> = notes.values().collect();
        sorted_notes.sort_by_key(|n| {
            (
                n.public_path.matches('/').count(),
                n.public_path.len(),
                n.public_path.clone(),
            )
        });
        for note in &sorted_notes {
            note_lookup
                .entry(normalize_lookup_key(&note.file_stem))
                .or_insert_with(|| note.rel_path.clone());
            note_lookup
                .entry(normalize_lookup_key(&note.file_name))
                .or_insert_with(|| note.rel_path.clone());
        }
        for note in &sorted_notes {
            for alias in &note.frontmatter.clean_aliases {
                note_lookup
                    .entry(normalize_lookup_key(alias))
                    .or_insert_with(|| note.rel_path.clone());
            }
        }

        // Build asset lookup: exact paths first, then shallower filenames.
        let mut asset_lookup: HashMap<String, String> = HashMap::new();
        for (rel_path, asset) in &assets {
            asset_lookup.insert(normalize_lookup_key(rel_path), rel_path.clone());
            asset_lookup.insert(normalize_lookup_key(&asset.public_path), rel_path.clone());
        }
        let mut sorted_assets: Vec<&IndexedAsset> = assets.values().collect();
        sorted_assets.sort_by_key(|a| {
            (
                a.public_path.matches('/').count(),
                a.public_path.len(),
                a.public_path.clone(),
            )
        });
        for asset in &sorted_assets {
            asset_lookup
                .entry(normalize_lookup_key(&asset.file_name))
                .or_insert_with(|| asset.rel_path.clone());
        }

        let mut incoming_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut outgoing_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut unresolved_count = 0usize;

        for (from_path, note) in &notes {
            for wl in &note.body.wikilinks {
                if wl.epub.is_some() || wl.target.trim().is_empty() {
                    continue;
                }
                if let Some(target_rel) = resolve_target(wl, &note_lookup, &asset_lookup) {
                    if target_rel != *from_path {
                        incoming_links
                            .entry(target_rel.clone())
                            .or_default()
                            .insert(from_path.clone());
                        outgoing_links
                            .entry(from_path.clone())
                            .or_default()
                            .insert(target_rel);
                    }
                } else {
                    unresolved_count += 1;
                }
            }
        }

        Ok(Self {
            vault_root,
            notes,
            assets,
            note_lookup,
            asset_lookup,
            incoming_links,
            outgoing_links,
            unresolved_count,
        })
    }

    pub fn related_notes(&self, rel_path: &str, limit: usize) -> Vec<(String, String)> {
        let Some(current) = self.notes.get(rel_path) else {
            return Vec::new();
        };
        let current_tags: BTreeSet<&str> =
            current.frontmatter.tags.iter().map(|s| s.as_str()).collect();
        let mut scored: Vec<(usize, String, String)> = Vec::new();

        for (other_path, other) in &self.notes {
            if other_path == rel_path {
                continue;
            }
            let shared_tags = other
                .frontmatter
                .tags
                .iter()
                .filter(|t| current_tags.contains(t.as_str()))
                .count();
            if shared_tags > 0 {
                scored.push((
                    shared_tags,
                    other_path.clone(),
                    format!("{} shared tag(s)", shared_tags),
                ));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, path, reason)| (path, reason))
            .collect()
    }

    pub fn build_graph_json(&self) -> serde_json::Value {
        let mut nodes = Vec::new();
        for note in self.notes.values() {
            nodes.push(json!({
                "id": note.rel_path,
                "label": note.frontmatter.title.as_deref().unwrap_or(&note.file_stem),
                "slug": note.slug,
                "type": "note",
                "tags": note.frontmatter.tags,
            }));
        }
        for asset in self.assets.values().filter(|a| a.is_pdf) {
            if let Some(ref vs) = asset.viewer_slug {
                nodes.push(json!({
                    "id": asset.rel_path,
                    "label": asset.file_stem,
                    "slug": vs,
                    "type": "pdf",
                }));
            }
        }

        let mut edges = Vec::new();
        for (src, targets) in &self.outgoing_links {
            for dst in targets {
                edges.push(json!({
                    "source": src,
                    "target": dst,
                }));
            }
        }

        json!({
            "nodes": nodes,
            "edges": edges,
        })
    }
}

fn resolve_target(
    wl: &WikiLink,
    note_lookup: &HashMap<String, String>,
    asset_lookup: &HashMap<String, String>,
) -> Option<String> {
    let key = normalize_lookup_key(&wl.target);
    if !wl.is_embed {
        if let Some(note_rel) = note_lookup.get(&key) {
            return Some(note_rel.clone());
        }
    }
    asset_lookup.get(&key).cloned()
}

fn is_dir_ignored(rel_path: &str, dir_name: &str, ignore_dirs: &[String]) -> bool {
    ignore_dirs.iter().any(|ig| {
        let ig = ig.trim_matches('/');
        if !ig.contains('/') && dir_name == ig {
            return true;
        }
        rel_path == ig || rel_path.starts_with(&format!("{}/", ig))
    })
}

fn is_file_ignored(rel_path: &str, file_name: &str, ignore_files: &[String]) -> bool {
    ignore_files.iter().any(|ig| {
        let ig = ig.trim_matches('/');
        rel_path == ig || (!rel_path.contains('/') && file_name == ig)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_scan_ignores_and_pdf_resolution() {
        let tmp = std::env::temp_dir().join(format!("site-gen-vault-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("content")).unwrap();
        fs::create_dir_all(tmp.join("Math/Induction")).unwrap();
        fs::create_dir_all(tmp.join("Math/_lib")).unwrap();
        fs::create_dir_all(tmp.join("site-gen/src")).unwrap();

        fs::write(tmp.join("README.md"), "# Repo Readme").unwrap();
        fs::write(tmp.join("content/index.md"), "# Home\n[[MATH_MOC]]").unwrap();
        fs::write(
            tmp.join("MATH_MOC.md"),
            "1. [[strong_induction.pdf]]\n2. ![[strong_induction.pdf]]",
        )
        .unwrap();
        fs::write(tmp.join("Math/Math_MOC.md"), "[[MATH_MOC]]").unwrap();
        fs::write(tmp.join("Math/Induction/strong_induction.typ"), "= Typst").unwrap();
        fs::write(tmp.join("Math/Induction/strong_induction.pdf"), b"%PDF-1.4").unwrap();
        fs::write(tmp.join("Math/_lib/vault.typ"), "// lib").unwrap();
        fs::write(tmp.join("site-gen/src/main.rs"), "fn main() {}").unwrap();

        let cfg = VaultConfig::default();
        let index = VaultIndex::scan(&tmp, &cfg).unwrap();

        assert!(!index.notes.contains_key("README.md"));
        assert!(index.notes.contains_key("content/index.md"));
        assert_eq!(index.notes["content/index.md"].slug, "index");
        assert!(index.notes.contains_key("MATH_MOC.md"));
        assert!(index.notes.contains_key("Math/Math_MOC.md"));

        // .typ is ignored, .pdf is indexed
        assert!(!index
            .assets
            .contains_key("Math/Induction/strong_induction.typ"));
        assert!(index
            .assets
            .contains_key("Math/Induction/strong_induction.pdf"));

        // Shallow MATH_MOC wins stem lookup
        assert_eq!(
            index.note_lookup.get("math_moc").map(String::as_str),
            Some("MATH_MOC.md")
        );
        // PDF backlinks tracked
        assert!(index
            .incoming_links
            .get("Math/Induction/strong_induction.pdf")
            .unwrap()
            .contains("MATH_MOC.md"));
        assert_eq!(index.unresolved_count, 0);

        let _ = fs::remove_dir_all(&tmp);
    }
}
