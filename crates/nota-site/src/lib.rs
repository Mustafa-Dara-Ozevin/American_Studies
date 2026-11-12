use anyhow::{Context, Result};
use chrono::Utc;
use nota_core::{
    normalize_lookup_key, parse_wikilink_inner, slug_anchor, sluggify_segment, slugify_file_path,
    IndexedNote, SiteConfig, VaultIndex,
};
use nota_logic::render_tree_dsl_as_svg;
use pulldown_cmark::{html, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteBuildSummary {
    pub out_dir: PathBuf,
    pub pages_built: usize,
    pub tag_pages_built: usize,
    pub alias_redirects_built: usize,
    pub assets_copied: usize,
}

/// Renders a Markdown note body into an HTML fragment with:
/// - Smart punctuation and arrow replacement strictly DISABLED (`<->`, `<-`, `~`, `===` preserved verbatim)
/// - ` ```tree ` blocks rendered as crisp SVG Truth Trees
/// - `[[wikilinks]]`, `![[embeds]]`, and `[[Book.epub#p=42|Title, s. 42]]` bibliographic chips
/// - `==highlight==` rendered as `<mark>` without colliding with ` === ` logical equivalence
/// - `github-slugger` compatible `id="..."` attributes on headings.
pub fn render_note_to_html_fragment(
    raw_markdown: &str,
    body_byte_offset: usize,
    index: &VaultIndex,
    base_url_path: &str,
) -> String {
    let body = &raw_markdown[body_byte_offset.min(raw_markdown.len())..];

    // Build lookup maps from VaultIndex
    let mut note_slug_lookup: HashMap<String, (String, String)> = HashMap::new();
    for (rel_path, note) in &index.notes {
        note_slug_lookup.insert(
            normalize_lookup_key(&note.file_stem),
            (note.slug.clone(), note.file_stem.clone()),
        );
        note_slug_lookup.insert(
            normalize_lookup_key(&note.file_name),
            (note.slug.clone(), note.file_stem.clone()),
        );
        note_slug_lookup.insert(
            normalize_lookup_key(rel_path),
            (note.slug.clone(), note.file_stem.clone()),
        );
    }
    for note in index.notes.values() {
        for alias in &note.frontmatter.clean_aliases {
            note_slug_lookup
                .entry(normalize_lookup_key(alias))
                .or_insert_with(|| (note.slug.clone(), note.file_stem.clone()));
        }
    }

    let mut asset_lookup: HashMap<String, String> = HashMap::new();
    for asset_path in index.assets.keys() {
        asset_lookup.insert(normalize_lookup_key(asset_path), asset_path.clone());
        if let Some(fname) = asset_path.rsplit('/').next() {
            asset_lookup.insert(normalize_lookup_key(fname), asset_path.clone());
        }
    }

    let preprocessed = preprocess_obsidian_and_trees(
        body,
        &note_slug_lookup,
        &asset_lookup,
        base_url_path,
    );

    // Configure pulldown-cmark: tables, tasks, footnotes, strikethrough ON; smart punctuation OFF!
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(&preprocessed, opts);
    let mut events: Vec<Event> = Vec::new();
    let mut in_heading: Option<HeadingLevel> = None;
    let mut heading_text = String::new();
    let mut heading_events: Vec<Event> = Vec::new();

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = Some(level);
                heading_text.clear();
                heading_events.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = in_heading.take() {
                    let anchor = slug_anchor(&heading_text);
                    let tag_name = match level {
                        HeadingLevel::H1 => "h1",
                        HeadingLevel::H2 => "h2",
                        HeadingLevel::H3 => "h3",
                        HeadingLevel::H4 => "h4",
                        HeadingLevel::H5 => "h5",
                        HeadingLevel::H6 => "h6",
                    };
                    events.push(Event::Html(
                        format!("<{} id=\"{}\">", tag_name, html_escape(&anchor)).into(),
                    ));
                    events.append(&mut heading_events);
                    events.push(Event::Html(format!("</{}>", tag_name).into()));
                }
            }
            other => {
                if in_heading.is_some() {
                    if let Event::Text(ref t) | Event::Code(ref t) = other {
                        heading_text.push_str(t);
                    }
                    heading_events.push(other);
                } else {
                    events.push(other);
                }
            }
        }
    }

    let mut html_out = String::new();
    html::push_html(&mut html_out, events.into_iter());
    html_out
}

