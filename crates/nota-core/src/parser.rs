use crate::slug::slug_anchor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingInfo {
    pub level: u8,
    pub text: String,
    pub anchor: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubLinkTarget {
    pub book_file: String,
    pub page: Option<usize>,
    pub chapter: Option<String>,
    pub cfi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiLink {
    pub raw: String,
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
    pub is_embed: bool,
    pub epub: Option<EpubLinkTarget>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLink {
    pub text: String,
    pub url: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    pub checked: bool,
    pub text: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FencedBlock {
    pub info: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightIssue {
    pub line: usize,
    pub snippet: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedNoteBody {
    pub headings: Vec<HeadingInfo>,
    pub wikilinks: Vec<WikiLink>,
    pub external_links: Vec<ExternalLink>,
    pub tables: Vec<MarkdownTable>,
    pub tasks: Vec<TaskItem>,
    pub fenced_blocks: Vec<FencedBlock>,
    pub footnote_refs: Vec<(String, usize)>,
    pub footnote_defs: Vec<(String, usize)>,
    pub has_blockquote: bool,
    pub highlight_count: usize,
    pub equivalence_triple_eq_count: usize,
    pub highlight_issues: Vec<HighlightIssue>,
    pub nbsp_count: usize,
    pub trailing_double_space_lines: usize,
}

pub fn parse_note_body(raw_markdown: &str, body_byte_offset: usize, body_start_line: usize) -> ParsedNoteBody {
    let mut parsed = ParsedNoteBody::default();

    // Count NBSP across full file and trailing double-spaces
    parsed.nbsp_count = raw_markdown.chars().filter(|&c| c == '\u{00a0}').count();
    for line in raw_markdown.lines() {
        let stripped = line.strip_suffix('\r').unwrap_or(line);
        if stripped.ends_with("  ") {
            parsed.trailing_double_space_lines += 1;
        }
    }

    let body = &raw_markdown[body_byte_offset..];
    let lines: Vec<&str> = body.lines().collect();

    let mut in_fence: Option<(String, char, usize, usize, Vec<String>)> = None;
    let mut i = 0;

    while i < lines.len() {
        let line_no = body_start_line + i;
        let line = lines[i];
        let trimmed_start = line.trim_start();

        // Handle fenced code blocks (``` or ~~~)
        if let Some((ref fence_info, fence_char, fence_len, fence_start, ref mut buf)) = in_fence {
            if let Some(close_len) = detect_fence_close(trimmed_start, fence_char) {
                if close_len >= fence_len {
                    parsed.fenced_blocks.push(FencedBlock {
                        info: fence_info.clone(),
                        content: buf.join("\n"),
                        start_line: fence_start,
                        end_line: line_no,
                    });
                    in_fence = None;
                    i += 1;
                    continue;
                }
            }
            buf.push(line.to_string());
            i += 1;
            continue;
        } else if let Some((fence_char, fence_len, info)) = detect_fence_open(trimmed_start) {
            in_fence = Some((info, fence_char, fence_len, line_no, Vec::new()));
            i += 1;
            continue;
        }

        // Check Markdown table start: header row + separator row (`| --- |`)
        if is_table_row(trimmed_start) && i + 1 < lines.len() && is_table_separator(lines[i + 1].trim()) {
            let start_line = line_no;
            let headers = split_table_cells(trimmed_start);
            // Parse wikilinks and highlights in header
            scan_inline_elements(line, line_no, &mut parsed);

            i += 2; // skip header & separator
            let mut rows = Vec::new();
            while i < lines.len() && is_table_row(lines[i].trim_start()) {
                let row_line_no = body_start_line + i;
                rows.push(split_table_cells(lines[i].trim_start()));
                scan_inline_elements(lines[i], row_line_no, &mut parsed);
                i += 1;
            }
            let end_line = body_start_line + i - 1;
            parsed.tables.push(MarkdownTable {
                headers,
                rows,
                start_line,
                end_line,
            });
            continue;
        }

        // Check ATX Headings (# .. ######)
        if let Some((level, heading_text)) = parse_atx_heading(trimmed_start) {
            parsed.headings.push(HeadingInfo {
                level,
                text: heading_text.clone(),
                anchor: slug_anchor(&heading_text),
                line: line_no,
            });
        }

        // Check Blockquotes
        if trimmed_start.starts_with('>') {
            parsed.has_blockquote = true;
        }

        // Check Tasks (`- [ ] `, `- [x] `, `* [ ] `, `1. [ ] `)
        if let Some((checked, task_text)) = parse_task_line(trimmed_start) {
            parsed.tasks.push(TaskItem {
                checked,
                text: task_text,
                line: line_no,
            });
        }

        // Check Footnote definition `[^id]:`
        if let Some(fn_id) = parse_footnote_definition(trimmed_start) {
            parsed.footnote_defs.push((fn_id, line_no));
        }

        scan_inline_elements(line, line_no, &mut parsed);
        i += 1;
    }

    parsed
}

fn detect_fence_open(trimmed_start: &str) -> Option<(char, usize, String)> {
    let first = trimmed_start.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = trimmed_start.chars().take_while(|&c| c == first).count();
    if count < 3 {
        return None;
    }
    let rest = &trimmed_start[count..];
    // In CommonMark, info strings for backtick fences cannot contain backticks
    if first == '`' && rest.contains('`') {
        return None;
    }
    Some((first, count, rest.trim().to_string()))
}

fn detect_fence_close(trimmed_start: &str, expected_char: char) -> Option<usize> {
    let count = trimmed_start
        .chars()
        .take_while(|&c| c == expected_char)
        .count();
    if count >= 3 && trimmed_start[count..].trim().is_empty() {
        Some(count)
    } else {
        None
    }
}

fn is_table_row(trimmed: &str) -> bool {
    trimmed.starts_with('|') && trimmed.len() > 1 && trimmed[1..].contains('|')
}

fn is_table_separator(trimmed: &str) -> bool {
    if !trimmed.starts_with('|') {
        return false;
    }
    let cells = split_table_cells(trimmed);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let t = c.trim().trim_matches(':');
            !t.is_empty() && t.chars().all(|ch| ch == '-')
        })
}

pub fn split_table_cells(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);

    // Split on `|` that is not inside `[[...]]` and not escaped `\|`
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut chars = inner.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(&next) = chars.peek() {
                if next == '|' {
                    chars.next();
                    current.push('|');
                    continue;
                }
            }
            current.push(ch);
        } else if ch == '[' && chars.peek() == Some(&'[') {
            chars.next();
            bracket_depth += 1;
            current.push_str("[[");
        } else if ch == ']' && chars.peek() == Some(&']') && bracket_depth > 0 {
            chars.next();
            bracket_depth -= 1;
            current.push_str("]]");
        } else if ch == '|' && bracket_depth == 0 {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

fn parse_atx_heading(trimmed_start: &str) -> Option<(u8, String)> {
    let hashes = trimmed_start.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) {
        let rest = &trimmed_start[hashes..];
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return Some((hashes as u8, rest.trim().to_string()));
        }
    }
    None
}

fn parse_task_line(trimmed_start: &str) -> Option<(bool, String)> {
    let after_marker = if let Some(r) = trimmed_start
        .strip_prefix("- ")
        .or_else(|| trimmed_start.strip_prefix("* "))
        .or_else(|| trimmed_start.strip_prefix("+ "))
    {
        r.trim_start()
    } else {
        let digits = trimmed_start
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits > 0 {
            trimmed_start[digits..]
                .strip_prefix(". ")?
                .trim_start()
        } else {
            return None;
        }
    };

    if let Some(rest) = after_marker.strip_prefix("[ ]") {
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return Some((false, rest.trim().to_string()));
        }
    } else if let Some(rest) = after_marker
        .strip_prefix("[x]")
        .or_else(|| after_marker.strip_prefix("[X]"))
    {
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return Some((true, rest.trim().to_string()));
        }
    }
    None
}

