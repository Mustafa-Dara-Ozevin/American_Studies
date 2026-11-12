use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinOp {
    And,                // ∧, ^, &&
    Or,                 // ∨, v, V
    Xor,                // ⊕
    Nand,               // ↑, !^
    Nor,                // ↓, !v
    Implies,            // →, ->
    ConverseImplies,    // ←, <-
    NonImplies,         // ↛, !->, ¬->
    ConverseNonImplies, // ↚, <!-
    Iff,                // ↔, <->
    NonIff,             // ↮, !↔, ¬↔, <!->
}

impl BinOp {
    pub fn symbol_unicode(self) -> &'static str {
        match self {
            BinOp::And => "∧",
            BinOp::Or => "∨",
            BinOp::Xor => "⊕",
            BinOp::Nand => "↑",
            BinOp::Nor => "↓",
            BinOp::Implies => "→",
            BinOp::ConverseImplies => "←",
            BinOp::NonImplies => "↛",
            BinOp::ConverseNonImplies => "↚",
            BinOp::Iff => "↔",
            BinOp::NonIff => "↮",
        }
    }

    pub fn eval(self, lhs: bool, rhs: bool) -> bool {
        match self {
            BinOp::And => lhs && rhs,
            BinOp::Or => lhs || rhs,
            BinOp::Xor => lhs ^ rhs,
            BinOp::Nand => !(lhs && rhs),
            BinOp::Nor => !(lhs || rhs),
            BinOp::Implies => !lhs || rhs,
            BinOp::ConverseImplies => lhs || !rhs,
            BinOp::NonImplies => lhs && !rhs,
            BinOp::ConverseNonImplies => !lhs && rhs,
            BinOp::Iff => lhs == rhs,
            BinOp::NonIff => lhs != rhs,
        }
    }

    pub fn precedence(self) -> u8 {
        match self {
            BinOp::Iff | BinOp::NonIff => 1,
            BinOp::Implies
            | BinOp::ConverseImplies
            | BinOp::NonImplies
            | BinOp::ConverseNonImplies => 2,
            BinOp::Or | BinOp::Xor | BinOp::Nor => 3,
            BinOp::And | BinOp::Nand => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuantKind {
    ForAll,    // ∀
    Exists,    // ∃
    NotExists, // ∄
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Formula {
    Const(bool),
    Atom(String),
    Predicate {
        name: String,
        args: Vec<String>,
    },
    Not(Box<Formula>),
    Binary {
        op: BinOp,
        lhs: Box<Formula>,
        rhs: Box<Formula>,
    },
    Quantified {
        kind: QuantKind,
        var: String,
        body: Box<Formula>,
    },
}

impl Formula {
    pub fn atom(name: impl Into<String>) -> Self {
        Formula::Atom(name.into())
    }

    pub fn negate(inner: Formula) -> Self {
        Formula::Not(Box::new(inner))
    }

    pub fn binary(op: BinOp, lhs: Formula, rhs: Formula) -> Self {
        Formula::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// Collects all propositional atoms in canonical alphabetical order.
    pub fn atoms_alphabetical(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        self.collect_atoms(&mut set);
        set.into_iter().collect()
    }

    pub fn collect_atoms(&self, out: &mut BTreeSet<String>) {
        match self {
            Formula::Const(_) => {}
            Formula::Atom(name) => {
                out.insert(name.clone());
            }
            Formula::Predicate { name, args } => {
                if args.is_empty() {
                    out.insert(name.clone());
                } else {
                    out.insert(format!("{}({})", name, args.join(",")));
                }
            }
            Formula::Not(inner) => inner.collect_atoms(out),
            Formula::Binary { lhs, rhs, .. } => {
                lhs.collect_atoms(out);
                rhs.collect_atoms(out);
            }
            Formula::Quantified { body, .. } => body.collect_atoms(out),
        }
    }

    /// Checks whether this formula is a literal (`p`, `¬p`, `⊤`, or `⊥`).
    pub fn is_literal(&self) -> bool {
        match self {
            Formula::Const(_) | Formula::Atom(_) | Formula::Predicate { .. } => true,
            Formula::Not(inner) => matches!(
                inner.as_ref(),
                Formula::Const(_) | Formula::Atom(_) | Formula::Predicate { .. }
            ),
            _ => false,
        }
    }

    /// Checks if `self` and `other` are direct syntactic complements (`A` and `¬A`).
    pub fn contradicts(&self, other: &Formula) -> bool {
        if matches!(self, Formula::Const(false)) || matches!(other, Formula::Const(false)) {
            return true;
        }
        if let Formula::Not(inner) = self {
            if inner.as_ref() == other {
                return true;
            }
        }
        if let Formula::Not(inner) = other {
            if inner.as_ref() == self {
                return true;
            }
        }
        false
    }

    pub fn substitute_var(&self, var: &str, constant: &str) -> Formula {
        match self {
            Formula::Const(b) => Formula::Const(*b),
            Formula::Atom(a) => {
                if a == var {
                    Formula::Atom(constant.to_string())
                } else {
                    Formula::Atom(a.clone())
                }
            }
            Formula::Predicate { name, args } => Formula::Predicate {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| {
                        if arg == var {
                            constant.to_string()
                        } else {
                            arg.clone()
                        }
                    })
                    .collect(),
            },
            Formula::Not(inner) => Formula::Not(Box::new(inner.substitute_var(var, constant))),
            Formula::Binary { op, lhs, rhs } => Formula::Binary {
                op: *op,
                lhs: Box::new(lhs.substitute_var(var, constant)),
                rhs: Box::new(rhs.substitute_var(var, constant)),
            },
            Formula::Quantified {
                kind,
                var: qvar,
                body,
            } => {
                if qvar == var {
                    self.clone()
                } else {
                    Formula::Quantified {
                        kind: *kind,
                        var: qvar.clone(),
                        body: Box::new(body.substitute_var(var, constant)),
                    }
                }
            }
        }
    }
}

impl fmt::Display for Formula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Formula::Const(true) => write!(f, "⊤"),
            Formula::Const(false) => write!(f, "⊥"),
            Formula::Atom(name) => write!(f, "{}", name),
            Formula::Predicate { name, args } => {
                if args.is_empty() {
                    write!(f, "{}", name)
                } else {
                    write!(f, "{}({})", name, args.join(", "))
                }
            }
            Formula::Not(inner) => match inner.as_ref() {
                Formula::Binary { .. } => write!(f, "¬({})", inner),
                _ => write!(f, "¬{}", inner),
            },
            Formula::Binary { op, lhs, rhs } => {
                let my_prec = op.precedence();
                let lhs_parens = match lhs.as_ref() {
                    Formula::Binary { op: lop, .. } => lop.precedence() <= my_prec || lop != op,
                    _ => false,
                };
                let rhs_parens = match rhs.as_ref() {
                    Formula::Binary { op: rop, .. } => rop.precedence() <= my_prec || rop != op,
                    _ => false,
                };
                if lhs_parens {
                    write!(f, "({})", lhs)?;
                } else {
                    write!(f, "{}", lhs)?;
                }
                write!(f, " {} ", op.symbol_unicode())?;
                if rhs_parens {
                    write!(f, "({})", rhs)
                } else {
                    write!(f, "{}", rhs)
                }
            }
            Formula::Quantified { kind, var, body } => {
                let sym = match kind {
                    QuantKind::ForAll => "∀",
                    QuantKind::Exists => "∃",
                    QuantKind::NotExists => "∄",
                };
                match body.as_ref() {
                    Formula::Binary { .. } => write!(f, "{}{}({})", sym, var, body),
                    _ => write!(f, "{}{}{}", sym, var, body),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetalogicStmt {
    Equivalence(Formula, Formula),
    NonEquivalence(Formula, Formula),
    Entailment {
        premises: Vec<Formula>,
        conclusion: Formula,
        negated: bool,
    },
    FormulaSet(Vec<Formula>),
    Single(Formula),
}