fn preprocess_obsidian_and_trees(
    body: &str,
    note_slug_lookup: &HashMap<String, (String, String)>,
    asset_lookup: &HashMap<String, String>,
    base_url_path: &str,
) -> String {
    let mut out = String::with_capacity(body.len() + 512);
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0usize;
    let mut in_fence = false;
    let mut in_tree_fence = false;
    let mut tree_buf = String::new();

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if !in_fence && !in_tree_fence && trimmed.starts_with("```tree") {
            in_tree_fence = true;
            tree_buf.clear();
            i += 1;
            continue;
        }
        if in_tree_fence {
            if trimmed.starts_with("```") {
                in_tree_fence = false;
                let svg = render_tree_dsl_as_svg(&tree_buf);
                out.push_str("\n<div class=\"nota-truth-tree\">\n");
                out.push_str(&svg);
                out.push_str("\n<details class=\"nota-tree-dsl\"><summary>Truth Tree Steps</summary><pre><code>");
                out.push_str(&html_escape(&tree_buf));
                out.push_str("</code></pre></details>\n</div>\n\n");
            } else {
                tree_buf.push_str(line);
                tree_buf.push('\n');
            }
            i += 1;
            continue;
        }

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }

        let processed_line = preprocess_inline_syntax(
            line,
            note_slug_lookup,
            asset_lookup,
            base_url_path,
        );
        out.push_str(&processed_line);
        out.push('\n');
        i += 1;
    }

    out
}