fn parse_footnote_definition(trimmed_start: &str) -> Option<String> {
    let rest = trimmed_start.strip_prefix("[^")?;
    let close = rest.find("]:")?;
    let id = &rest[..close];
    if !id.is_empty() && !id.contains(char::is_whitespace) {
        Some(id.to_string())
    } else {
        None
    }
}

fn scan_inline_elements(line: &str, line_no: usize, parsed: &mut ParsedNoteBody) {
    // Strip inline code spans (`...`) before scanning links/highlights/footnotes
    let stripped = mask_inline_code_and_comments(line);
    let bytes = stripped.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Check for wikilinks `[[...]]` or embeds `![[...]]`
        let is_embed = bytes[i] == b'!' && i + 2 < len && bytes[i + 1] == b'[' && bytes[i + 2] == b'[';
        let is_wikilink = bytes[i] == b'[' && i + 1 < len && bytes[i + 1] == b'[';

        if is_embed || is_wikilink {
            let open_pos = if is_embed { i + 3 } else { i + 2 };
            if let Some(rel_close) = stripped[open_pos..].find("]]") {
                let close_pos = open_pos + rel_close;
                let inner = &line[open_pos..close_pos];
                let raw_end = close_pos + 2;
                let raw = line[i..raw_end].to_string();

                if !inner.trim().is_empty() {
                    if let Some(wl) = parse_wikilink_inner(raw, inner, is_embed, line_no) {
                        parsed.wikilinks.push(wl);
                    }
                }
                i = raw_end;
                continue;
            }
        }

        // Check for footnote references `[^id]` (but not definitions `[^id]:` at line start)
        if bytes[i] == b'[' && i + 2 < len && bytes[i + 1] == b'^' {
            if let Some(rel_close) = stripped[i + 2..].find(']') {
                let close_pos = i + 2 + rel_close;
                let is_def = close_pos + 1 < len && bytes[close_pos + 1] == b':';
                let id = &stripped[i + 2..close_pos];
                if !is_def && !id.is_empty() && !id.contains(char::is_whitespace) {
                    parsed.footnote_refs.push((id.to_string(), line_no));
                    i = close_pos + 1;
                    continue;
                }
            }
        }

        // Check for standard external Markdown links `[text](http...)`
        if bytes[i] == b'[' && (i + 1 >= len || bytes[i + 1] != b'[') {
            if let Some(rel_bracket) = stripped[i + 1..].find("](") {
                let text_end = i + 1 + rel_bracket;
                let url_start = text_end + 2;
                if let Some(rel_paren) = stripped[url_start..].find(')') {
                    let url_end = url_start + rel_paren;
                    let url = stripped[url_start..url_end].trim();
                    if url.starts_with("http://") || url.starts_with("https://") {
                        parsed.external_links.push(ExternalLink {
                            text: line[i + 1..text_end].to_string(),
                            url: url.to_string(),
                            line: line_no,
                        });
                        i = url_end + 1;
                        continue;
                    }
                }
            }
        }

        i += 1;
    }

    // Scan `==` and `===` runs contextually on the line
    scan_equals_runs(&stripped, line, line_no, parsed);
}

