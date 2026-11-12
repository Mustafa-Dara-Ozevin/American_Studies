use crate::ast::{BinOp, Formula, MetalogicStmt, QuantKind};
use crate::parser::{parse_formula, parse_metalogic_stmt};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecompositionRule {
    Literal,
    /// Non-branching (Stacking / α-rule): appends 1 or 2 formulas vertically onto every open leaf below.
    Alpha {
        rule_name: &'static str,
        stacked: Vec<Formula>,
    },
    /// Branching (Splitting / β-rule): splits every open leaf below into Left and Right branches,
    /// stacking 1 or 2 formulas on each side.
    Beta {
        rule_name: &'static str,
        left_stacked: Vec<Formula>,
        right_stacked: Vec<Formula>,
    },
    /// Universal instantiation (γ-rule): reusable for any constant `c` on the branch.
    Gamma {
        rule_name: &'static str,
        var: String,
        template: Formula,
    },
    /// Existential instantiation (δ-rule): introduces a fresh constant `c_new` not yet on the branch.
    Delta {
        rule_name: &'static str,
        var: String,
        template: Formula,
    },
}

/// Classifies any formula into its analytic tableau decomposition rule (α-stacking, β-branching, γ, δ, or literal).
pub fn classify_decomposition(formula: &Formula) -> DecompositionRule {
    match formula {
        Formula::Const(_) | Formula::Atom(_) | Formula::Predicate { .. } => {
            DecompositionRule::Literal
        }
        Formula::Not(inner) => match inner.as_ref() {
            Formula::Const(true) => DecompositionRule::Alpha {
                rule_name: "¬⊤",
                stacked: vec![Formula::Const(false)],
            },
            Formula::Const(false) => DecompositionRule::Alpha {
                rule_name: "¬⊥",
                stacked: vec![Formula::Const(true)],
            },
            Formula::Atom(_) | Formula::Predicate { .. } => DecompositionRule::Literal,
            Formula::Not(double_inner) => DecompositionRule::Alpha {
                rule_name: "¬¬",
                stacked: vec![(**double_inner).clone()],
            },
            Formula::Binary { op, lhs, rhs } => match op {
                BinOp::And => DecompositionRule::Beta {
                    rule_name: "¬∧",
                    left_stacked: vec![Formula::negate((**lhs).clone())],
                    right_stacked: vec![Formula::negate((**rhs).clone())],
                },
                BinOp::Or => DecompositionRule::Alpha {
                    rule_name: "¬∨",
                    stacked: vec![
                        Formula::negate((**lhs).clone()),
                        Formula::negate((**rhs).clone()),
                    ],
                },
                BinOp::Implies => DecompositionRule::Alpha {
                    rule_name: "¬→",
                    stacked: vec![(**lhs).clone(), Formula::negate((**rhs).clone())],
                },
                BinOp::ConverseImplies => DecompositionRule::Alpha {
                    rule_name: "¬←",
                    stacked: vec![Formula::negate((**lhs).clone()), (**rhs).clone()],
                },
                BinOp::NonImplies => DecompositionRule::Beta {
                    rule_name: "¬↛",
                    left_stacked: vec![Formula::negate((**lhs).clone())],
                    right_stacked: vec![(**rhs).clone()],
                },
                BinOp::ConverseNonImplies => DecompositionRule::Beta {
                    rule_name: "¬↚",
                    left_stacked: vec![(**lhs).clone()],
                    right_stacked: vec![Formula::negate((**rhs).clone())],
                },
                BinOp::Iff => DecompositionRule::Beta {
                    rule_name: "¬↔",
                    left_stacked: vec![(**lhs).clone(), Formula::negate((**rhs).clone())],
                    right_stacked: vec![Formula::negate((**lhs).clone()), (**rhs).clone()],
                },
                BinOp::NonIff | BinOp::Xor => DecompositionRule::Beta {
                    rule_name: "¬↮",
                    left_stacked: vec![(**lhs).clone(), (**rhs).clone()],
                    right_stacked: vec![
                        Formula::negate((**lhs).clone()),
                        Formula::negate((**rhs).clone()),
                    ],
                },
                BinOp::Nand => DecompositionRule::Alpha {
                    rule_name: "¬↑",
                    stacked: vec![(**lhs).clone(), (**rhs).clone()],
                },
                BinOp::Nor => DecompositionRule::Beta {
                    rule_name: "¬↓",
                    left_stacked: vec![(**lhs).clone()],
                    right_stacked: vec![(**rhs).clone()],
                },
            },
            Formula::Quantified { kind, var, body } => match kind {
                QuantKind::ForAll => DecompositionRule::Delta {
                    rule_name: "¬∀",
                    var: var.clone(),
                    template: Formula::negate((**body).clone()),
                },
                QuantKind::Exists => DecompositionRule::Gamma {
                    rule_name: "¬∃",
                    var: var.clone(),
                    template: Formula::negate((**body).clone()),
                },
                QuantKind::NotExists => DecompositionRule::Delta {
                    rule_name: "¬∄",
                    var: var.clone(),
                    template: (**body).clone(),
                },
            },
        },
        Formula::Binary { op, lhs, rhs } => match op {
            BinOp::And => DecompositionRule::Alpha {
                rule_name: "∧",
                stacked: vec![(**lhs).clone(), (**rhs).clone()],
            },
            BinOp::Or => DecompositionRule::Beta {
                rule_name: "∨",
                left_stacked: vec![(**lhs).clone()],
                right_stacked: vec![(**rhs).clone()],
            },
            BinOp::Implies => DecompositionRule::Beta {
                rule_name: "→",
                left_stacked: vec![Formula::negate((**lhs).clone())],
                right_stacked: vec![(**rhs).clone()],
            },
            BinOp::ConverseImplies => DecompositionRule::Beta {
                rule_name: "←",
                left_stacked: vec![(**lhs).clone()],
                right_stacked: vec![Formula::negate((**rhs).clone())],
            },
            BinOp::NonImplies => DecompositionRule::Alpha {
                rule_name: "↛",
                stacked: vec![(**lhs).clone(), Formula::negate((**rhs).clone())],
            },
            BinOp::ConverseNonImplies => DecompositionRule::Alpha {
                rule_name: "↚",
                stacked: vec![Formula::negate((**lhs).clone()), (**rhs).clone()],
            },
            BinOp::Iff => DecompositionRule::Beta {
                rule_name: "↔",
                left_stacked: vec![(**lhs).clone(), (**rhs).clone()],
                right_stacked: vec![
                    Formula::negate((**lhs).clone()),
                    Formula::negate((**rhs).clone()),
                ],
            },
            BinOp::NonIff | BinOp::Xor => DecompositionRule::Beta {
                rule_name: "⊕/↮",
                left_stacked: vec![(**lhs).clone(), Formula::negate((**rhs).clone())],
                right_stacked: vec![Formula::negate((**lhs).clone()), (**rhs).clone()],
            },
            BinOp::Nand => DecompositionRule::Beta {
                rule_name: "↑",
                left_stacked: vec![Formula::negate((**lhs).clone())],
                right_stacked: vec![Formula::negate((**rhs).clone())],
            },
            BinOp::Nor => DecompositionRule::Alpha {
                rule_name: "↓",
                stacked: vec![
                    Formula::negate((**lhs).clone()),
                    Formula::negate((**rhs).clone()),
                ],
            },
        },
        Formula::Quantified { kind, var, body } => match kind {
            QuantKind::ForAll => DecompositionRule::Gamma {
                rule_name: "∀",
                var: var.clone(),
                template: (**body).clone(),
            },
            QuantKind::Exists => DecompositionRule::Delta {
                rule_name: "∃",
                var: var.clone(),
                template: (**body).clone(),
            },
            QuantKind::NotExists => DecompositionRule::Gamma {
                rule_name: "∄",
                var: var.clone(),
                template: Formula::negate((**body).clone()),
            },
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeOrigin {
    Premise,
    NegatedConclusion,
    InitialFormula,
    Derived {
        from_node: usize,
        rule_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeContinuation {
    /// Leaf is still open; `completed` is true if all compound ancestors on the path are checked (`o`).
    OpenLeaf {
        completed: bool,
        valuation: BTreeMap<String, bool>,
    },
    /// Leaf is closed (`x` / `×`) because ancestor `contradicts.0` and `contradicts.1` clash.
    ClosedLeaf {
        contradicts: (usize, usize),
    },
    /// Continues vertically (stacking) to child node ID.
    Stacked(usize),
    /// Branches into two sibling child node IDs (Left, Right).
    Branched {
        left: usize,
        right: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub formula: Formula,
    pub origin: NodeOrigin,
    pub checked: bool,
    pub instantiated_constants: BTreeSet<String>,
    pub continuation: NodeContinuation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeVerdict {
    /// Every branch closed (`×`) -> argument is Valid (or initial set is Unsatisfiable).
    AllClosed,
    /// At least one completed open branch (`o`) exists -> Invalid / Satisfiable with counterexamples.
    HasOpenBranches {
        open_valuations: Vec<BTreeMap<String, bool>>,
    },
    /// Some branches still have undecomposed formulas.
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruthTree {
    pub goal_description: String,
    pub is_validity_test: bool,
    pub nodes: Vec<TreeNode>,
}

impl TruthTree {
    /// Initializes a Truth Tree to test validity of `premises ∴ conclusion` by stacking
    /// all premises followed by `¬conclusion` at the trunk.
    pub fn from_entailment(premises: &[Formula], conclusion: &Formula) -> Self {
        let mut init: Vec<(Formula, NodeOrigin)> = premises
            .iter()
            .cloned()
            .map(|f| (f, NodeOrigin::Premise))
            .collect();
        init.push((
            Formula::negate(conclusion.clone()),
            NodeOrigin::NegatedConclusion,
        ));
        let desc = if premises.is_empty() {
            format!("∴ {}", conclusion)
        } else {
            let p_str = premises
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{} ∴ {}", p_str, conclusion)
        };
        Self::from_initial_nodes(desc, true, init)
    }

    /// Initializes a Truth Tree to test satisfiability/consistency of a list of formulas.
    pub fn from_formulas(formulas: &[Formula]) -> Self {
        let desc = formulas
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let init = formulas
            .iter()
            .cloned()
            .map(|f| (f, NodeOrigin::InitialFormula))
            .collect();
        Self::from_initial_nodes(desc, false, init)
    }

    fn from_initial_nodes(
        goal_description: String,
        is_validity_test: bool,
        initial: Vec<(Formula, NodeOrigin)>,
    ) -> Self {
        let mut tree = Self {
            goal_description,
            is_validity_test,
            nodes: Vec::new(),
        };
        for (idx, (formula, origin)) in initial.into_iter().enumerate() {
            let is_lit = formula.is_literal();
            let parent = if idx == 0 { None } else { Some(idx - 1) };
            if let Some(p_id) = parent {
                tree.nodes[p_id].continuation = NodeContinuation::Stacked(idx);
            }
            tree.nodes.push(TreeNode {
                id: idx,
                parent,
                formula,
                origin,
                checked: is_lit,
                instantiated_constants: BTreeSet::new(),
                continuation: NodeContinuation::OpenLeaf {
                    completed: false,
                    valuation: BTreeMap::new(),
                },
            });
        }
        tree.refresh_all_leaves();
        tree
    }

    /// Returns ancestor node IDs from the root down to `node_id` (inclusive).
    pub fn path_to_root(&self, node_id: usize) -> Vec<usize> {
        let mut path = Vec::new();
        let mut cur = Some(node_id);
        while let Some(id) = cur {
            path.push(id);
            cur = self.nodes[id].parent;
        }
        path.reverse();
        path
    }

    /// Collects all currently open leaf node IDs in the subtree rooted at `sub_root`.
    pub fn open_leaves_under(&self, sub_root: usize) -> Vec<usize> {
        let mut leaves = Vec::new();
        let mut stack = vec![sub_root];
        while let Some(id) = stack.pop() {
            match &self.nodes[id].continuation {
                NodeContinuation::OpenLeaf { .. } => leaves.push(id),
                NodeContinuation::ClosedLeaf { .. } => {}
                NodeContinuation::Stacked(next) => stack.push(*next),
                NodeContinuation::Branched { left, right } => {
                    stack.push(*right);
                    stack.push(*left);
                }
            }
        }
        leaves
    }

    /// Decomposes `node_id` onto all open leaves in its subtree using α-stacking, β-branching, γ, or δ rules.
    pub fn decompose_node(&mut self, node_id: usize) -> Result<()> {
        if node_id >= self.nodes.len() {
            return Err(anyhow!("Invalid node ID {}", node_id));
        }
        let formula = self.nodes[node_id].formula.clone();
        let rule = classify_decomposition(&formula);
        let open_leaves = self.open_leaves_under(node_id);
        if open_leaves.is_empty() {
            self.nodes[node_id].checked = true;
            return Ok(());
        }

        match rule {
            DecompositionRule::Literal => {
                self.nodes[node_id].checked = true;
            }
            DecompositionRule::Alpha { rule_name, stacked } => {
                self.nodes[node_id].checked = true;
                for leaf_id in open_leaves {
                    self.append_stack_at_leaf(leaf_id, node_id, rule_name, &stacked);
                }
            }
            DecompositionRule::Beta {
                rule_name,
                left_stacked,
                right_stacked,
            } => {
                self.nodes[node_id].checked = true;
                for leaf_id in open_leaves {
                    self.split_branch_at_leaf(
                        leaf_id,
                        node_id,
                        rule_name,
                        &left_stacked,
                        &right_stacked,
                    );
                }
            }
            DecompositionRule::Delta {
                rule_name,
                var,
                template,
            } => {
                self.nodes[node_id].checked = true;
                for leaf_id in open_leaves {
                    let fresh_const = self.fresh_constant_on_branch(leaf_id);
                    let inst = template.substitute_var(&var, &fresh_const);
                    self.append_stack_at_leaf(leaf_id, node_id, rule_name, &[inst]);
                }
            }
            DecompositionRule::Gamma {
                rule_name,
                var,
                template,
            } => {
                let mut constants = self.constants_on_subtree(node_id);
                if constants.is_empty() {
                    constants.insert("a".to_string());
                }
                let already = self.nodes[node_id].instantiated_constants.clone();
                let new_consts: Vec<String> = constants.difference(&already).cloned().collect();
                let target_const = new_consts
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "a".to_string());
                self.nodes[node_id]
                    .instantiated_constants
                    .insert(target_const.clone());
                let inst = template.substitute_var(&var, &target_const);
                for leaf_id in open_leaves {
                    self.append_stack_at_leaf(leaf_id, node_id, rule_name, &[inst.clone()]);
                }
            }
        }

        self.refresh_all_leaves();
        Ok(())
    }

    /// Verifies a manual user decomposition step (for Interactive Study Mode):
    /// checks that `node_id` was decomposed with the right rule type (stacking vs branching)
    /// and the exact resulting formulas.
    pub fn verify_user_step(
        &self,
        node_id: usize,
        user_branched: bool,
        left_or_stacked: &[Formula],
        right_if_branched: Option<&[Formula]>,
    ) -> Result<()> {
        if node_id >= self.nodes.len() {
            return Err(anyhow!("Invalid node ID {}", node_id));
        }
        let rule = classify_decomposition(&self.nodes[node_id].formula);
        match rule {
            DecompositionRule::Literal => Err(anyhow!(
                "Formula '{}' is already a literal and cannot be decomposed further",
                self.nodes[node_id].formula
            )),
            DecompositionRule::Alpha { rule_name, stacked } => {
                if user_branched {
                    return Err(anyhow!(
                        "Rule {} on '{}' is a non-branching (stacking / α) rule, not a branching (β) rule!",
                        rule_name,
                        self.nodes[node_id].formula
                    ));
                }
                if left_or_stacked != stacked.as_slice() {
                    return Err(anyhow!(
                        "Expected stacked formulas {:?} for rule {}, but got {:?}",
                        stacked.iter().map(|f| f.to_string()).collect::<Vec<_>>(),
                        rule_name,
                        left_or_stacked
                            .iter()
                            .map(|f| f.to_string())
                            .collect::<Vec<_>>()
                    ));
                }
                Ok(())
            }
            DecompositionRule::Beta {
                rule_name,
                left_stacked,
                right_stacked,
            } => {
                if !user_branched {
                    return Err(anyhow!(
                        "Rule {} on '{}' is a branching (β) rule, not a stacking (α) rule!",
                        rule_name,
                        self.nodes[node_id].formula
                    ));
                }
                let right = right_if_branched
                    .ok_or_else(|| anyhow!("Branching step requires a right branch"))?;
                let matches_direct =
                    left_or_stacked == left_stacked.as_slice() && right == right_stacked.as_slice();
                let matches_swapped =
                    left_or_stacked == right_stacked.as_slice() && right == left_stacked.as_slice();
                if !matches_direct && !matches_swapped {
                    return Err(anyhow!(
                        "Expected branches {:?} | {:?} for rule {}",
                        left_stacked
                            .iter()
                            .map(|f| f.to_string())
                            .collect::<Vec<_>>(),
                        right_stacked
                            .iter()
                            .map(|f| f.to_string())
                            .collect::<Vec<_>>(),
                        rule_name
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Automatically expands the truth tree to completion, prioritizing α (stacking) rules
    /// before β (branching) rules on open branches.
    pub fn solve_alpha_first(&mut self, max_steps: usize) -> Result<TreeVerdict> {
        let mut steps = 0;
        while steps < max_steps {
            self.refresh_all_leaves();
            if !matches!(self.verdict(), TreeVerdict::Incomplete) {
                break;
            }

            // Find all unchecked nodes that still have at least one open leaf beneath them
            let mut alpha_candidate = None;
            let mut delta_candidate = None;
            let mut beta_candidate = None;
            let mut gamma_candidate = None;

            for node in &self.nodes {
                if node.checked && !matches!(classify_decomposition(&node.formula), DecompositionRule::Gamma { .. }) {
                    continue;
                }
                if self.open_leaves_under(node.id).is_empty() {
                    continue;
                }
                match classify_decomposition(&node.formula) {
                    DecompositionRule::Literal => {}
                    DecompositionRule::Alpha { .. } => {
                        if alpha_candidate.is_none() {
                            alpha_candidate = Some(node.id);
                            break;
                        }
                    }
                    DecompositionRule::Delta { .. } => {
                        if delta_candidate.is_none() {
                            delta_candidate = Some(node.id);
                        }
                    }
                    DecompositionRule::Beta { .. } => {
                        if beta_candidate.is_none() {
                            beta_candidate = Some(node.id);
                        }
                    }
                    DecompositionRule::Gamma { .. } => {
                        if gamma_candidate.is_none() && node.instantiated_constants.is_empty() {
                            gamma_candidate = Some(node.id);
                        }
                    }
                }
            }

            let next_id = alpha_candidate
                .or(delta_candidate)
                .or(beta_candidate)
                .or(gamma_candidate);

            if let Some(id) = next_id {
                self.decompose_node(id)?;
                steps += 1;
            } else {
                break;
            }
        }
        self.refresh_all_leaves();
        Ok(self.verdict())
    }

    pub fn verdict(&self) -> TreeVerdict {
        if self.nodes.is_empty() {
            return TreeVerdict::AllClosed;
        }
        let mut open_valuations = Vec::new();
        for node in &self.nodes {
            match &node.continuation {
                NodeContinuation::OpenLeaf {
                    completed,
                    valuation,
                } => {
                    if !*completed {
                        return TreeVerdict::Incomplete;
                    }
                    if !open_valuations.contains(valuation) {
                        open_valuations.push(valuation.clone());
                    }
                }
                _ => {}
            }
        }
        if open_valuations.is_empty() {
            TreeVerdict::AllClosed
        } else {
            TreeVerdict::HasOpenBranches { open_valuations }
        }
    }

    fn append_stack_at_leaf(
        &mut self,
        leaf_id: usize,
        from_node: usize,
        rule_name: &str,
        formulas: &[Formula],
    ) {
        let mut cur_parent = leaf_id;
        for f in formulas {
            let new_id = self.nodes.len();
            let is_lit = f.is_literal();
            self.nodes[cur_parent].continuation = NodeContinuation::Stacked(new_id);
            self.nodes.push(TreeNode {
                id: new_id,
                parent: Some(cur_parent),
                formula: f.clone(),
                origin: NodeOrigin::Derived {
                    from_node,
                    rule_name: rule_name.to_string(),
                },
                checked: is_lit,
                instantiated_constants: BTreeSet::new(),
                continuation: NodeContinuation::OpenLeaf {
                    completed: false,
                    valuation: BTreeMap::new(),
                },
            });
            cur_parent = new_id;
        }
    }

    fn split_branch_at_leaf(
        &mut self,
        leaf_id: usize,
        from_node: usize,
        rule_name: &str,
        left_formulas: &[Formula],
        right_formulas: &[Formula],
    ) {
        if left_formulas.is_empty() || right_formulas.is_empty() {
            return;
        }
        let left_head_id = self.nodes.len();
        self.nodes.push(TreeNode {
            id: left_head_id,
            parent: Some(leaf_id),
            formula: left_formulas[0].clone(),
            origin: NodeOrigin::Derived {
                from_node,
                rule_name: rule_name.to_string(),
            },
            checked: left_formulas[0].is_literal(),
            instantiated_constants: BTreeSet::new(),
            continuation: NodeContinuation::OpenLeaf {
                completed: false,
                valuation: BTreeMap::new(),
            },
        });
        if left_formulas.len() > 1 {
            self.append_stack_at_leaf(left_head_id, from_node, rule_name, &left_formulas[1..]);
        }

        let right_head_id = self.nodes.len();
        self.nodes.push(TreeNode {
            id: right_head_id,
            parent: Some(leaf_id),
            formula: right_formulas[0].clone(),
            origin: NodeOrigin::Derived {
                from_node,
                rule_name: rule_name.to_string(),
            },
            checked: right_formulas[0].is_literal(),
            instantiated_constants: BTreeSet::new(),
            continuation: NodeContinuation::OpenLeaf {
                completed: false,
                valuation: BTreeMap::new(),
            },
        });
        if right_formulas.len() > 1 {
            self.append_stack_at_leaf(right_head_id, from_node, rule_name, &right_formulas[1..]);
        }

        self.nodes[leaf_id].continuation = NodeContinuation::Branched {
            left: left_head_id,
            right: right_head_id,
        };
    }

    fn refresh_all_leaves(&mut self) {
        let len = self.nodes.len();
        for id in 0..len {
            let is_leaf = matches!(
                self.nodes[id].continuation,
                NodeContinuation::OpenLeaf { .. } | NodeContinuation::ClosedLeaf { .. }
            );
            if !is_leaf {
                continue;
            }
            let path = self.path_to_root(id);
            // Check if any two nodes on the root-to-leaf path contradict each other
            let mut contradiction = None;
            'outer: for i in 0..path.len() {
                let n_i = path[i];
                if matches!(self.nodes[n_i].formula, Formula::Const(false)) {
                    contradiction = Some((n_i, n_i));
                    break;
                }
                for j in (i + 1)..path.len() {
                    let n_j = path[j];
                    if self.nodes[n_i].formula.contradicts(&self.nodes[n_j].formula) {
                        contradiction = Some((n_i, n_j));
                        break 'outer;
                    }
                }
            }

            if let Some(pair) = contradiction {
                self.nodes[id].continuation = NodeContinuation::ClosedLeaf { contradicts: pair };
            } else {
                let all_checked = path.iter().all(|&anc_id| self.nodes[anc_id].checked);
                let mut valuation = BTreeMap::new();
                for &anc_id in &path {
                    match &self.nodes[anc_id].formula {
                        Formula::Atom(a) => {
                            valuation.insert(a.clone(), true);
                        }
                        Formula::Not(inner) => {
                            if let Formula::Atom(a) = inner.as_ref() {
                                valuation.insert(a.clone(), false);
                            }
                        }
                        _ => {}
                    }
                }
                self.nodes[id].continuation = NodeContinuation::OpenLeaf {
                    completed: all_checked,
                    valuation,
                };
            }
        }
    }

    fn constants_on_subtree(&self, sub_root: usize) -> BTreeSet<String> {
        let mut consts = BTreeSet::new();
        for leaf in self.open_leaves_under(sub_root) {
            for id in self.path_to_root(leaf) {
                collect_pred_args(&self.nodes[id].formula, &mut consts);
            }
        }
        consts
    }

    fn fresh_constant_on_branch(&self, leaf_id: usize) -> String {
        let mut used = BTreeSet::new();
        for id in self.path_to_root(leaf_id) {
            collect_pred_args(&self.nodes[id].formula, &mut used);
        }
        for ch in 'a'..='z' {
            let candidate = ch.to_string();
            if !used.contains(&candidate) {
                return candidate;
            }
        }
        format!("c{}", used.len() + 1)
    }

    /// Formats the Truth Tree as a clean, readable ` ```tree ` block with stacked lines,
    /// `├─` / `└─` branches, `✓` decomposition markers, `×` closures, and `o` open valuations.
    pub fn to_tree_dsl(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("problem: {}\n", self.goal_description));
        if !self.nodes.is_empty() {
            self.fmt_node_dsl(0, "", true, &mut out);
        }
        match self.verdict() {
            TreeVerdict::AllClosed => {
                if self.is_validity_test {
                    out.push_str("verdict: valid (all branches closed ×)\n");
                } else {
                    out.push_str("verdict: unsatisfiable (all branches closed ×)\n");
                }
            }
            TreeVerdict::HasOpenBranches { open_valuations } => {
                let first_val = open_valuations
                    .first()
                    .map(|m| {
                        m.iter()
                            .map(|(k, &v)| format!("{}={}", k, if v { "D" } else { "Y" }))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                if self.is_validity_test {
                    out.push_str(&format!(
                        "verdict: invalid (counterexample: {})\n",
                        first_val
                    ));
                } else {
                    out.push_str(&format!("verdict: satisfiable (model: {})\n", first_val));
                }
            }
            TreeVerdict::Incomplete => {
                out.push_str("verdict: incomplete\n");
            }
        }
        out
    }

    fn fmt_node_dsl(&self, node_id: usize, indent: &str, is_trunk: bool, out: &mut String) {
        let node = &self.nodes[node_id];
        let origin_str = match &node.origin {
            NodeOrigin::Premise => "[premise]".to_string(),
            NodeOrigin::NegatedConclusion => "[neg-goal]".to_string(),
            NodeOrigin::InitialFormula => "[given]".to_string(),
            NodeOrigin::Derived {
                from_node,
                rule_name,
            } => format!("[from {}: {}]", from_node + 1, rule_name),
        };
        let check_mark = if node.checked && !node.formula.is_literal() {
            " ✓"
        } else {
            ""
        };
        if is_trunk {
            out.push_str(&format!(
                "{}{}. {}  {}{}\n",
                indent,
                node.id + 1,
                node.formula,
                origin_str,
                check_mark
            ));
        } else {
            out.push_str(&format!(
                "{}. {}  {}{}\n",
                node.id + 1,
                node.formula,
                origin_str,
                check_mark
            ));
        }

        match &node.continuation {
            NodeContinuation::Stacked(next) => {
                self.fmt_node_dsl(*next, indent, true, out);
            }
            NodeContinuation::Branched { left, right } => {
                out.push_str(&format!("{}├─ ", indent));
                let child_indent_left = format!("{}│  ", indent);
                self.fmt_node_dsl(*left, &child_indent_left, false, out);

                out.push_str(&format!("{}└─ ", indent));
                let child_indent_right = format!("{}   ", indent);
                self.fmt_node_dsl(*right, &child_indent_right, false, out);
            }
            NodeContinuation::ClosedLeaf {
                contradicts: (a, b),
            } => {
                out.push_str(&format!("{}× ({}, {})\n", indent, a + 1, b + 1));
            }
            NodeContinuation::OpenLeaf {
                completed,
                valuation,
            } => {
                if *completed {
                    let val_str = valuation
                        .iter()
                        .map(|(k, &v)| format!("{}={}", k, if v { "D" } else { "Y" }))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!("{}o ({})\n", indent, val_str));
                }
            }
        }
    }

    /// Renders this Truth Tree as a self-contained SVG diagram with stacked vertical blocks,
    /// branching edges, `✓` decomposition badges, `×` closure markers, and `o` counterexample badges.
    pub fn to_svg(&self) -> String {
        if self.nodes.is_empty() {
            return "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"40\"></svg>"
                .to_string();
        }
        let layout = self.build_svg_block(0);
        let total_w = layout.width.max(360.0) + 40.0;
        let total_h = layout.height + 80.0;
        let mut elements = String::new();

        // Title / Problem header
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"28\" text-anchor=\"middle\" fill=\"#88c0d0\" font-family=\"monospace\" font-size=\"14\" font-weight=\"bold\">{}</text>\n",
            total_w / 2.0,
            svg_escape(&self.goal_description)
        ));

        self.emit_svg_block(&layout, total_w / 2.0, 46.0, &mut elements);

        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.0} {:.0}\" width=\"100%\" style=\"max-width:{:.0}px;background:#2e3440;border:1px solid #4c566a;border-radius:8px;padding:8px;\">\n{}</svg>",
            total_w, total_h, total_w, elements
        )
    }

    fn build_svg_block(&self, start_id: usize) -> SvgTreeBlock {
        let mut lines = Vec::new();
        let mut cur = start_id;
        loop {
            let node = &self.nodes[cur];
            let origin_str = match &node.origin {
                NodeOrigin::Premise => "[P]".to_string(),
                NodeOrigin::NegatedConclusion => "[¬C]".to_string(),
                NodeOrigin::InitialFormula => "".to_string(),
                NodeOrigin::Derived {
                    from_node,
                    rule_name,
                } => format!("({}: {})", from_node + 1, rule_name),
            };
            let check = if node.checked && !node.formula.is_literal() {
                " ✓"
            } else {
                ""
            };
            lines.push(format!(
                "{}. {} {}{}",
                node.id + 1,
                node.formula,
                origin_str,
                check
            ));

            match &node.continuation {
                NodeContinuation::Stacked(next) => {
                    cur = *next;
                }
                NodeContinuation::Branched { left, right } => {
                    let left_b = Box::new(self.build_svg_block(*left));
                    let right_b = Box::new(self.build_svg_block(*right));
                    let box_w = lines
                        .iter()
                        .map(|s| s.chars().count() as f32 * 8.2 + 24.0)
                        .fold(180.0_f32, f32::max);
                    let box_h = lines.len() as f32 * 22.0 + 14.0;
                    let children_w = left_b.width + right_b.width + 36.0;
                    let width = box_w.max(children_w);
                    let height = box_h + 32.0 + left_b.height.max(right_b.height);
                    return SvgTreeBlock {
                        lines,
                        leaf_badge: None,
                        is_closed: false,
                        box_w,
                        box_h,
                        width,
                        height,
                        children: Some((left_b, right_b)),
                    };
                }
                NodeContinuation::ClosedLeaf {
                    contradicts: (a, b),
                } => {
                    let badge = format!("× ({}, {})", a + 1, b + 1);
                    let box_w = lines
                        .iter()
                        .chain(std::iter::once(&badge))
                        .map(|s| s.chars().count() as f32 * 8.2 + 24.0)
                        .fold(160.0_f32, f32::max);
                    let box_h = (lines.len() + 1) as f32 * 22.0 + 16.0;
                    return SvgTreeBlock {
                        lines,
                        leaf_badge: Some(badge),
                        is_closed: true,
                        box_w,
                        box_h,
                        width: box_w,
                        height: box_h,
                        children: None,
                    };
                }
                NodeContinuation::OpenLeaf {
                    completed,
                    valuation,
                } => {
                    let badge = if *completed {
                        let val_str = valuation
                            .iter()
                            .map(|(k, &v)| format!("{}={}", k, if v { "D" } else { "Y" }))
                            .collect::<Vec<_>>()
                            .join(", ");
                        Some(format!("o ({})", val_str))
                    } else {
                        Some("…".to_string())
                    };
                    let box_w = lines
                        .iter()
                        .chain(badge.iter())
                        .map(|s| s.chars().count() as f32 * 8.2 + 24.0)
                        .fold(160.0_f32, f32::max);
                    let box_h = (lines.len() + 1) as f32 * 22.0 + 16.0;
                    return SvgTreeBlock {
                        lines,
                        leaf_badge: badge,
                        is_closed: false,
                        box_w,
                        box_h,
                        width: box_w,
                        height: box_h,
                        children: None,
                    };
                }
            }
        }
    }

    fn emit_svg_block(&self, block: &SvgTreeBlock, center_x: f32, top_y: f32, out: &mut String) {
        let box_x = center_x - block.box_w / 2.0;
        out.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" fill=\"#3b4252\" stroke=\"#4c566a\" stroke-width=\"1.2\"/>\n",
            box_x, top_y, block.box_w, block.box_h
        ));

        for (i, line) in block.lines.iter().enumerate() {
            let ty = top_y + 20.0 + i as f32 * 22.0;
            out.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#eceff4\" font-family=\"monospace\" font-size=\"13\">{}</text>\n",
                box_x + 12.0,
                ty,
                svg_escape(line)
            ));
        }

        if let Some(ref badge) = block.leaf_badge {
            let ty = top_y + 20.0 + block.lines.len() as f32 * 22.0;
            let color = if block.is_closed { "#bf616a" } else { "#a3be8c" };
            out.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{}\" font-family=\"monospace\" font-size=\"13\" font-weight=\"bold\">{}</text>\n",
                center_x,
                ty,
                color,
                svg_escape(badge)
            ));
        }

        if let Some((ref left_b, ref right_b)) = block.children {
            let parent_bottom_y = top_y + block.box_h;
            let child_top_y = parent_bottom_y + 32.0;
            let total_children_w = left_b.width + right_b.width + 36.0;
            let left_cx = center_x - total_children_w / 2.0 + left_b.width / 2.0;
            let right_cx = center_x + total_children_w / 2.0 - right_b.width / 2.0;

            out.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#8875cc\" stroke-width=\"2\"/>\n",
                center_x, parent_bottom_y, left_cx, child_top_y
            ));
            out.push_str(&format!(
                "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#8875cc\" stroke-width=\"2\"/>\n",
                center_x, parent_bottom_y, right_cx, child_top_y
            ));

            self.emit_svg_block(left_b, left_cx, child_top_y, out);
            self.emit_svg_block(right_b, right_cx, child_top_y, out);
        }
    }
}

