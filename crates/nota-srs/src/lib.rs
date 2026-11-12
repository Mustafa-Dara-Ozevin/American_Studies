use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use fsrs::{FSRS, MemoryState};
use nota_logic::{
    are_equivalent, check_entailment, classify_decomposition, classify_formula,
    generate_markdown_truth_table, parse_formula, parse_metalogic_stmt, BinOp, DecompositionRule,
    Formula, MetalogicStmt, TruthTree,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use ulid::Ulid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardKind {
    Basic,
    ReversedForward,
    ReversedBackward,
    Cloze { index: u32, hint: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flashcard {
    pub id: String,
    pub has_explicit_id: bool,
    pub note_path: String,
    pub deck: String,
    pub line: usize,
    pub kind: CardKind,
    pub front: String,
    pub back: String,
    pub obsidian_sr: Option<ObsidianSrSchedule>,
}

/// Represents Obsidian Spaced Repetition's inline `<!--SR:!2026-11-02,4,250-->` comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsidianSrSchedule {
    pub due_date: String,
    pub interval_days: u32,
    pub ease: u32,
}

impl ObsidianSrSchedule {
    pub fn parse_comment(comment: &str) -> Option<Self> {
        let inner = comment
            .trim()
            .strip_prefix("<!--SR:")?
            .strip_suffix("-->")?
            .trim();
        let entry = inner.split('!').nth(1)?;
        let mut parts = entry.split(',');
        let due_date = parts.next()?.trim().to_string();
        let interval_days = parts.next()?.trim().parse::<u32>().ok()?;
        let ease = parts.next()?.trim().parse::<u32>().ok()?;
        Some(Self {
            due_date,
            interval_days,
            ease,
        })
    }

    pub fn to_comment(&self) -> String {
        format!(
            "<!--SR:!{},{},{}-->",
            self.due_date, self.interval_days, self.ease
        )
    }

    /// Converts SM-2 `(interval_days, ease)` to initial FSRS `(stability, difficulty)`.
    pub fn to_fsrs_memory_state(&self) -> MemoryState {
        let stability = (self.interval_days as f32).max(0.5);
        // Map SM-2 ease [130..300] (default 250) to FSRS difficulty [1.0..10.0]
        let clamped_ease = (self.ease as f32).clamp(130.0, 320.0);
        let difficulty = (10.0 - ((clamped_ease - 130.0) / 190.0) * 8.0).clamp(1.0, 10.0);
        MemoryState {
            stability,
            difficulty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardState {
    New,
    Learning,
    Review,
    Relearning,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewEvent {
    pub card: String,
    #[serde(default)]
    pub note: String,
    /// Rating: 1 = Again, 2 = Hard, 3 = Good, 4 = Easy
    pub rating: u8,
    pub at: DateTime<Utc>,
    pub elapsed_days: u32,
    pub scheduled_days: u32,
    pub due: String,
    pub state: CardState,
    pub stability: f32,
    pub difficulty: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardScheduleEntry {
    pub note: String,
    pub due: String,
    pub state: CardState,
    pub stability: f32,
    pub difficulty: f32,
    pub reps: u32,
    pub lapses: u32,
    pub last_review: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleSnapshot {
    pub version: u32,
    pub cards: BTreeMap<String, CardScheduleEntry>,
}

impl Default for ScheduleSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            cards: BTreeMap::new(),
        }
    }
}

/// Extracts flashcards from a Markdown note without modifying the file.
/// - Single-line `Front::Back`, `Front:::Back`, and `{{c1::...}}` clozes are parsed outside code fences/tables.
/// - Multi-line `?` / `??` blocks are parsed when `opt_in_multiline` is true (`flashcards: true`, `deck: ...`, or `#flashcards`).
pub fn extract_cards_from_note(
    note_path: &str,
    raw_markdown: &str,
    body_byte_offset: usize,
    body_start_line: usize,
    default_deck: &str,
    opt_in_multiline: bool,
) -> Vec<Flashcard> {
    let body = &raw_markdown[body_byte_offset..];
    let lines: Vec<&str> = body.lines().collect();
    let mut cards = Vec::new();
    let mut in_fence = false;
    let mut i = 0;

    while i < lines.len() {
        let line_no = body_start_line + i;
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            i += 1;
            continue;
        }
        if in_fence || trimmed.is_empty() || trimmed.starts_with('|') {
            i += 1;
            continue;
        }

        // Look ahead for optional `<!--c:ID-->` and `<!--SR:...-->` on current or immediately following lines
        let (inline_id, inline_sr, cleaned_line) = extract_card_comments(trimmed);
        let next_line_comments = if i + 1 < lines.len() {
            let (nid, nsr, rem) = extract_card_comments(lines[i + 1].trim());
            if rem.is_empty() && (nid.is_some() || nsr.is_some()) {
                Some((nid, nsr))
            } else {
                None
            }
        } else {
            None
        };

        let explicit_id = inline_id
            .clone()
            .or_else(|| next_line_comments.as_ref().and_then(|(id, _)| id.clone()));
        let sr_sched = inline_sr.or_else(|| next_line_comments.as_ref().and_then(|(_, sr)| sr.clone()));

        // 1. Cloze deletions `{{c1::...}}`
        let cloze_indices = find_cloze_indices(&cleaned_line);
        if !cloze_indices.is_empty() {
            for (c_idx, hint) in cloze_indices {
                let (front, back) = render_cloze_pair(&cleaned_line, c_idx);
                let (id, has_explicit) = resolve_card_id(
                    explicit_id.as_deref(),
                    note_path,
                    &format!("c{}:{}", c_idx, front),
                );
                cards.push(Flashcard {
                    id,
                    has_explicit_id: has_explicit,
                    note_path: note_path.to_string(),
                    deck: default_deck.to_string(),
                    line: line_no,
                    kind: CardKind::Cloze { index: c_idx, hint },
                    front,
                    back,
                    obsidian_sr: sr_sched.clone(),
                });
            }
            i += if next_line_comments.is_some() { 2 } else { 1 };
            continue;
        }

        // 2. Bidirectional single-line `Front:::Back`
        if let Some((lhs, rhs)) = split_card_separator(&cleaned_line, ":::") {
            let (id_fwd, has_exp) =
                resolve_card_id(explicit_id.as_deref(), note_path, &format!("fwd:{}", lhs));
            let (id_rev, _) = resolve_card_id(
                explicit_id.as_ref().map(|s| format!("{}-rev", s)).as_deref(),
                note_path,
                &format!("rev:{}", rhs),
            );
            cards.push(Flashcard {
                id: id_fwd,
                has_explicit_id: has_exp,
                note_path: note_path.to_string(),
                deck: default_deck.to_string(),
                line: line_no,
                kind: CardKind::ReversedForward,
                front: lhs.to_string(),
                back: rhs.to_string(),
                obsidian_sr: sr_sched.clone(),
            });
            cards.push(Flashcard {
                id: id_rev,
                has_explicit_id: has_exp,
                note_path: note_path.to_string(),
                deck: default_deck.to_string(),
                line: line_no,
                kind: CardKind::ReversedBackward,
                front: rhs.to_string(),
                back: lhs.to_string(),
                obsidian_sr: sr_sched,
            });
            i += if next_line_comments.is_some() { 2 } else { 1 };
            continue;
        }

        // 3. Basic single-line `Front::Back`
        if let Some((lhs, rhs)) = split_card_separator(&cleaned_line, "::") {
            let (id, has_exp) = resolve_card_id(explicit_id.as_deref(), note_path, lhs);
            cards.push(Flashcard {
                id,
                has_explicit_id: has_exp,
                note_path: note_path.to_string(),
                deck: default_deck.to_string(),
                line: line_no,
                kind: CardKind::Basic,
                front: lhs.to_string(),
                back: rhs.to_string(),
                obsidian_sr: sr_sched,
            });
            i += if next_line_comments.is_some() { 2 } else { 1 };
            continue;
        }

        // 4. Multi-line `?` or `??` cards (in opted-in flashcard notes)
        if opt_in_multiline && i + 2 < lines.len() {
            let sep = lines[i + 1].trim();
            if sep == "?" || sep == "??" {
                let front = cleaned_line;
                let mut back_lines = Vec::new();
                let mut j = i + 2;
                let mut block_id = explicit_id;
                let mut block_sr = sr_sched;
                while j < lines.len() && !lines[j].trim().is_empty() {
                    let (cid, csr, rem) = extract_card_comments(lines[j].trim());
                    if cid.is_some() {
                        block_id = cid;
                    }
                    if csr.is_some() {
                        block_sr = csr;
                    }
                    if !rem.is_empty() {
                        back_lines.push(rem);
                    }
                    j += 1;
                }
                if !back_lines.is_empty() {
                    let back = back_lines.join("\n");
                    let (id, has_exp) =
                        resolve_card_id(block_id.as_deref(), note_path, &front);
                    cards.push(Flashcard {
                        id,
                        has_explicit_id: has_exp,
                        note_path: note_path.to_string(),
                        deck: default_deck.to_string(),
                        line: line_no,
                        kind: if sep == "??" {
                            CardKind::ReversedForward
                        } else {
                            CardKind::Basic
                        },
                        front: front.clone(),
                        back: back.clone(),
                        obsidian_sr: block_sr.clone(),
                    });
                    if sep == "??" {
                        let (id_rev, _) = resolve_card_id(
                            block_id.as_ref().map(|s| format!("{}-rev", s)).as_deref(),
                            note_path,
                            &back,
                        );
                        cards.push(Flashcard {
                            id: id_rev,
                            has_explicit_id: has_exp,
                            note_path: note_path.to_string(),
                            deck: default_deck.to_string(),
                            line: line_no,
                            kind: CardKind::ReversedBackward,
                            front: back,
                            back: front,
                            obsidian_sr: block_sr,
                        });
                    }
                    i = j;
                    continue;
                }
            }
        }

        i += 1;
    }

    cards
}

/// Stamps missing `<!--c:ULID-->` comments onto single-line cards when explicitly requested (`nota cards stamp`).
pub fn stamp_missing_card_ids(raw_markdown: &str) -> (String, usize) {
    let mut out = String::with_capacity(raw_markdown.len() + 128);
    let mut stamped = 0usize;
    let mut in_fence = false;

    for line in raw_markdown.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if !in_fence && !trimmed.starts_with('|') {
            let (existing_id, _, cleaned) = extract_card_comments(trimmed);
            let is_card = !find_cloze_indices(&cleaned).is_empty()
                || split_card_separator(&cleaned, ":::").is_some()
                || split_card_separator(&cleaned, "::").is_some();
            if is_card && existing_id.is_none() {
                let new_id = Ulid::new().to_string();
                let line_no_nl = line.trim_end_matches(&['\r', '\n'][..]);
                let nl = &line[line_no_nl.len()..];
                out.push_str(&format!("{} <!--c:{}-->{}", line_no_nl, new_id, nl));
                stamped += 1;
                continue;
            }
        }
        out.push_str(line);
    }

    (out, stamped)
}

fn resolve_card_id(explicit: Option<&str>, note_path: &str, front: &str) -> (String, bool) {
    if let Some(id) = explicit {
        (id.to_string(), true)
    } else {
        let mut hasher = blake3::Hasher::new();
        hasher.update(note_path.as_bytes());
        hasher.update(b"::");
        hasher.update(front.trim().as_bytes());
        let hex = hasher.finalize().to_hex();
        (format!("v:{}", &hex[..10]), false)
    }
}

fn extract_card_comments(line: &str) -> (Option<String>, Option<ObsidianSrSchedule>, String) {
    let mut card_id = None;
    let mut sr_sched = None;
    let mut cleaned = line.to_string();

    while let Some(start) = cleaned.find("<!--") {
        let Some(end_rel) = cleaned[start + 4..].find("-->") else {
            break;
        };
        let end = start + 4 + end_rel + 3;
        let comment = &cleaned[start..end];
        if let Some(id_inner) = comment
            .strip_prefix("<!--c:")
            .and_then(|s| s.strip_suffix("-->"))
        {
            card_id = Some(id_inner.trim().to_string());
        } else if let Some(sr) = ObsidianSrSchedule::parse_comment(comment) {
            sr_sched = Some(sr);
        }
        cleaned.replace_range(start..end, "");
    }

    (card_id, sr_sched, cleaned.trim().to_string())
}

fn split_card_separator<'a>(line: &'a str, sep: &str) -> Option<(&'a str, &'a str)> {
    // Ignore inline code and URLs
    if line.contains('`') || line.contains("http://") || line.contains("https://") {
        return None;
    }
    let (lhs, rhs) = line.split_once(sep)?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    // Avoid matching `:::` inside `::` check
    if sep == "::" && (lhs.ends_with(':') || rhs.starts_with(':')) {
        return None;
    }
    Some((lhs, rhs))
}

fn find_cloze_indices(line: &str) -> Vec<(u32, Option<String>)> {
    let mut indices = Vec::new();
    let mut search_pos = 0;
    while let Some(rel_open) = line[search_pos..].find("{{c") {
        let open = search_pos + rel_open;
        let after_c = &line[open + 3..];
        let digits_len = after_c.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits_len > 0 && after_c[digits_len..].starts_with("::") {
            if let Ok(idx) = after_c[..digits_len].parse::<u32>() {
                let content_start = open + 3 + digits_len + 2;
                if let Some(rel_close) = line[content_start..].find("}}") {
                    let inner = &line[content_start..content_start + rel_close];
                    let hint = inner.split_once("::").map(|(_, h)| h.to_string());
                    if !indices.iter().any(|(existing, _)| *existing == idx) {
                        indices.push((idx, hint));
                    }
                    search_pos = content_start + rel_close + 2;
                    continue;
                }
            }
        }
        search_pos = open + 3;
    }
    indices
}

fn render_cloze_pair(line: &str, active_index: u32) -> (String, String) {
    let mut front = String::new();
    let mut back = String::new();
    let mut cursor = 0;

    while let Some(rel_open) = line[cursor..].find("{{c") {
        let open = cursor + rel_open;
        front.push_str(&line[cursor..open]);
        back.push_str(&line[cursor..open]);

        let after_c = &line[open + 3..];
        let digits_len = after_c.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits_len > 0 && after_c[digits_len..].starts_with("::") {
            if let Ok(idx) = after_c[..digits_len].parse::<u32>() {
                let content_start = open + 3 + digits_len + 2;
                if let Some(rel_close) = line[content_start..].find("}}") {
                    let inner = &line[content_start..content_start + rel_close];
                    let (answer, hint) = match inner.split_once("::") {
                        Some((a, h)) => (a, Some(h)),
                        None => (inner, None),
                    };
                    if idx == active_index {
                        match hint {
                            Some(h) => front.push_str(&format!("[...{}...]", h)),
                            None => front.push_str("[...]"),
                        }
                        back.push_str(&format!("**{}**", answer));
                    } else {
                        front.push_str(answer);
                        back.push_str(answer);
                    }
                    cursor = content_start + rel_close + 2;
                    continue;
                }
            }
        }
        front.push_str("{{c");
        back.push_str("{{c");
        cursor = open + 3;
    }
    front.push_str(&line[cursor..]);
    back.push_str(&line[cursor..]);
    (front, back)
}

pub struct FsrsScheduler {
    fsrs: FSRS,
    pub desired_retention: f32,
}

impl FsrsScheduler {
    pub fn new(desired_retention: f32) -> Result<Self> {
        let fsrs = FSRS::new(&[])
            .map_err(|e| anyhow!("Failed to initialize FSRS engine: {:?}", e))?;
        Ok(Self {
            fsrs,
            desired_retention,
        })
    }

    /// Schedules a review event for `card_id` with `rating` (`1=Again`, `2=Hard`, `3=Good`, `4=Easy`).
    pub fn record_review(
        &self,
        card_id: &str,
        note_path: &str,
        prev_entry: Option<&CardScheduleEntry>,
        rating: u8,
        now: DateTime<Utc>,
    ) -> Result<ReviewEvent> {
        if !(1..=4).contains(&rating) {
            return Err(anyhow!("Invalid FSRS rating {} (must be 1..=4)", rating));
        }

        let prev_memory = prev_entry.map(|e| MemoryState {
            stability: e.stability,
            difficulty: e.difficulty,
        });
        let elapsed_days = match prev_entry {
            Some(prev) => {
                let diff = now.signed_duration_since(prev.last_review).num_days();
                diff.max(0) as u32
            }
            None => 0,
        };

        let next_states = self
            .fsrs
            .next_states(prev_memory, self.desired_retention, elapsed_days)
            .map_err(|e| anyhow!("FSRS calculation error: {:?}", e))?;

        let chosen = match rating {
            1 => &next_states.again,
            2 => &next_states.hard,
            3 => &next_states.good,
            _ => &next_states.easy,
        };

        let scheduled_days = if rating == 1 {
            0
        } else {
            (chosen.interval.round() as i64).max(1) as u32
        };
        let state = match (prev_entry.map(|e| e.state), rating) {
            (None | Some(CardState::New), 1 | 2) => CardState::Learning,
            (_, 1) => CardState::Relearning,
            _ => CardState::Review,
        };
        let due_date = (now + Duration::days(scheduled_days as i64))
            .format("%Y-%m-%d")
            .to_string();

        Ok(ReviewEvent {
            card: card_id.to_string(),
            note: note_path.to_string(),
            rating,
            at: now,
            elapsed_days,
            scheduled_days,
            due: due_date,
            state,
            stability: chosen.memory.stability,
            difficulty: chosen.memory.difficulty,
        })
    }
}

/// Reads `revlog.jsonl`, deduplicates by `(at, card)` for conflict-free git merges,
/// and deterministically materializes `ScheduleSnapshot`.
pub fn materialize_schedule_from_events(events: &[ReviewEvent]) -> ScheduleSnapshot {
    let mut sorted: Vec<ReviewEvent> = events.to_vec();
    sorted.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.card.cmp(&b.card)));
    sorted.dedup_by(|a, b| a.at == b.at && a.card == b.card);

    let mut snapshot = ScheduleSnapshot::default();
    for ev in sorted {
        let entry = snapshot
            .cards
            .entry(ev.card.clone())
            .or_insert_with(|| CardScheduleEntry {
                note: ev.note.clone(),
                due: ev.due.clone(),
                state: ev.state,
                stability: ev.stability,
                difficulty: ev.difficulty,
                reps: 0,
                lapses: 0,
                last_review: ev.at,
            });
        if !ev.note.is_empty() {
            entry.note = ev.note.clone();
        }
        entry.due = ev.due;
        entry.state = ev.state;
        entry.stability = ev.stability;
        entry.difficulty = ev.difficulty;
        entry.reps += 1;
        if ev.rating == 1 {
            entry.lapses += 1;
        }
        entry.last_review = ev.at;
    }

    snapshot
}

