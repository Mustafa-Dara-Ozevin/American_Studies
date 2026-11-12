use crate::config::VaultConfig;
use crate::frontmatter::{parse_frontmatter, NoteFrontmatter};
use crate::parser::{parse_note_body, ParsedNoteBody, WikiLink};
use crate::slug::{normalize_lookup_key, slugify_file_path};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedNote {
    /// Relative path inside the vault, e.g. `Sembolik Mantık Giriş.md`
    pub rel_path: String,
    /// Filename with extension, e.g. `Sembolik Mantık Giriş.md`
    pub file_name: String,
    /// Filename without `.md`, e.g. `Sembolik Mantık Giriş`
    pub file_stem: String,
    /// Quartz-compatible URL slug
    pub slug: String,
    pub is_moc: bool,
    pub frontmatter: NoteFrontmatter,
    pub body: ParsedNoteBody,
    pub raw_len_bytes: usize,
}

impl IndexedNote {
    pub fn h1_count(&self) -> usize {
        self.body.headings.iter().filter(|h| h.level == 1).count()
    }

    pub fn title_mismatch(&self) -> Option<(&str, &str)> {
        if let Some(ref t) = self.frontmatter.title {
            if t.trim() != self.file_stem.trim() {
                return Some((t.as_str(), self.file_stem.as_str()));
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedLink {
    pub from_note: String,
    pub line: usize,
    pub raw: String,
    pub target_raw: String,
    pub resolved_note: Option<String>,
    pub resolved_asset: Option<String>,
    pub is_embed: bool,
    pub is_epub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultIndex {
    pub vault_root: PathBuf,
    pub notes: BTreeMap<String, IndexedNote>,
    pub assets: BTreeMap<String, u64>,
    pub excalidraw_files: Vec<String>,
    pub canvas_files: Vec<String>,
    pub base_files: Vec<String>,
    pub template_files: Vec<String>,
    pub resolved_links: Vec<ResolvedLink>,
    pub incoming_links: BTreeMap<String, BTreeSet<String>>,
    pub unresolved_targets: BTreeMap<String, Vec<(String, usize)>>,
    pub exact_unresolved_targets: BTreeMap<String, Vec<(String, usize)>>,
    pub exact_incoming_links: BTreeMap<String, BTreeSet<String>>,
}

impl VaultIndex {
    pub fn scan(vault_root: impl AsRef<Path>, config: &VaultConfig) -> Result<Self> {
        let vault_root = vault_root.as_ref().to_path_buf();
        let mut notes = BTreeMap::new();
        let mut assets = BTreeMap::new();
        let mut excalidraw_files = Vec::new();
        let mut canvas_files = Vec::new();
        let mut base_files = Vec::new();
        let mut template_files = Vec::new();

        for entry in WalkDir::new(&vault_root).into_iter().filter_entry(|e| {
            if e.path() == vault_root {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            // Always skip `.obsidian` and `.trash` and `private`
            if name == ".obsidian" || name == ".trash" || name == "private" {
                return false;
            }
            true
        }) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs_path = entry.path();
            let rel_path_buf = abs_path.strip_prefix(&vault_root).unwrap_or(abs_path);
            let rel_path = rel_path_buf
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");

            let file_name = entry.file_name().to_string_lossy().to_string();
            let lower_name = file_name.to_ascii_lowercase();

            if rel_path.starts_with("templates/") {
                template_files.push(rel_path);
                continue;
            }

            if lower_name.ends_with(".excalidraw.md") || rel_path.starts_with("Excalidraw/") {
                excalidraw_files.push(rel_path.clone());
                if rel_path.starts_with("assets/") {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    assets.insert(rel_path, size);
                }
                continue;
            }

            if lower_name.ends_with(".canvas") {
                canvas_files.push(rel_path);
                continue;
            }

            if lower_name.ends_with(".base") {
                base_files.push(rel_path);
                continue;
            }

            if rel_path.starts_with("assets/") || !lower_name.ends_with(".md") {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                assets.insert(rel_path, size);
                continue;
            }

            // Skip any additional configured ignore_paths
            if config.ignore_paths.iter().any(|ig| {
                rel_path == *ig || rel_path.starts_with(&format!("{}/", ig))
            }) {
                continue;
            }

            let raw_markdown = fs::read_to_string(abs_path)
                .with_context(|| format!("Failed to read note {}", abs_path.display()))?;
            let frontmatter = parse_frontmatter(&raw_markdown);
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
            let slug = slugify_file_path(&rel_path);
            let is_moc = file_stem.to_ascii_lowercase().contains("moc");

            notes.insert(
                rel_path.clone(),
                IndexedNote {
                    rel_path,
                    file_name,
                    file_stem,
                    slug,
                    is_moc,
                    frontmatter,
                    body,
                    raw_len_bytes: raw_markdown.len(),
                },
            );
        }

        // Build lookup tables for note resolution (by rel_path, file_stem, and clean_aliases)
        let mut note_lookup: HashMap<String, String> = HashMap::new();
        let mut exact_note_lookup: HashMap<String, String> = HashMap::new();
        for (rel_path, note) in &notes {
            exact_note_lookup.insert(note.file_stem.clone(), rel_path.clone());
            exact_note_lookup.insert(note.file_name.clone(), rel_path.clone());
            exact_note_lookup.insert(rel_path.clone(), rel_path.clone());

            // Priority 1: exact normalized stem and rel_path
            note_lookup.insert(normalize_lookup_key(&note.file_stem), rel_path.clone());
            note_lookup.insert(normalize_lookup_key(&note.file_name), rel_path.clone());
            note_lookup.insert(normalize_lookup_key(rel_path), rel_path.clone());
        }
        for (rel_path, note) in &notes {
            // Priority 2: clean frontmatter aliases (excluding "[]")
            for alias in &note.frontmatter.clean_aliases {
                let key = normalize_lookup_key(alias);
                note_lookup.entry(key).or_insert_with(|| rel_path.clone());
            }
        }

        // Build lookup table for assets (by rel_path and filename)
        let mut asset_lookup: HashMap<String, String> = HashMap::new();
        for asset_path in assets.keys() {
            asset_lookup.insert(normalize_lookup_key(asset_path), asset_path.clone());
            if let Some(fname) = asset_path.rsplit('/').next() {
                asset_lookup.insert(normalize_lookup_key(fname), asset_path.clone());
            }
        }

        let mut incoming_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut exact_incoming_links: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for rel_path in notes.keys() {
            incoming_links.insert(rel_path.clone(), BTreeSet::new());
            exact_incoming_links.insert(rel_path.clone(), BTreeSet::new());
        }

        let mut resolved_links = Vec::new();
        let mut unresolved_targets: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
        let mut exact_unresolved_targets: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();

        for (from_path, note) in &notes {
            for wl in &note.body.wikilinks {
                let resolved = resolve_wikilink(
                    from_path,
                    wl,
                    &note_lookup,
                    &asset_lookup,
                );
                if let Some(ref target_note) = resolved.resolved_note {
                    if target_note != from_path {
                        incoming_links
                            .entry(target_note.clone())
                            .or_default()
                            .insert(from_path.clone());
                    }
                } else if resolved.resolved_asset.is_none() && !resolved.is_epub && !wl.target.is_empty() {
                    unresolved_targets
                        .entry(wl.target.clone())
                        .or_default()
                        .push((from_path.clone(), wl.line));
                }

                // Also track strict exact-case stem resolution (without case-folding or aliases)
                if !wl.is_embed && !resolved.is_epub && !wl.target.is_empty() {
                    if let Some(exact_target) = exact_note_lookup.get(wl.target.trim()) {
                        if exact_target != from_path {
                            exact_incoming_links
                                .entry(exact_target.clone())
                                .or_default()
                                .insert(from_path.clone());
                        }
                    } else if resolved.resolved_asset.is_none() {
                        exact_unresolved_targets
                            .entry(wl.target.clone())
                            .or_default()
                            .push((from_path.clone(), wl.line));
                    }
                }

                resolved_links.push(resolved);
            }
        }

        Ok(Self {
            vault_root,
            notes,
            assets,
            excalidraw_files,
            canvas_files,
            base_files,
            template_files,
            resolved_links,
            incoming_links,
            unresolved_targets,
            exact_unresolved_targets,
            exact_incoming_links,
        })
    }

    /// Returns notes that have 0 incoming wikilinks from other notes (using case-insensitive + alias resolution).
    pub fn orphan_notes(&self) -> Vec<&IndexedNote> {
        self.notes
            .values()
            .filter(|n| {
                self.incoming_links
                    .get(&n.rel_path)
                    .map(|set| set.is_empty())
                    .unwrap_or(true)
            })
            .collect()
    }

    /// Returns notes that have 0 incoming wikilinks under strict exact-case filename matching.
    pub fn exact_orphan_notes(&self) -> Vec<&IndexedNote> {
        self.notes
            .values()
            .filter(|n| {
                self.exact_incoming_links
                    .get(&n.rel_path)
                    .map(|set| set.is_empty())
                    .unwrap_or(true)
            })
            .collect()
    }

    pub fn generate_markdown_report(&self) -> String {
        let total_notes = self.notes.len();
        let moc_count = self.notes.values().filter(|n| n.is_moc).count();
        let total_asset_bytes: u64 = self.assets.values().sum();
        let png_count = self
            .assets
            .keys()
            .filter(|k| k.to_ascii_lowercase().ends_with(".png"))
            .count();

        let wikilink_count = self
            .resolved_links
            .iter()
            .filter(|l| !l.is_embed)
            .count();
        let embed_count = self
            .resolved_links
            .iter()
            .filter(|l| l.is_embed)
            .count();
        let external_link_count: usize = self
            .notes
            .values()
            .map(|n| n.body.external_links.len())
            .sum();
        let unresolved_target_count = self.unresolved_targets.len();
        let unresolved_occurrence_count: usize =
            self.unresolved_targets.values().map(|v| v.len()).sum();
        let exact_unresolved_count = self.exact_unresolved_targets.len();
        let exact_unresolved_occ: usize = self
            .exact_unresolved_targets
            .values()
            .map(|v| v.len())
            .sum();
        let orphans = self.orphan_notes();
        let exact_orphans = self.exact_orphan_notes();

        let notes_with_fm = self
            .notes
            .values()
            .filter(|n| n.frontmatter.has_frontmatter)
            .count();
        let notes_without_fm = total_notes - notes_with_fm;
        let notes_with_title = self
            .notes
            .values()
            .filter(|n| n.frontmatter.title.is_some())
            .count();
        let notes_with_tags_key = self
            .notes
            .values()
            .filter(|n| n.frontmatter.has_tags_key)
            .count();
        let notes_with_empty_tags = self
            .notes
            .values()
            .filter(|n| n.frontmatter.has_tags_key && n.frontmatter.tags.is_empty())
            .count();
        let notes_with_aliases = self
            .notes
            .values()
            .filter(|n| !n.frontmatter.raw_aliases.is_empty())
            .count();
        let notes_with_placeholder_alias = self
            .notes
            .values()
            .filter(|n| n.frontmatter.has_placeholder_alias)
            .count();
        let title_mismatches: Vec<_> = self
            .notes
            .values()
            .filter_map(|n| n.title_mismatch().map(|(t, s)| (&n.rel_path, t, s)))
            .collect();
        let multi_h1_notes: Vec<_> = self
            .notes
            .values()
            .filter(|n| n.h1_count() > 1)
            .collect();

        let notes_with_tables = self
            .notes
            .values()
            .filter(|n| !n.body.tables.is_empty())
            .count();
        let total_tables: usize = self.notes.values().map(|n| n.body.tables.len()).sum();
        let total_tasks: usize = self.notes.values().map(|n| n.body.tasks.len()).sum();
        let total_fn_refs: usize = self
            .notes
            .values()
            .map(|n| n.body.footnote_refs.len())
            .sum();
        let total_fn_defs: usize = self
            .notes
            .values()
            .map(|n| n.body.footnote_defs.len())
            .sum();
        let notes_with_blockquotes = self
            .notes
            .values()
            .filter(|n| n.body.has_blockquote)
            .count();
        let total_nbsp: usize = self.notes.values().map(|n| n.body.nbsp_count).sum();
        let total_highlight_issues: usize = self
            .notes
            .values()
            .map(|n| n.body.highlight_issues.len())
            .sum();

        let mut out = String::new();
        out.push_str("# Nota Vault Index & Health Report\n\n");
        out.push_str("## Summary Metrics\n\n");
        out.push_str("| Category | Metric | Value |\n");
        out.push_str("| :--- | :--- | :--- |\n");
        out.push_str(&format!("| **Content** | Active Markdown Notes | {} |\n", total_notes));
        out.push_str(&format!("| **Content** | Maps of Content (MOCs) | {} |\n", moc_count));
        out.push_str(&format!(
            "| **Content** | Assets | {} files ({:.2} MB, {} PNGs) |\n",
            self.assets.len(),
            total_asset_bytes as f64 / (1024.0 * 1024.0),
            png_count
        ));
        out.push_str(&format!(
            "| **Content** | Non-Note Files | {} Excalidraw, {} Canvas, {} Base, {} Templates |\n",
            self.excalidraw_files.len(),
            self.canvas_files.len(),
            self.base_files.len(),
            self.template_files.len()
        ));
        out.push_str(&format!("| **Linking** | Wikilinks | {} |\n", wikilink_count));
        out.push_str(&format!("| **Linking** | Image/File Embeds | {} |\n", embed_count));
        out.push_str(&format!("| **Linking** | External Links | {} |\n", external_link_count));
        out.push_str(&format!(
            "| **Linking** | Unresolved Targets (Exact Stem Match) | {} targets ({} occurrences) |\n",
            exact_unresolved_count, exact_unresolved_occ
        ));
        out.push_str(&format!(
            "| **Linking** | Unresolved Targets (Case-Folded + Aliases) | {} targets ({} occurrences) |\n",
            unresolved_target_count, unresolved_occurrence_count
        ));
        out.push_str(&format!(
            "| **Linking** | Orphan Notes (0 incoming links) | {} exact ({} after case/alias folding) |\n",
            exact_orphans.len(),
            orphans.len()
        ));
        out.push_str(&format!(
            "| **Metadata** | Frontmatter Presence | {} with frontmatter, {} without |\n",
            notes_with_fm, notes_without_fm
        ));
        out.push_str(&format!("| **Metadata** | Notes with `title` | {} |\n", notes_with_title));
        out.push_str(&format!(
            "| **Metadata** | Notes with `tags` key | {} ({} empty tag lists) |\n",
            notes_with_tags_key, notes_with_empty_tags
        ));
        out.push_str(&format!(
            "| **Metadata** | Notes with `aliases` | {} ({} with placeholder `[\"[]\"]`) |\n",
            notes_with_aliases, notes_with_placeholder_alias
        ));
        out.push_str(&format!(
            "| **Metadata** | `title != filename` Mismatches | {} |\n",
            title_mismatches.len()
        ));
        out.push_str(&format!(
            "| **Metadata** | Notes with Multiple H1s | {} |\n",
            multi_h1_notes.len()
        ));
        out.push_str(&format!(
            "| **Markdown** | Tables | {} notes ({} total tables) |\n",
            notes_with_tables, total_tables
        ));
        out.push_str(&format!("| **Markdown** | Task Lines | {} |\n", total_tasks));
        out.push_str(&format!(
            "| **Markdown** | Footnotes | {} references ({} definitions) |\n",
            total_fn_refs, total_fn_defs
        ));
        out.push_str(&format!(
            "| **Markdown** | Notes with Blockquotes | {} |\n",
            notes_with_blockquotes
        ));
        out.push_str(&format!(
            "| **Markdown** | Non-Standard `===` Highlight Issues | {} |\n",
            total_highlight_issues
        ));
        out.push_str(&format!("| **Text** | Non-Breaking Spaces (NBSP) | {} |\n\n", total_nbsp));

        out.push_str("## Top Unresolved Link Targets\n\n");
        let mut sorted_unresolved: Vec<_> = self.unresolved_targets.iter().collect();
        sorted_unresolved.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
        out.push_str("| Unresolved Target | Occurrences | Sample Source |\n");
        out.push_str("| :--- | :--- | :--- |\n");
        for (target, locs) in sorted_unresolved.iter().take(30) {
            let sample = locs
                .first()
                .map(|(f, l)| format!("{}:{}", f, l))
                .unwrap_or_default();
            out.push_str(&format!("| `[[{}]]` | {} | `{}` |\n", target, locs.len(), sample));
        }
        out.push('\n');

        out.push_str("## Orphan Notes (No Incoming Links)\n\n");
        for orphan in &orphans {
            out.push_str(&format!("- `{}`\n", orphan.rel_path));
        }
        out.push('\n');

        out.push_str("## Title vs. Filename Mismatches\n\n");
        out.push_str("| File | Frontmatter `title` | Filename Stem |\n");
        out.push_str("| :--- | :--- | :--- |\n");
        for (path, title, stem) in &title_mismatches {
            out.push_str(&format!("| `{}` | `{}` | `{}` |\n", path, title, stem));
        }
        out.push('\n');

        out.push_str("## Notes with Multiple H1 Headings\n\n");
        for note in &multi_h1_notes {
            out.push_str(&format!("- `{}` ({} H1 headings)\n", note.rel_path, note.h1_count()));
        }
        out.push('\n');

        if total_highlight_issues > 0 {
            out.push_str("## Non-Standard `===` Highlight Delimiters\n\n");
            for note in self.notes.values() {
                for issue in &note.body.highlight_issues {
                    out.push_str(&format!(
                        "- `{}:{}` — `{}` ({})\n",
                        note.rel_path, issue.line, issue.snippet, issue.description
                    ));
                }
            }
            out.push('\n');
        }

        out
    }

    /// Computes related notes for `note_path` based on shared tags, co-citations (shared outgoing links),
    /// and bibliographic coupling (shared incoming backlinks), excluding already directly linked notes.
    pub fn related_notes(&self, note_path: &str, limit: usize) -> Vec<(String, usize, String)> {
        let Some(target) = self.notes.get(note_path) else {
            return Vec::new();
        };
        let target_tags: BTreeSet<&str> =
            target.frontmatter.tags.iter().map(|s| s.as_str()).collect();
        let direct_incoming = self
            .incoming_links
            .get(note_path)
            .cloned()
            .unwrap_or_default();
        let direct_outgoing: BTreeSet<String> = self
            .resolved_links
            .iter()
            .filter(|l| l.from_note == note_path)
            .filter_map(|l| l.resolved_note.clone())
            .collect();

        let mut scored = Vec::new();
        for (cand_path, cand) in &self.notes {
            if cand_path == note_path {
                continue;
            }
            let shared_tags: Vec<&str> = cand
                .frontmatter
                .tags
                .iter()
                .map(|s| s.as_str())
                .filter(|t| target_tags.contains(t))
                .collect();

            let cand_outgoing: BTreeSet<String> = self
                .resolved_links
                .iter()
                .filter(|l| &l.from_note == cand_path)
                .filter_map(|l| l.resolved_note.clone())
                .collect();
            let shared_out = direct_outgoing.intersection(&cand_outgoing).count();

            let cand_incoming = self
                .incoming_links
                .get(cand_path)
                .cloned()
                .unwrap_or_default();
            let shared_in = direct_incoming.intersection(&cand_incoming).count();

            let score = shared_tags.len() * 3 + shared_out * 2 + shared_in * 2;
            if score > 0 {
                let mut reasons = Vec::new();
                if !shared_tags.is_empty() {
                    reasons.push(format!("tags: {}", shared_tags.join(", ")));
                }
                if shared_out > 0 {
                    reasons.push(format!("{} shared links", shared_out));
                }
                if shared_in > 0 {
                    reasons.push(format!("{} shared backlinks", shared_in));
                }
                scored.push((cand_path.clone(), score, reasons.join(" · ")));
            }
        }

        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(limit);
        scored
    }

    /// Generates a Map of Content (MOC) Markdown document from a tag filter or keyword.
    pub fn generate_moc_markdown(&self, title: &str, filter_tag: Option<&str>) -> String {
        let mut matching_notes: Vec<&IndexedNote> = self
            .notes
            .values()
            .filter(|n| {
                if let Some(tag) = filter_tag {
                    let q = tag.trim().to_lowercase();
                    if q.is_empty() {
                        true
                    } else {
                        n.frontmatter
                            .tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&q))
                            || n.file_stem.to_lowercase().contains(&q)
                    }
                } else {
                    true
                }
            })
            .collect();

        matching_notes.sort_by(|a, b| {
            let deg_a = self.incoming_links.get(&a.rel_path).map_or(0, |s| s.len());
            let deg_b = self.incoming_links.get(&b.rel_path).map_or(0, |s| s.len());
            deg_b
                .cmp(&deg_a)
                .then_with(|| a.file_stem.cmp(&b.file_stem))
        });

        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("title: {}\n", title));
        out.push_str("tags:\n  - MOC\n");
        if let Some(t) = filter_tag {
            if !t.trim().is_empty() {
                out.push_str(&format!("  - {}\n", t.trim()));
            }
        }
        out.push_str("---\n\n");
        out.push_str(&format!("# {}\n\n", title));
        out.push_str("## Core Notes\n\n");
        for note in matching_notes.iter().take(60) {
            let inc = self
                .incoming_links
                .get(&note.rel_path)
                .map_or(0, |s| s.len());
            out.push_str(&format!(
                "- [[{}]] ({} incoming links)\n",
                note.file_stem, inc
            ));
        }
        out
    }

    /// Builds a 2D graph layout (`[0.0..1.0]` normalized coordinates) for either the Global Vault Graph
    /// or a Local Neighborhood Graph around `focus_note`.
    pub fn build_graph(
        &self,
        focus_note: Option<&str>,
        include_orphans: bool,
        include_unresolved: bool,
        only_mocs: bool,
        tag_filter: Option<&str>,
    ) -> VaultGraph {
        let mut active_set: BTreeSet<String> = BTreeSet::new();

        if let Some(center) = focus_note {
            if self.notes.contains_key(center) {
                active_set.insert(center.to_string());
                if let Some(inc) = self.incoming_links.get(center) {
                    for src in inc {
                        active_set.insert(src.clone());
                    }
                }
                for link in &self.resolved_links {
                    if link.from_note == center {
                        if let Some(ref dst) = link.resolved_note {
                            active_set.insert(dst.clone());
                        }
                    }
                }
            }
        } else {
            for (path, note) in &self.notes {
                if only_mocs && !note.is_moc {
                    continue;
                }
                if let Some(tf) = tag_filter {
                    let q = tf.trim().to_lowercase();
                    if !q.is_empty()
                        && !note
                            .frontmatter
                            .tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&q))
                    {
                        continue;
                    }
                }
                let inc_deg = self.incoming_links.get(path).map_or(0, |s| s.len());
                let out_deg = self
                    .resolved_links
                    .iter()
                    .filter(|l| &l.from_note == path && l.resolved_note.is_some())
                    .count();
                if !include_orphans && inc_deg == 0 && out_deg == 0 {
                    continue;
                }
                active_set.insert(path.clone());
            }
        }

        let mut nodes = Vec::new();
        let mut path_to_idx: BTreeMap<String, usize> = BTreeMap::new();

        for path in &active_set {
            if let Some(note) = self.notes.get(path) {
                let idx = nodes.len();
                path_to_idx.insert(path.clone(), idx);
                let degree = self.incoming_links.get(path).map_or(0, |s| s.len())
                    + self
                        .resolved_links
                        .iter()
                        .filter(|l| &l.from_note == path && l.resolved_note.is_some())
                        .count();
                nodes.push(GraphNode {
                    id: path.clone(),
                    label: note.file_stem.clone(),
                    is_moc: note.is_moc,
                    is_unresolved: false,
                    is_orphan: self
                        .incoming_links
                        .get(path)
                        .map_or(true, |s| s.is_empty()),
                    degree,
                    x: 0.5,
                    y: 0.5,
                });
            }
        }

        let mut edges = Vec::new();
        let mut seen_edges = BTreeSet::new();

        for link in &self.resolved_links {
            let Some(&src_idx) = path_to_idx.get(&link.from_note) else {
                continue;
            };
            if let Some(ref target_path) = link.resolved_note {
                if let Some(&dst_idx) = path_to_idx.get(target_path) {
                    if src_idx != dst_idx {
                        let pair = if src_idx < dst_idx {
                            (src_idx, dst_idx)
                        } else {
                            (dst_idx, src_idx)
                        };
                        if seen_edges.insert(pair) {
                            edges.push(GraphEdge {
                                source: src_idx,
                                target: dst_idx,
                            });
                        }
                    }
                }
            } else if include_unresolved && !link.is_embed && !link.is_epub && !link.target_raw.is_empty() {
                let unres_key = format!("unres:{}", link.target_raw);
                let dst_idx = if let Some(&idx) = path_to_idx.get(&unres_key) {
                    idx
                } else if nodes.len() < 450 {
                    let idx = nodes.len();
                    path_to_idx.insert(unres_key.clone(), idx);
                    nodes.push(GraphNode {
                        id: link.target_raw.clone(),
                        label: format!("? {}", link.target_raw),
                        is_moc: false,
                        is_unresolved: true,
                        is_orphan: false,
                        degree: 1,
                        x: 0.5,
                        y: 0.5,
                    });
                    idx
                } else {
                    continue;
                };
                let pair = if src_idx < dst_idx {
                    (src_idx, dst_idx)
                } else {
                    (dst_idx, src_idx)
                };
                if seen_edges.insert(pair) {
                    edges.push(GraphEdge {
                        source: src_idx,
                        target: dst_idx,
                    });
                }
            }
        }

        // Compute concentric radial + spring layout so hubs/MOCs sit near the center and connected nodes cluster cleanly
        let n = nodes.len();
        if n > 0 {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| {
                let a_focus = focus_note.is_some_and(|f| nodes[a].id == f);
                let b_focus = focus_note.is_some_and(|f| nodes[b].id == f);
                b_focus
                    .cmp(&a_focus)
                    .then_with(|| nodes[b].is_moc.cmp(&nodes[a].is_moc))
                    .then_with(|| nodes[b].degree.cmp(&nodes[a].degree))
            });

            for (rank, &node_idx) in order.iter().enumerate() {
                if rank == 0 && focus_note.is_some() {
                    nodes[node_idx].x = 0.5;
                    nodes[node_idx].y = 0.5;
                    continue;
                }
                let ring = ((rank as f32).sqrt() / (n as f32).sqrt().max(1.0)).clamp(0.12, 0.92);
                let golden_angle = 2.399_963_1_f32;
                let theta = rank as f32 * golden_angle;
                nodes[node_idx].x = (0.5 + 0.44 * ring * theta.cos()).clamp(0.05, 0.95);
                nodes[node_idx].y = (0.5 + 0.44 * ring * theta.sin()).clamp(0.05, 0.95);
            }

            // 15 iterations of spring relaxation along edges
            for _ in 0..15 {
                let mut dx = vec![0.0_f32; n];
                let mut dy = vec![0.0_f32; n];
                for e in &edges {
                    let ux = nodes[e.source].x;
                    let uy = nodes[e.source].y;
                    let vx = nodes[e.target].x;
                    let vy = nodes[e.target].y;
                    dx[e.source] += (vx - ux) * 0.04;
                    dy[e.source] += (vy - uy) * 0.04;
                    dx[e.target] += (ux - vx) * 0.04;
                    dy[e.target] += (uy - vy) * 0.04;
                }
                for idx in 0..n {
                    if focus_note.is_some_and(|f| nodes[idx].id == f) {
                        continue;
                    }
                    nodes[idx].x = (nodes[idx].x + dx[idx]).clamp(0.05, 0.95);
                    nodes[idx].y = (nodes[idx].y + dy[idx]).clamp(0.05, 0.95);
                }
            }
        }

