use crate::config::SiteConfig;
use crate::parser::parse_wikilink_inner;
use crate::slug::{
    normalize_lookup_key, slug_anchor, sluggify_segment, url_encode_path,
};
use crate::vault::{IndexedAsset, IndexedNote, VaultIndex};
use anyhow::{Context, Result};
use chrono::Utc;
use pulldown_cmark::{html, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteBuildSummary {
    pub out_dir: PathBuf,
    pub pages_built: usize,
    pub pdf_viewers_built: usize,
    pub tag_pages_built: usize,
    pub alias_redirects_built: usize,
    pub assets_copied: usize,
    pub unresolved_links: usize,
}

pub fn extract_base_url_path(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.starts_with('/') {
        return trimmed.trim_end_matches('/').to_string();
    }
    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    if let Some((_host, path)) = no_scheme.split_once('/') {
        let clean = path.trim_end_matches('/');
        if clean.is_empty() {
            String::new()
        } else {
            format!("/{}", clean)
        }
    } else {
        String::new()
    }
}

pub fn render_note_to_html_fragment(
    raw_markdown: &str,
    body_byte_offset: usize,
    index: &VaultIndex,
    base_url_path: &str,
) -> String {
    let body = &raw_markdown[body_byte_offset.min(raw_markdown.len())..];
    let preprocessed = preprocess_obsidian_markdown(body, index, base_url_path);
    render_markdown_with_heading_ids(&preprocessed)
}

fn render_markdown_with_heading_ids(markdown: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown, opts);
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

fn preprocess_obsidian_markdown(body: &str, index: &VaultIndex, base_url_path: &str) -> String {
    let mut out = String::with_capacity(body.len() + 512);
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0usize;
    let mut in_fence = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

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

        // Obsidian callout block: `> [!note] Optional Title`
        if let Some((callout_type, callout_title)) = parse_callout_header(trimmed) {
            let mut inner_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                let next_trimmed = lines[i].trim_start();
                if let Some(rest) = next_trimmed.strip_prefix('>') {
                    let content_line = rest.strip_prefix(' ').unwrap_or(rest);
                    inner_lines.push(preprocess_inline_syntax(content_line, index, base_url_path));
                    i += 1;
                } else {
                    break;
                }
            }
            let inner_md = inner_lines.join("\n");
            let inner_html = render_markdown_with_heading_ids(&inner_md);
            out.push_str(&format!(
                "\n<div class=\"nota-callout nota-callout-{}\">\n<div class=\"nota-callout-title\">{}</div>\n<div class=\"nota-callout-body\">{}</div>\n</div>\n\n",
                html_escape(&callout_type),
                html_escape(&callout_title),
                inner_html
            ));
            continue;
        }

        let processed_line = preprocess_inline_syntax(line, index, base_url_path);
        out.push_str(&processed_line);
        out.push('\n');
        i += 1;
    }

    out
}

fn parse_callout_header(trimmed: &str) -> Option<(String, String)> {
    let rest = trimmed.strip_prefix('>')?.trim_start();
    let after_open = rest.strip_prefix("[!")?;
    let close_idx = after_open.find(']')?;
    let raw_type = after_open[..close_idx].trim();
    if raw_type.is_empty() || raw_type.contains(char::is_whitespace) {
        return None;
    }
    let callout_type = raw_type.to_ascii_lowercase();
    let after_close = after_open[close_idx + 1..].trim();
    let title = if after_close.is_empty() {
        let mut chars = callout_type.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
            None => "Note".to_string(),
        }
    } else {
        after_close.to_string()
    };
    Some((callout_type, title))
}

fn url_prefix(base_url_path: &str) -> String {
    if base_url_path.is_empty() {
        "/".to_string()
    } else {
        format!("{}/", base_url_path.trim_end_matches('/'))
    }
}

