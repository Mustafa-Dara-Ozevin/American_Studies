//! `nota-editor`: Pure Rust UTF-8 `TextBuffer` and `NotaEditorTab` state controller
//! with live 61-rule Typing Transformer range replacement, Undo/Redo, `[[` wikilink autocomplete,
//! template expansion, atomic saves, and vault-wide link updates on rename.
//! Designed for zero-overhead synchronization with GPUI (`gpui-component` `TextareaState` / `EditorState`).

use anyhow::{Context, Result};
use chrono::Local;
use nota_core::{normalize_lookup_key, TypingRuleEngine};
use nota_logic::{Formula, TruthTree};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Line and byte-column position within a note buffer (0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Cursor and optional selection anchor inside [`TextBuffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cursor {
    pub position: Position,
    pub selection: Option<Position>,
}

/// Primitive edit operation on [`TextBuffer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Insert(char),
    Paste(Arc<String>),
    Backspace,
    Delete,
}

/// Action dispatched to [`NotaEditorTab::apply_editor_action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Edit(Edit),
    Move(Position),
    SyncText {
        text: String,
        cursor: Position,
        is_char_insert: bool,
    },
}

/// Converts raw HTML `<table>...</table>` blocks (`<thead>`, `<tbody>`, `<tr>`, `<th>`, `<td>`)
/// outside of fenced code blocks into aligned Markdown pipe tables.
/// Returns `(converted_markdown, tables_converted_count)`.
pub fn convert_html_tables_in_markdown(raw: &str) -> (String, usize) {
    if !raw.to_ascii_lowercase().contains("</table>") {
        return (raw.to_string(), 0);
    }
    let mut out = String::with_capacity(raw.len());
    let mut count = 0usize;
    let mut in_fence = false;
    let mut i = 0usize;
    let lines: Vec<&str> = raw.split('\n').collect();

    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }
        if !in_fence && contains_tag_ci(lines[i], "<table") {
            let start_line = i;
            let mut block = String::new();
            let mut found_end = false;
            while i < lines.len() {
                block.push_str(lines[i]);
                block.push('\n');
                if contains_tag_ci(lines[i], "</table>") {
                    found_end = true;
                    i += 1;
                    break;
                }
                i += 1;
            }
            if found_end {
                if let Some(md_table) = parse_single_html_table(&block) {
                    out.push_str(md_table.trim_end_matches('\n'));
                    if i < lines.len() {
                        out.push('\n');
                    }
                    count += 1;
                    continue;
                }
            }
            // Fallback: restore original lines if incomplete or empty
            for (idx, orig) in lines[start_line..i].iter().enumerate() {
                out.push_str(orig);
                if start_line + idx + 1 < lines.len() {
                    out.push('\n');
                }
            }
            continue;
        }
        out.push_str(lines[i]);
        if i + 1 < lines.len() {
            out.push('\n');
        }
        i += 1;
    }

    (out, count)
}

fn contains_tag_ci(s: &str, needle_lower: &str) -> bool {
    s.to_ascii_lowercase().contains(needle_lower)
}

fn parse_single_html_table(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut pos = 0usize;
    while let Some(tr_rel) = lower[pos..].find("<tr") {
        let tr_open = pos + tr_rel;
        let Some(gt_rel) = lower[tr_open..].find('>') else {
            break;
        };
        let row_body_start = tr_open + gt_rel + 1;
        let Some(tr_end_rel) = lower[row_body_start..].find("</tr>") else {
            break;
        };
        let row_body_end = row_body_start + tr_end_rel;
        let row_html = &html[row_body_start..row_body_end];
        let row_lower = &lower[row_body_start..row_body_end];

        let mut cells: Vec<String> = Vec::new();
        let mut cpos = 0usize;
        while cpos < row_lower.len() {
            let next_th = row_lower[cpos..].find("<th").map(|o| (cpos + o, "</th>"));
            let next_td = row_lower[cpos..].find("<td").map(|o| (cpos + o, "</td>"));
            let next_cell = match (next_th, next_td) {
                (Some(a), Some(b)) => {
                    if a.0 <= b.0 {
                        Some(a)
                    } else {
                        Some(b)
                    }
                }
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            let Some((cell_open, close_tag)) = next_cell else {
                break;
            };
            let Some(cgt_rel) = row_lower[cell_open..].find('>') else {
                break;
            };
            let cell_start = cell_open + cgt_rel + 1;
            let Some(cend_rel) = row_lower[cell_start..].find(close_tag) else {
                break;
            };
            let cell_end = cell_start + cend_rel;
            let cell_raw = &row_html[cell_start..cell_end];
            cells.push(strip_html_tags_and_entities(cell_raw));
            cpos = cell_end + close_tag.len();
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
        pos = row_body_end + "</tr>".len();
    }
    if rows.is_empty() {
        return None;
    }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols == 0 {
        return None;
    }
    for r in &mut rows {
        r.resize(ncols, String::new());
    }
    let headers = rows.remove(0);
    Some(nota_logic::format_aligned_markdown_table(&headers, &rows))
}

fn strip_html_tags_and_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '\n' | '\r' => {
                if !in_tag {
                    out.push(' ');
                }
            }
            _ => {
                if !in_tag {
                    out.push(ch);
                }
            }
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace('|', "\\|")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl Action {
    pub fn is_edit(&self) -> bool {
        matches!(self, Action::Edit(_) | Action::SyncText { .. })
    }
}

/// Borrowed view of a single line inside [`TextBuffer`].
#[derive(Debug, Clone)]
pub struct LineView<'a> {
    pub text: &'a str,
}

/// Pure Rust UTF-8 multi-line buffer with direct byte-offset and line/column cursor mapping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    raw: String,
    cursor: Cursor,
}

