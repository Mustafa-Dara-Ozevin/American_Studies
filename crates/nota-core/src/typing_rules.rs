use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Represents a single Typing Transformer conversion rule: `'trigger' -> 'replacement'`
/// where `|` marks the cursor position (in char/byte offset) and `\|` is a literal pipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypingRule {
    /// Text required immediately before the cursor to trigger the rule
    pub trigger_before_cursor: String,
    /// Optional text required immediately after the cursor to trigger the rule
    pub trigger_after_cursor: String,
    /// Replacement text inserted before the new cursor position
    pub result_before_cursor: String,
    /// Replacement text inserted after the new cursor position
    pub result_after_cursor: String,
    /// Original line number in config
    pub line_number: usize,
}

impl TypingRule {
    pub fn full_trigger(&self) -> String {
        format!("{}{}", self.trigger_before_cursor, self.trigger_after_cursor)
    }

    pub fn full_result(&self) -> String {
        format!("{}{}", self.result_before_cursor, self.result_after_cursor)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypingRuleEngine {
    pub rules: Vec<TypingRule>,
    pub auto_format_on: bool,
}

#[derive(Debug, Deserialize)]
struct ObsidianTypingTransformerJson {
    #[serde(rename = "convertRules")]
    convert_rules: String,
    #[serde(rename = "autoFormatOn", default)]
    auto_format_on: bool,
}

impl TypingRuleEngine {
    pub fn from_conf_str(input: &str) -> Result<Self> {
        let mut rules = Vec::new();
        for (idx, raw_line) in input.lines().enumerate() {
            let line_no = idx + 1;
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(rule) = parse_rule_line(trimmed, line_no)? {
                rules.push(rule);
            }
        }
        Ok(Self {
            rules,
            auto_format_on: true,
        })
    }

    pub fn from_conf_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read {}", path.as_ref().display()))?;
        Self::from_conf_str(&content)
    }

    pub fn from_obsidian_data_json(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read {}", path.as_ref().display()))?;
        let parsed: ObsidianTypingTransformerJson = serde_json::from_str(&raw)
            .with_context(|| "Failed to parse typing-transformer data.json")?;
        let mut engine = Self::from_conf_str(&parsed.convert_rules)?;
        engine.auto_format_on = parsed.auto_format_on;
        Ok(engine)
    }

    /// Evaluates the rules in file order (first match wins) against `text` at `cursor_byte_offset`.
    /// Returns `(new_text, new_cursor_byte_offset, matched_rule_index)` if a rule triggered.
    pub fn apply_at_cursor(
        &self,
        text: &str,
        cursor_byte_offset: usize,
    ) -> Option<(String, usize, usize)> {
        if cursor_byte_offset > text.len() || !text.is_char_boundary(cursor_byte_offset) {
            return None;
        }
        let before = &text[..cursor_byte_offset];
        let after = &text[cursor_byte_offset..];

        // Do not transform inside inline code (`...`) on the current line
        let current_line_before = before.rsplit('\n').next().unwrap_or(before);
        let backtick_count = current_line_before.chars().filter(|&c| c == '`').count();
        if backtick_count % 2 == 1 {
            return None;
        }

        for (idx, rule) in self.rules.iter().enumerate() {
            if before.ends_with(&rule.trigger_before_cursor)
                && after.starts_with(&rule.trigger_after_cursor)
            {
                let replace_start = cursor_byte_offset - rule.trigger_before_cursor.len();
                let replace_end = cursor_byte_offset + rule.trigger_after_cursor.len();

                // Skip URLs like http:// or https://
                let prefix = &text[..replace_start];
                if prefix.ends_with("http://") || prefix.ends_with("https://") {
                    continue;
                }

                let mut out = String::with_capacity(
                    text.len() - (replace_end - replace_start)
                        + rule.result_before_cursor.len()
                        + rule.result_after_cursor.len(),
                );
                out.push_str(&text[..replace_start]);
                out.push_str(&rule.result_before_cursor);
                let new_cursor = out.len();
                out.push_str(&rule.result_after_cursor);
                out.push_str(&text[replace_end..]);
                return Some((out, new_cursor, idx));
            }
        }
        None
    }

    /// Simulates typing a string character-by-character into an empty or existing buffer,
    /// returning the final buffer and cursor offset.
    pub fn simulate_typing(&self, typed_chars: &str) -> (String, usize) {
        let mut buf = String::new();
        let mut cursor = 0;
        for ch in typed_chars.chars() {
            buf.insert(cursor, ch);
            cursor += ch.len_utf8();
            if let Some((next_buf, next_cursor, _)) = self.apply_at_cursor(&buf, cursor) {
                buf = next_buf;
                cursor = next_cursor;
            }
        }
        (buf, cursor)
    }
}

fn parse_rule_line(line: &str, line_number: usize) -> Result<Option<TypingRule>> {
    // Parse `'trigger' -> 'result'`
    let (lhs_raw, rhs_part) = extract_single_quoted(line)
        .ok_or_else(|| anyhow!("Line {}: missing opening single-quoted trigger", line_number))?;
    let rhs_part = rhs_part.trim_start();
    let after_arrow = rhs_part
        .strip_prefix("->")
        .ok_or_else(|| anyhow!("Line {}: expected '->' after trigger", line_number))?
        .trim_start();
    let (rhs_raw, trailing) = extract_single_quoted(after_arrow)
        .ok_or_else(|| anyhow!("Line {}: missing single-quoted replacement", line_number))?;

    let trailing = trailing.trim();
    if !trailing.is_empty() && !trailing.starts_with('#') {
        return Err(anyhow!(
            "Line {}: unexpected trailing characters: {}",
            line_number,
            trailing
        ));
    }

    let (trigger_before_cursor, trigger_after_cursor) = parse_pattern_with_cursor(&lhs_raw);
    let (result_before_cursor, result_after_cursor) = parse_pattern_with_cursor(&rhs_raw);

    Ok(Some(TypingRule {
        trigger_before_cursor,
        trigger_after_cursor,
        result_before_cursor,
        result_after_cursor,
        line_number,
    }))
}

fn extract_single_quoted(input: &str) -> Option<(String, &str)> {
    let trimmed = input.trim_start();
    let rest = trimmed.strip_prefix('\'')?;
    let mut chars = rest.char_indices();
    let mut collected = String::new();
    while let Some((idx, ch)) = chars.next() {
        if ch == '\'' {
            let remaining = &rest[idx + 1..];
            return Some((collected, remaining));
        }
        collected.push(ch);
    }
    None
}

/// Unescapes `\|` -> literal `|` and `\\` -> literal `\`, while splitting on unescaped `|`
/// which marks the cursor offset.
fn parse_pattern_with_cursor(raw: &str) -> (String, String) {
    let mut before = String::new();
    let mut after = String::new();
    let mut seen_cursor = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        let target = if seen_cursor { &mut after } else { &mut before };
        if ch == '\\' {
            match chars.peek().copied() {
                Some('|') => {
                    chars.next();
                    target.push('|');
                }
                Some('\\') => {
                    chars.next();
                    target.push('\\');
                }
                Some('n') => {
                    chars.next();
                    target.push('\n');
                }
                Some('t') => {
                    chars.next();
                    target.push('\t');
                }
                _ => {
                    target.push('\\');
                }
            }
        } else if ch == '|' && !seen_cursor {
            seen_cursor = true;
        } else {
            target.push(ch);
        }
    }

