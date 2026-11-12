use crate::ast::{BinOp, Formula};
use crate::parser::{clean_formula_input, parse_formula};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormulaClass {
    Tautology,
    Contradiction,
    Contingent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntailmentResult {
    pub is_valid: bool,
    pub counterexamples: Vec<BTreeMap<String, bool>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistencyResult {
    pub is_consistent: bool,
    pub models: Vec<BTreeMap<String, bool>>,
}

pub fn eval_formula(formula: &Formula, env: &BTreeMap<String, bool>) -> Result<bool> {
    match formula {
        Formula::Const(b) => Ok(*b),
        Formula::Atom(name) => env
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("Unbound propositional atom '{}'", name)),
        Formula::Predicate { name, args } => {
            let key = if args.is_empty() {
                name.clone()
            } else {
                format!("{}({})", name, args.join(","))
            };
            env.get(&key)
                .copied()
                .ok_or_else(|| anyhow!("Unbound predicate '{}'", key))
        }
        Formula::Not(inner) => Ok(!eval_formula(inner, env)?),
        Formula::Binary { op, lhs, rhs } => {
            let l = eval_formula(lhs, env)?;
            let r = eval_formula(rhs, env)?;
            Ok(op.eval(l, r))
        }
        Formula::Quantified { .. } => Err(anyhow!(
            "Quantified formula '{}' cannot be evaluated in a propositional truth table",
            formula
        )),
    }
}

/// Generates all $2^n$ truth assignments for `atoms` (ordered `D`/`true` first, matching the vault convention:
/// row 0 = `D D ... D`, last row = `Y Y ... Y`).
pub fn all_valuations(atoms: &[String]) -> Vec<BTreeMap<String, bool>> {
    let n = atoms.len();
    let total = 1usize << n;
    let mut out = Vec::with_capacity(total);
    for mask in 0..total {
        let mut env = BTreeMap::new();
        for (i, atom) in atoms.iter().enumerate() {
            // Bit 0 (MSB) = true first (`mask` bit is 0 -> true, 1 -> false)
            let bit = (mask >> (n - 1 - i)) & 1;
            env.insert(atom.clone(), bit == 0);
        }
        out.push(env);
    }
    out
}

pub fn classify_formula(formula: &Formula) -> Result<FormulaClass> {
    let atoms = formula.atoms_alphabetical();
    let mut any_true = false;
    let mut any_false = false;
    for env in all_valuations(&atoms) {
        if eval_formula(formula, &env)? {
            any_true = true;
        } else {
            any_false = true;
        }
    }
    Ok(match (any_true, any_false) {
        (true, false) => FormulaClass::Tautology,
        (false, true) => FormulaClass::Contradiction,
        _ => FormulaClass::Contingent,
    })
}

pub fn are_equivalent(lhs: &Formula, rhs: &Formula) -> Result<bool> {
    let mut atoms_set = BTreeSet::new();
    lhs.collect_atoms(&mut atoms_set);
    rhs.collect_atoms(&mut atoms_set);
    let atoms: Vec<String> = atoms_set.into_iter().collect();

    for env in all_valuations(&atoms) {
        if eval_formula(lhs, &env)? != eval_formula(rhs, &env)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn check_entailment(premises: &[Formula], conclusion: &Formula) -> Result<EntailmentResult> {
    let mut atoms_set = BTreeSet::new();
    for p in premises {
        p.collect_atoms(&mut atoms_set);
    }
    conclusion.collect_atoms(&mut atoms_set);
    let atoms: Vec<String> = atoms_set.into_iter().collect();

    let mut counterexamples = Vec::new();
    for env in all_valuations(&atoms) {
        let mut all_premises_true = true;
        for p in premises {
            if !eval_formula(p, &env)? {
                all_premises_true = false;
                break;
            }
        }
        if all_premises_true && !eval_formula(conclusion, &env)? {
            counterexamples.push(env);
        }
    }

    Ok(EntailmentResult {
        is_valid: counterexamples.is_empty(),
        counterexamples,
    })
}

pub fn check_consistency(formulas: &[Formula]) -> Result<ConsistencyResult> {
    let mut atoms_set = BTreeSet::new();
    for f in formulas {
        f.collect_atoms(&mut atoms_set);
    }
    let atoms: Vec<String> = atoms_set.into_iter().collect();

    let mut models = Vec::new();
    for env in all_valuations(&atoms) {
        let mut all_true = true;
        for f in formulas {
            if !eval_formula(f, &env)? {
                all_true = false;
                break;
            }
        }
        if all_true {
            models.push(env);
        }
    }

    Ok(ConsistencyResult {
        is_consistent: !models.is_empty(),
        models,
    })
}

/// Generates a Markdown truth table with canonical alphabetical atomic variable order.
pub fn generate_markdown_truth_table(
    formulas: &[Formula],
    packed_atoms: bool,
    true_sym: &str,
    false_sym: &str,
) -> Result<String> {
    let mut atoms_set = BTreeSet::new();
    for f in formulas {
        f.collect_atoms(&mut atoms_set);
    }
    let atoms: Vec<String> = atoms_set.into_iter().collect();
    let valuations = all_valuations(&atoms);

    let mut headers = Vec::new();
    if packed_atoms {
        headers.push(atoms.join(" "));
    } else {
        for a in &atoms {
            headers.push(a.clone());
        }
    }
    for f in formulas {
        headers.push(f.to_string());
    }

    let mut out = String::new();
    out.push_str("| ");
    out.push_str(&headers.join(" | "));
    out.push_str(" |\n| ");
    let sep: Vec<String> = headers
        .iter()
        .map(|h| "-".repeat(h.chars().count().max(3)))
        .collect();
    out.push_str(&sep.join(" | "));
    out.push_str(" |\n");

    for env in valuations {
        let mut row_cells = Vec::new();
        if packed_atoms {
            let vals: Vec<&str> = atoms
                .iter()
                .map(|a| if env[a] { true_sym } else { false_sym })
                .collect();
            row_cells.push(vals.join(" "));
        } else {
            for a in &atoms {
                row_cells.push(if env[a] { true_sym } else { false_sym }.to_string());
            }
        }
        for f in formulas {
            let v = eval_formula(f, &env)?;
            row_cells.push(if v { true_sym } else { false_sym }.to_string());
        }
        out.push_str("| ");
        out.push_str(&row_cells.join(" | "));
        out.push_str(" |\n");
    }

    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableIssueKind {
    NonAlphabeticalAtomHeader {
        written: Vec<String>,
        canonical: Vec<String>,
    },
    EmptyOrIncompleteTable,
    RowCountMismatch {
        expected: usize,
        actual: usize,
    },
    DuplicateValuationRow {
        row_index: usize,
        valuation: String,
    },
    NonCanonicalTruthSymbol {
        row_index: usize,
        col_index: usize,
        raw: String,
    },
    UnparseableColumnHeader {
        col_index: usize,
        header: String,
    },
    CellValueError {
        row_index: usize,
        col_index: usize,
        header: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableLintReport {
    pub is_truth_table: bool,
    pub start_line: usize,
    pub atoms: Vec<String>,
    pub issues: Vec<TableIssueKind>,
}

enum ColumnSpec {
    PackedFormulas(Vec<Formula>),
    SingleFormula(Formula),
    ShorthandIffOfPrevTwo,
    Unknown,
}

pub fn lint_markdown_table(
    headers: &[String],
    rows: &[Vec<String>],
    start_line: usize,
) -> TableLintReport {
    if headers.is_empty() {
        return TableLintReport {
            is_truth_table: false,
            start_line,
            atoms: Vec::new(),
            issues: Vec::new(),
        };
    }

    // Check if the first header is a space-separated list of atomic identifiers (e.g. `p q`, `p q r`, `A B`)
    let first_cleaned = clean_formula_input(&headers[0]);
    let written_atoms: Vec<String> = first_cleaned
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    if written_atoms.is_empty()
        || !written_atoms.iter().all(|tok| {
            tok.chars().count() <= 2
                && tok.chars().all(|c| c.is_ascii_alphabetic())
                && tok != "v"
                && tok != "V"
                && tok != "or"
        })
    {
        return TableLintReport {
            is_truth_table: false,
            start_line,
            atoms: Vec::new(),
            issues: Vec::new(),
        };
    }

    // Check for completely empty tables (like the blank stub in `içerme.md:30`)
    let non_empty_rows: Vec<(usize, &Vec<String>)> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.iter().any(|c| !clean_formula_input(c).is_empty()))
        .collect();

    // Verify that non-empty rows actually have truth tokens (`D`/`Y`/`T`/`F`/`1`/`0`) in column 0
    if !non_empty_rows.is_empty() {
        let first_col_is_truth_vals = non_empty_rows.iter().all(|(_, r)| {
            if r.is_empty() {
                return false;
            }
            let cleaned = clean_formula_input(&r[0]);
            let toks: Vec<&str> = cleaned.split_whitespace().collect();
            !toks.is_empty()
                && toks.iter().all(|&t| {
                    matches!(t, "D" | "Y" | "d" | "y" | "T" | "F" | "t" | "f" | "1" | "0")
                })
        });
        if !first_col_is_truth_vals {
            return TableLintReport {
                is_truth_table: false,
                start_line,
                atoms: Vec::new(),
                issues: Vec::new(),
            };
        }
    } else {
        // For an empty table stub, require that remaining headers parse as formulas
        let headers_look_like_logic = headers.len() > 1
            && headers
                .iter()
                .skip(1)
                .all(|h| parse_formula(&clean_formula_input(h)).is_ok());
        if !headers_look_like_logic {
            return TableLintReport {
                is_truth_table: false,
                start_line,
                atoms: Vec::new(),
                issues: Vec::new(),
            };
        }
    }

    let mut canonical_atoms = written_atoms.clone();
    canonical_atoms.sort();
    canonical_atoms.dedup();

    let mut issues = Vec::new();
    if written_atoms != canonical_atoms {
        issues.push(TableIssueKind::NonAlphabeticalAtomHeader {
            written: written_atoms.clone(),
            canonical: canonical_atoms.clone(),
        });
    }

    if non_empty_rows.is_empty() {
        issues.push(TableIssueKind::EmptyOrIncompleteTable);
        return TableLintReport {
            is_truth_table: true,
            start_line,
            atoms: canonical_atoms,
            issues,
        };
    }

    let expected_rows = 1usize << canonical_atoms.len();
    if non_empty_rows.len() != expected_rows {
        issues.push(TableIssueKind::RowCountMismatch {
            expected: expected_rows,
            actual: non_empty_rows.len(),
        });
    }

    // Parse remaining column headers
    let mut col_specs = Vec::with_capacity(headers.len());
    col_specs.push(ColumnSpec::PackedFormulas(
        canonical_atoms
            .iter()
            .map(|a| Formula::Atom(a.clone()))
            .collect(),
    ));

    for (col_idx, raw_hdr) in headers.iter().enumerate().skip(1) {
        let cleaned_hdr = clean_formula_input(raw_hdr);
        if cleaned_hdr.is_empty() {
            col_specs.push(ColumnSpec::Unknown);
            issues.push(TableIssueKind::UnparseableColumnHeader {
                col_index: col_idx,
                header: "<empty>".to_string(),
            });
            continue;
        }
        if cleaned_hdr == "<->" || cleaned_hdr == "↔" {
            col_specs.push(ColumnSpec::ShorthandIffOfPrevTwo);
            continue;
        }
        if let Ok(f) = parse_formula(&cleaned_hdr) {
            col_specs.push(ColumnSpec::SingleFormula(f));
            continue;
        }
        // Try space-separated packed negated atoms (e.g. `~p ~q` or `~A ~B` or `~p ~q ~r`)
        let parts: Vec<&str> = cleaned_hdr.split_whitespace().collect();
        if parts.len() > 1 {
            let mut sub_formulas = Vec::new();
            let mut all_ok = true;
            for part in parts {
                if let Ok(f) = parse_formula(part) {
                    sub_formulas.push(f);
                } else {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                col_specs.push(ColumnSpec::PackedFormulas(sub_formulas));
                continue;
            }
        }

        col_specs.push(ColumnSpec::Unknown);
        issues.push(TableIssueKind::UnparseableColumnHeader {
            col_index: col_idx,
            header: cleaned_hdr,
        });
    }

    let mut seen_valuations = BTreeSet::new();

    for (row_idx, row) in non_empty_rows {
        if row.is_empty() {
            continue;
        }
        let first_cell_tokens = parse_truth_tokens(&row[0], row_idx, 0, &mut issues);
        if first_cell_tokens.len() != canonical_atoms.len() {
            continue;
        }

        let val_signature = first_cell_tokens
            .iter()
            .map(|&b| if b { "D" } else { "Y" })
            .collect::<Vec<_>>()
            .join(" ");
        if !seen_valuations.insert(val_signature.clone()) {
            issues.push(TableIssueKind::DuplicateValuationRow {
                row_index: row_idx + 1,
                valuation: val_signature,
            });
        }

        // Bind truth values using canonical alphabetical atomic order
        let mut env = BTreeMap::new();
        for (atom, val) in canonical_atoms.iter().zip(first_cell_tokens.iter()) {
            env.insert(atom.clone(), *val);
        }

        for col_idx in 1..headers.len().min(row.len()) {
            let raw_cell = clean_formula_input(&row[col_idx]);
            if raw_cell.is_empty() {
                continue;
            }
            let cell_vals = parse_truth_tokens(&row[col_idx], row_idx, col_idx, &mut issues);
            match &col_specs[col_idx] {
                ColumnSpec::SingleFormula(f) => {
                    if let (Ok(expected_b), Some(&actual_b)) =
                        (eval_formula(f, &env), cell_vals.first())
                    {
                        if expected_b != actual_b {
                            issues.push(TableIssueKind::CellValueError {
                                row_index: row_idx + 1,
                                col_index: col_idx + 1,
                                header: headers[col_idx].clone(),
                                expected: if expected_b { "D" } else { "Y" }.to_string(),
                                actual: if actual_b { "D" } else { "Y" }.to_string(),
                            });
                        }
                    }
                }
                ColumnSpec::PackedFormulas(fs) => {
                    if cell_vals.len() == fs.len() {
                        let mut exp_vec = Vec::new();
                        let mut all_eval_ok = true;
                        for f in fs {
                            if let Ok(b) = eval_formula(f, &env) {
                                exp_vec.push(b);
                            } else {
                                all_eval_ok = false;
                                break;
                            }
                        }
                        if all_eval_ok && exp_vec != cell_vals {
                            let exp_str = exp_vec
                                .iter()
                                .map(|&b| if b { "D" } else { "Y" })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let act_str = cell_vals
                                .iter()
                                .map(|&b| if b { "D" } else { "Y" })
                                .collect::<Vec<_>>()
                                .join(" ");
                            issues.push(TableIssueKind::CellValueError {
                                row_index: row_idx + 1,
                                col_index: col_idx + 1,
                                header: headers[col_idx].clone(),
                                expected: exp_str,
                                actual: act_str,
                            });
                        }
                    }
                }
                ColumnSpec::ShorthandIffOfPrevTwo => {
                    if col_idx >= 2 {
                        if let (
                            ColumnSpec::SingleFormula(f1),
                            ColumnSpec::SingleFormula(f2),
                            Some(&actual_b),
                        ) = (
                            &col_specs[col_idx - 2],
                            &col_specs[col_idx - 1],
                            cell_vals.first(),
                        ) {
                            let iff_formula = Formula::binary(BinOp::Iff, f1.clone(), f2.clone());
                            if let Ok(expected_b) = eval_formula(&iff_formula, &env) {
                                if expected_b != actual_b {
                                    issues.push(TableIssueKind::CellValueError {
                                        row_index: row_idx + 1,
                                        col_index: col_idx + 1,
                                        header: headers[col_idx].clone(),
                                        expected: if expected_b { "D" } else { "Y" }.to_string(),
                                        actual: if actual_b { "D" } else { "Y" }.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
                ColumnSpec::Unknown => {}
            }
        }
    }

    TableLintReport {
        is_truth_table: true,
        start_line,
        atoms: canonical_atoms,
        issues,
    }
}

fn parse_truth_tokens(
    raw_cell: &str,
    row_idx: usize,
    col_idx: usize,
    issues: &mut Vec<TableIssueKind>,
) -> Vec<bool> {
    let cleaned = clean_formula_input(raw_cell);
    let mut vals = Vec::new();
    for tok in cleaned.split_whitespace() {
        match tok {
            "D" | "1" => vals.push(true),
            "Y" | "0" => vals.push(false),
            "d" | "T" | "t" => {
                issues.push(TableIssueKind::NonCanonicalTruthSymbol {
                    row_index: row_idx + 1,
                    col_index: col_idx + 1,
                    raw: tok.to_string(),
                });
                vals.push(true);
            }
            "y" | "F" | "f" => {
                issues.push(TableIssueKind::NonCanonicalTruthSymbol {
                    row_index: row_idx + 1,
                    col_index: col_idx + 1,
                    raw: tok.to_string(),
                });
                vals.push(false);
            }
            _ => {}
        }
    }
    vals
}

/// Formats any Markdown table (`headers` + `rows`) with auto-aligned column widths.
pub fn format_aligned_markdown_table(headers: &[String], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }
    let cols = headers.len();
    let mut widths: Vec<usize> = headers
        .iter()
        .map(|h| h.trim().chars().count().max(3))
        .collect();
    for r in rows {
        for (c, cell) in r.iter().enumerate().take(cols) {
            let w = cell.trim().chars().count();
            if w > widths[c] {
                widths[c] = w;
            }
        }
    }

    let mut out = String::new();
    out.push('|');
    for (c, h) in headers.iter().enumerate() {
        let trimmed = h.trim();
        let pad = widths[c].saturating_sub(trimmed.chars().count());
        out.push_str(&format!(" {}{} |", trimmed, " ".repeat(pad)));
    }
    out.push('\n');

    out.push('|');
    for &w in &widths {
        out.push_str(&format!(" {} |", "-".repeat(w)));
    }
    out.push('\n');

    for r in rows {
        out.push('|');
        for c in 0..cols {
            let cell = r.get(c).map(|s| s.trim()).unwrap_or("");
            let pad = widths[c].saturating_sub(cell.chars().count());
            out.push_str(&format!(" {}{} |", cell, " ".repeat(pad)));
        }
        out.push('\n');
    }

    out
}

/// Automatically repairs a Markdown truth table:
/// - Canonicalizes the atomic variable header into alphabetical order (`| q p r |` -> `| p q r |`)
/// - Generates all $2^n$ canonical rows (`D` first, `Y` second)
/// - Re-evaluates all formula columns (`SingleFormula`, `PackedFormulas`, `ShorthandIffOfPrevTwo`)
/// - Auto-aligns the resulting Markdown table.
pub fn auto_fix_markdown_truth_table(
    headers: &[String],
    _rows: &[Vec<String>],
    true_sym: &str,
    false_sym: &str,
) -> Result<String> {
    if headers.is_empty() {
        return Err(anyhow!("Cannot auto-fix an empty table"));
    }
    let first_cleaned = clean_formula_input(&headers[0]);
    let mut canonical_atoms: Vec<String> = first_cleaned
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if canonical_atoms.is_empty() {
        return Err(anyhow!("First column does not contain atomic variables"));
    }
    canonical_atoms.sort();
    canonical_atoms.dedup();

    let mut fixed_headers = Vec::with_capacity(headers.len());
    fixed_headers.push(canonical_atoms.join(" "));
    for h in headers.iter().skip(1) {
        fixed_headers.push(h.trim().to_string());
    }

    let mut col_specs = Vec::with_capacity(headers.len());
    col_specs.push(ColumnSpec::PackedFormulas(
        canonical_atoms
            .iter()
            .map(|a| Formula::Atom(a.clone()))
            .collect(),
    ));

    for raw_hdr in headers.iter().skip(1) {
        let cleaned_hdr = clean_formula_input(raw_hdr);
        if cleaned_hdr == "<->" || cleaned_hdr == "↔" {
            col_specs.push(ColumnSpec::ShorthandIffOfPrevTwo);
            continue;
        }
        if let Ok(f) = parse_formula(&cleaned_hdr) {
            col_specs.push(ColumnSpec::SingleFormula(f));
            continue;
        }
        let parts: Vec<&str> = cleaned_hdr.split_whitespace().collect();
        if parts.len() > 1 {
            let mut sub_formulas = Vec::new();
            let mut all_ok = true;
            for part in parts {
                if let Ok(f) = parse_formula(part) {
                    sub_formulas.push(f);
                } else {
                    all_ok = false;
                    break;
                }
            }
            if all_ok {
                col_specs.push(ColumnSpec::PackedFormulas(sub_formulas));
                continue;
            }
        }
        col_specs.push(ColumnSpec::Unknown);
    }

    let valuations = all_valuations(&canonical_atoms);
    let mut fixed_rows = Vec::with_capacity(valuations.len());

    for env in valuations {
        let mut row = Vec::with_capacity(fixed_headers.len());
        for col_idx in 0..fixed_headers.len() {
            match &col_specs[col_idx] {
                ColumnSpec::SingleFormula(f) => {
                    let b = eval_formula(f, &env)?;
                    row.push(if b { true_sym } else { false_sym }.to_string());
                }
                ColumnSpec::PackedFormulas(fs) => {
                    let mut vals = Vec::with_capacity(fs.len());
                    for f in fs {
                        let b = eval_formula(f, &env)?;
                        vals.push(if b { true_sym } else { false_sym });
                    }
                    row.push(vals.join(" "));
                }
                ColumnSpec::ShorthandIffOfPrevTwo => {
                    if col_idx >= 2 {
                        if let (ColumnSpec::SingleFormula(f1), ColumnSpec::SingleFormula(f2)) =
                            (&col_specs[col_idx - 2], &col_specs[col_idx - 1])
                        {
                            let iff = Formula::binary(BinOp::Iff, f1.clone(), f2.clone());
                            let b = eval_formula(&iff, &env)?;
                            row.push(if b { true_sym } else { false_sym }.to_string());
                        } else {
                            row.push("?".to_string());
                        }
                    } else {
                        row.push("?".to_string());
                    }
                }
                ColumnSpec::Unknown => {
                    row.push(String::new());
                }
            }
        }
        fixed_rows.push(row);
    }

    Ok(format_aligned_markdown_table(&fixed_headers, &fixed_rows))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditableTableGrid {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub start_line: usize,
    pub end_line: usize,
}

impl EditableTableGrid {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>, start_line: usize, end_line: usize) -> Self {
        let cols = headers.len();
        let normalized_rows = rows
            .into_iter()
            .map(|mut r| {
                r.resize(cols, String::new());
                r
            })
            .collect();
        Self {
            headers,
            rows: normalized_rows,
            start_line,
            end_line,
        }
    }

    pub fn set_header(&mut self, col: usize, value: String) {
        if let Some(h) = self.headers.get_mut(col) {
            *h = value;
        }
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: String) {
        if let Some(r) = self.rows.get_mut(row) {
            if let Some(c) = r.get_mut(col) {
                *c = value;
            }
        }
    }

    pub fn add_row(&mut self) {
        self.rows.push(vec![String::new(); self.headers.len()]);
    }

    pub fn remove_row(&mut self, row: usize) {
        if row < self.rows.len() {
            self.rows.remove(row);
        }
    }

    pub fn add_column(&mut self, header: String) {
        self.headers.push(header);
        for r in &mut self.rows {
            r.push(String::new());
        }
    }

    pub fn remove_column(&mut self, col: usize) {
        if col < self.headers.len() && self.headers.len() > 1 {
            self.headers.remove(col);
            for r in &mut self.rows {
                if col < r.len() {
                    r.remove(col);
                }
            }
        }
    }

    pub fn to_aligned_markdown(&self) -> String {
        format_aligned_markdown_table(&self.headers, &self.rows)
    }

    pub fn auto_fix_as_truth_table(&mut self, true_sym: &str, false_sym: &str) -> Result<String> {
        let fixed_md = auto_fix_markdown_truth_table(&self.headers, &self.rows, true_sym, false_sym)?;
        // Re-parse into self.headers and self.rows
        let mut lines = fixed_md.lines();
        if let Some(hdr_line) = lines.next() {
            self.headers = hdr_line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|s| s.trim().to_string())
                .collect();
        }
        let _ = lines.next(); // separator
        self.rows.clear();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let cells: Vec<String> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|s| s.trim().to_string())
                .collect();
            self.rows.push(cells);
        }
        Ok(fixed_md)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_truth_table_generation_and_linting() {
        let f = parse_formula("p -> q").unwrap();
        let md = generate_markdown_truth_table(&[f], true, "D", "Y").unwrap();
        assert!(md.contains("| p q | p → q |"));
        assert!(md.contains("| D Y | Y |"));

        // Lint a table with non-alphabetical header `q p r` and lowercase `y`
        let headers = vec!["q p r".to_string(), "p v q".to_string()];
        let rows = vec![
            vec!["D D D".to_string(), "D".to_string()],
            vec!["D D y".to_string(), "D".to_string()],
        ];
        let report = lint_markdown_table(&headers, &rows, 10);
        assert!(report.is_truth_table);
        assert!(report.issues.iter().any(|i| matches!(
            i,
            TableIssueKind::NonAlphabeticalAtomHeader { written, canonical }
            if written == &vec!["q", "p", "r"] && canonical == &vec!["p", "q", "r"]
        )));
        assert!(report.issues.iter().any(|i| matches!(
            i,
            TableIssueKind::NonCanonicalTruthSymbol { raw, .. } if raw == "y"
        )));

        // Auto-fix the table and verify alphabetical header + 8 canonical rows
        let mut grid = EditableTableGrid::new(headers, rows, 10, 14);
        let fixed = grid.auto_fix_as_truth_table("D", "Y").unwrap();
        assert!(fixed.contains("| p q r | p v q |"));
        assert_eq!(grid.rows.len(), 8);
    }
}
