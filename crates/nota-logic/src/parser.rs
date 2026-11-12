use crate::ast::{BinOp, Formula, MetalogicStmt, QuantKind};
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Atom(String),
    Const(bool),
    Not,
    BinOp(BinOp),
    Quantifier(QuantKind),
    LParen,
    RParen,
    Comma,
}

/// Cleans Markdown emphasis (`*`, `**`, `***`), `<br>`, and surrounding whitespace from a formula string.
pub fn clean_formula_input(raw: &str) -> String {
    let mut s = raw
        .replace("<br>", " ")
        .replace("<br/>", " ")
        .replace("<br />", " ")
        .replace('\u{00a0}', " ");
    // Strip Markdown emphasis asterisks `*` since `*` is not a logical operator in this vault
    s.retain(|c| c != '*');
    s.trim().to_string()
}

pub fn tokenize_formula(input: &str) -> Result<Vec<Token>> {
    let cleaned = clean_formula_input(input);
    let chars: Vec<char> = cleaned.chars().collect();
    let len = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        // Check multi-character operators (longest match first)
        let rem: String = chars[i..].iter().collect();
        if let Some((tok, consumed_chars)) = match_operator_prefix(&rem) {
            tokens.push(tok);
            i += consumed_chars;
            continue;
        }

        match ch {
            '(' | '[' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' | ']' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '¬' | '~' | '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '∧' | '^' | '&' => {
                tokens.push(Token::BinOp(BinOp::And));
                i += 1;
            }
            '∨' => {
                tokens.push(Token::BinOp(BinOp::Or));
                i += 1;
            }
            '⊕' => {
                tokens.push(Token::BinOp(BinOp::Xor));
                i += 1;
            }
            '↑' => {
                tokens.push(Token::BinOp(BinOp::Nand));
                i += 1;
            }
            '↓' => {
                tokens.push(Token::BinOp(BinOp::Nor));
                i += 1;
            }
            '→' => {
                tokens.push(Token::BinOp(BinOp::Implies));
                i += 1;
            }
            '←' => {
                tokens.push(Token::BinOp(BinOp::ConverseImplies));
                i += 1;
            }
            '↛' => {
                tokens.push(Token::BinOp(BinOp::NonImplies));
                i += 1;
            }
            '↚' => {
                tokens.push(Token::BinOp(BinOp::ConverseNonImplies));
                i += 1;
            }
            '↔' | '⇔' | '≡' => {
                tokens.push(Token::BinOp(BinOp::Iff));
                i += 1;
            }
            '↮' | '≢' => {
                tokens.push(Token::BinOp(BinOp::NonIff));
                i += 1;
            }
            '⊤' => {
                tokens.push(Token::Const(true));
                i += 1;
            }
            '⊥' => {
                tokens.push(Token::Const(false));
                i += 1;
            }
            '∀' => {
                tokens.push(Token::Quantifier(QuantKind::ForAll));
                i += 1;
            }
            '∃' => {
                tokens.push(Token::Quantifier(QuantKind::Exists));
                i += 1;
            }
            '∄' => {
                tokens.push(Token::Quantifier(QuantKind::NotExists));
                i += 1;
            }
            _ if ch.is_alphabetic() => {
                // Handle compact propositional strings like `pVq`, `pvq`, `~pvq`
                // where `v` or `V` is used as infix disjunction without spaces!
                if (ch == 'v' || ch == 'V') && is_infix_disjunction_context(&tokens) {
                    tokens.push(Token::BinOp(BinOp::Or));
                    i += 1;
                    continue;
                }

                // Read identifier, but stop before an infix `v`/`V` if this is a single-letter atom
                // followed by `v`/`V` and another operand (e.g. `pVq` or `pv(q)`).
                let start = i;
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    if (chars[i] == 'v' || chars[i] == 'V')
                        && i == start + 1
                        && i + 1 < len
                        && (chars[i + 1].is_alphabetic()
                            || chars[i + 1] == '('
                            || chars[i + 1] == '['
                            || chars[i + 1] == '~'
                            || chars[i + 1] == '¬')
                    {
                        break;
                    }
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(Token::Atom(ident));
            }
            _ => {
                return Err(anyhow!(
                    "Unexpected character '{}' at position {} in '{}'",
                    ch,
                    i,
                    cleaned
                ));
            }
        }
    }

    Ok(tokens)
}

fn is_infix_disjunction_context(prev_tokens: &[Token]) -> bool {
    matches!(
        prev_tokens.last(),
        Some(Token::Atom(_)) | Some(Token::Const(_)) | Some(Token::RParen)
    )
}