impl Content {
    pub fn new() -> Self {
        Self {
            raw: String::new(),
            cursor: Cursor::default(),
        }
    }

    pub fn with_text(text: &str) -> Self {
        Self {
            raw: text.to_string(),
            cursor: Cursor::default(),
        }
    }

    pub fn text(&self) -> String {
        self.raw.clone()
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn move_to(&mut self, cursor: Cursor) {
        self.cursor = self.clamp_cursor(cursor);
    }

    pub fn line_count(&self) -> usize {
        self.raw.split('\n').count()
    }

    pub fn line(&self, line_idx: usize) -> Option<LineView<'_>> {
        self.raw
            .split('\n')
            .nth(line_idx)
            .map(|s| LineView { text: s.strip_suffix('\r').unwrap_or(s) })
    }

    /// Converts a `(line, column_byte)` [`Position`] into an absolute byte offset in `self.raw`.
    pub fn position_to_byte_offset(&self, pos: Position) -> usize {
        let mut cur_line = 0usize;
        let mut line_start = 0usize;
        for (idx, ch) in self.raw.char_indices() {
            if cur_line == pos.line {
                line_start = idx;
                break;
            }
            if ch == '\n' {
                cur_line += 1;
                line_start = idx + 1;
            }
        }
        if cur_line < pos.line {
            return self.raw.len();
        }
        let line_end = self.raw[line_start..]
            .find('\n')
            .map(|rel| line_start + rel)
            .unwrap_or(self.raw.len());
        let line_len = line_end.saturating_sub(line_start);
        let clamped_col = pos.column.min(line_len);
        let mut byte_off = line_start + clamped_col;
        while byte_off > line_start && !self.raw.is_char_boundary(byte_off) {
            byte_off -= 1;
        }
        byte_off
    }

    /// Converts an absolute byte offset in `self.raw` into a `(line, column_byte)` [`Position`].
    pub fn byte_offset_to_position(&self, byte_offset: usize) -> Position {
        let clamped = byte_offset.min(self.raw.len());
        let mut line = 0usize;
        let mut line_start = 0usize;
        for (idx, ch) in self.raw[..clamped].char_indices() {
            if ch == '\n' {
                line += 1;
                line_start = idx + 1;
            }
        }
        Position {
            line,
            column: clamped.saturating_sub(line_start),
        }
    }

    pub fn cursor_byte_offset(&self) -> usize {
        self.position_to_byte_offset(self.cursor.position)
    }

    pub fn set_text_and_cursor_byte(&mut self, new_text: String, cursor_byte: usize) {
        self.raw = new_text;
        let pos = self.byte_offset_to_position(cursor_byte);
        self.cursor = Cursor {
            position: pos,
            selection: None,
        };
    }