/// Parses `target#heading|alias` and detects `.epub` links with `#p=42&ch=3&cfi=...`
pub fn parse_wikilink_inner(raw: String, inner: &str, is_embed: bool, line: usize) -> Option<WikiLink> {
    let (target_and_anchor, alias) = match inner.split_once('|') {
        Some((t, a)) => (t.trim(), Some(a.trim().to_string())),
        None => (inner.trim(), None),
    };

    let (target_raw, heading_raw) = match target_and_anchor.split_once('#') {
        Some((t, h)) => (t.trim(), Some(h.trim().to_string())),
        None => (target_and_anchor, None),
    };

    if target_raw.is_empty() && heading_raw.is_none() {
        return None;
    }

    let epub = if target_raw.to_ascii_lowercase().ends_with(".epub") {
        Some(parse_epub_link_target(target_raw, heading_raw.as_deref()))
    } else {
        None
    };

    Some(WikiLink {
        raw,
        target: target_raw.to_string(),
        heading: heading_raw,
        alias,
        is_embed,
        epub,
        line,
    })
}

pub fn parse_epub_link_target(book_file: &str, fragment: Option<&str>) -> EpubLinkTarget {
    let mut page = None;
    let mut chapter = None;
    let mut cfi = None;

    if let Some(frag) = fragment {
        for part in frag.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                match k.trim() {
                    "p" | "page" => {
                        page = v.trim().parse::<usize>().ok();
                    }
                    "ch" | "chapter" => {
                        chapter = Some(v.trim().to_string());
                    }
                    "cfi" => {
                        cfi = Some(v.trim().to_string());
                    }
                    _ => {}
                }
            } else if let Some(rest) = part.trim().strip_prefix("epubcfi(") {
                if let Some(inner) = rest.strip_suffix(')') {
                    cfi = Some(inner.to_string());
                }
            } else if let Ok(p) = part.trim().parse::<usize>() {
                page = Some(p);
            }
        }
    }

    EpubLinkTarget {
        book_file: book_file.to_string(),
        page,
        chapter,
        cfi,
    }
}