fn preprocess_inline_syntax(line: &str, index: &VaultIndex, base_url_path: &str) -> String {
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

    let prefix = url_prefix(base_url_path);
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

        if bytes[i] == b'~'
            && (i + 1 >= bytes.len() || bytes[i + 1] != b'~')
            && (i == 0 || bytes[i - 1] != b'~')
        {
            out.push_str("&#126;");
            i += 1;
            continue;
        }

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

        let is_embed =
            bytes[i] == b'!' && i + 2 < bytes.len() && bytes[i + 1] == b'[' && bytes[i + 2] == b'[';
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
                        if let Some(asset_rel) = index.asset_lookup.get(&key) {
                            let asset = &index.assets[asset_rel];
                            let encoded_asset = url_encode_path(&asset.public_path);
                            if asset.is_pdf {
                                let viewer = asset
                                    .viewer_slug
                                    .as_deref()
                                    .map(url_encode_path)
                                    .unwrap_or_else(|| encoded_asset.clone());
                                out.push_str(&format!(
                                    "<div class=\"nota-pdf-embed\"><div class=\"nota-pdf-embed-bar\"><span class=\"nota-pdf-title\">📄 {}</span><div class=\"nota-pdf-actions\"><a href=\"{}{}.html\">Viewer</a><a href=\"{}{}\" target=\"_blank\" rel=\"noopener\">Open PDF ↗</a></div></div><iframe src=\"{}{}#view=FitH\" title=\"{}\" loading=\"lazy\"></iframe></div>",
                                    html_escape(&display),
                                    prefix,
                                    html_escape(&viewer),
                                    prefix,
                                    html_escape(&encoded_asset),
                                    prefix,
                                    html_escape(&encoded_asset),
                                    html_escape(&display),
                                ));
                            } else {
                                out.push_str(&format!(
                                    "<img class=\"nota-embed\" src=\"{}{}\" alt=\"{}\" loading=\"lazy\" />",
                                    prefix,
                                    html_escape(&encoded_asset),
                                    html_escape(&display)
                                ));
                            }
                        } else {
                            out.push_str(&format!(
                                "<span class=\"nota-unresolved\" title=\"Unresolved embed: {}\">{}</span>",
                                html_escape(&target),
                                html_escape(&display)
                            ));
                        }
                    } else if target.is_empty() {
                        let anchor = subpath.as_deref().map(slug_anchor).unwrap_or_default();
                        out.push_str(&format!(
                            "<a class=\"nota-wikilink\" href=\"#{}\">{}</a>",
                            html_escape(&anchor),
                            html_escape(&display)
                        ));
                    } else {
                        let key = normalize_lookup_key(&target);
                        if let Some(note_rel) = index.note_lookup.get(&key) {
                            let note = &index.notes[note_rel];
                            let encoded_slug = url_encode_path(&note.slug);
                            let anchor_suffix = subpath
                                .as_deref()
                                .map(|s| format!("#{}", slug_anchor(s)))
                                .unwrap_or_default();
                            out.push_str(&format!(
                                "<a class=\"nota-wikilink\" href=\"{}{}.html{}\">{}</a>",
                                prefix,
                                html_escape(&encoded_slug),
                                html_escape(&anchor_suffix),
                                html_escape(&display)
                            ));
                        } else if let Some(asset_rel) = index.asset_lookup.get(&key) {
                            let asset = &index.assets[asset_rel];
                            if asset.is_pdf {
                                let viewer = asset
                                    .viewer_slug
                                    .as_deref()
                                    .map(url_encode_path)
                                    .unwrap_or_else(|| url_encode_path(&asset.public_path));
                                out.push_str(&format!(
                                    "<a class=\"nota-wikilink nota-pdf-link\" href=\"{}{}.html\">📄 {}</a>",
                                    prefix,
                                    html_escape(&viewer),
                                    html_escape(&display)
                                ));
                            } else {
                                let encoded_asset = url_encode_path(&asset.public_path);
                                out.push_str(&format!(
                                    "<a class=\"nota-wikilink\" href=\"{}{}\">{}</a>",
                                    prefix,
                                    html_escape(&encoded_asset),
                                    html_escape(&display)
                                ));
                            }
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

        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"==" {
            let mut eq_len = 2;
            while i + eq_len < bytes.len() && bytes[i + eq_len] == b'=' {
                eq_len += 1;
            }
            let prev_space = i == 0 || bytes[i - 1].is_ascii_whitespace();
            let next_space =
                i + eq_len >= bytes.len() || bytes[i + eq_len].is_ascii_whitespace();

            if eq_len == 3 && prev_space && next_space {
                out.push_str("===");
                i += 3;
                continue;
            } else if !next_space {
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

pub fn build_static_site(
    index: &VaultIndex,
    site_config: &SiteConfig,
    out_dir: &Path,
) -> Result<SiteBuildSummary> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)
            .with_context(|| format!("Failed to clean output directory {}", out_dir.display()))?;
    }
    fs::create_dir_all(out_dir)
        .with_context(|| format!("Failed to create output directory {}", out_dir.display()))?;

    let base_url_path = extract_base_url_path(&site_config.base_url);
    let prefix = url_prefix(&base_url_path);
    let mut created_dirs: HashSet<PathBuf> = HashSet::new();
    created_dirs.insert(out_dir.to_path_buf());
    let mut ensure_dir = |dir: &Path| -> Result<()> {
        if created_dirs.insert(dir.to_path_buf()) {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    };

    let mut summary = SiteBuildSummary {
        out_dir: out_dir.to_path_buf(),
        unresolved_links: index.unresolved_count,
        ..Default::default()
    };

    fs::write(out_dir.join("styles.css"), NOTA_SITE_CSS)?;

    // Build Explorer sidebar HTML: Notes + Typst/PDFs
    let mut nav_items_html = String::new();
    for note in index.notes.values() {
        let badge = if note.is_moc {
            " <small>[MOC]</small>"
        } else {
            ""
        };
        let encoded_slug = url_encode_path(&note.slug);
        nav_items_html.push_str(&format!(
            "<li><a href=\"{}{}.html\">{}{}</a></li>\n",
            prefix,
            html_escape(&encoded_slug),
            html_escape(&note.file_stem),
            badge
        ));
    }

    let pdf_assets: Vec<&IndexedAsset> = index.assets.values().filter(|a| a.is_pdf).collect();
    let mut pdf_nav_html = String::new();
    if !pdf_assets.is_empty() {
        pdf_nav_html.push_str(&format!(
            "<h3>Typst / PDFs ({})</h3><ul class=\"nota-pdf-nav\">",
            pdf_assets.len()
        ));
        for asset in &pdf_assets {
            if let Some(ref vs) = asset.viewer_slug {
                let encoded_vs = url_encode_path(vs);
                pdf_nav_html.push_str(&format!(
                    "<li><a href=\"{}{}.html\">📄 {}</a></li>\n",
                    prefix,
                    html_escape(&encoded_vs),
                    html_escape(&asset.file_stem)
                ));
            }
        }
        pdf_nav_html.push_str("</ul>");
    }

    let mut tags_map: BTreeMap<String, Vec<&IndexedNote>> = BTreeMap::new();
    let mut search_docs = Vec::new();

    // 1. Render all note pages + alias redirect pages
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
            &pdf_nav_html,
        );

        let dest_html = out_dir.join(format!("{}.html", note.slug));
        if let Some(parent) = dest_html.parent() {
            ensure_dir(parent)?;
        }
        fs::write(&dest_html, page_html)?;
        summary.pages_built += 1;

        search_docs.push(serde_json::json!({
            "slug": note.slug,
            "title": note.frontmatter.title.as_deref().unwrap_or(&note.file_stem),
            "tags": note.frontmatter.tags,
            "type": "note",
            "url": format!("{}{}.html", prefix, url_encode_path(&note.slug)),
        }));

        for alias in &note.frontmatter.clean_aliases {
            let alias_slug = crate::slug::slugify_file_path(alias);
            if alias_slug.is_empty() || alias_slug == note.slug || alias == "[]" {
                continue;
            }
            let redirect_path = out_dir.join(format!("{}.html", alias_slug));
            if !redirect_path.exists() {
                if let Some(parent) = redirect_path.parent() {
                    ensure_dir(parent)?;
                }
                let target_url = format!("{}{}.html", prefix, url_encode_path(&note.slug));
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

    // 2. Copy vault assets & generate PDF viewer pages
    for asset in index.assets.values() {
        let src = index.vault_root.join(&asset.rel_path);
        let dst = out_dir.join(&asset.public_path);
        if src.exists() {
            if let Some(parent) = dst.parent() {
                ensure_dir(parent)?;
            }
            fs::copy(&src, &dst)?;
            summary.assets_copied += 1;
        }

        if asset.is_pdf {
            if let Some(ref viewer_slug) = asset.viewer_slug {
                let viewer_html = render_pdf_viewer_page(
                    asset,
                    index,
                    site_config,
                    &base_url_path,
                    &nav_items_html,
                    &pdf_nav_html,
                );
                let viewer_dest = out_dir.join(format!("{}.html", viewer_slug));
                if let Some(parent) = viewer_dest.parent() {
                    ensure_dir(parent)?;
                }
                fs::write(viewer_dest, viewer_html)?;
                summary.pdf_viewers_built += 1;

                search_docs.push(serde_json::json!({
                    "slug": viewer_slug,
                    "title": asset.file_stem,
                    "tags": ["pdf", "typst"],
                    "type": "pdf",
                    "url": format!("{}{}.html", prefix, url_encode_path(viewer_slug)),
                }));
            }
        }
    }

    // 3. Build Tag pages (`/tags/<tag>.html` and `/tags/index.html`)
    let tags_dir = out_dir.join("tags");
    fs::create_dir_all(&tags_dir)?;
    let mut tags_index_body = String::from("<h1>Tags</h1><ul>");
    for (tag, tagged_notes) in &tags_map {
        let tag_slug = sluggify_segment(tag);
        let encoded_tag_slug = url_encode_path(&tag_slug);
        tags_index_body.push_str(&format!(
            "<li><a href=\"{}tags/{}.html\">#{}</a> ({} notes)</li>",
            prefix,
            html_escape(&encoded_tag_slug),
            html_escape(tag),
            tagged_notes.len()
        ));

        let mut tag_page_body = format!("<h1>Tag: #{}</h1><ul>", html_escape(tag));
        for n in tagged_notes {
            tag_page_body.push_str(&format!(
                "<li><a href=\"{}{}.html\">{}</a></li>",
                prefix,
                html_escape(&url_encode_path(&n.slug)),
                html_escape(&n.file_stem)
            ));
        }
        tag_page_body.push_str("</ul>");
        let tag_html = wrap_utility_page(
            &format!("Tag: #{}", tag),
            &tag_page_body,
            site_config,
            &base_url_path,
            index.notes.len(),
            &nav_items_html,
            &pdf_nav_html,
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
            index.notes.len(),
            &nav_items_html,
            &pdf_nav_html,
        ),
    )?;

    // 4. Fallback `index.html` if vault has no `index.md`
    let index_html_path = out_dir.join("index.html");
    if !index_html_path.exists() {
        let mut home_body = format!(
            "<h1>{}</h1><p>{} notes indexed. Browse via the Explorer sidebar or <a href=\"{}tags/index.html\">All Tags</a>.</p><ul>",
            html_escape(&site_config.title),
            index.notes.len(),
            prefix
        );
        for note in index.notes.values().filter(|n| n.is_moc).take(20) {
            home_body.push_str(&format!(
                "<li><a href=\"{}{}.html\">{}</a> <small>[MOC]</small></li>",
                prefix,
                html_escape(&url_encode_path(&note.slug)),
                html_escape(&note.file_stem)
            ));
        }
        home_body.push_str("</ul>");
        fs::write(
            &index_html_path,
            wrap_utility_page(
                &site_config.title,
                &home_body,
                site_config,
                &base_url_path,
                index.notes.len(),
                &nav_items_html,
                &pdf_nav_html,
            ),
        )?;
    }

    // 5. Write `404.html`, `graph.json`, `search-index.json`, `sitemap.xml`, `index.xml`
    let not_found_html = wrap_utility_page(
        "404 — Page Not Found",
        "<h1>404 — Sayfa Bulunamadı</h1><p>Aradığınız not mevcut değil veya taşınmış olabilir.</p>",
        site_config,
        &base_url_path,
        index.notes.len(),
        &nav_items_html,
        &pdf_nav_html,
    );
    fs::write(out_dir.join("404.html"), not_found_html)?;

    let graph = index.build_graph_json();
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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\"><channel><title>{}</title><link>{}</link><description>Obsidian + Typst Static Site</description>\n",
        html_escape(&site_config.title),
        html_escape(&full_base)
    );
    for note in index.notes.values() {
        let loc = format!("{}/{}.html", full_base, html_escape(&url_encode_path(&note.slug)));
        sitemap.push_str(&format!("  <url><loc>{}</loc></url>\n", loc));
        rss.push_str(&format!(
            "  <item><title>{}</title><link>{}</link><guid>{}</guid></item>\n",
            html_escape(&note.file_stem),
            loc,
            loc
        ));
    }
    for asset in &pdf_assets {
        if let Some(ref vs) = asset.viewer_slug {
            let loc = format!("{}/{}.html", full_base, html_escape(&url_encode_path(vs)));
            sitemap.push_str(&format!("  <url><loc>{}</loc></url>\n", loc));
        }
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

fn render_backlinks_panel(rel_path: &str, index: &VaultIndex, prefix: &str) -> String {
    let mut backlinks_html = String::new();
    if let Some(inc) = index.incoming_links.get(rel_path) {
        if !inc.is_empty() {
            backlinks_html.push_str("<div class=\"nota-panel\"><h3>Backlinks</h3><ul>");
            for src in inc {
                if let Some(src_note) = index.notes.get(src) {
                    backlinks_html.push_str(&format!(
                        "<li><a href=\"{}{}.html\">{}</a></li>",
                        prefix,
                        html_escape(&url_encode_path(&src_note.slug)),
                        html_escape(&src_note.file_stem)
                    ));
                }
            }
            backlinks_html.push_str("</ul></div>");
        }
    }
    backlinks_html
}

fn render_full_site_page(
    note: &IndexedNote,
    body_html: &str,
    index: &VaultIndex,
    site_config: &SiteConfig,
    base_url_path: &str,
    nav_items_html: &str,
    pdf_nav_html: &str,
) -> String {
    let prefix = url_prefix(base_url_path);
    let title = note.frontmatter.title.as_deref().unwrap_or(&note.file_stem);

    let mut tags_html = String::new();
    if !note.frontmatter.tags.is_empty() {
        tags_html.push_str("<div class=\"nota-tags\">");
        for t in &note.frontmatter.tags {
            let ts = sluggify_segment(t);
            tags_html.push_str(&format!(
                "<a class=\"nota-tag\" href=\"{}tags/{}.html\">#{}</a> ",
                prefix,
                html_escape(&url_encode_path(&ts)),
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

    let backlinks_html = render_backlinks_panel(&note.rel_path, index, &prefix);

    let related = index.related_notes(&note.rel_path, 6);
    let mut related_html = String::new();
    if !related.is_empty() {
        related_html.push_str("<div class=\"nota-panel\"><h3>Related Notes</h3><ul>");
        for (rel_path, reason) in related {
            if let Some(rn) = index.notes.get(&rel_path) {
                related_html.push_str(&format!(
                    "<li><a href=\"{}{}.html\">{}</a> <small>({})</small></li>",
                    prefix,
                    html_escape(&url_encode_path(&rn.slug)),
                    html_escape(&rn.file_stem),
                    html_escape(&reason)
                ));
            }
        }
        related_html.push_str("</ul></div>");
    }

    render_shell(
        title,
        &note.file_stem,
        &format!("{tags_html}<div class=\"nota-content\">{body_html}</div>"),
        &format!("{toc_html}{backlinks_html}{related_html}"),
        site_config,
        base_url_path,
        index.notes.len(),
        nav_items_html,
        pdf_nav_html,
    )
}

fn render_pdf_viewer_page(
    asset: &IndexedAsset,
    index: &VaultIndex,
    site_config: &SiteConfig,
    base_url_path: &str,
    nav_items_html: &str,
    pdf_nav_html: &str,
) -> String {
    let prefix = url_prefix(base_url_path);
    let encoded_pdf = url_encode_path(&asset.public_path);
    let pdf_url = format!("{}{}", prefix, html_escape(&encoded_pdf));
    let body_html = format!(
        r#"<div class="nota-pdf-viewer">
  <div class="nota-pdf-toolbar">
    <span class="nota-pdf-path">📄 {public_path}</span>
    <div class="nota-pdf-actions">
      <a href="{pdf_url}" target="_blank" rel="noopener">Open Raw PDF ↗</a>
      <a href="{pdf_url}" download="{file_name}">Download PDF</a>
    </div>
  </div>
  <iframe class="nota-pdf-frame" src="{pdf_url}#view=FitH" title="{file_stem}"></iframe>
</div>"#,
        public_path = html_escape(&asset.public_path),
        pdf_url = pdf_url,
        file_name = html_escape(&asset.file_name),
        file_stem = html_escape(&asset.file_stem),
    );
    let backlinks_html = render_backlinks_panel(&asset.rel_path, index, &prefix);

    render_shell(
        &asset.file_stem,
        &asset.file_name,
        &body_html,
        &backlinks_html,
        site_config,
        base_url_path,
        index.notes.len(),
        nav_items_html,
        pdf_nav_html,
    )
}

fn wrap_utility_page(
    title: &str,
    body_html: &str,
    site_config: &SiteConfig,
    base_url_path: &str,
    note_count: usize,
    nav_items_html: &str,
    pdf_nav_html: &str,
) -> String {
    render_shell(
        title,
        title,
        body_html,
        "",
        site_config,
        base_url_path,
        note_count,
        nav_items_html,
        pdf_nav_html,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_shell(
    title: &str,
    breadcrumb: &str,
    article_body_html: &str,
    sidebar_html: &str,
    site_config: &SiteConfig,
    base_url_path: &str,
    note_count: usize,
    nav_items_html: &str,
    pdf_nav_html: &str,
) -> String {
    let prefix = url_prefix(base_url_path);
    format!(
        r#"<!DOCTYPE html>
<html lang="tr" data-theme="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title} — {site_title}</title>
  <meta property="og:title" content="{title}" />
  <meta property="og:site_name" content="{site_title}" />
  <link rel="stylesheet" href="{prefix}styles.css" />
</head>
<body>
  <header class="nota-topbar">
    <div class="nota-topbar-left">
      <button id="nota-menu-btn" class="nota-menu-btn" aria-expanded="false" aria-controls="nota-explorer" onclick="toggleNav()">☰ Notes</button>
      <a class="nota-brand" href="{prefix}index.html">{site_title}</a>
    </div>
    <div class="nota-controls">
      <input id="nota-search" type="search" placeholder="Notlarda ara..." oninput="filterNav(this.value)" />
      <button onclick="toggleTheme()">Dark / Light</button>
      <button onclick="document.body.classList.toggle('reader-mode')">Reader Mode</button>
    </div>
  </header>
  <div class="nota-layout">
    <nav id="nota-explorer" class="nota-explorer">
      <h3>Notes ({note_count})</h3>
      <ul id="nota-nav-list">
        {nav_items_html}
      </ul>
      {pdf_nav_html}
    </nav>
    <main class="nota-article" data-pagefind-body>
      <div class="nota-breadcrumbs"><a href="{prefix}index.html">Home</a> › <span>{breadcrumb}</span></div>
      <h1>{title}</h1>
      {article_body_html}
    </main>
    <aside class="nota-sidebar">
      {sidebar_html}
    </aside>
  </div>
  <script>
    function toggleTheme() {{
      const root = document.documentElement;
      root.dataset.theme = root.dataset.theme === 'light' ? 'dark' : 'light';
    }}
    function toggleNav(forceOpen) {{
      const open = typeof forceOpen === 'boolean'
        ? document.body.classList.toggle('nav-open', forceOpen)
        : document.body.classList.toggle('nav-open');
      const btn = document.getElementById('nota-menu-btn');
      if (btn) btn.setAttribute('aria-expanded', open ? 'true' : 'false');
    }}
    function filterNav(q) {{
      const needle = q.toLocaleLowerCase('tr-TR');
      if (needle.trim().length > 0 && window.innerWidth <= 1024) {{
        toggleNav(true);
      }}
      for (const li of document.querySelectorAll('.nota-explorer li')) {{
        li.style.display = li.textContent.toLocaleLowerCase('tr-TR').includes(needle) ? '' : 'none';
      }}
    }}
  </script>
</body>
</html>"#,
        title = html_escape(title),
        site_title = html_escape(&site_config.title),
        prefix = prefix,
        note_count = note_count,
        nav_items_html = nav_items_html,
        pdf_nav_html = pdf_nav_html,
        breadcrumb = html_escape(breadcrumb),
        article_body_html = article_body_html,
        sidebar_html = sidebar_html
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
  flex-wrap: wrap;
  gap: 0.75rem;
  padding: 0.75rem 1.5rem;
  background: var(--bg-alt);
  border-bottom: 1px solid var(--border);
}
.nota-topbar-left {
  display: flex;
  align-items: center;
  gap: 0.65rem;
}
.nota-brand {
  color: var(--accent);
  font-weight: 700;
  text-decoration: none;
  font-size: 1.15rem;
}
.nota-controls { display: flex; gap: 0.5rem; flex-wrap: wrap; }
.nota-controls input, .nota-controls button, .nota-menu-btn {
  background: var(--bg);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 0.35rem 0.65rem;
  cursor: pointer;
}
.nota-menu-btn { display: none; }
.nota-layout {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr) 260px;
  gap: 1.5rem;
  max-width: 1480px;
  margin: 0 auto;
  padding: 1.5rem;
}
.nota-article { min-width: 0; overflow-wrap: break-word; }
pre { overflow-x: auto; max-width: 100%; }
body.reader-mode .nota-explorer, body.reader-mode .nota-sidebar { display: none; }
body.reader-mode .nota-layout { grid-template-columns: minmax(0, 860px); justify-content: center; }
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
.nota-callout {
  margin: 1rem 0;
  padding: 0.75rem 1rem;
  border-left: 4px solid var(--link);
  background: var(--bg-alt);
  border-radius: 0 6px 6px 0;
}
.nota-callout-title {
  font-weight: 700;
  margin-bottom: 0.35rem;
  color: var(--link);
}
.nota-callout-warning, .nota-callout-caution { border-left-color: #ebcb8b; }
.nota-callout-warning .nota-callout-title, .nota-callout-caution .nota-callout-title { color: #ebcb8b; }
.nota-callout-tip { border-left-color: #a3be8c; }
.nota-callout-tip .nota-callout-title { color: #a3be8c; }
.nota-pdf-embed, .nota-pdf-viewer {
  margin: 1.25rem 0;
  border: 1px solid var(--border);
  border-radius: 6px;
  overflow: hidden;
  background: var(--bg-alt);
}
.nota-pdf-embed-bar, .nota-pdf-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
  padding: 0.5rem 0.85rem;
  border-bottom: 1px solid var(--border);
  font-size: 0.9rem;
}
.nota-pdf-actions { display: flex; gap: 0.75rem; }
.nota-pdf-actions a {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 0.2rem 0.6rem;
  font-size: 0.85rem;
}
.nota-pdf-embed iframe {
  width: 100%;
  height: 680px;
  border: none;
  display: block;
  background: #fff;
}
.nota-pdf-frame {
  width: 100%;
  height: 82vh;
  min-height: 640px;
  border: none;
  display: block;
  background: #fff;
}
.nota-embed { max-width: 100%; height: auto; border-radius: 4px; }
table { display: block; overflow-x: auto; max-width: 100%; border-collapse: collapse; margin: 1rem 0; width: auto; }
th, td { border: 1px solid var(--border); padding: 0.4rem 0.75rem; text-align: center; }
th { background: var(--bg-alt); }
strong { color: #e0af68; }
em { color: #bb9af7; }
del, s { color: #f7768e; text-decoration-color: #f7768e; }
code { color: #9ece6a; background: var(--bg-alt); padding: 0.1em 0.35em; border-radius: 3px; }
mark { background: rgba(224, 175, 104, 0.28); color: #fff3c4; padding: 0 0.25em; border-radius: 3px; }
@media (max-width: 1024px) {
  .nota-topbar { padding: 0.6rem 1rem; gap: 0.5rem; }
  .nota-menu-btn { display: inline-flex; align-items: center; gap: 0.35rem; }
  .nota-controls { width: 100%; }
  .nota-controls input { flex: 1 1 140px; min-width: 0; }
  .nota-layout {
    grid-template-columns: minmax(0, 1fr);
    padding: 1rem;
    gap: 1rem;
  }
  .nota-explorer {
    display: none;
    max-height: 60vh;
    overflow-y: auto;
    background: var(--bg-alt);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.75rem 1rem;
  }
  body.nav-open .nota-explorer { display: block; }
  .nota-sidebar {
    max-height: none;
    border-top: 1px solid var(--border);
    padding-top: 1rem;
  }
  .nota-pdf-embed iframe { height: 65vh; min-height: 380px; }
  .nota-pdf-frame { height: 70vh; min-height: 440px; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VaultConfig;

    #[test]
    fn test_logic_callout_and_pdf_rendering() {
        let tmp = std::env::temp_dir().join(format!("site-gen-render-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("Math/Pumping Lemma")).unwrap();
        fs::write(
            tmp.join("Math/Pumping Lemma/exercise.pdf"),
            b"%PDF-1.4 mock",
        )
        .unwrap();
        fs::write(
            tmp.join("MATH_MOC.md"),
            "> [!note] Workflow\n> Typst PDFs.\n\nA <-> B === ~A v B and ==important==\n\n- [[exercise.pdf|Pumping Lemma PDF]]\n- ![[exercise.pdf]]\n",
        )
        .unwrap();

        let index = VaultIndex::scan(&tmp, &VaultConfig::default()).unwrap();
        let raw = fs::read_to_string(tmp.join("MATH_MOC.md")).unwrap();
        let html = render_note_to_html_fragment(&raw, 0, &index, "/American_Studies");

        assert_eq!(url_prefix(""), "/");
        assert_eq!(url_prefix("/American_Studies"), "/American_Studies/");
        assert!(!html.contains("←>"));
        assert!(!html.contains("\\~"));
        assert!(html.contains("&lt;-&gt;"));
        assert!(html.contains("==="));
        assert!(html.contains("<mark>important</mark>"));
        assert!(html.contains("nota-callout-note"));
        assert!(html.contains("Workflow"));
        assert!(html.contains("/American_Studies/Math/Pumping-Lemma/exercise.pdf.html"));
        assert!(html.contains("<iframe src=\"/American_Studies/Math/Pumping%20Lemma/exercise.pdf#view=FitH\""));
        assert!(!html.contains("nota-unresolved"));

        let out_dir = tmp.join("out");
        let cfg = SiteConfig::default();
        let summary = build_static_site(&index, &cfg, &out_dir).unwrap();
        assert_eq!(summary.pages_built, 1);
        assert_eq!(summary.pdf_viewers_built, 1);
        assert!(out_dir
            .join("Math/Pumping-Lemma/exercise.pdf.html")
            .exists());
        assert!(out_dir
            .join("Math/Pumping Lemma/exercise.pdf")
            .exists());
        assert!(out_dir.join("index.html").exists());
        let math_moc_html = fs::read_to_string(out_dir.join("MATH_MOC.html")).unwrap();
        assert!(math_moc_html.contains("id=\"nota-menu-btn\""));
        assert!(math_moc_html.contains("toggleNav("));
        let css = fs::read_to_string(out_dir.join("styles.css")).unwrap();
        assert!(css.contains("body.nav-open .nota-explorer"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