fn preprocess_inline_syntax(
    line: &str,
    note_slug_lookup: &HashMap<String, (String, String)>,
    asset_lookup: &HashMap<String, String>,
    base_url_path: &str,
) -> String {
    // Strip invisible SRS comments (`<!--c:...-->` and `<!--SR:...-->`)
    let mut clean_line = line.to_string();
    while let Some(start) = clean_line.find("<!--") {
        let Some(end_rel) = clean_line[start + 4..].find("-->") else {
            break;
        };
        let end = start + 4 + end_rel + 3;
        let comment = &clean_line[start..end];
        if comment.starts_with("<!--c:") || comment.starts_with("<!--SR:") {
            clean_line.replace_range(start..end, "");
        } else {
            break;
        }
    }

    let bytes = clean_line.as_bytes();
    let mut out = String::with_capacity(clean_line.len() + 64);
    let mut i = 0usize;
    let mut in_code = false;

    while i < bytes.len() {
        if bytes[i] == b'`' {
            in_code = !in_code;
            out.push('`');
            i += 1;
            continue;
        }
        if in_code {
            let ch = clean_line[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }

        // Prevent pulldown-cmark from treating single `~A` as strikethrough if a second `~` exists later on the line,
        // while keeping paired `~~strikethrough~~` intact.
        if bytes[i] == b'~' && (i + 1 >= bytes.len() || bytes[i + 1] != b'~') && (i == 0 || bytes[i - 1] != b'~') {
            out.push_str("&#126;");
            i += 1;
            continue;
        }

        // Protect `<->` and `<-` inside text outside HTML tags so `<` isn't parsed as a raw HTML tag start
        if bytes[i] == b'<' {
            let rem = &clean_line[i..];
            if rem.starts_with("<->") {
                out.push_str("&lt;-&gt;");
                i += 3;
                continue;
            } else if rem.starts_with("<=>") {
                out.push_str("&lt;=&gt;");
                i += 3;
                continue;
            } else if rem.starts_with("<!->") {
                out.push_str("&lt;!-&gt;");
                i += 4;
                continue;
            } else if rem.starts_with("<!-") && !rem.starts_with("<!--") {
                out.push_str("&lt;!-");
                i += 3;
                continue;
            } else if rem.starts_with("<-") && !rem.starts_with("<!--") {
                out.push_str("&lt;-");
                i += 2;
                continue;
            } else if rem.starts_with("<=") {
                out.push_str("&lt;=");
                i += 2;
                continue;
            }
        }

        // Image/file embed `![[...]]` or wikilink `[[...]]`
        let is_embed = bytes[i] == b'!' && i + 2 < bytes.len() && bytes[i + 1] == b'[' && bytes[i + 2] == b'[';
        let is_wikilink = bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[';
        if is_embed || is_wikilink {
            let open_offset = if is_embed { i + 3 } else { i + 2 };
            if let Some(close_rel) = clean_line[open_offset..].find("]]") {
                let inner = &clean_line[open_offset..open_offset + close_rel];
                if let Some(wl) = parse_wikilink_inner(String::new(), inner, is_embed, 1) {
                    let target = wl.target;
                    let subpath = wl.heading;
                    let display = wl.alias.unwrap_or_else(|| {
                        if target.is_empty() {
                            subpath.clone().unwrap_or_default()
                        } else if let Some(ref sub) = subpath {
                            format!("{}#{}", target, sub)
                        } else {
                            target.clone()
                        }
                    });

                    if let Some(epub_loc) = wl.epub {
                        let page_badge = epub_loc
                            .page
                            .map(|p| format!("s. {}", p))
                            .unwrap_or_else(|| "EPUB".to_string());
                        out.push_str(&format!(
                            "<span class=\"nota-epub-chip\" title=\"{} ({})\">📖 {}</span>",
                            html_escape(&epub_loc.book_file),
                            html_escape(&page_badge),
                            html_escape(&display)
                        ));
                    } else if is_embed {
                        let key = normalize_lookup_key(&target);
                        let asset_rel = asset_lookup
                            .get(&key)
                            .cloned()
                            .unwrap_or_else(|| format!("assets/{}", target));
                        let prefix = if base_url_path.is_empty() {
                            String::new()
                        } else {
                            format!("{}/", base_url_path.trim_end_matches('/'))
                        };
                        out.push_str(&format!(
                            "<img class=\"nota-embed\" src=\"{}{}\" alt=\"{}\" loading=\"lazy\" />",
                            prefix,
                            html_escape(&asset_rel),
                            html_escape(&display)
                        ));
                    } else if target.is_empty() {
                        let anchor = subpath.as_deref().map(slug_anchor).unwrap_or_default();
                        out.push_str(&format!(
                            "<a class=\"nota-wikilink\" href=\"#{}\">{}</a>",
                            html_escape(&anchor),
                            html_escape(&display)
                        ));
                    } else {
                        let key = normalize_lookup_key(&target);
                        if let Some((slug, _stem)) = note_slug_lookup.get(&key) {
                            let prefix = if base_url_path.is_empty() {
                                String::new()
                            } else {
                                format!("{}/", base_url_path.trim_end_matches('/'))
                            };
                            let anchor_suffix = subpath
                                .as_deref()
                                .map(|s| format!("#{}", slug_anchor(s)))
                                .unwrap_or_default();
                            out.push_str(&format!(
                                "<a class=\"nota-wikilink\" href=\"{}{}.html{}\">{}</a>",
                                prefix,
                                html_escape(slug),
                                html_escape(&anchor_suffix),
                                html_escape(&display)
                            ));
                        } else {
                            out.push_str(&format!(
                                "<span class=\"nota-unresolved\" title=\"Unresolved link: {}\">{}</span>",
                                html_escape(&target),
                                html_escape(&display)
                            ));
                        }
                    }

                    i = open_offset + close_rel + 2;
                    continue;
                }
            }
        }

        // Disambiguate `===` logical equivalence from `==highlight==`
        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"==" {
            let mut eq_len = 2;
            while i + eq_len < bytes.len() && bytes[i + eq_len] == b'=' {
                eq_len += 1;
            }
            let prev_space = i == 0 || bytes[i - 1].is_ascii_whitespace();
            let next_space =
                i + eq_len >= bytes.len() || bytes[i + eq_len].is_ascii_whitespace();

            if eq_len == 3 && prev_space && next_space {
                // Logical equivalence ` === ` -> preserve verbatim!
                out.push_str("===");
                i += 3;
                continue;
            } else if !next_space {
                // Look for closing `==` or `===` on the same line without crossing ` === `
                let content_start = i + eq_len;
                if let Some(close_rel) = clean_line[content_start..].find("==") {
                    let close_pos = content_start + close_rel;
                    let mut close_eq_len = 2;
                    while close_pos + close_eq_len < bytes.len()
                        && bytes[close_pos + close_eq_len] == b'='
                    {
                        close_eq_len += 1;
                    }
                    let inner_text = &clean_line[content_start..close_pos];
                    if !inner_text.contains(" === ") {
                        out.push_str("<mark>");
                        out.push_str(&html_escape(inner_text.trim()));
                        out.push_str("</mark>");
                        i = close_pos + close_eq_len;
                        continue;
                    }
                }
            }
            for _ in 0..eq_len {
                out.push('=');
            }
            i += eq_len;
            continue;
        }

        let ch = clean_line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

/// Exports a single note as a standalone, self-contained HTML file with Nord theme styles,
/// SVG Truth Trees, TOC, and Backlinks.
pub fn export_note_standalone_html(
    note: &IndexedNote,
    raw_markdown: &str,
    index: &VaultIndex,
) -> String {
    let title = note
        .frontmatter
        .title
        .as_deref()
        .unwrap_or(&note.file_stem);
    let body_html =
        render_note_to_html_fragment(raw_markdown, note.frontmatter.body_byte_offset, index, "");

    let mut toc_html = String::new();
    if !note.body.headings.is_empty() {
        toc_html.push_str("<nav class=\"nota-toc\"><h3>Outline</h3><ul>");
        for h in &note.body.headings {
            toc_html.push_str(&format!(
                "<li class=\"toc-h{}\"><a href=\"#{}\">{}</a></li>",
                h.level,
                html_escape(&h.anchor),
                html_escape(&h.text)
            ));
        }
        toc_html.push_str("</ul></nav>");
    }

    let mut backlinks_html = String::new();
    if let Some(inc) = index.incoming_links.get(&note.rel_path) {
        if !inc.is_empty() {
            backlinks_html.push_str("<section class=\"nota-backlinks\"><h3>Backlinks</h3><ul>");
            for src in inc {
                let slug = index
                    .notes
                    .get(src)
                    .map(|n| n.slug.as_str())
                    .unwrap_or(src.as_str());
                let stem = src.strip_suffix(".md").unwrap_or(src);
                backlinks_html.push_str(&format!(
                    "<li><a href=\"{}.html\">{}</a></li>",
                    html_escape(slug),
                    html_escape(stem)
                ));
            }
            backlinks_html.push_str("</ul></section>");
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="tr" data-theme="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <style>{css}</style>
</head>
<body>
  <div class="nota-layout">
    <main class="nota-article" data-pagefind-body>
      <header class="nota-header">
        <h1>{title}</h1>
      </header>
      {body_html}
      {backlinks_html}
    </main>
    <aside class="nota-sidebar">
      {toc_html}
    </aside>
  </div>
</body>
</html>"#,
        title = html_escape(title),
        css = NOTA_SITE_CSS,
        body_html = body_html,
        backlinks_html = backlinks_html,
        toc_html = toc_html
    )
}

/// Exports a Markdown note into a self-contained Typst (`.typ`) document ready for PDF compilation
/// (`typst compile note.typ note.pdf`), preserving Turkish/Greek/symbolic-logic glyphs, truth tables,
/// and Truth Trees.
pub fn export_note_typst(note: &IndexedNote, raw_markdown: &str) -> String {
    let title = note
        .frontmatter
        .title
        .as_deref()
        .unwrap_or(&note.file_stem);
    let body = &raw_markdown[note.frontmatter.body_byte_offset.min(raw_markdown.len())..];

    let mut out = String::new();
    out.push_str("// Generated by Nota Typst Exporter\n");
    out.push_str("#set page(paper: \"a4\", margin: (x: 2cm, y: 2.2cm))\n");
    out.push_str("#set text(font: (\"Noto Serif\", \"DejaVu Serif\", \"Liberation Serif\"), size: 11pt, lang: \"tr\")\n");
    out.push_str("#set par(justify: true, leading: 0.65em)\n\n");
    out.push_str(&format!(
        "#align(center)[#text(size: 18pt, weight: \"bold\")[{}]]\n#v(1em)\n\n",
        typst_escape(title)
    ));

    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0usize;
    let mut in_fence = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if !in_fence {
                in_fence = true;
                out.push_str("#block(fill: rgb(\"#2e3440\"), inset: 10pt, radius: 4pt, width: 100%)[\n#text(fill: rgb(\"#eceff4\"), font: \"DejaVu Sans Mono\", size: 9.5pt)[\n```\n");
            } else {
                in_fence = false;
                out.push_str("```\n]]\n\n");
            }
            i += 1;
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }

        // Markdown table block -> Typst `#table(...)`
        if trimmed.starts_with('|') && i + 1 < lines.len() && lines[i + 1].trim().starts_with('|') {
            let headers = nota_core::split_table_cells(trimmed);
            let cols = headers.len().max(1);
            out.push_str(&format!("#table(\n  columns: {},\n  align: center + horizon,\n", cols));
            for h in &headers {
                out.push_str(&format!("  [*{}*],\n", typst_escape(h)));
            }
            i += 2; // skip header + separator
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let cells = nota_core::split_table_cells(lines[i].trim());
                for c in 0..cols {
                    let val = cells.get(c).map(|s| s.as_str()).unwrap_or("");
                    out.push_str(&format!("  [{}],\n", typst_escape(val)));
                }
                i += 1;
            }
            out.push_str(")\n\n");
            continue;
        }

        // Headings
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed[level..].trim();
            out.push_str(&format!("{} {}\n\n", "=".repeat(level), typst_escape(text)));
            i += 1;
            continue;
        }

        // Task items & bullets
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            out.push_str(&format!("- ☐ {}\n", typst_escape(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "))
        {
            out.push_str(&format!("- ☑ {}\n", typst_escape(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            out.push_str(&format!("- {}\n", typst_escape(rest)));
            i += 1;
            continue;
        }

        out.push_str(&typst_escape(line));
        out.push('\n');
        i += 1;
    }

    out
}

/// Builds the full Quartz-compatible static site in `out_dir` (`public-nota/` by default).
pub fn build_static_site(
    index: &VaultIndex,
    site_config: &SiteConfig,
    out_dir: &Path,
) -> Result<SiteBuildSummary> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create output directory {}", out_dir.display()))?;

    // Extract base URL path (e.g. `/American_Studies` from `mustafa-dara-ozevin.github.io/American_Studies`)
    let base_url_path = if site_config.base_url.starts_with('/') {
        site_config.base_url.trim_end_matches('/').to_string()
    } else if let Some((_host, path)) = site_config.base_url.split_once('/') {
        format!("/{}", path.trim_end_matches('/'))
    } else {
        String::new()
    };

    let mut summary = SiteBuildSummary {
        out_dir: out_dir.to_path_buf(),
        ..Default::default()
    };

    // Write shared stylesheet `styles.css`
    fs::write(out_dir.join("styles.css"), NOTA_SITE_CSS)?;

    // Build sidebar navigation list
    let mut nav_items_html = String::new();
    for note in index.notes.values() {
        let badge = if note.is_moc { " <small>[MOC]</small>" } else { "" };
        nav_items_html.push_str(&format!(
            "<li><a href=\"{}/{}.html\">{}{}</a></li>\n",
            base_url_path,
            html_escape(&note.slug),
            html_escape(&note.file_stem),
            badge
        ));
    }

    let mut tags_map: BTreeMap<String, Vec<&IndexedNote>> = BTreeMap::new();
    let mut search_docs = Vec::new();

    // 1. Render every note page + alias redirect pages
    for note in index.notes.values() {
        let abs_note_path = index.vault_root.join(&note.rel_path);
        let raw_md = fs::read_to_string(&abs_note_path).unwrap_or_default();
        let body_html = render_note_to_html_fragment(
            &raw_md,
            note.frontmatter.body_byte_offset,
            index,
            &base_url_path,
        );

        for tag in &note.frontmatter.tags {
            tags_map.entry(tag.clone()).or_default().push(note);
        }

        let page_html = render_full_site_page(
            note,
            &body_html,
            index,
            site_config,
            &base_url_path,
            &nav_items_html,
        );

        let dest_html = out_dir.join(format!("{}.html", note.slug));
        if let Some(parent) = dest_html.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest_html, page_html)?;
        summary.pages_built += 1;

        search_docs.push(serde_json::json!({
            "slug": note.slug,
            "title": note.frontmatter.title.as_deref().unwrap_or(&note.file_stem),
            "tags": note.frontmatter.tags,
            "url": format!("{}/{}.html", base_url_path, note.slug),
        }));

        // Generate alias redirect pages for clean_aliases (strictly excluding `"[]"`)
        for alias in &note.frontmatter.clean_aliases {
            let alias_slug = slugify_file_path(alias);
            if alias_slug.is_empty() || alias_slug == note.slug || alias == "[]" {
                continue;
            }
            let redirect_path = out_dir.join(format!("{}.html", alias_slug));
            if !redirect_path.exists() {
                if let Some(parent) = redirect_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let target_url = format!("{}/{}.html", base_url_path, note.slug);
                let redirect_html = format!(
                    r#"<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="utf-8" />
  <title>Redirecting to {title}</title>
  <link rel="canonical" href="{target_url}" />
  <meta http-equiv="refresh" content="0; url={target_url}" />
</head>
<body>
  <p>Redirecting to <a href="{target_url}">{title}</a>...</p>
</body>
</html>"#,
                    title = html_escape(&note.file_stem),
                    target_url = html_escape(&target_url),
                );
                fs::write(redirect_path, redirect_html)?;
                summary.alias_redirects_built += 1;
            }
        }
    }

    // 2. Build Tag pages (`/tags/<tag>.html` and `/tags/index.html`)
    let tags_dir = out_dir.join("tags");
    fs::create_dir_all(&tags_dir)?;
    let mut tags_index_body = String::from("<h1>Tags</h1><ul>");
    for (tag, tagged_notes) in &tags_map {
        let tag_slug = sluggify_segment(tag);
        tags_index_body.push_str(&format!(
            "<li><a href=\"{}/tags/{}.html\">#{}</a> ({} notes)</li>",
            base_url_path,
            html_escape(&tag_slug),
            html_escape(tag),
            tagged_notes.len()
        ));

        let mut tag_page_body = format!("<h1>Tag: #{}</h1><ul>", html_escape(tag));
        for n in tagged_notes {
            tag_page_body.push_str(&format!(
                "<li><a href=\"{}/{}.html\">{}</a></li>",
                base_url_path,
                html_escape(&n.slug),
                html_escape(&n.file_stem)
            ));
        }
        tag_page_body.push_str("</ul>");
        let tag_html = wrap_utility_page(
            &format!("Tag: #{}", tag),
            &tag_page_body,
            site_config,
            &base_url_path,
            &nav_items_html,
        );
        fs::write(tags_dir.join(format!("{}.html", tag_slug)), tag_html)?;
        summary.tag_pages_built += 1;
    }
    tags_index_body.push_str("</ul>");
    fs::write(
        tags_dir.join("index.html"),
        wrap_utility_page(
            "All Tags",
            &tags_index_body,
            site_config,
            &base_url_path,
            &nav_items_html,
        ),
    )?;

    // 3. Copy vault assets (respecting `publish_epubs = false`)
    for asset_rel in index.assets.keys() {
        if !site_config.publish_epubs && asset_rel.to_ascii_lowercase().ends_with(".epub") {
            continue;
        }
        let src = index.vault_root.join(asset_rel);
        let dst = out_dir.join(asset_rel);
        if src.exists() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)?;
            summary.assets_copied += 1;
        }
    }

    // 4. Write `404.html`, `sitemap.xml`, `index.xml` (RSS), `graph.json`, and `search-index.json`
    let not_found_html = wrap_utility_page(
        "404 — Page Not Found",
        "<h1>404 — Sayfa Bulunamadı</h1><p>Aradığınız not mevcut değil veya taşınmış olabilir.</p>",
        site_config,
        &base_url_path,
        &nav_items_html,
    );
    fs::write(out_dir.join("404.html"), not_found_html)?;

    let graph = index.build_graph(None, true, false, false, None);
    fs::write(
        out_dir.join("graph.json"),
        serde_json::to_string_pretty(&graph)?,
    )?;
    fs::write(
        out_dir.join("search-index.json"),
        serde_json::to_string_pretty(&search_docs)?,
    )?;

    let full_base = format!(
        "https://{}",
        site_config
            .base_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
    );
    let mut sitemap = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    let mut rss = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\"><channel><title>{}</title><link>{}</link><description>Nota Static Site</description>\n",
        html_escape(&site_config.title),
        html_escape(&full_base)
    );
    for note in index.notes.values() {
        let loc = format!("{}/{}.html", full_base, html_escape(&note.slug));
        sitemap.push_str(&format!("  <url><loc>{}</loc></url>\n", loc));
        rss.push_str(&format!(
            "  <item><title>{}</title><link>{}</link><guid>{}</guid></item>\n",
            html_escape(&note.file_stem),
            loc,
            loc
        ));
    }
    sitemap.push_str("</urlset>\n");
    rss.push_str(&format!(
        "<lastBuildDate>{}</lastBuildDate></channel></rss>\n",
        Utc::now().to_rfc2822()
    ));
    fs::write(out_dir.join("sitemap.xml"), sitemap)?;
    fs::write(out_dir.join("index.xml"), rss)?;

    Ok(summary)
}

fn render_full_site_page(
    note: &IndexedNote,
    body_html: &str,
    index: &VaultIndex,
    site_config: &SiteConfig,
    base_url_path: &str,
    nav_items_html: &str,
) -> String {
    let title = note
        .frontmatter
        .title
        .as_deref()
        .unwrap_or(&note.file_stem);

    let mut tags_html = String::new();
    if !note.frontmatter.tags.is_empty() {
        tags_html.push_str("<div class=\"nota-tags\">");
        for t in &note.frontmatter.tags {
            let ts = sluggify_segment(t);
            tags_html.push_str(&format!(
                "<a class=\"nota-tag\" href=\"{}/tags/{}.html\">#{}</a> ",
                base_url_path,
                html_escape(&ts),
                html_escape(t)
            ));
        }
        tags_html.push_str("</div>");
    }

    let mut toc_html = String::new();
    if !note.body.headings.is_empty() {
        toc_html.push_str("<div class=\"nota-panel\"><h3>Table of Contents</h3><ul>");
        for h in &note.body.headings {
            toc_html.push_str(&format!(
                "<li class=\"toc-h{}\"><a href=\"#{}\">{}</a></li>",
                h.level,
                html_escape(&h.anchor),
                html_escape(&h.text)
            ));
        }
        toc_html.push_str("</ul></div>");
    }

    let mut backlinks_html = String::new();
    if let Some(inc) = index.incoming_links.get(&note.rel_path) {
        if !inc.is_empty() {
            backlinks_html.push_str("<div class=\"nota-panel\"><h3>Backlinks</h3><ul>");
            for src in inc {
                if let Some(src_note) = index.notes.get(src) {
                    backlinks_html.push_str(&format!(
                        "<li><a href=\"{}/{}.html\">{}</a></li>",
                        base_url_path,
                        html_escape(&src_note.slug),
                        html_escape(&src_note.file_stem)
                    ));
                }
            }
            backlinks_html.push_str("</ul></div>");
        }
    }

    let related = index.related_notes(&note.rel_path, 6);
    let mut related_html = String::new();
    if !related.is_empty() {
        related_html.push_str("<div class=\"nota-panel\"><h3>Related Notes</h3><ul>");
        for (rel_path, _score, reason) in related {
            if let Some(rn) = index.notes.get(&rel_path) {
                related_html.push_str(&format!(
                    "<li><a href=\"{}/{}.html\">{}</a> <small>({})</small></li>",
                    base_url_path,
                    html_escape(&rn.slug),
                    html_escape(&rn.file_stem),
                    html_escape(&reason)
                ));
            }
        }
        related_html.push_str("</ul></div>");
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="tr" data-theme="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title} — {site_title}</title>
  <meta property="og:title" content="{title}" />
  <meta property="og:site_name" content="{site_title}" />
  <link rel="stylesheet" href="{base_url_path}/styles.css" />
</head>
<body>
  <header class="nota-topbar">
    <a class="nota-brand" href="{base_url_path}/index.html">{site_title}</a>
    <div class="nota-controls">
      <input id="nota-search" type="search" placeholder="Notlarda ara (Ctrl+K)..." oninput="filterNav(this.value)" />
      <button onclick="toggleTheme()">Dark / Light</button>
      <button onclick="document.body.classList.toggle('reader-mode')">Reader Mode</button>
    </div>
  </header>
  <div class="nota-layout">
    <nav class="nota-explorer">
      <h3>Explorer ({note_count})</h3>
      <ul id="nota-nav-list">
        {nav_items_html}
      </ul>
    </nav>
    <main class="nota-article" data-pagefind-body>
      <div class="nota-breadcrumbs"><a href="{base_url_path}/index.html">Home</a> › <span>{stem}</span></div>
      <h1>{title}</h1>
      {tags_html}
      <div class="nota-content">
        {body_html}
      </div>
    </main>
    <aside class="nota-sidebar">
      {toc_html}
      {backlinks_html}
      {related_html}
    </aside>
  </div>
  <script>
    function toggleTheme() {{
      const root = document.documentElement;
      root.dataset.theme = root.dataset.theme === 'light' ? 'dark' : 'light';
    }}
    function filterNav(q) {{
      const needle = q.toLocaleLowerCase('tr-TR');
      for (const li of document.querySelectorAll('#nota-nav-list li')) {{
        li.style.display = li.textContent.toLocaleLowerCase('tr-TR').includes(needle) ? '' : 'none';
      }}
    }}
  </script>
</body>
</html>"#,
        title = html_escape(title),
        site_title = html_escape(&site_config.title),
        base_url_path = base_url_path,
        note_count = index.notes.len(),
        nav_items_html = nav_items_html,
        stem = html_escape(&note.file_stem),
        tags_html = tags_html,
        body_html = body_html,
        toc_html = toc_html,
        backlinks_html = backlinks_html,
        related_html = related_html
    )
}

fn wrap_utility_page(
    title: &str,
    body_html: &str,
    site_config: &SiteConfig,
    base_url_path: &str,
    nav_items_html: &str,
) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="tr" data-theme="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title} — {site_title}</title>
  <link rel="stylesheet" href="{base_url_path}/styles.css" />
</head>
<body>
  <header class="nota-topbar">
    <a class="nota-brand" href="{base_url_path}/index.html">{site_title}</a>
  </header>
  <div class="nota-layout">
    <nav class="nota-explorer">
      <ul id="nota-nav-list">{nav_items_html}</ul>
    </nav>
    <main class="nota-article">{body_html}</main>
  </div>
</body>
</html>"#,
        title = html_escape(title),
        site_title = html_escape(&site_config.title),
        base_url_path = base_url_path,
        nav_items_html = nav_items_html,
        body_html = body_html
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn typst_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('$', "\\$")
        .replace('@', "\\@")
        .replace('<', "\\<")
        .replace('>', "\\>")
}

const NOTA_SITE_CSS: &str = r#"
:root, [data-theme="dark"] {
  --bg: #2e3440;
  --bg-alt: #3b4252;
  --fg: #eceff4;
  --fg-muted: #d8dee9;
  --accent: #8875cc;
  --link: #88c0d0;
  --border: #4c566a;
}
[data-theme="light"] {
  --bg: #eceff4;
  --bg-alt: #e5e9f0;
  --fg: #2e3440;
  --fg-muted: #4c566a;
  --accent: #8875cc;
  --link: #5e81ac;
  --border: #d8dee9;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font-family: system-ui, -apple-system, "Segoe UI", Roboto, "Noto Sans", sans-serif;
  line-height: 1.65;
}
.nota-topbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1.5rem;
  background: var(--bg-alt);
  border-bottom: 1px solid var(--border);
}
.nota-brand {
  color: var(--accent);
  font-weight: 700;
  text-decoration: none;
  font-size: 1.15rem;
}
.nota-controls { display: flex; gap: 0.5rem; }
.nota-controls input, .nota-controls button {
  background: var(--bg);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 0.35rem 0.65rem;
}
.nota-layout {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr) 280px;
  gap: 1.5rem;
  max-width: 1480px;
  margin: 0 auto;
  padding: 1.5rem;
}
body.reader-mode .nota-explorer, body.reader-mode .nota-sidebar { display: none; }
body.reader-mode .nota-layout { grid-template-columns: minmax(0, 820px); justify-content: center; }
.nota-explorer, .nota-sidebar {
  max-height: calc(100vh - 5rem);
  overflow-y: auto;
  padding-right: 0.5rem;
}
.nota-explorer ul, .nota-sidebar ul { list-style: none; padding-left: 0; }
.nota-explorer li, .nota-sidebar li { margin: 0.25rem 0; }
a, .nota-wikilink { color: var(--link); text-decoration: none; }
a:hover, .nota-wikilink:hover { text-decoration: underline; }
.nota-unresolved { color: #d08770; border-bottom: 1px dashed #d08770; }
.nota-epub-chip {
  display: inline-block;
  background: var(--bg-alt);
  border: 1px solid var(--accent);
  border-radius: 4px;
  padding: 0.1rem 0.45rem;
  font-size: 0.9em;
}
.nota-tag {
  display: inline-block;
  background: var(--bg-alt);
  color: var(--accent);
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.85rem;
  margin-right: 0.35rem;
}
table {
  border-collapse: collapse;
  margin: 1rem 0;
  width: auto;
}
th, td {
  border: 1px solid var(--border);
  padding: 0.4rem 0.75rem;
  text-align: center;
}
th { background: var(--bg-alt); }
mark { background: rgba(235, 203, 139, 0.35); color: var(--fg); padding: 0 0.2em; border-radius: 2px; }
.nota-truth-tree { margin: 1.2rem 0; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use nota_core::VaultConfig;

    #[test]
    fn test_logic_rendering_without_quartz_corruption() {
        let vault_index = VaultIndex {
            vault_root: PathBuf::from("content"),
            notes: BTreeMap::new(),
            assets: BTreeMap::new(),
            excalidraw_files: Vec::new(),
            canvas_files: Vec::new(),
            base_files: Vec::new(),
            template_files: Vec::new(),
            resolved_links: Vec::new(),
            incoming_links: BTreeMap::new(),
            unresolved_targets: BTreeMap::new(),
            exact_unresolved_targets: BTreeMap::new(),
            exact_incoming_links: BTreeMap::new(),
        };

        let sample = "A <-> B === ~A v B and [[Priest - Mantık.epub#p=42|Mantık, s. 42]] and ==important==\n\n```tree\nproblem: p -> q, p ∴ q\n```\n";
        let html = render_note_to_html_fragment(sample, 0, &vault_index, "/American_Studies");

        // Verify Quartz's `←>` and `\~A` corruptions never happen!
        assert!(!html.contains("←>"));
        assert!(!html.contains("\\~"));
        assert!(html.contains("&lt;-&gt;"));
        assert!(html.contains("==="));
        assert!(html.contains("<mark>important</mark>"));
        assert!(html.contains("nota-epub-chip"));
        assert!(html.contains("<svg"));
    }

    #[test]
    fn test_typst_export_preserves_tables_and_logic() {
        let _cfg = VaultConfig::default();
        let raw = "---\ntitle: Mantık Devreleri\n---\n# Koşul\n| p q | p → q |\n| --- | --- |\n| D Y | Y |\n";
        let fm = nota_core::parse_frontmatter(raw);
        let body = nota_core::parse_note_body(raw, fm.body_byte_offset, fm.body_start_line);
        let note = IndexedNote {
            rel_path: "Mantık Devreleri.md".into(),
            file_name: "Mantık Devreleri.md".into(),
            file_stem: "Mantık Devreleri".into(),
            slug: "Mantık-Devreleri".into(),
            is_moc: false,
            frontmatter: fm,
            body,
            raw_len_bytes: raw.len(),
        };
        let typst = export_note_typst(&note, raw);
        assert!(typst.contains("#table("));
        assert!(typst.contains("[*p → q*]"));
    }
}