/// Disambiguates infix `===` (logical equivalence) from `==highlight==` and malformed `===highlight===`.
fn scan_equals_runs(stripped: &str, original_line: &str, line_no: usize, parsed: &mut ParsedNoteBody) {
    let bytes = stripped.as_bytes();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'=' {
                i += 1;
            }
            let count = i - start;
            if count >= 2 {
                runs.push((start, count));
            }
        } else {
            i += 1;
        }
    }

    if runs.is_empty() {
        return;
    }

    let mut idx = 0;
    while idx < runs.len() {
        let (start, len) = runs[idx];
        let end = start + len;

        // Check if this run can open a highlight paired with the next run
        if idx + 1 < runs.len() {
            let (next_start, next_len) = runs[idx + 1];
            let between = &stripped[end..next_start];
            let starts_without_space = !between.is_empty()
                && !between.starts_with(char::is_whitespace)
                && !between.ends_with(char::is_whitespace);

            if len == 2 && next_len == 2 && !between.trim().is_empty() {
                parsed.highlight_count += 1;
                idx += 2;
                continue;
            }

            if (len >= 3 || next_len >= 3) && !between.trim().is_empty() && (starts_without_space || len == 2) {
                // Asymmetric or triple-equals highlight in prose (e.g. `===Kennedy and Nixon===` or `==Attack ===`)
                parsed.highlight_count += 1;
                if len == 3 {
                    parsed.equivalence_triple_eq_count += 1;
                }
                if next_len == 3 {
                    parsed.equivalence_triple_eq_count += 1;
                }
                let snippet_end = (next_start + next_len).min(original_line.len());
                parsed.highlight_issues.push(HighlightIssue {
                    line: line_no,
                    snippet: original_line[start..snippet_end].to_string(),
                    description: format!(
                        "Non-standard highlight delimiter ({} opening '=' and {} closing '='); normalize to ==...==",
                        len, next_len
                    ),
                });
                idx += 2;
                continue;
            }
        }

        if len == 3 {
            parsed.equivalence_triple_eq_count += 1;
        }
        idx += 1;
    }
}

fn mask_inline_code_and_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut i = 0;
    let mut in_code = false;

    while i < chars.len() {
        let (byte_idx, ch) = chars[i];
        if !in_code && line[byte_idx..].starts_with("<!--") {
            if let Some(end_rel) = line[byte_idx + 4..].find("-->") {
                let comment_len = 4 + end_rel + 3;
                for _ in 0..comment_len {
                    out.push(' ');
                }
                while i < chars.len() && chars[i].0 < byte_idx + comment_len {
                    i += 1;
                }
                continue;
            }
        }
        if ch == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            // Preserve UTF-8 byte length with ASCII spaces so byte offsets match `line`
            for _ in 0..ch.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epub_wikilink_parsing() {
        let mut parsed = ParsedNoteBody::default();
        scan_inline_elements(
            "See [[Priest - Mantık.epub#p=42&ch=3&cfi=/6/8!/4/2|Mantık, s. 42]] for details.",
            10,
            &mut parsed,
        );
        assert_eq!(parsed.wikilinks.len(), 1);
        let wl = &parsed.wikilinks[0];
        assert_eq!(wl.target, "Priest - Mantık.epub");
        assert_eq!(wl.alias.as_deref(), Some("Mantık, s. 42"));
        let epub = wl.epub.as_ref().unwrap();
        assert_eq!(epub.page, Some(42));
        assert_eq!(epub.chapter.as_deref(), Some("3"));
        assert_eq!(epub.cfi.as_deref(), Some("/6/8!/4/2"));
    }

    #[test]
    fn test_equivalence_vs_highlight_disambiguation() {
        let mut logic_parsed = ParsedNoteBody::default();
        scan_inline_elements("- p -> q === ~p v q", 1, &mut logic_parsed);
        assert_eq!(logic_parsed.highlight_count, 0);
        assert_eq!(logic_parsed.equivalence_triple_eq_count, 1);
        assert!(logic_parsed.highlight_issues.is_empty());

        let mut prose_parsed = ParsedNoteBody::default();
        scan_inline_elements(
            "The debate between ===[[Kennedy]] and Nixon=== was decisive.",
            2,
            &mut prose_parsed,
        );
        assert_eq!(prose_parsed.highlight_count, 1);
        assert_eq!(prose_parsed.highlight_issues.len(), 1);
        assert_eq!(prose_parsed.wikilinks.len(), 1);
    }
}