pub fn load_revlog(path: impl AsRef<Path>) -> Result<Vec<ReviewEvent>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read revlog at {}", path.display()))?;
    let mut events = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("<<<<<<<")
            || trimmed.starts_with("=======")
            || trimmed.starts_with(">>>>>>>")
        {
            continue;
        }
        let ev: ReviewEvent = serde_json::from_str(trimmed)
            .with_context(|| format!("Invalid JSONL on line {} of {}", idx + 1, path.display()))?;
        events.push(ev);
    }
    Ok(events)
}

/// Logic-specific flashcard generators powered by `nota-logic`.
pub fn generate_logic_flashcards_for_formula(formula: &Formula) -> Result<Vec<(String, String)>> {
    let mut pairs = Vec::new();

    // 1. Formula classification card
    let class = classify_formula(formula)?;
    let class_str = match class {
        nota_logic::FormulaClass::Tautology => "Tautology (Totoloji — her değerde D)",
        nota_logic::FormulaClass::Contradiction => "Contradiction (Çelişki — her değerde Y)",
        nota_logic::FormulaClass::Contingent => "Contingent (Olumsal — bazı değerlerde D, bazılarında Y)",
    };
    pairs.push((
        format!("Classify the formula: `{}`", formula),
        class_str.to_string(),
    ));

    // 2. Truth table card
    let table_md =
        generate_markdown_truth_table(std::slice::from_ref(formula), true, "D", "Y")?;
    pairs.push((
        format!("Construct the truth table for `{}`", formula),
        table_md,
    ));

    // 3. Truth Tree decomposition step card
    match classify_decomposition(formula) {
        DecompositionRule::Alpha { rule_name, stacked } => {
            let children = stacked
                .iter()
                .map(|f| format!("`{}`", f))
                .collect::<Vec<_>>()
                .join(", ");
            pairs.push((
                format!(
                    "In a Truth Tree, how does `{}` decompose (Stacking α vs Branching β)?",
                    formula
                ),
                format!(
                    "**Stacking (α-rule `{}`)**: stacks {} vertically onto every open leaf.",
                    rule_name, children
                ),
            ));
        }
        DecompositionRule::Beta {
            rule_name,
            left_stacked,
            right_stacked,
        } => {
            let left_str = left_stacked
                .iter()
                .map(|f| format!("`{}`", f))
                .collect::<Vec<_>>()
                .join(", ");
            let right_str = right_stacked
                .iter()
                .map(|f| format!("`{}`", f))
                .collect::<Vec<_>>()
                .join(", ");
            pairs.push((
                format!(
                    "In a Truth Tree, how does `{}` decompose (Stacking α vs Branching β)?",
                    formula
                ),
                format!(
                    "**Branching (β-rule `{}`)**: splits every open leaf into Left ({}) | Right ({}).",
                    rule_name, left_str, right_str
                ),
            ));
        }
        _ => {}
    }

    // 4. Full Truth Tree card
    let mut tree = TruthTree::from_formulas(std::slice::from_ref(formula));
    let _ = tree.solve_alpha_first(50);
    pairs.push((
        format!("Solve the Truth Tree for `{}`", formula),
        format!("```tree\n{}```", tree.to_tree_dsl()),
    ));

    // 5. De Morgan / Negated Binary Equivalence card (if applicable)
    if let Formula::Not(inner) = formula {
        if let Formula::Binary { op, lhs, rhs } = inner.as_ref() {
            let equiv_opt = match op {
                BinOp::And => Some(Formula::binary(
                    BinOp::Or,
                    Formula::negate((**lhs).clone()),
                    Formula::negate((**rhs).clone()),
                )),
                BinOp::Or => Some(Formula::binary(
                    BinOp::And,
                    Formula::negate((**lhs).clone()),
                    Formula::negate((**rhs).clone()),
                )),
                BinOp::Implies => Some(Formula::binary(
                    BinOp::And,
                    (**lhs).clone(),
                    Formula::negate((**rhs).clone()),
                )),
                _ => None,
            };
            if let Some(eq_f) = equiv_opt {
                pairs.push((
                    format!("Push negation inward (De Morgan / NNF): `{}` ≡ ?", formula),
                    format!("`{}`", eq_f),
                ));
            }
        }
    }

    Ok(pairs)
}