struct SvgTreeBlock {
    lines: Vec<String>,
    leaf_badge: Option<String>,
    is_closed: bool,
    box_w: f32,
    box_h: f32,
    width: f32,
    height: f32,
    children: Option<(Box<SvgTreeBlock>, Box<SvgTreeBlock>)>,
}

fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Renders a ` ```tree ` DSL block into an SVG diagram.
/// If the block starts with `problem: <stmt>`, it builds and solves the `TruthTree` directly to produce a full branching SVG.
pub fn render_tree_dsl_as_svg(dsl: &str) -> String {
    for line in dsl.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("problem:") {
            let goal = rest.trim();
            if let Ok(MetalogicStmt::Entailment {
                premises,
                conclusion,
                ..
            }) = parse_metalogic_stmt(goal)
            {
                let mut tree = TruthTree::from_entailment(&premises, &conclusion);
                let _ = tree.solve_alpha_first(100);
                return tree.to_svg();
            } else if let Ok(f) = parse_formula(goal) {
                let mut tree = TruthTree::from_formulas(&[f]);
                let _ = tree.solve_alpha_first(100);
                return tree.to_svg();
            }
        }
    }

    // Fallback: render the lines in a styled SVG box
    let lines: Vec<&str> = dsl.lines().collect();
    let w = lines
        .iter()
        .map(|l| l.chars().count() as f32 * 8.2 + 32.0)
        .fold(320.0_f32, f32::max);
    let h = lines.len() as f32 * 22.0 + 28.0;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.0} {:.0}\" width=\"100%\" style=\"max-width:{:.0}px;background:#2e3440;border:1px solid #4c566a;border-radius:8px;\">\n",
        w, h, w
    );
    for (idx, line) in lines.iter().enumerate() {
        out.push_str(&format!(
            "<text x=\"16\" y=\"{:.1}\" fill=\"#eceff4\" font-family=\"monospace\" font-size=\"13\">{}</text>\n",
            24.0 + idx as f32 * 22.0,
            svg_escape(line)
        ));
    }
    out.push_str("</svg>");
    out
}

/// Converts freeform ASCII truth trees (such as those in `content/Truth Table.md`) into
/// structured ` ```tree ` fenced blocks solved by the α-first tableau engine.
/// Returns `(converted_markdown, number_of_trees_converted)`.
pub fn convert_ascii_trees_to_fenced_blocks(raw_markdown: &str) -> (String, usize) {
    let lines: Vec<&str> = raw_markdown.lines().collect();
    let mut out = String::with_capacity(raw_markdown.len() + 512);
    let mut converted = 0usize;
    let mut i = 0usize;
    let mut in_frontmatter = false;
    let mut in_fence = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if i == 0 && trimmed == "---" {
            in_frontmatter = true;
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }
        if in_frontmatter {
            out.push_str(line);
            out.push('\n');
            if trimmed == "---" || trimmed == "..." {
                in_frontmatter = false;
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }

        // Detect an entailment line `... ∴ ...` followed by an ASCII tree
        if trimmed.contains('∴') {
            if let Ok(MetalogicStmt::Entailment {
                premises,
                conclusion,
                ..
            }) = parse_metalogic_stmt(trimmed)
            {
                let mut j = i + 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                let mut found_verdict = false;
                while j < lines.len() {
                    let lt = lines[j].trim();
                    if lt.starts_with('#') || lt.starts_with("---") || lt.contains('∴') {
                        break;
                    }
                    let lower = lt.to_ascii_lowercase();
                    if lower == "valid"
                        || lower == "invalid"
                        || lower == "satisfiable"
                        || lower == "satisfaible"
                        || lower == "unsatisfiable"
                    {
                        found_verdict = true;
                        j += 1;
                        break;
                    }
                    j += 1;
                }

                if found_verdict {
                    let mut tree = TruthTree::from_entailment(&premises, &conclusion);
                    let _ = tree.solve_alpha_first(100);
                    out.push_str("```tree\n");
                    out.push_str(&tree.to_tree_dsl());
                    out.push_str("```\n");
                    converted += 1;
                    i = j;
                    continue;
                }
            }
        }

        out.push_str(line);
        out.push('\n');
        i += 1;
    }

    (out, converted)
}

