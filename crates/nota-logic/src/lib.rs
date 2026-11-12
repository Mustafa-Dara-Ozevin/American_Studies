pub mod ast;
pub mod palette;
pub mod parser;
pub mod tableau;
pub mod truth_table;

pub use ast::{BinOp, Formula, MetalogicStmt, QuantKind};
pub use palette::{
    default_symbol_palette, normalize_ascii_operators_in_text, search_symbol_palette,
    LogicSymbolEntry,
};
pub use parser::{clean_formula_input, parse_formula, parse_metalogic_stmt, tokenize_formula, Token};
pub use tableau::{
    classify_decomposition, convert_ascii_trees_to_fenced_blocks, lint_ascii_trees_in_note,
    render_tree_dsl_as_svg, AsciiTreeLintIssue, AsciiTreeReport, DecompositionRule,
    NodeContinuation, NodeOrigin, TreeNode, TreeVerdict, TruthTree,
};
pub use truth_table::{
    all_valuations, are_equivalent, auto_fix_markdown_truth_table, check_consistency,
    check_entailment, classify_formula, eval_formula, format_aligned_markdown_table,
    generate_markdown_truth_table, lint_markdown_table, ConsistencyResult, EditableTableGrid,
    EntailmentResult, FormulaClass, TableIssueKind, TableLintReport,
};
