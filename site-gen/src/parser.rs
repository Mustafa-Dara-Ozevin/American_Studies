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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedNoteBody {
    pub headings: Vec<HeadingInfo>,
    pub wikilinks: Vec<WikiLink>,
}

pub fn parse_note_body(
    raw_markdown: &str,
    body_byte_offset: usize,
    body_start_line: usize,
) -> ParsedNoteBody {
    let mut parsed = ParsedNoteBody::default();
    let body = &raw_markdown[body_byte_offset.min(raw_markdown.len())..];
    let lines: Vec<&str> = body.lines().collect();

    let mut in_fence: Option<(char, usize)> = None;
    for (idx, line) in lines.iter().enumerate() {
        let line_no = body_start_line + idx;
        let trimmed_start = line.trim_start();

        if let Some((fence_char, fence_len)) = in_fence {
            if let Some(close_len) = detect_fence_close(trimmed_start, fence_char) {
                if close_len >= fence_len {
                    in_fence = None;
                }
            }
            continue;
        } else if let Some((fence_char, fence_len)) = detect_fence_open(trimmed_start) {
            in_fence = Some((fence_char, fence_len));
            continue;
        }

        if let Some((level, heading_text)) = parse_atx_heading(trimmed_start) {
            parsed.headings.push(HeadingInfo {
                level,
                text: heading_text.clone(),
                anchor: slug_anchor(&heading_text),
                line: line_no,
            });
        }

        scan_wikilinks(line, line_no, &mut parsed);
    }

    parsed
}

fn detect_fence_open(trimmed_start: &str) -> Option<(char, usize)> {
    let first = trimmed_start.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = trimmed_start.chars().take_while(|&c| c == first).count();
    if count < 3 {
        return None;
    }
    let rest = &trimmed_start[count..];
    if first == '`' && rest.contains('`') {
        return None;
    }
    Some((first, count))
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

fn scan_wikilinks(line: &str, line_no: usize, parsed: &mut ParsedNoteBody) {
    let stripped = mask_inline_code_and_comments(line);
    let bytes = stripped.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let is_embed =
            bytes[i] == b'!' && i + 2 < len && bytes[i + 1] == b'[' && bytes[i + 2] == b'[';
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
        i += 1;
    }
}

pub fn parse_wikilink_inner(
    raw: String,
    inner: &str,
    is_embed: bool,
    line: usize,
) -> Option<WikiLink> {
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
                    "p" | "page" => page = v.trim().parse::<usize>().ok(),
                    "ch" | "chapter" => chapter = Some(v.trim().to_string()),
                    "cfi" => cfi = Some(v.trim().to_string()),
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