    pub fn perform(&mut self, action: Action) {
        match action {
            Action::Move(pos) => {
                self.cursor = self.clamp_cursor(Cursor {
                    position: pos,
                    selection: None,
                });
            }
            Action::SyncText { text, cursor, .. } => {
                self.raw = text;
                self.cursor = self.clamp_cursor(Cursor {
                    position: cursor,
                    selection: None,
                });
            }
            Action::Edit(edit) => {
                let (sel_start, sel_end) = self.selection_byte_range();
                match edit {
                    Edit::Insert(ch) => {
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        self.raw.replace_range(sel_start..sel_end, s);
                        let new_byte = sel_start + s.len();
                        self.cursor = Cursor {
                            position: self.byte_offset_to_position(new_byte),
                            selection: None,
                        };
                    }
                    Edit::Paste(text) => {
                        self.raw.replace_range(sel_start..sel_end, text.as_str());
                        let new_byte = sel_start + text.len();
                        self.cursor = Cursor {
                            position: self.byte_offset_to_position(new_byte),
                            selection: None,
                        };
                    }
                    Edit::Backspace => {
                        if sel_start != sel_end {
                            self.raw.replace_range(sel_start..sel_end, "");
                            self.cursor = Cursor {
                                position: self.byte_offset_to_position(sel_start),
                                selection: None,
                            };
                        } else if sel_start > 0 {
                            let prev_char_start = self.raw[..sel_start]
                                .char_indices()
                                .last()
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            self.raw.replace_range(prev_char_start..sel_start, "");
                            self.cursor = Cursor {
                                position: self.byte_offset_to_position(prev_char_start),
                                selection: None,
                            };
                        }
                    }
                    Edit::Delete => {
                        if sel_start != sel_end {
                            self.raw.replace_range(sel_start..sel_end, "");
                            self.cursor = Cursor {
                                position: self.byte_offset_to_position(sel_start),
                                selection: None,
                            };
                        } else if sel_start < self.raw.len() {
                            let next_char_end = self.raw[sel_start..]
                                .chars()
                                .next()
                                .map(|c| sel_start + c.len_utf8())
                                .unwrap_or(self.raw.len());
                            self.raw.replace_range(sel_start..next_char_end, "");
                        }
                    }
                }
            }
        }
    }

    fn selection_byte_range(&self) -> (usize, usize) {
        let a = self.position_to_byte_offset(self.cursor.position);
        if let Some(sel) = self.cursor.selection {
            let b = self.position_to_byte_offset(sel);
            if a <= b {
                (a, b)
            } else {
                (b, a)
            }
        } else {
            (a, a)
        }
    }

    fn clamp_cursor(&self, cursor: Cursor) -> Cursor {
        let pos_byte = self.position_to_byte_offset(cursor.position);
        let position = self.byte_offset_to_position(pos_byte);
        let selection = cursor.selection.map(|s| {
            let b = self.position_to_byte_offset(s);
            self.byte_offset_to_position(b)
        });
        Cursor {
            position,
            selection,
        }
    }
}

/// Represents an open note tab's editor state backed by a pure UTF-8 [`Content`] buffer.
#[derive(Debug, Clone)]
pub struct NotaEditorTab {
    pub rel_path: String,
    pub title: String,
    pub content: Content,
    pub is_dirty: bool,
    pub is_pinned: bool,
    pub undo_stack: Vec<(String, Position)>,
    pub redo_stack: Vec<(String, Position)>,
    pub last_matched_rule: Option<String>,
}

impl NotaEditorTab {
    pub fn new(rel_path: impl Into<String>, raw_text: &str) -> Self {
        let rel_path = rel_path.into();
        let title = rel_path
            .strip_suffix(".md")
            .unwrap_or(&rel_path)
            .to_string();
        Self {
            rel_path,
            title,
            content: Content::with_text(raw_text),
            is_dirty: false,
            is_pinned: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_matched_rule: None,
        }
    }

    pub fn text(&self) -> String {
        self.content.text()
    }

    pub fn cursor_position(&self) -> Position {
        self.content.cursor().position
    }

    pub fn word_and_char_count(&self) -> (usize, usize) {
        let t = self.content.as_str();
        let words = t.split_whitespace().count();
        let chars = t.chars().count();
        (words, chars)
    }

    /// Performs an editor [`Action`] and, if `typing_transformer_enabled` is true
    /// and the action inserted a character outside a code fence, evaluates the 61 Typing Transformer
    /// rules at the cursor in $O(1)$ slice operations.
    pub fn apply_editor_action(
        &mut self,
        action: Action,
        rules: &TypingRuleEngine,
        typing_transformer_enabled: bool,
    ) {
        let is_edit = action.is_edit();
        let is_char_insert = matches!(
            action,
            Action::Edit(Edit::Insert(_))
                | Action::SyncText {
                    is_char_insert: true,
                    ..
                }
        );

        if is_edit {
            let snapshot_text = self.content.text();
            let snapshot_pos = self.content.cursor().position;
            if self
                .undo_stack
                .last()
                .map(|(t, _)| t != &snapshot_text)
                .unwrap_or(true)
            {
                self.undo_stack.push((snapshot_text, snapshot_pos));
                if self.undo_stack.len() > 200 {
                    self.undo_stack.remove(0);
                }
            }
            self.redo_stack.clear();
            self.is_dirty = true;
        }

        self.content.perform(action);

        if is_char_insert && typing_transformer_enabled && !self.is_cursor_inside_code_fence() {
            self.try_apply_typing_rule_at_cursor(rules);
        }
    }