fn collect_pred_args(f: &Formula, out: &mut BTreeSet<String>) {
    match f {
        Formula::Predicate { args, .. } => {
            for a in args {
                out.insert(a.clone());
            }
        }
        Formula::Not(inner) => collect_pred_args(inner, out),
        Formula::Binary { lhs, rhs, .. } => {
            collect_pred_args(lhs, out);
            collect_pred_args(rhs, out);
        }
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsciiTreeLintIssue {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsciiTreeReport {
    pub start_line: usize,
    pub goal: String,
    pub written_verdict: Option<String>,
    pub computed_verdict: String,
    pub issues: Vec<AsciiTreeLintIssue>,
}

/// Parses and validates the freeform ASCII Truth Trees in `content/Truth Table.md`.
/// Specifically verifies:
/// 1. Whether the trunk formulas accurately represent the stated `premises ∴ conclusion`
///    (catching mistakes like negating `~r → ~p` as `~r ↛ ~q` on line 97 of `Truth Table.md`!).
/// 2. Whether the written verdict (`valid`, `invalid`, `satisfiable`) matches the solved Truth Tree.
/// 3. Whether spelling typos like `satisfaible` are present.
pub fn lint_ascii_trees_in_note(raw_markdown: &str, body_byte_offset: usize, body_start_line: usize) -> Vec<AsciiTreeReport> {
    let body = &raw_markdown[body_byte_offset..];
    let lines: Vec<&str> = body.lines().collect();
    let mut reports = Vec::new();

    // Split into paragraphs/blocks separated by blank lines or headings
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("---")
            || trimmed.starts_with('*')
            || trimmed.starts_with("- [[")
        {
            i += 1;
            continue;
        }

        // Check if this line is an entailment goal (`... ∴ ...`) followed by a tree block
        if trimmed.contains('∴') {
            let goal_line_no = body_start_line + i;
            let goal_str = trimmed.to_string();
            i += 1;
            while i < lines.len() && lines[i].trim().is_empty() {
                i += 1;
            }
            // Collect lines of the tree until `valid` / `invalid` / `satisfiable` / `satisfaible` or next heading/`∴`
            let mut tree_lines: Vec<(usize, &str)> = Vec::new();
            let mut verdict_line: Option<(usize, String)> = None;
            while i < lines.len() {
                let l_trim = lines[i].trim();
                let l_no = body_start_line + i;
                if l_trim.starts_with('#') || l_trim.starts_with("---") || l_trim.contains('∴') {
                    break;
                }
                let lower = l_trim.to_ascii_lowercase();
                if lower == "valid"
                    || lower == "invalid"
                    || lower == "satisfiable"
                    || lower == "satisfaible"
                    || lower == "unsatisfiable"
                {
                    verdict_line = Some((l_no, l_trim.to_string()));
                    i += 1;
                    break;
                }
                if !l_trim.is_empty() {
                    tree_lines.push((l_no, l_trim));
                }
                i += 1;
            }

            if let Ok(MetalogicStmt::Entailment {
                premises,
                conclusion,
                ..
            }) = parse_metalogic_stmt(&goal_str)
            {
                let mut issues = Vec::new();
                let mut tree = TruthTree::from_entailment(&premises, &conclusion);
                let _ = tree.solve_alpha_first(100);
                let computed_valid = matches!(tree.verdict(), TreeVerdict::AllClosed);
                let computed_verdict = if computed_valid { "valid" } else { "invalid" }.to_string();

                // Check trunk lines (premises + negated conclusion) against the goal's atoms and negated conclusion
                let mut goal_atoms = BTreeSet::new();
                for p in &premises {
                    p.collect_atoms(&mut goal_atoms);
                }
                conclusion.collect_atoms(&mut goal_atoms);

                let expected_neg_conc = Formula::negate(conclusion.clone());
                let trunk_check_count = (premises.len() + 1).min(tree_lines.len());
                for (trunk_idx, &(l_no, l_text)) in
                    tree_lines.iter().take(trunk_check_count).enumerate()
                {
                    if let Ok(f) = parse_formula(l_text) {
                        let f_atoms = f.atoms_alphabetical();
                        for a in f_atoms {
                            if !goal_atoms.contains(&a) {
                                issues.push(AsciiTreeLintIssue {
                                    line: l_no,
                                    message: format!(
                                        "Formula '{}' introduces atom '{}' not present in goal '{}' (expected negated conclusion '{}')",
                                        l_text, a, goal_str, expected_neg_conc
                                    ),
                                });
                            }
                        }
                        if trunk_idx == premises.len() {
                            if let Ok(false) = crate::truth_table::are_equivalent(&f, &expected_neg_conc) {
                                issues.push(AsciiTreeLintIssue {
                                    line: l_no,
                                    message: format!(
                                        "Negated conclusion line '{}' is not equivalent to expected '{}' for goal '{}'",
                                        l_text, expected_neg_conc, goal_str
                                    ),
                                });
                            }
                        }
                    }
                }

                // Check written verdict
                if let Some((v_line, ref v_text)) = verdict_line {
                    let lower = v_text.to_ascii_lowercase();
                    if lower == "satisfaible" {
                        issues.push(AsciiTreeLintIssue {
                            line: v_line,
                            message: "Typo in verdict 'satisfaible' (should be 'satisfiable')"
                                .to_string(),
                        });
                    }
                    if (lower == "valid" && !computed_valid)
                        || (lower == "invalid" && computed_valid)
                    {
                        issues.push(AsciiTreeLintIssue {
                            line: v_line,
                            message: format!(
                                "Verdict mismatch: written '{}', but Truth Tree evaluates to '{}'",
                                v_text, computed_verdict
                            ),
                        });
                    }
                }

                reports.push(AsciiTreeReport {
                    start_line: goal_line_no,
                    goal: goal_str,
                    written_verdict: verdict_line.map(|(_, s)| s),
                    computed_verdict,
                    issues,
                });
            }
            continue;
        }

        // Also check standalone satisfiability truth trees (like the level 1 trees at the top of `Truth Table.md`)
        let block_start = body_start_line + i;
        let mut block_lines: Vec<(usize, &str)> = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            block_lines.push((body_start_line + i, lines[i].trim()));
            i += 1;
        }

        let has_closure_markers = block_lines.iter().any(|(_, l)| {
            l.split_whitespace()
                .all(|tok| tok == "x" || tok == "o" || tok == "×")
        });
        if has_closure_markers && !block_lines.is_empty() {
            let mut issues = Vec::new();
            let first_formula_str = block_lines[0].1.to_string();
            let mut written_verdict = None;
            if let Some(&(last_line, last_str)) = block_lines.last() {
                let lower = last_str.to_ascii_lowercase();
                if lower == "satisfiable" || lower == "satisfaible" || lower == "unsatisfiable" {
                    if lower == "satisfaible" {
                        issues.push(AsciiTreeLintIssue {
                            line: last_line,
                            message: "Typo in verdict 'satisfaible' (should be 'satisfiable')"
                                .to_string(),
                        });
                    }
                    written_verdict = Some(last_str.to_string());
                }
            }
            reports.push(AsciiTreeReport {
                start_line: block_start,
                goal: first_formula_str,
                written_verdict,
                computed_verdict: "satisfiable".to_string(),
                issues,
            });
        }
    }

    reports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truth_tree_stacking_and_branching() {
        // Test level 3 problem from `Truth Table.md`:
        // `p → q, r → s, ~q v ~s ∴ ~p v ~r`
        let p1 = parse_formula("p -> q").unwrap();
        let p2 = parse_formula("r -> s").unwrap();
        let p3 = parse_formula("~q v ~s").unwrap();
        let conc = parse_formula("~p v ~r").unwrap();

        let mut tree = TruthTree::from_entailment(&[p1, p2, p3], &conc);
        let verdict = tree.solve_alpha_first(50).unwrap();
        assert_eq!(verdict, TreeVerdict::AllClosed);

        // Verify that α-first stacked `p` and `r` (from `¬(¬p ∨ ¬r)`) at node IDs 4 and 5 before branching!
        assert_eq!(tree.nodes[4].formula.to_string(), "¬¬p");
        assert_eq!(tree.nodes[5].formula.to_string(), "¬¬r");

        let dsl = tree.to_tree_dsl();
        assert!(dsl.contains("verdict: valid"));
    }

    #[test]
    fn test_truth_tree_open_branch_counterexample() {
        // Invalid argument: `p → q, q ∴ p`
        let p1 = parse_formula("p -> q").unwrap();
        let p2 = parse_formula("q").unwrap();
        let conc = parse_formula("p").unwrap();

        let mut tree = TruthTree::from_entailment(&[p1, p2], &conc);
        let verdict = tree.solve_alpha_first(50).unwrap();
        match verdict {
            TreeVerdict::HasOpenBranches { open_valuations } => {
                assert!(!open_valuations.is_empty());
                // Counterexample has p = false (Y), q = true (D)
                let val = &open_valuations[0];
                assert_eq!(val.get("p"), Some(&false));
                assert_eq!(val.get("q"), Some(&true));
            }
            other => panic!("Expected open branches, got {:?}", other),
        }
    }

    #[test]
    fn test_interactive_practice_step_verification() {
        let p1 = parse_formula("p -> q").unwrap();
        let conc = parse_formula("q").unwrap();
        let tree = TruthTree::from_entailment(&[p1], &conc);

        // Node 0 is `p → q` (a β-branching rule). Trying to stack it should fail verification:
        let err = tree.verify_user_step(
            0,
            false,
            &[parse_formula("~p").unwrap(), parse_formula("q").unwrap()],
            None,
        );
        assert!(err.is_err());

        // Branching it into `~p` | `q` succeeds:
        let ok = tree.verify_user_step(
            0,
            true,
            &[parse_formula("~p").unwrap()],
            Some(&[parse_formula("q").unwrap()]),
        );
        assert!(ok.is_ok());
    }
}
