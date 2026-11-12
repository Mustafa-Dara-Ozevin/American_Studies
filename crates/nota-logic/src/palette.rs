use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicSymbolEntry {
    pub symbol: String,
    pub name: String,
    pub category: String,
    pub triggers: Vec<String>,
    pub description: String,
}

pub fn default_symbol_palette() -> Vec<LogicSymbolEntry> {
    vec![
        // NOR / NAND
        LogicSymbolEntry {
            symbol: "↓".into(),
            name: "NOR (Peirce Arrow / Joint Denial)".into(),
            category: "NOR / NAND".into(),
            triggers: vec!["!v".into(), ":nor".into(), "\\downarrow".into()],
            description: "True iff both operands are false: ¬(p ∨ q)".into(),
        },
        LogicSymbolEntry {
            symbol: "↑".into(),
            name: "NAND (Sheffer Stroke / Alternative Denial)".into(),
            category: "NOR / NAND".into(),
            triggers: vec!["!^".into(), ":nand".into(), "\\uparrow".into()],
            description: "True iff not both operands are true: ¬(p ∧ q)".into(),
        },
        // Conditional
        LogicSymbolEntry {
            symbol: "→".into(),
            name: "Material Conditional (İse / Koşul)".into(),
            category: "Conditional".into(),
            triggers: vec!["->".into()],
            description: "False only when antecedent is D and consequent is Y".into(),
        },
        LogicSymbolEntry {
            symbol: "←".into(),
            name: "Converse Conditional".into(),
            category: "Conditional".into(),
            triggers: vec!["<-".into()],
            description: "True unless left is Y and right is D (p ← q ≡ q → p)".into(),
        },
        LogicSymbolEntry {
            symbol: "↛".into(),
            name: "Negated Conditional (Nonimplication)".into(),
            category: "Conditional".into(),
            triggers: vec!["!->".into(), "¬->".into()],
            description: "Equivalent to ¬(p → q) ≡ p ∧ ¬q (α-stacking rule)".into(),
        },
        LogicSymbolEntry {
            symbol: "↚".into(),
            name: "Converse Nonimplication".into(),
            category: "Conditional".into(),
            triggers: vec!["<!-".into()],
            description: "Equivalent to ¬(p ← q) ≡ ¬p ∧ q (α-stacking rule)".into(),
        },
        // Biconditional
        LogicSymbolEntry {
            symbol: "↔".into(),
            name: "Material Biconditional (Ancak ve Ancak)".into(),
            category: "Biconditional".into(),
            triggers: vec!["<->".into(), "←>".into()],
            description: "True when both operands share the same truth value".into(),
        },
        LogicSymbolEntry {
            symbol: "↮".into(),
            name: "Negated Biconditional (Exclusive Or)".into(),
            category: "Biconditional".into(),
            triggers: vec!["<!->".into(), "↚>".into(), "!↔".into(), "¬↔".into()],
            description: "True when operands differ in truth value: ¬(p ↔ q)".into(),
        },
        // Metalogic arrows
        LogicSymbolEntry {
            symbol: "⇒".into(),
            name: "Metalogical Implies".into(),
            category: "Metalogic Arrows".into(),
            triggers: vec!["=>".into()],
            description: "Metalogical implication arrow".into(),
        },
        LogicSymbolEntry {
            symbol: "⇐".into(),
            name: "Metalogical Implied By".into(),
            category: "Metalogic Arrows".into(),
            triggers: vec!["<=".into()],
            description: "Converse metalogical implication arrow".into(),
        },
        LogicSymbolEntry {
            symbol: "⇔".into(),
            name: "Metalogical Equivalence".into(),
            category: "Metalogic Arrows".into(),
            triggers: vec!["<=>".into(), "⇐>".into()],
            description: "Metalogical biconditional / equivalence arrow".into(),
        },
        // Negation
        LogicSymbolEntry {
            symbol: "¬".into(),
            name: "Negation (Değilleme)".into(),
            category: "Negation".into(),
            triggers: vec![":not".into(), "\\neg".into(), "~~".into()],
            description: "Inverts truth value (D ↔ Y)".into(),
        },
        // Conjunction
        LogicSymbolEntry {
            symbol: "∧".into(),
            name: "Conjunction (Tümel Evetleme / Ve)".into(),
            category: "Conjunction".into(),
            triggers: vec![":and".into(), "\\land".into(), "&&".into()],
            description: "True iff both operands are D".into(),
        },
        // Disjunction
        LogicSymbolEntry {
            symbol: "∨".into(),
            name: "Disjunction (Tikel Evetleme / Veya)".into(),
            category: "Disjunction".into(),
            triggers: vec![":or".into(), "\\lor".into()],
            description: "True iff at least one operand is D".into(),
        },
        // XOR
        LogicSymbolEntry {
            symbol: "⊕".into(),
            name: "Exclusive Disjunction (XOR)".into(),
            category: "XOR".into(),
            triggers: vec![":xor".into(), "\\oplus".into()],
            description: "True iff exactly one operand is D".into(),
        },
        // Turnstile & Models
        LogicSymbolEntry {
            symbol: "⊢".into(),
            name: "Syntactic Turnstile (Provable)".into(),
            category: "Turnstile / Models".into(),
            triggers: vec!["|-".into()],
            description: "Syntactic derivability: Γ ⊢ φ".into(),
        },
        LogicSymbolEntry {
            symbol: "⊬".into(),
            name: "Negated Turnstile (Unprovable)".into(),
            category: "Turnstile / Models".into(),
            triggers: vec!["!|-".into(), "¬|-".into(), "¬⊢".into()],
            description: "Not syntactically derivable: Γ ⊬ φ".into(),
        },
        LogicSymbolEntry {
            symbol: "⊨".into(),
            name: "Semantic Entailment / Models (Geçerlilik)".into(),
            category: "Turnstile / Models".into(),
            triggers: vec!["|=".into()],
            description: "Semantic entailment / tautology: Γ ⊨ φ".into(),
        },
        LogicSymbolEntry {
            symbol: "⊭".into(),
            name: "Negated Semantic Entailment (Invalid)".into(),
            category: "Turnstile / Models".into(),
            triggers: vec!["!|=".into(), "¬|=".into(), "¬⊨".into()],
            description: "Counterexample exists: Γ ⊭ φ".into(),
        },
        // Constants
        LogicSymbolEntry {
            symbol: "⊤".into(),
            name: "Verum / Tautology Constant".into(),
            category: "Constants".into(),
            triggers: vec![":top".into(), "\\top".into()],
            description: "Always true (D)".into(),
        },
        LogicSymbolEntry {
            symbol: "⊥".into(),
            name: "Falsum / Contradiction Constant".into(),
            category: "Constants".into(),
            triggers: vec![":bot".into(), "\\bot".into()],
            description: "Always false (Y); closes any truth tree branch".into(),
        },
        LogicSymbolEntry {
            symbol: "∴".into(),
            name: "Therefore (O Halde / Sonuç)".into(),
            category: "Constants".into(),
            triggers: vec![":tf".into(), "\\therefore".into()],
            description: "Separates premises from conclusion: p → q, p ∴ q".into(),
        },
        LogicSymbolEntry {
            symbol: "∵".into(),
            name: "Because (Çünkü)".into(),
            category: "Constants".into(),
            triggers: vec![":bc".into(), "\\because".into()],
            description: "Indicates justification / premise reason".into(),
        },
        // Quantifiers
        LogicSymbolEntry {
            symbol: "∀".into(),
            name: "Universal Quantifier (Her / Tümel Niceleyici)".into(),
            category: "Quantifiers".into(),
            triggers: vec![":fa".into(), "\\forall".into()],
            description: "For all x: ∀x P(x) (γ-rule)".into(),
        },
        LogicSymbolEntry {
            symbol: "∃".into(),
            name: "Existential Quantifier (Bazı / Tikel Niceleyici)".into(),
            category: "Quantifiers".into(),
            triggers: vec![":ex".into(), "\\exists".into()],
            description: "There exists x: ∃x P(x) (δ-rule)".into(),
        },
        LogicSymbolEntry {
            symbol: "∄".into(),
            name: "Negated Existential Quantifier".into(),
            category: "Quantifiers".into(),
            triggers: vec!["!∃".into(), "¬∃".into(), "\\nexists".into()],
            description: "There does not exist x: ¬∃x P(x)".into(),
        },
        // Identity & Equivalence
        LogicSymbolEntry {
            symbol: "≡".into(),
            name: "Logical Equivalence (Eşdeğerlik)".into(),
            category: "Identity / Equivalence".into(),
            triggers: vec!["===".into()],
            description: "Metalogical equivalence between formulas: φ ≡ ψ".into(),
        },
        LogicSymbolEntry {
            symbol: "≢".into(),
            name: "Logical Inequivalence".into(),
            category: "Identity / Equivalence".into(),
            triggers: vec!["≠=".into(), "!≡".into(), "¬≡".into(), "≡/".into()],
            description: "Formulas are not logically equivalent".into(),
        },
        LogicSymbolEntry {
            symbol: "≠".into(),
            name: "Not Equal".into(),
            category: "Identity / Equivalence".into(),
            triggers: vec!["!=".into()],
            description: "Non-identity relation: x ≠ y".into(),
        },
    ]
}