/// Generates flashcards for any logic input: either a metalogic statement (`P1, P2 ∴ C` or `A === B`)
/// or a single formula (`(p -> q) <-> (~p v q)`).
pub fn generate_logic_flashcards_for_input(input: &str) -> Result<Vec<(String, String)>> {
    if let Ok(stmt) = parse_metalogic_stmt(input) {
        let mut pairs = Vec::new();
        match stmt {
            MetalogicStmt::Entailment {
                premises,
                conclusion,
                ..
            } => {
                let prem_str = premises
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let goal_str = format!("{} ∴ {}", prem_str, conclusion);
                let ent = check_entailment(&premises, &conclusion)?;
                if ent.is_valid {
                    pairs.push((
                        format!("Is the argument `{}` valid?", goal_str),
                        "**Valid (Geçerli)** — every valuation where the premises are D also makes the conclusion D.".to_string(),
                    ));
                } else {
                    let ce_str = ent
                        .counterexamples
                        .first()
                        .map(|m| {
                            m.iter()
                                .map(|(k, &v)| format!("{}={}", k, if v { "D" } else { "Y" }))
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    pairs.push((
                        format!("Is the argument `{}` valid?", goal_str),
                        format!("**Invalid (Geçersiz)** — Counterexample: `{}`", ce_str),
                    ));
                    pairs.push((
                        format!("Find a counterexample valuation for `{}`", goal_str),
                        format!("`{}` (makes premises D and conclusion Y)", ce_str),
                    ));
                }
                let mut tree = TruthTree::from_entailment(&premises, &conclusion);
                let _ = tree.solve_alpha_first(100);
                pairs.push((
                    format!("Solve the Truth Tree for `{}`", goal_str),
                    format!("```tree\n{}```", tree.to_tree_dsl()),
                ));
            }
            MetalogicStmt::Equivalence(lhs, rhs) | MetalogicStmt::NonEquivalence(lhs, rhs) => {
                let eq = are_equivalent(&lhs, &rhs)?;
                pairs.push((
                    format!("Are `{}` and `{}` logically equivalent (`≡`)?", lhs, rhs),
                    if eq {
                        format!("**Yes (`{} ≡ {}`)** — identical truth values on all rows.", lhs, rhs)
                    } else {
                        format!("**No (`{} ≢ {}`)** — they differ on at least one valuation.", lhs, rhs)
                    },
                ));
                if eq {
                    pairs.push((
                        format!("Logical equivalence for `{}`", lhs),
                        format!("`{}`", rhs),
                    ));
                }
            }
            MetalogicStmt::FormulaSet(formulas) => {
                let mut tree = TruthTree::from_formulas(&formulas);
                let _ = tree.solve_alpha_first(100);
                let list_str = formulas
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                pairs.push((
                    format!("Test consistency of `{{{}}}` using a Truth Tree", list_str),
                    format!("```tree\n{}```", tree.to_tree_dsl()),
                ));
            }
            MetalogicStmt::Single(f) => {
                return generate_logic_flashcards_for_formula(&f);
            }
        }
        return Ok(pairs);
    }

    let formula = parse_formula(input)?;
    generate_logic_flashcards_for_formula(&formula)
}

pub const DEFAULT_LEECH_THRESHOLD: u32 = 8;

impl CardScheduleEntry {
    pub fn is_leech(&self, threshold: u32) -> bool {
        self.lapses >= threshold
    }
}

/// Builds the daily review queue with sibling burying (at most 1 card per `(note_path, line)` in a session),
/// deck filtering, suspended card exclusion, and daily new/review limits.
pub fn build_daily_review_queue(
    all_cards: &[Flashcard],
    schedule: &ScheduleSnapshot,
    today_yyyy_mm_dd: &str,
    deck_filter: Option<&str>,
    bury_siblings: bool,
    max_new: usize,
    max_reviews: usize,
) -> Vec<Flashcard> {
    let mut queue = Vec::new();
    let mut seen_sibling_sources: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut new_count = 0usize;
    let mut review_count = 0usize;

    for card in all_cards {
        if let Some(df) = deck_filter {
            if !df.is_empty() && card.deck != df {
                continue;
            }
        }
        let entry = schedule.cards.get(&card.id);
        if let Some(e) = entry {
            if e.state == CardState::Suspended {
                continue;
            }
            if e.due.as_str() > today_yyyy_mm_dd {
                continue;
            }
            if review_count >= max_reviews {
                continue;
            }
        } else if new_count >= max_new {
            continue;
        }

        if bury_siblings {
            let key = (card.note_path.clone(), card.line);
            if !seen_sibling_sources.insert(key) {
                continue;
            }
        }

        if entry.is_some() {
            review_count += 1;
        } else {
            new_count += 1;
        }
        queue.push(card.clone());
    }

    queue
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardBrowserFilter {
    All,
    Due,
    New,
    Suspended,
    Leeches,
    Deck(String),
}

pub fn filter_cards_for_browser(
    all_cards: &[Flashcard],
    schedule: &ScheduleSnapshot,
    filter: &CardBrowserFilter,
    search_query: &str,
    today_yyyy_mm_dd: &str,
) -> Vec<Flashcard> {
    let q = search_query.trim().to_lowercase();
    all_cards
        .iter()
        .filter(|card| {
            if !q.is_empty()
                && !card.front.to_lowercase().contains(&q)
                && !card.back.to_lowercase().contains(&q)
                && !card.note_path.to_lowercase().contains(&q)
                && !card.deck.to_lowercase().contains(&q)
            {
                return false;
            }
            let entry = schedule.cards.get(&card.id);
            match filter {
                CardBrowserFilter::All => true,
                CardBrowserFilter::Due => entry
                    .map(|e| e.state != CardState::Suspended && e.due.as_str() <= today_yyyy_mm_dd)
                    .unwrap_or(true),
                CardBrowserFilter::New => entry.is_none() || entry.is_some_and(|e| e.state == CardState::New),
                CardBrowserFilter::Suspended => entry.is_some_and(|e| e.state == CardState::Suspended),
                CardBrowserFilter::Leeches => {
                    entry.is_some_and(|e| e.is_leech(DEFAULT_LEECH_THRESHOLD))
                }
                CardBrowserFilter::Deck(d) => &card.deck == d,
            }
        })
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SrsStatistics {
    pub total_cards: usize,
    pub due_today: usize,
    pub new_cards: usize,
    pub learning_cards: usize,
    pub review_cards: usize,
    pub suspended_cards: usize,
    pub leech_cards: usize,
    pub total_reviews_logged: usize,
    pub retention_rate_percent: f32,
    pub forecast_next_30_days: Vec<(String, usize)>,
    pub per_deck_counts: BTreeMap<String, (usize, usize)>,
}

pub fn compute_srs_statistics(
    all_cards: &[Flashcard],
    schedule: &ScheduleSnapshot,
    events: &[ReviewEvent],
    now: DateTime<Utc>,
) -> SrsStatistics {
    let today_str = now.format("%Y-%m-%d").to_string();
    let mut stats = SrsStatistics {
        total_cards: all_cards.len(),
        total_reviews_logged: events.len(),
        ..Default::default()
    };

    for card in all_cards {
        let deck_entry = stats
            .per_deck_counts
            .entry(card.deck.clone())
            .or_insert((0, 0));
        deck_entry.0 += 1;

        match schedule.cards.get(&card.id) {
            None => {
                stats.new_cards += 1;
                stats.due_today += 1;
                deck_entry.1 += 1;
            }
            Some(entry) => {
                if entry.is_leech(DEFAULT_LEECH_THRESHOLD) {
                    stats.leech_cards += 1;
                }
                match entry.state {
                    CardState::New => stats.new_cards += 1,
                    CardState::Learning | CardState::Relearning => stats.learning_cards += 1,
                    CardState::Review => stats.review_cards += 1,
                    CardState::Suspended => stats.suspended_cards += 1,
                }
                if entry.state != CardState::Suspended && entry.due.as_str() <= today_str.as_str() {
                    stats.due_today += 1;
                    deck_entry.1 += 1;
                }
            }
        }
    }

    if !events.is_empty() {
        let passed = events.iter().filter(|e| e.rating >= 2).count();
        stats.retention_rate_percent = (passed as f32 / events.len() as f32) * 100.0;
    } else {
        stats.retention_rate_percent = 100.0;
    }

    for day_offset in 0..30 {
        let date_str = (now + Duration::days(day_offset))
            .format("%Y-%m-%d")
            .to_string();
        let count = all_cards
            .iter()
            .filter(|c| {
                schedule
                    .cards
                    .get(&c.id)
                    .is_some_and(|e| e.state != CardState::Suspended && e.due == date_str)
            })
            .count();
        stats.forecast_next_30_days.push((date_str, count));
    }

    stats
}

/// Imports inline Obsidian SR `<!--SR:!due,interval,ease-->` comments into `ScheduleSnapshot`
/// for cards not yet tracked in `schedule.json`.
pub fn import_obsidian_sr_to_schedule(
    cards: &[Flashcard],
    schedule: &mut ScheduleSnapshot,
    now: DateTime<Utc>,
) -> usize {
    let mut imported = 0usize;
    for card in cards {
        if schedule.cards.contains_key(&card.id) {
            continue;
        }
        if let Some(ref sr) = card.obsidian_sr {
            let mem = sr.to_fsrs_memory_state();
            schedule.cards.insert(
                card.id.clone(),
                CardScheduleEntry {
                    note: card.note_path.clone(),
                    due: sr.due_date.clone(),
                    state: CardState::Review,
                    stability: mem.stability,
                    difficulty: mem.difficulty,
                    reps: 1,
                    lapses: 0,
                    last_review: now,
                },
            );
            imported += 1;
        }
    }
    imported
}

/// Converts a `CardScheduleEntry` back to an Obsidian SR `<!--SR:!due,interval,ease-->` struct.
pub fn export_schedule_to_obsidian_sr(entry: &CardScheduleEntry) -> ObsidianSrSchedule {
    let interval_days = (entry.stability.round() as i64).max(1) as u32;
    let ease = (320.0 - ((entry.difficulty.clamp(1.0, 10.0) - 1.0) / 9.0) * 190.0)
        .round()
        .clamp(130.0, 320.0) as u32;
    ObsidianSrSchedule {
        due_date: entry.due.clone(),
        interval_days,
        ease,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_card_extraction_and_stamping() {
        let md = r#"---
title: Logic Study
flashcards: true
deck: logic
---
Modus Ponens::If p → q and p, then q. <!--c:01J8Z3K4Q7-->
p → q, p ⊢ q:::Modus Ponens
Modus Tollens: p → q, {{c1::¬q}} ⊢ {{c2::¬p}}.

What is the difference between validity and truth?
?
Validity concerns form; truth concerns content.
<!--SR:!2026-11-02,4,250-->
"#;
        let fm = nota_core::parse_frontmatter(md);
        let cards = extract_cards_from_note(
            "Logic Study.md",
            md,
            fm.body_byte_offset,
            fm.body_start_line,
            fm.deck.as_deref().unwrap_or("default"),
            fm.flashcards,
        );
        // 1 basic + 2 reversed + 2 clozes + 1 multiline = 6 cards
        assert_eq!(cards.len(), 6);
        assert_eq!(cards[0].id, "01J8Z3K4Q7");
        assert!(cards[0].has_explicit_id);
        assert_eq!(cards[5].obsidian_sr.as_ref().unwrap().interval_days, 4);

        let (stamped_md, stamped_count) = stamp_missing_card_ids(md);
        assert_eq!(stamped_count, 2); // stamps the `:::` line and the `{{c1::}}` line
        assert!(stamped_md.contains("<!--c:"));
    }

    #[test]
    fn test_fsrs_schedule_replay_determinism() {
        let scheduler = FsrsScheduler::new(0.90).unwrap();
        let t0 = Utc.with_ymd_and_hms(2026, 10, 31, 10, 0, 0).unwrap();
        let ev1 = scheduler
            .record_review("01J8Z3K4Q7", "Logic.md", None, 3, t0)
            .unwrap();

        let snap1 = materialize_schedule_from_events(std::slice::from_ref(&ev1));
        let entry1 = snap1.cards.get("01J8Z3K4Q7").unwrap();
        assert_eq!(entry1.reps, 1);

        let t1 = t0 + Duration::days(ev1.scheduled_days as i64);
        let ev2 = scheduler
            .record_review("01J8Z3K4Q7", "Logic.md", Some(entry1), 3, t1)
            .unwrap();

        // Replay in reverse order and with duplicates to test merge safety:
        let snap_merged =
            materialize_schedule_from_events(&[ev2.clone(), ev1.clone(), ev2.clone()]);
        let snap_clean = materialize_schedule_from_events(&[ev1, ev2]);
        assert_eq!(snap_merged, snap_clean);
    }
}