fn match_operator_prefix(rem: &str) -> Option<(Token, usize)> {
    const PREFIXES: &[(&str, Token)] = &[
        ("\\downarrow", Token::BinOp(BinOp::Nor)),
        ("\\uparrow", Token::BinOp(BinOp::Nand)),
        ("\\nexists", Token::Quantifier(QuantKind::NotExists)),
        ("\\forall", Token::Quantifier(QuantKind::ForAll)),
        ("\\exists", Token::Quantifier(QuantKind::Exists)),
        ("\\oplus", Token::BinOp(BinOp::Xor)),
        ("\\land", Token::BinOp(BinOp::And)),
        ("\\lor", Token::BinOp(BinOp::Or)),
        ("\\neg", Token::Not),
        ("\\top", Token::Const(true)),
        ("\\bot", Token::Const(false)),
        (":nand", Token::BinOp(BinOp::Nand)),
        (":nor", Token::BinOp(BinOp::Nor)),
        (":not", Token::Not),
        (":and", Token::BinOp(BinOp::And)),
        (":xor", Token::BinOp(BinOp::Xor)),
        (":top", Token::Const(true)),
        (":bot", Token::Const(false)),
        (":or", Token::BinOp(BinOp::Or)),
        (":fa", Token::Quantifier(QuantKind::ForAll)),
        (":ex", Token::Quantifier(QuantKind::Exists)),
        ("<!->", Token::BinOp(BinOp::NonIff)),
        ("<->", Token::BinOp(BinOp::Iff)),
        ("<=>", Token::BinOp(BinOp::Iff)),
        ("===", Token::BinOp(BinOp::Iff)),
        ("!->", Token::BinOp(BinOp::NonImplies)),
        ("¬->", Token::BinOp(BinOp::NonImplies)),
        ("<!-", Token::BinOp(BinOp::ConverseNonImplies)),
        ("↚>", Token::BinOp(BinOp::NonIff)),
        ("!↔", Token::BinOp(BinOp::NonIff)),
        ("¬↔", Token::BinOp(BinOp::NonIff)),
        ("←>", Token::BinOp(BinOp::Iff)),
        ("⇐>", Token::BinOp(BinOp::Iff)),
        ("!v", Token::BinOp(BinOp::Nor)),
        ("!^", Token::BinOp(BinOp::Nand)),
        ("->", Token::BinOp(BinOp::Implies)),
        ("=>", Token::BinOp(BinOp::Implies)),
        ("<-", Token::BinOp(BinOp::ConverseImplies)),
        ("<=", Token::BinOp(BinOp::ConverseImplies)),
        ("&&", Token::BinOp(BinOp::And)),
    ];

    for (pat, tok) in PREFIXES {
        if rem.starts_with(pat) {
            return Some((tok.clone(), pat.chars().count()));
        }
    }
    None
}