    (before, after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_61_rules_from_conf() {
        let conf_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("typing-rules.conf");
        let engine = TypingRuleEngine::from_conf_file(&conf_path).unwrap();
        assert_eq!(engine.rules.len(), 61, "Expected all 61 rules to load");

        // Verify chained biconditionals & HTML comment restoration:
        // Typing `<!-` -> `↚`, then `-` -> `<!--`, or typing `<->` -> `↔` (`<-` -> `←`, then `>` -> `↔`)
        let (out, _) = engine.simulate_typing("<->");
        assert_eq!(out, "↔");

        let (out_comment, _) = engine.simulate_typing("<!--");
        assert_eq!(out_comment, "<!--");

        let (out_neg_bicond, _) = engine.simulate_typing("<!->");
        assert_eq!(out_neg_bicond, "↮");

        // Verify escaped pipe rules (`|-` -> `⊢`, `|=` -> `⊨`, `!|-` -> `⊬`)
        let (out_turnstile, _) = engine.simulate_typing("|-");
        assert_eq!(out_turnstile, "⊢");

        let (out_not_turnstile, _) = engine.simulate_typing("!|-");
        assert_eq!(out_not_turnstile, "⊬");

        let (out_models, _) = engine.simulate_typing("|=");
        assert_eq!(out_models, "⊨");

        // Verify backslash commands (`\forall` -> `∀`, `\downarrow` -> `↓`)
        let (out_forall, _) = engine.simulate_typing("\\forall");
        assert_eq!(out_forall, "∀");

        let (out_nor, _) = engine.simulate_typing("\\downarrow");
        assert_eq!(out_nor, "↓");

        // Verify equivalence and non-equivalence (`===` -> `≡`, `!=` -> `≠`, `≠=` -> `≢`)
        let (out_equiv, _) = engine.simulate_typing("===");
        assert_eq!(out_equiv, "≡");

        let (out_nequiv, _) = engine.simulate_typing("!==");
        assert_eq!(out_nequiv, "≢");
    }
}
