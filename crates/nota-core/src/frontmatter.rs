use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoteFrontmatter {
    pub has_frontmatter: bool,
    pub parse_error: Option<String>,
    pub title: Option<String>,
    pub has_tags_key: bool,
    pub tags: Vec<String>,
    pub has_aliases_key: bool,
    pub raw_aliases: Vec<String>,
    pub clean_aliases: Vec<String>,
    pub has_placeholder_alias: bool,
    pub flashcards: bool,
    pub deck: Option<String>,
    pub body_byte_offset: usize,
    pub body_start_line: usize,
}

pub fn parse_frontmatter(raw_markdown: &str) -> NoteFrontmatter {
    let mut fm = NoteFrontmatter {
        body_start_line: 1,
        ..Default::default()
    };

    // Check if the file starts with `---\n` or `---\r\n`
    let after_open = if let Some(rest) = raw_markdown.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = raw_markdown.strip_prefix("---\r\n") {
        rest
    } else {
        return fm;
    };

    let open_len = raw_markdown.len() - after_open.len();
    let mut current_offset = open_len;
    let mut line_count = 1usize;
    let mut close_span: Option<(usize, usize)> = None;

    for line in after_open.split_inclusive('\n') {
        line_count += 1;
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]).trim_end();
        if trimmed == "---" || trimmed == "..." {
            close_span = Some((current_offset, current_offset + line.len()));
            break;
        }
        current_offset += line.len();
    }

    let Some((yaml_end, body_start)) = close_span else {
        return fm;
    };

    fm.has_frontmatter = true;
    fm.body_byte_offset = body_start;
    fm.body_start_line = line_count + 1;

    let yaml_slice = &raw_markdown[open_len..yaml_end];
    match serde_yaml::from_str::<Value>(yaml_slice) {
        Ok(Value::Mapping(map)) => {
            if let Some(title_val) = map.get(Value::String("title".to_string())) {
                if let Some(s) = value_to_string(title_val) {
                    if !s.trim().is_empty() {
                        fm.title = Some(s);
                    }
                }
            }

            if let Some(tags_val) = map.get(Value::String("tags".to_string())) {
                fm.has_tags_key = true;
                fm.tags = extract_string_list(tags_val)
                    .into_iter()
                    .filter(|t| !t.trim().is_empty() && t.trim() != "[]")
                    .collect();
            }

            if let Some(aliases_val) = map.get(Value::String("aliases".to_string())) {
                fm.has_aliases_key = true;
                let raw_list = extract_string_list(aliases_val);
                for item in raw_list {
                    let trimmed = item.trim();
                    if trimmed == "[]" {
                        fm.has_placeholder_alias = true;
                        fm.raw_aliases.push(item);
                    } else if !trimmed.is_empty() {
                        fm.raw_aliases.push(item.clone());
                        fm.clean_aliases.push(trimmed.to_string());
                    }
                }
            }

            if let Some(fc_val) = map.get(Value::String("flashcards".to_string())) {
                fm.flashcards = match fc_val {
                    Value::Bool(b) => *b,
                    Value::String(s) => s.eq_ignore_ascii_case("true") || s == "1",
                    _ => false,
                };
            }

            if let Some(deck_val) = map.get(Value::String("deck".to_string())) {
                if let Some(s) = value_to_string(deck_val) {
                    if !s.trim().is_empty() {
                        fm.deck = Some(s.trim().to_string());
                        fm.flashcards = true;
                    }
                }
            }
        }
        Ok(_) => {}
        Err(err) => {
            fm.parse_error = Some(err.to_string());
        }
    }

    fm
}

fn value_to_string(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn extract_string_list(val: &Value) -> Vec<String> {
    match val {
        Value::Sequence(seq) => seq
            .iter()
            .map(|item| match item {
                Value::String(s) => s.clone(),
                Value::Sequence(inner) if inner.is_empty() => "[]".to_string(),
                other => value_to_string(other).unwrap_or_default(),
            })
            .collect(),
        Value::String(s) => {
            if s.trim().is_empty() {
                Vec::new()
            } else {
                vec![s.clone()]
            }
        }
        Value::Null => Vec::new(),
        _ => Vec::new(),
    }
}