pub fn parse_formula(input: &str) -> Result<Formula> {
    let tokens = tokenize_formula(input)?;
    if tokens.is_empty() {
        return Err(anyhow!("Empty formula"));
    }
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
    };
    let expr = parser.parse_expr(1)?;
    if parser.pos < parser.tokens.len() {
        return Err(anyhow!(
            "Unexpected trailing token {:?} at token index {} in '{}'",
            parser.tokens[parser.pos],
            parser.pos,
            input
        ));
    }
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn parse_expr(&mut self, min_prec: u8) -> Result<Formula> {
        let mut lhs = self.parse_prefix()?;

        while let Some(Token::BinOp(op)) = self.peek() {
            let op = *op;
            let prec = op.precedence();
            if prec < min_prec {
                break;
            }
            self.next();
            // Right-associative for Implies, left-associative for others
            let next_min_prec = match op {
                BinOp::Implies => prec,
                _ => prec + 1,
            };
            let rhs = self.parse_expr(next_min_prec)?;
            lhs = Formula::binary(op, lhs, rhs);
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Formula> {
        match self.next() {
            Some(Token::Not) => {
                let inner = self.parse_prefix()?;
                Ok(Formula::negate(inner))
            }
            Some(Token::Quantifier(kind)) => {
                let kind = *kind;
                let var = match self.next() {
                    Some(Token::Atom(v)) => v.clone(),
                    other => {
                        return Err(anyhow!(
                            "Expected variable after quantifier {:?}, found {:?}",
                            kind,
                            other
                        ))
                    }
                };
                let body = self.parse_prefix()?;
                Ok(Formula::Quantified {
                    kind,
                    var,
                    body: Box::new(body),
                })
            }
            Some(Token::LParen) => {
                let inner = self.parse_expr(1)?;
                match self.next() {
                    Some(Token::RParen) => Ok(inner),
                    other => Err(anyhow!("Expected closing parenthesis, found {:?}", other)),
                }
            }
            Some(Token::Const(b)) => Ok(Formula::Const(*b)),
            Some(Token::Atom(name)) => {
                // Check for predicate arguments `F(x, y)`
                if matches!(self.peek(), Some(Token::LParen)) {
                    let save_pos = self.pos;
                    self.next(); // consume LParen
                    let mut args = Vec::new();
                    let mut valid_pred = true;
                    loop {
                        match self.next() {
                            Some(Token::Atom(arg)) => args.push(arg.clone()),
                            _ => {
                                valid_pred = false;
                                break;
                            }
                        }
                        match self.peek() {
                            Some(Token::Comma) => {
                                self.next();
                            }
                            Some(Token::RParen) => {
                                self.next();
                                break;
                            }
                            _ => {
                                valid_pred = false;
                                break;
                            }
                        }
                    }
                    if valid_pred {
                        return Ok(Formula::Predicate {
                            name: name.clone(),
                            args,
                        });
                    } else {
                        self.pos = save_pos;
                    }
                }
                Ok(Formula::Atom(name.clone()))
            }
            other => Err(anyhow!("Unexpected token {:?} when parsing formula", other)),
        }
    }
}

/// Parses a metalogical statement such as:
/// - `p -> q === ~p v q`
/// - `p -> q, q -> r ∴ p -> r`
/// - `(p <-> q) |= (r v p) <-> (r v q)`
/// - `{p -> q, q -> p, p ^ ~q}`
pub fn parse_metalogic_stmt(raw: &str) -> Result<MetalogicStmt> {
    let cleaned = clean_formula_input(raw);
    // Strip trailing verdict annotations like `=> Geçerli`, `=> Tutarlı`, `- Kontrol et`
    let main_part = strip_trailing_verdict(&cleaned);

    // 1. Check set syntax `{A, B, C}`
    if let Some(inner) = main_part.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        let mut formulas = Vec::new();
        for item in split_top_level_commas(inner) {
            if !item.trim().is_empty() {
                formulas.push(parse_formula(item)?);
            }
        }
        if formulas.is_empty() {
            return Err(anyhow!("Empty formula set"));
        }
        return Ok(MetalogicStmt::FormulaSet(formulas));
    }

    // 2. Check Entailment / Turnstile (`|=`, `⊨`, `|-`, `⊢`, `∴`, `⊭`, `⊬`, `!|=`, `!|-`)
    for (sep, negated) in [
        ("!|=", true),
        ("¬|=", true),
        ("!|-", true),
        ("¬|-", true),
        ("⊭", true),
        ("⊬", true),
        ("|=", false),
        ("⊨", false),
        ("|-", false),
        ("⊢", false),
        ("∴", false),
    ] {
        if let Some((lhs_str, rhs_str)) = main_part.split_once(sep) {
            let mut premises = Vec::new();
            for item in split_top_level_commas(lhs_str) {
                if !item.trim().is_empty() {
                    premises.push(parse_formula(item)?);
                }
            }
            let conclusion = parse_formula(rhs_str)?;
            return Ok(MetalogicStmt::Entailment {
                premises,
                conclusion,
                negated,
            });
        }
    }

    // 3. Check Equivalence (`===` or `≡`) and Non-Equivalence (`!==` or `≢`)
    for (sep, is_neg) in [("!==", true), ("≢", true), ("===", false), ("≡", false)] {
        if let Some((lhs_str, rhs_str)) = main_part.split_once(sep) {
            let lhs = parse_formula(lhs_str)?;
            let rhs = parse_formula(rhs_str)?;
            return if is_neg {
                Ok(MetalogicStmt::NonEquivalence(lhs, rhs))
            } else {
                Ok(MetalogicStmt::Equivalence(lhs, rhs))
            };
        }
    }

    Ok(MetalogicStmt::Single(parse_formula(main_part)?))
}

fn strip_trailing_verdict(s: &str) -> &str {
    for marker in [
        "=> Geçersiz",
        "=> Geçerli",
        "=> Tutarsız",
        "=> Tutarlı",
        "- ===Kontrol et===",
        "- Kontrol et",
    ] {
        if let Some((before, _)) = s.split_once(marker) {
            return before.trim();
        }
    }
    s.trim()
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (idx, ch) in s.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(s[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(s[start..].trim());
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vault_formulas() {
        // Compact disjunction without spaces:
        let f1 = parse_formula("pVq").unwrap();
        assert_eq!(f1.to_string(), "p ∨ q");

        let f2 = parse_formula("((pvq)->p)v((pvq)->r)").unwrap();
        assert_eq!(f2.to_string(), "((p ∨ q) → p) ∨ ((p ∨ q) → r)");

        // Square brackets and uppercase V:
        let f3 = parse_formula("~[(A -> B) -> ~(B -> A)]").unwrap();
        assert_eq!(f3.to_string(), "¬((A → B) → ¬(B → A))");

        // NOR, NAND, Non-implication:
        let f4 = parse_formula("~p ↓ ~r").unwrap();
        assert_eq!(f4.to_string(), "¬p ↓ ¬r");

        let f5 = parse_formula("p ↛ r").unwrap();
        assert_eq!(f5.to_string(), "p ↛ r");
    }

    #[test]
    fn test_parse_metalogic_statements() {
        let eq = parse_metalogic_stmt("***~(p v q) === ~p ^ ~q****").unwrap();
        assert!(matches!(eq, MetalogicStmt::Equivalence(_, _)));

        let ent = parse_metalogic_stmt("p -> (q ^ r), ~r V q |= p => Geçersiz").unwrap();
        if let MetalogicStmt::Entailment {
            premises,
            conclusion,
            negated,
        } = ent
        {
            assert_eq!(premises.len(), 2);
            assert_eq!(conclusion.to_string(), "p");
            assert!(!negated);
        } else {
            panic!("Expected entailment");
        }
    }
}