    /// Synchronizes an edit originating from a GPUI `TextareaState` / `EditorState` rope,
    /// applying the 61-rule Typing Transformer if a character was inserted.
    /// Returns `Some((new_text, new_cursor_byte_offset))` if a Typing Transformer rule modified the text.
    pub fn sync_from_gpui_editor(
        &mut self,
        new_text: &str,
        cursor_byte_offset: usize,
        rules: &TypingRuleEngine,
        typing_transformer_enabled: bool,
    ) -> Option<(String, usize)> {
        let old_len = self.content.as_str().len();
        let is_char_insert = new_text.len() > old_len && new_text.len() - old_len <= 4;
        let cursor_pos = {
            let mut temp = Content::with_text(new_text);
            let pos = temp.byte_offset_to_position(cursor_byte_offset);
            temp.move_to(Cursor {
                position: pos,
                selection: None,
            });
            pos
        };
        if new_text == self.content.as_str() {
            self.content.move_to(Cursor {
                position: cursor_pos,
                selection: None,
            });
            return None;
        }

        self.apply_editor_action(
            Action::SyncText {
                text: new_text.to_string(),
                cursor: cursor_pos,
                is_char_insert,
            },
            rules,
            typing_transformer_enabled,
        );

        if typing_transformer_enabled {
            let cur_text = self.content.as_str();
            if cur_text.to_ascii_lowercase().contains("</table>") {
                let (converted, count) = convert_html_tables_in_markdown(cur_text);
                if count > 0 && converted != cur_text {
                    let new_byte = cursor_byte_offset.min(converted.len());
                    self.content.set_text_and_cursor_byte(converted, new_byte);
                    self.last_matched_rule = Some(format!(
                        "Converted {} HTML <table> block(s) → Markdown pipe table",
                        count
                    ));
                }
            }
        }

        if self.content.as_str() != new_text {
            Some((self.content.text(), self.content.cursor_byte_offset()))
        } else {
            None
        }
    }

    fn try_apply_typing_rule_at_cursor(&mut self, rules: &TypingRuleEngine) -> bool {
        let cursor = self.content.cursor().position;
        let Some(line_obj) = self.content.line(cursor.line) else {
            return false;
        };
        let line_str = line_obj.text;
        let col_byte = cursor.column.min(line_str.len());
        if !line_str.is_char_boundary(col_byte) {
            return false;
        }
        let Some((_new_line, new_col_byte, rule_idx)) = rules.apply_at_cursor(line_str, col_byte)
        else {
            return false;
        };

        let rule = &rules.rules[rule_idx];
        let replace_start_col = col_byte.saturating_sub(rule.trigger_before_cursor.len());
        let replace_end_col = (col_byte + rule.trigger_after_cursor.len()).min(line_str.len());
        let replacement = rule.full_result();

        self.content.move_to(Cursor {
            position: Position {
                line: cursor.line,
                column: replace_start_col,
            },
            selection: Some(Position {
                line: cursor.line,
                column: replace_end_col,
            }),
        });
        self.content
            .perform(Action::Edit(Edit::Paste(Arc::new(replacement))));
        self.content.move_to(Cursor {
            position: Position {
                line: cursor.line,
                column: new_col_byte,
            },
            selection: None,
        });
        self.last_matched_rule = Some(format!(
            "'{}' → '{}'",
            rule.full_trigger(),
            rule.full_result()
        ));
        true
    }