        VaultGraph { nodes, edges }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub is_moc: bool,
    pub is_unresolved: bool,
    pub is_orphan: bool,
    pub degree: usize,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: usize,
    pub target: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Safely cleans up frontmatter metadata and non-standard `===text===` prose highlights in a note
/// without altering logical ` === ` equivalence operators.
/// Returns `(cleaned_markdown, list_of_applied_fixes)`.
pub fn clean_note_metadata_and_highlights(
    raw_markdown: &str,
    file_stem: &str,
    remove_placeholder_aliases: bool,
    sync_title_to_stem: bool,
    fix_prose_highlights: bool,
) -> (String, Vec<String>) {
    let mut changes = Vec::new();
    let mut out = String::with_capacity(raw_markdown.len());
    let mut in_frontmatter = false;

    for (line_idx, line) in raw_markdown.split_inclusive('\n').enumerate() {
        let trimmed = line.trim();
        let line_no_nl = line.trim_end_matches(&['\r', '\n'][..]);
        let nl = &line[line_no_nl.len()..];

        if line_idx == 0 && trimmed == "---" {
            in_frontmatter = true;
            out.push_str(line);
            continue;
        }
        if in_frontmatter {
            if trimmed == "---" || trimmed == "..." {
                in_frontmatter = false;
                out.push_str(line);
                continue;
            }
            // 1. Remove `- "[]"` or `aliases: ["[]"]`
            if remove_placeholder_aliases {
                if trimmed == "- \"[]\"" || trimmed == "- '[]'" || trimmed == "- []" {
                    changes.push("Removed placeholder alias `- \"[]\"`".to_string());
                    continue;
                }
                if trimmed.starts_with("aliases:") && trimmed.contains("\"[]\"") {
                    out.push_str(&format!("aliases: []{}", nl));
                    changes.push("Cleared placeholder `aliases: [\"[]\"]`".to_string());
                    continue;
                }
            }
            // 2. Normalize known tag typo `techonlogy` -> `technology`
            if trimmed == "- techonlogy" {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                out.push_str(&format!("{}- technology{}", indent, nl));
                changes.push("Normalized tag `techonlogy` → `technology`".to_string());
                continue;
            }
            // 3. Optionally sync `title:` with `file_stem`
            if sync_title_to_stem && trimmed.starts_with("title:") {
                let current_title = trimmed
                    .strip_prefix("title:")
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !current_title.is_empty() && current_title != file_stem {
                    out.push_str(&format!("title: {}{}", file_stem, nl));
                    changes.push(format!(
                        "Synced frontmatter title `{}` → `{}`",
                        current_title, file_stem
                    ));
                    continue;
                }
            }
            out.push_str(line);
            continue;
        }

        // Body: optionally fix `===text===` / `==text ===` prose highlights while keeping ` === ` equivalence untouched
        if fix_prose_highlights && line_no_nl.contains("===") {
            let fixed_line = fix_line_prose_highlights(line_no_nl, &mut changes);
            out.push_str(&fixed_line);
            out.push_str(nl);
        } else {
            out.push_str(line);
        }
    }

    (out, changes)
}

fn fix_line_prose_highlights(line: &str, changes: &mut Vec<String>) -> String {
    // Only rewrite `===...===` when `===` is immediately adjacent to non-whitespace text (prose highlight)
    // and NOT surrounded by whitespace (` === ` logical equivalence).
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 3 <= bytes.len() && &bytes[i..i + 3] == b"===" {
            let mut eq_len = 3;
            while i + eq_len < bytes.len() && bytes[i + eq_len] == b'=' {
                eq_len += 1;
            }
            let prev_space = i == 0 || bytes[i - 1].is_ascii_whitespace();
            let next_space =
                i + eq_len >= bytes.len() || bytes[i + eq_len].is_ascii_whitespace();
            // If surrounded by spaces on both sides, it's logical equivalence ` === ` — preserve literally!
            if prev_space && next_space && eq_len == 3 {
                out.push_str("===");
                i += 3;
                continue;
            }
            // Otherwise it's a highlight delimiter (`===text===` or `==text ===`) -> normalize to `==`
            out.push_str("==");
            changes.push(format!("Normalized `{}` highlight delimiter to `==`", "=".repeat(eq_len)));
            i += eq_len;
            continue;
        }
        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn resolve_wikilink(
    from_note: &str,
    wl: &WikiLink,
    note_lookup: &HashMap<String, String>,
    asset_lookup: &HashMap<String, String>,
) -> ResolvedLink {
    if wl.epub.is_some() {
        return ResolvedLink {
            from_note: from_note.to_string(),
            line: wl.line,
            raw: wl.raw.clone(),
            target_raw: wl.target.clone(),
            resolved_note: None,
            resolved_asset: None,
            is_embed: wl.is_embed,
            is_epub: true,
        };
    }

    if wl.target.is_empty() {
        // Same-note heading link `[[#heading]]`
        return ResolvedLink {
            from_note: from_note.to_string(),
            line: wl.line,
            raw: wl.raw.clone(),
            target_raw: wl.target.clone(),
            resolved_note: Some(from_note.to_string()),
            resolved_asset: None,
            is_embed: wl.is_embed,
            is_epub: false,
        };
    }

    let key = normalize_lookup_key(&wl.target);

    if wl.is_embed {
        if let Some(asset) = asset_lookup.get(&key) {
            return ResolvedLink {
                from_note: from_note.to_string(),
                line: wl.line,
                raw: wl.raw.clone(),
                target_raw: wl.target.clone(),
                resolved_note: None,
                resolved_asset: Some(asset.clone()),
                is_embed: true,
                is_epub: false,
            };
        }
    }

    if let Some(note_path) = note_lookup.get(&key) {
        return ResolvedLink {
            from_note: from_note.to_string(),
            line: wl.line,
            raw: wl.raw.clone(),
            target_raw: wl.target.clone(),
            resolved_note: Some(note_path.clone()),
            resolved_asset: None,
            is_embed: wl.is_embed,
            is_epub: false,
        };
    }

    if let Some(asset) = asset_lookup.get(&key) {
        return ResolvedLink {
            from_note: from_note.to_string(),
            line: wl.line,
            raw: wl.raw.clone(),
            target_raw: wl.target.clone(),
            resolved_note: None,
            resolved_asset: Some(asset.clone()),
            is_embed: wl.is_embed,
            is_epub: false,
        };
    }

    ResolvedLink {
        from_note: from_note.to_string(),
        line: wl.line,
        raw: wl.raw.clone(),
        target_raw: wl.target.clone(),
        resolved_note: None,
        resolved_asset: None,
        is_embed: wl.is_embed,
        is_epub: false,
    }
}