pub fn search_symbol_palette(query: &str) -> Vec<LogicSymbolEntry> {
    let q = query.trim().to_lowercase();
    let all = default_symbol_palette();
    if q.is_empty() {
        return all;
    }
    all.into_iter()
        .filter(|e| {
            e.symbol.to_lowercase().contains(&q)
                || e.name.to_lowercase().contains(&q)
                || e.category.to_lowercase().contains(&q)
                || e.description.to_lowercase().contains(&q)
                || e.triggers.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect()
}

/// Normalizes ASCII logic operators (`->`, `<->`, `!->`, `<!-`, `^`, `~`, `|-`, `|=`, `:tf`,
/// and binary `v`/`V` between formula tokens) into canonical Unicode symbols while skipping
/// fenced code blocks, inline code, wikilinks, and URLs.
/// Returns `(normalized_text, number_of_replacements)`.
pub fn normalize_ascii_operators_in_text(input: &str) -> (String, usize) {
    let mut out = String::with_capacity(input.len());
    let mut count = 0usize;
    let mut in_fence = false;

    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence || trimmed.starts_with("---") {
            out.push_str(line);
            continue;
        }
        let (norm_line, c) = normalize_line_operators(line);
        count += c;
        out.push_str(&norm_line);
    }

    (out, count)
}

fn normalize_line_operators(line: &str) -> (String, usize) {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut count = 0usize;
    let mut i = 0usize;
    let mut in_inline_code = false;
    let mut in_wikilink = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '`' {
            in_inline_code = !in_inline_code;
            out.push(ch);
            i += 1;
            continue;
        }
        if in_inline_code {
            out.push(ch);
            i += 1;
            continue;
        }
        if ch == '[' && i + 1 < chars.len() && chars[i + 1] == '[' {
            in_wikilink += 1;
            out.push_str("[[");
            i += 2;
            continue;
        }
        if ch == ']' && i + 1 < chars.len() && chars[i + 1] == ']' && in_wikilink > 0 {
            in_wikilink -= 1;
            out.push_str("]]");
            i += 2;
            continue;
        }
        if in_wikilink > 0 {
            out.push(ch);
            i += 1;
            continue;
        }

        // Skip HTML comments `<!-- ... -->`
        if ch == '<'
            && i + 3 < chars.len()
            && chars[i + 1] == '!'
            && chars[i + 2] == '-'
            && chars[i + 3] == '-'
        {
            while i < chars.len() {
                out.push(chars[i]);
                if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '-' && chars[i] == '>' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Multi-character operator replacements (longest match first)
        let rem: String = chars[i..].iter().take(6).collect();
        let multi_rules: &[(&str, &str)] = &[
            ("<!->", "↮"),
            ("!->", "↛"),
            ("¬->", "↛"),
            ("<!-", "↚"),
            ("<->", "↔"),
            ("<=>", "⇔"),
            ("!|-", "⊬"),
            ("!|=", "⊭"),
            ("->", "→"),
            ("<-", "←"),
            ("|-", "⊢"),
            ("|=", "⊨"),
            ("!v", "↓"),
            ("!^", "↑"),
            ("&&", "∧"),
            (":tf", "∴"),
            (":fa", "∀"),
            (":ex", "∃"),
        ];
        let mut matched_multi = false;
        for &(pat, rep) in multi_rules {
            if rem.starts_with(pat) {
                out.push_str(rep);
                i += pat.chars().count();
                count += 1;
                matched_multi = true;
                break;
            }
        }
        if matched_multi {
            continue;
        }

        // Single-char `^` -> `∧`
        if ch == '^' {
            out.push('∧');
            i += 1;
            count += 1;
            continue;
        }

        // Single-char `~` -> `¬` when followed by an identifier, `(`, or `~` (and not paired `~~text~~` strikethrough)
        if ch == '~' {
            let next_is_formula = i + 1 < chars.len()
                && (chars[i + 1] == '('
                    || chars[i + 1] == '['
                    || chars[i + 1] == '~'
                    || chars[i + 1] == '¬'
                    || (chars[i + 1].is_ascii_alphabetic()
                        && (i + 2 >= chars.len() || !chars[i + 2].is_ascii_alphabetic())));
            if next_is_formula {
                out.push('¬');
                i += 1;
                count += 1;
                continue;
            }
        }

        // Infix `v` or `V` surrounded by whitespace between single-char logic tokens or parens:
        // e.g. `p v q`, `(p → q) v r`, `A V B`
        if (ch == 'v' || ch == 'V') && i > 0 && i + 1 < chars.len() {
            let prev = chars[i - 1];
            let next = chars[i + 1];
            if prev == ' ' && next == ' ' {
                let left_non_space = chars[..i - 1].iter().rev().find(|c| !c.is_whitespace());
                let right_non_space = chars[i + 2..].iter().find(|c| !c.is_whitespace());
                let left_ok = left_non_space.is_some_and(|&c| {
                    c == ')' || c == ']' || c.is_ascii_alphabetic()
                });
                let right_ok = right_non_space.is_some_and(|&c| {
                    c == '(' || c == '[' || c == '~' || c == '¬' || c.is_ascii_alphabetic()
                });
                // Avoid replacing English/Turkish prose words by checking that the left token is a single letter or `)`
                let left_is_single_atom_or_paren = {
                    let trimmed_left: Vec<char> = chars[..i - 1]
                        .iter()
                        .copied()
                        .rev()
                        .skip_while(|c| c.is_whitespace())
                        .collect();
                    if let Some(&first) = trimmed_left.first() {
                        first == ')'
                            || first == ']'
                            || (first.is_ascii_alphabetic()
                                && trimmed_left
                                    .get(1)
                                    .is_none_or(|c| !c.is_ascii_alphabetic()))
                    } else {
                        false
                    }
                };
                if left_ok && right_ok && left_is_single_atom_or_paren {
                    out.push('∨');
                    i += 1;
                    count += 1;
                    continue;
                }
            }
        }

        out.push(ch);
        i += 1;
    }

    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_palette_and_ascii_normalizer() {
        let results = search_symbol_palette("biconditional");
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.symbol == "↔"));

        let (norm, count) = normalize_ascii_operators_in_text(
            "~(p -> q) === (p ^ ~q) and (p v q) <-> (q V p) in [[Link->Stay]]",
        );
        assert!(count >= 7);
        assert_eq!(
            norm,
            "¬(p → q) === (p ∧ ¬q) and (p ∨ q) ↔ (q ∨ p) in [[Link->Stay]]"
        );
    }
}