    pub fn undo(&mut self) -> bool {
        if let Some((prev_text, prev_pos)) = self.undo_stack.pop() {
            let cur_text = self.content.text();
            let cur_pos = self.content.cursor().position;
            self.redo_stack.push((cur_text, cur_pos));
            self.content = Content::with_text(&prev_text);
            self.content.move_to(Cursor {
                position: prev_pos,
                selection: None,
            });
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some((next_text, next_pos)) = self.redo_stack.pop() {
            let cur_text = self.content.text();
            let cur_pos = self.content.cursor().position;
            self.undo_stack.push((cur_text, cur_pos));
            self.content = Content::with_text(&next_text);
            self.content.move_to(Cursor {
                position: next_pos,
                selection: None,
            });
            self.is_dirty = true;
            true
        } else {
            false
        }
    }

    /// Inserts arbitrary text (e.g. a completed Truth Tree block, a Truth Table, an EPUB quote citation,
    /// or an expanded template) at the current cursor position as a single Undo step.
    pub fn insert_block_at_cursor(&mut self, snippet: &str) {
        let cur_text = self.content.text();
        let cur_pos = self.content.cursor().position;
        self.undo_stack.push((cur_text, cur_pos));
        self.redo_stack.clear();
        self.content
            .perform(Action::Edit(Edit::Paste(Arc::new(snippet.to_string()))));
        self.is_dirty = true;
    }

    /// If the user is currently typing inside an unclosed `[[query` before the cursor on the active line,
    /// returns `(open_bracket_col, query_str)` so the UI can show live wikilink autocomplete & "Create Note".
    pub fn active_wikilink_query(&self) -> Option<(usize, String)> {
        let pos = self.content.cursor().position;
        let line_obj = self.content.line(pos.line)?;
        let line_str = line_obj.text;
        let col = pos.column.min(line_str.len());
        if !line_str.is_char_boundary(col) {
            return None;
        }
        let before = &line_str[..col];
        let open_idx = before.rfind("[[")?;
        let after_open = &before[open_idx + 2..];
        if after_open.contains("]]") {
            return None;
        }
        Some((open_idx, after_open.to_string()))
    }

    /// Completes an open `[[query` at the cursor with `[[target_stem]]`.
    pub fn complete_wikilink(&mut self, target_stem: &str) {
        if let Some((open_idx, _query)) = self.active_wikilink_query() {
            let pos = self.content.cursor().position;
            let cur_text = self.content.text();
            self.undo_stack.push((cur_text, pos));
            self.redo_stack.clear();

            self.content.move_to(Cursor {
                position: Position {
                    line: pos.line,
                    column: open_idx,
                },
                selection: Some(pos),
            });
            let completed = format!("[[{}]]", target_stem);
            let new_col = open_idx + completed.len();
            self.content
                .perform(Action::Edit(Edit::Paste(Arc::new(completed))));
            self.content.move_to(Cursor {
                position: Position {
                    line: pos.line,
                    column: new_col,
                },
                selection: None,
            });
            self.is_dirty = true;
        }
    }

    /// Saves the note atomically to `vault_root/rel_path` via a temporary file rename,
    /// preserving UTF-8, NBSP (`\u{00a0}`), and trailing double-spaces.
    pub fn save_atomic(&mut self, vault_root: &Path) -> Result<()> {
        let target_path = vault_root.join(&self.rel_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = target_path.with_extension("md.nota-tmp");
        let raw = self.content.text();
        fs::write(&tmp_path, raw.as_bytes())
            .with_context(|| format!("Failed to write temp file {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &target_path)
            .with_context(|| format!("Failed to atomically replace {}", target_path.display()))?;
        self.is_dirty = false;
        Ok(())
    }

    fn is_cursor_inside_code_fence(&self) -> bool {
        let cur_line = self.content.cursor().position.line;
        let mut in_fence = false;
        for line_idx in 0..cur_line {
            if let Some(l) = self.content.line(line_idx) {
                let t = l.text.trim_start();
                if t.starts_with("```") || t.starts_with("~~~") {
                    in_fence = !in_fence;
                }
            }
        }
        in_fence
    }

    /// Normalizes ASCII logic operators (`->`, `<->`, `^`, `~`, `v`/`V`) in the note as a single undoable step.
    pub fn normalize_ascii_operators_in_note(&mut self) -> usize {
        let cur = self.content.text();
        let (normalized, count) = nota_logic::normalize_ascii_operators_in_text(&cur);
        if count > 0 && normalized != cur {
            let pos = self.content.cursor().position;
            self.undo_stack.push((cur, pos));
            self.redo_stack.clear();
            self.content = Content::with_text(&normalized);
            self.is_dirty = true;
        }
        count
    }

    /// Converts legacy ASCII truth trees in the note into fenced ` ```tree ` blocks as an undoable step.
    pub fn convert_ascii_trees_in_note(&mut self) -> usize {
        let cur = self.content.text();
        let (converted_md, count) = nota_logic::convert_ascii_trees_to_fenced_blocks(&cur);
        if count > 0 && converted_md != cur {
            let pos = self.content.cursor().position;
            self.undo_stack.push((cur, pos));
            self.redo_stack.clear();
            self.content = Content::with_text(&converted_md);
            self.is_dirty = true;
        }
        count
    }

    /// Converts raw HTML `<table>...</table>` blocks in the note into aligned Markdown pipe tables as an undoable step.
    pub fn convert_html_tables_in_note(&mut self) -> usize {
        let cur = self.content.text();
        let (converted_md, count) = convert_html_tables_in_markdown(&cur);
        if count > 0 && converted_md != cur {
            let pos = self.content.cursor().position;
            self.undo_stack.push((cur, pos));
            self.redo_stack.clear();
            self.content = Content::with_text(&converted_md);
            self.is_dirty = true;
        }
        count
    }

    /// Cleans frontmatter metadata (`aliases: ["[]"]`, tag typos, title mismatch) and `===text===` prose highlights
    /// in the note as a single undoable step.
    pub fn clean_metadata_and_highlights_in_note(&mut self) -> Vec<String> {
        let cur = self.content.text();
        let (cleaned, changes) = nota_core::clean_note_metadata_and_highlights(
            &cur,
            &self.title,
            true,
            true,
            true,
        );
        if !changes.is_empty() && cleaned != cur {
            let pos = self.content.cursor().position;
            self.undo_stack.push((cur, pos));
            self.redo_stack.clear();
            self.content = Content::with_text(&cleaned);
            self.is_dirty = true;
        }
        changes
    }

    /// Replaces a Markdown table spanning 1-indexed `start_line..=end_line` with `new_table_md` as an undoable step.
    pub fn replace_table_lines(
        &mut self,
        start_line: usize,
        end_line: usize,
        new_table_md: &str,
    ) {
        let cur = self.content.text();
        let lines: Vec<&str> = cur.lines().collect();
        if start_line == 0 || start_line > lines.len() {
            return;
        }
        let s_idx = start_line - 1;
        let e_idx = end_line.min(lines.len());
        let mut out = String::with_capacity(cur.len() + 128);
        for l in &lines[..s_idx] {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(new_table_md.trim_end());
        out.push('\n');
        for l in &lines[e_idx..] {
            out.push_str(l);
            out.push('\n');
        }
        let pos = self.content.cursor().position;
        self.undo_stack.push((cur, pos));
        self.redo_stack.clear();
        self.content = Content::with_text(&out);
        self.is_dirty = true;
    }
}

/// Expands a template (such as `content/templates/Basic Note.md`) replacing `{{title}}`, `{{date}}`, `{{time}}`.
pub fn expand_note_template(template_raw: &str, note_title: &str) -> String {
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let time_str = now.format("%H:%M").to_string();

    template_raw
        .replace("{{title}}", note_title)
        .replace("{{date}}", &date_str)
        .replace("{{time}}", &time_str)
        // Clean up the default `aliases:\n  - "[]"` placeholder in Basic Note.md if present
        .replace("aliases:\n  - \"[]\"", "aliases: []")
}

/// Renames a note from `old_rel_path` to `new_rel_path` and updates every `[[old_stem]]`,
/// `[[old_stem|alias]]`, and `[[old_stem#heading]]` reference across all Markdown files in `vault_root`
/// (honoring Obsidian's `alwaysUpdateLinks: true`).
pub fn rename_note_and_update_links(
    vault_root: &Path,
    old_rel_path: &str,
    new_rel_path: &str,
    note_rel_paths: &[String],
) -> Result<usize> {
    let old_stem = old_rel_path.strip_suffix(".md").unwrap_or(old_rel_path);
    let new_stem = new_rel_path.strip_suffix(".md").unwrap_or(new_rel_path);
    let old_key = normalize_lookup_key(old_stem);

    let old_abs = vault_root.join(old_rel_path);
    let new_abs = vault_root.join(new_rel_path);
    if old_abs.exists() && old_abs != new_abs {
        if let Some(parent) = new_abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&old_abs, &new_abs)?;
    }

    let mut updated_files = 0usize;
    for rel in note_rel_paths {
        let target_rel = if rel == old_rel_path {
            new_rel_path
        } else {
            rel.as_str()
        };
        let abs = vault_root.join(target_rel);
        if !abs.exists() {
            continue;
        }
        let raw = fs::read_to_string(&abs)?;
        if let Some(rewritten) = rewrite_wikilinks_in_markdown(&raw, &old_key, new_stem) {
            let tmp = abs.with_extension("md.nota-tmp");
            fs::write(&tmp, rewritten.as_bytes())?;
            fs::rename(&tmp, &abs)?;
            updated_files += 1;
        }
    }

    Ok(updated_files)
}

pub fn rewrite_wikilinks_in_markdown(
    raw: &str,
    old_normalized_key: &str,
    new_stem: &str,
) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut cursor = 0usize;
    let mut changed = false;

    while let Some(rel_open) = raw[cursor..].find("[[") {
        let open = cursor + rel_open;
        out.push_str(&raw[cursor..open]);
        let inner_start = open + 2;
        if let Some(rel_close) = raw[inner_start..].find("]]") {
            let close = inner_start + rel_close;
            let inner = &raw[inner_start..close];
            let (target_and_hash, alias_part) = match inner.split_once('|') {
                Some((t, a)) => (t, Some(a)),
                None => (inner, None),
            };
            let (target_part, hash_part) = match target_and_hash.split_once('#') {
                Some((t, h)) => (t, Some(h)),
                None => (target_and_hash, None),
            };

            if normalize_lookup_key(target_part) == old_normalized_key {
                changed = true;
                out.push_str("[[");
                out.push_str(new_stem);
                if let Some(h) = hash_part {
                    out.push('#');
                    out.push_str(h);
                }
                if let Some(a) = alias_part {
                    out.push('|');
                    out.push_str(a);
                }
                out.push_str("]]");
            } else {
                out.push_str(&raw[open..close + 2]);
            }
            cursor = close + 2;
        } else {
            out.push_str("[[");
            cursor = inner_start;
        }
    }
    out.push_str(&raw[cursor..]);

    if changed {
        Some(out)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveTruthTreeSession {
    pub problem_input: String,
    pub tree: TruthTree,
    pub selected_node: Option<usize>,
    pub practice_mode: bool,
    pub user_branch_mode: bool,
    pub user_left_input: String,
    pub user_right_input: String,
    pub last_feedback: Option<(bool, String)>,
}

impl InteractiveTruthTreeSession {
    pub fn from_problem_str(problem_input: &str, practice_mode: bool) -> Result<Self> {
        let stmt = nota_logic::parse_metalogic_stmt(problem_input)?;
        let tree = match stmt {
            nota_logic::MetalogicStmt::Entailment {
                premises,
                conclusion,
                ..
            } => TruthTree::from_entailment(&premises, &conclusion),
            nota_logic::MetalogicStmt::Equivalence(lhs, rhs) => {
                let iff = Formula::binary(nota_logic::BinOp::Iff, lhs, rhs);
                TruthTree::from_entailment(&[], &iff)
            }
            nota_logic::MetalogicStmt::FormulaSet(fs) => TruthTree::from_formulas(&fs),
            nota_logic::MetalogicStmt::Single(f) => TruthTree::from_formulas(&[f]),
            nota_logic::MetalogicStmt::NonEquivalence(lhs, rhs) => {
                let niff = Formula::binary(nota_logic::BinOp::NonIff, lhs, rhs);
                TruthTree::from_formulas(&[niff])
            }
        };
        let first_unexpanded = tree.nodes.iter().find(|n| !n.checked).map(|n| n.id);
        Ok(Self {
            problem_input: problem_input.to_string(),
            tree,
            selected_node: first_unexpanded,
            practice_mode,
            user_branch_mode: false,
            user_left_input: String::new(),
            user_right_input: String::new(),
            last_feedback: None,
        })
    }

    pub fn decompose_selected(&mut self) -> Result<()> {
        let Some(node_id) = self.selected_node else {
            return Ok(());
        };
        if self.practice_mode {
            let left_formulas = parse_comma_formulas(&self.user_left_input)?;
            let right_formulas = if self.user_branch_mode {
                Some(parse_comma_formulas(&self.user_right_input)?)
            } else {
                None
            };
            match self.tree.verify_user_step(
                node_id,
                self.user_branch_mode,
                &left_formulas,
                right_formulas.as_deref(),
            ) {
                Ok(()) => {
                    self.tree.decompose_node(node_id)?;
                    self.last_feedback = Some((
                        true,
                        format!("Correct! Line {} decomposed ✓", node_id + 1),
                    ));
                    self.user_left_input.clear();
                    self.user_right_input.clear();
                }
                Err(err) => {
                    self.last_feedback = Some((false, err.to_string()));
                    return Ok(());
                }
            }
        } else {
            self.tree.decompose_node(node_id)?;
            self.last_feedback = Some((true, format!("Decomposed line {} ✓", node_id + 1)));
        }
        self.selected_node = self.tree.nodes.iter().find(|n| !n.checked).map(|n| n.id);
        Ok(())
    }

    pub fn solve_all(&mut self) -> Result<()> {
        self.tree.solve_alpha_first(100)?;
        self.selected_node = None;
        self.last_feedback = Some((
            true,
            "Solved complete Truth Tree using α-first (stacking before branching) priority."
                .to_string(),
        ));
        Ok(())
    }
}

fn parse_comma_formulas(input: &str) -> Result<Vec<Formula>> {
    let mut out = Vec::new();
    for part in input.split(',') {
        if !part.trim().is_empty() {
            out.push(nota_logic::parse_formula(part.trim())?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_typing_transformer_replacement() {
        let conf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("typing-rules.conf");
        let rules = TypingRuleEngine::from_conf_file(conf_path).unwrap();
        let mut tab = NotaEditorTab::new("Logic.md", "");

        // Type `p <-> q` character by character into `Content`
        for ch in "p <-> q".chars() {
            tab.apply_editor_action(Action::Edit(Edit::Insert(ch)), &rules, true);
        }
        assert_eq!(tab.text(), "p ↔ q");

        // Type ` <!--` and verify HTML comment restoration works on `Content`
        for ch in " <!--".chars() {
            tab.apply_editor_action(Action::Edit(Edit::Insert(ch)), &rules, true);
        }
        assert_eq!(tab.text(), "p ↔ q <!--");
    }

    #[test]
    fn test_gpui_sync_typing_transformer_and_undo() {
        let conf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("typing-rules.conf");
        let rules = TypingRuleEngine::from_conf_file(conf_path).unwrap();
        let mut tab = NotaEditorTab::new("Logic.md", "p <-");

        // Simulate GPUI TextareaState inserting `>` at byte offset 5 (`"p <->"`)
        let transformed = tab.sync_from_gpui_editor("p <->", 5, &rules, true);
        assert_eq!(transformed, Some(("p ↔".to_string(), "p ↔".len())));
        assert_eq!(tab.text(), "p ↔");

        assert!(tab.undo());
        assert_eq!(tab.text(), "p <-");
        assert!(tab.redo());
        assert_eq!(tab.text(), "p ↔");
    }

    #[test]
    fn test_wikilink_autocomplete_and_rename_rewrite() {
        let rules = TypingRuleEngine::default();
        let mut tab = NotaEditorTab::new("Test.md", "See ");
        tab.content.move_to(Cursor {
            position: Position { line: 0, column: 4 },
            selection: None,
        });
        for ch in "[[Sembolik".chars() {
            tab.apply_editor_action(Action::Edit(Edit::Insert(ch)), &rules, false);
        }
        assert_eq!(
            tab.active_wikilink_query(),
            Some((4, "Sembolik".to_string()))
        );
        tab.complete_wikilink("Sembolik Mantık Giriş");
        assert_eq!(tab.text(), "See [[Sembolik Mantık Giriş]]");

        let rewritten = rewrite_wikilinks_in_markdown(
            "Links: [[Sembolik Mantık Giriş|Giriş]] and [[sembolik mantık giriş#Summary]].",
            &normalize_lookup_key("Sembolik Mantık Giriş"),
            "Mantık 101",
        )
        .unwrap();
        assert_eq!(
            rewritten,
            "Links: [[Mantık 101|Giriş]] and [[Mantık 101#Summary]]."
        );
    }

    #[test]
    fn test_auto_html_table_conversion_in_source_editor() {
        let rules = TypingRuleEngine::default();
        let mut tab = NotaEditorTab::new("TableTest.md", "Intro\n");
        let pasted = "Intro\n<table><tr><th>p q</th><th>p → q</th></tr><tr><td>D D</td><td>D</td></tr><tr><td>D Y</td><td>Y</td></tr></table>\nOutro";
        let res = tab.sync_from_gpui_editor(pasted, pasted.len(), &rules, true);
        assert!(res.is_some());
        let converted = tab.text();
        assert!(converted.contains("| p q | p → q |"));
        assert!(converted.contains("| D D | D     |"));
        assert!(converted.contains("| D Y | Y     |"));
        assert!(!converted.contains("<table>"));
    }
}
