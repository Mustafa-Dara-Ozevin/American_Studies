use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nota_core::{NotaConfig, TypingRuleEngine, VaultIndex};
use nota_epub::EpubBook;
use nota_logic::{
    are_equivalent, check_consistency, check_entailment, convert_ascii_trees_to_fenced_blocks,
    generate_markdown_truth_table, lint_ascii_trees_in_note, lint_markdown_table,
    normalize_ascii_operators_in_text, parse_formula, parse_metalogic_stmt, MetalogicStmt,
    TableIssueKind, TruthTree,
};
use nota_site::{build_static_site, export_note_standalone_html, export_note_typst};
use nota_srs::{
    extract_cards_from_note, load_revlog, materialize_schedule_from_events, stamp_missing_card_ids,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "nota",
    version,
    about = "Nota: Markdown Vault Indexer, Symbolic Logic & Truth Tree Engine, EPUB Paginator & FSRS Study CLI"
)]
struct Cli {
    /// Path to nota.toml configuration file
    #[arg(long, default_value = "nota.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch the Nota desktop application (iced 0.14 GUI)
    Open {
        #[arg(long, default_value = "content")]
        vault: PathBuf,
        #[arg(long)]
        note: Option<String>,
    },
    /// Scan and index the vault (100% read-only) and optionally write a Markdown health report
    Index {
        #[arg(long, default_value = "content")]
        vault: PathBuf,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Build the Quartz-compatible static site (HTML, SVG Truth Trees, Tag Pages, Alias Redirects, RSS/Sitemap)
    Build {
        #[arg(long, default_value = "content")]
        vault: PathBuf,
        #[arg(long, default_value = "public-nota")]
        out: PathBuf,
        #[arg(long)]
        base_url: Option<String>,
    },
    /// Export a single note to standalone HTML or Typst (.typ) for PDF compilation
    Export {
        /// Note filename or stem, e.g. "Mantık Devreleri.md"
        note: String,
        #[arg(long, default_value = "content")]
        vault: PathBuf,
        /// Format: "html" or "typst"
        #[arg(long, default_value = "html")]
        format: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Symbolic logic tools: check notes/tables/trees, generate truth tables, or build truth trees
    Logic {
        #[command(subcommand)]
        action: LogicCommand,
    },
    /// Spaced repetition card validator, ULID stamper, and FSRS schedule verifier
    Cards {
        #[command(subcommand)]
        action: CardsCommand,
    },
    /// EPUB deterministic paginator and deep link inspector
    Epub {
        #[command(subcommand)]
        action: EpubCommand,
    },
}

#[derive(Subcommand, Debug)]
enum LogicCommand {
    /// Parse formulas, lint all truth tables (enforcing alphabetical atoms), and validate truth trees across the vault
    Check {
        #[arg(long, default_value = "content")]
        vault: PathBuf,
    },
    /// Decompose and solve a Truth Tree (analytic tableau) for an argument or formula
    Tree {
        /// Argument or formula, e.g. "p -> q, q -> r ∴ p -> r"
        statement: String,
    },
    /// Generate a canonical Markdown truth table for a formula
    Table {
        /// Formula, e.g. "(p -> q) <-> (~p v q)"
        formula: String,
    },
    /// Convert freeform ASCII Truth Trees in a note into structured ```tree blocks
    ConvertTrees {
        /// Path to Markdown note (e.g. "content/Truth Table.md")
        note_path: PathBuf,
        /// Write converted trees back to the file (dry-run by default)
        #[arg(long)]
        write: bool,
    },
    /// Normalize ASCII logic operators (->, <->, ^, ~, v/V) into Unicode in a note
    Normalize {
        /// Path to Markdown note
        note_path: PathBuf,
        /// Write normalized text back to the file (dry-run by default)
        #[arg(long)]
        write: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CardsCommand {
    /// Read-only check of flashcards, IDs, and FSRS revlog/schedule replay across the vault
    Check {
        #[arg(long, default_value = "content")]
        vault: PathBuf,
    },
    /// Explicitly stamp missing <!--c:ULID--> comments onto flashcards in a note
    Stamp {
        /// Path to Markdown note
        note_path: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum EpubCommand {
    /// Inspect an EPUB file's metadata, spine chapters, and deterministic canonical pages
    Inspect {
        /// Path to the .epub file
        epub_path: PathBuf,
        /// Optional page number to resolve
        #[arg(long)]
        page: Option<usize>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = NotaConfig::load_from_path(&cli.config)?;

    match cli.command {
        Commands::Open { vault, note } => {
            let mut cfg = cfg;
            cfg.vault.path = vault;
            nota_ui::run_desktop_app(cfg, note)
                .map_err(|e| anyhow::anyhow!("UI error: {}", e))
        }
        Commands::Index { vault, report } => run_index(&vault, report.as_deref(), &cfg),
        Commands::Build {
            vault,
            out,
            base_url,
        } => {
            let index = VaultIndex::scan(&vault, &cfg.vault)?;
            let mut site_cfg = cfg.site.clone();
            if let Some(b) = base_url {
                site_cfg.base_url = b;
            }
            let summary = build_static_site(&index, &site_cfg, &out)?;
            println!("=== Nota Static Site Build Complete ===");
            println!("Output directory:       {}", summary.out_dir.display());
            println!("Note HTML pages:        {}", summary.pages_built);
            println!("Tag index pages:        {}", summary.tag_pages_built);
            println!("Alias redirects:        {}", summary.alias_redirects_built);
            println!("Assets copied:          {}", summary.assets_copied);
            Ok(())
        }
        Commands::Export {
            note,
            vault,
            format,
            out,
        } => {
            let index = VaultIndex::scan(&vault, &cfg.vault)?;
            let normalized = if note.ends_with(".md") {
                note
            } else {
                format!("{}.md", note)
            };
            let indexed_note = index
                .notes
                .get(&normalized)
                .ok_or_else(|| anyhow::anyhow!("Note '{}' not found in {}", normalized, vault.display()))?;
            let raw = fs::read_to_string(vault.join(&indexed_note.rel_path))?;
            let content = if format.eq_ignore_ascii_case("typst") || format.eq_ignore_ascii_case("typ") {
                export_note_typst(indexed_note, &raw)
            } else {
                export_note_standalone_html(indexed_note, &raw, &index)
            };
            if let Some(out_path) = out {
                if let Some(p) = out_path.parent() {
                    fs::create_dir_all(p)?;
                }
                fs::write(&out_path, content.as_bytes())?;
                println!("Exported {} to {}", indexed_note.rel_path, out_path.display());
            } else {
                print!("{}", content);
            }
            Ok(())
        }
        Commands::Logic { action } => match action {
            LogicCommand::Check { vault } => run_logic_check(&vault, &cfg),
            LogicCommand::Tree { statement } => run_logic_tree(&statement),
            LogicCommand::Table { formula } => run_logic_table(&formula, &cfg),
            LogicCommand::ConvertTrees { note_path, write } => {
                let raw = fs::read_to_string(&note_path)?;
                let (converted, count) = convert_ascii_trees_to_fenced_blocks(&raw);
                println!("Converted {} ASCII truth tree(s) in {}", count, note_path.display());
                if write && count > 0 {
                    fs::write(&note_path, converted.as_bytes())?;
                    println!("Updated {}", note_path.display());
                } else {
                    print!("{}", converted);
                }
                Ok(())
            }
            LogicCommand::Normalize { note_path, write } => {
                let raw = fs::read_to_string(&note_path)?;
                let (norm, count) = normalize_ascii_operators_in_text(&raw);
                println!("Normalized {} ASCII operator(s) in {}", count, note_path.display());
                if write && count > 0 {
                    fs::write(&note_path, norm.as_bytes())?;
                    println!("Updated {}", note_path.display());
                } else {
                    print!("{}", norm);
                }
                Ok(())
            }
        },
        Commands::Cards { action } => match action {
            CardsCommand::Check { vault } => run_cards_check(&vault, &cfg),
            CardsCommand::Stamp { note_path } => {
                let raw = fs::read_to_string(&note_path)?;
                let (stamped, count) = stamp_missing_card_ids(&raw);
                if count > 0 {
                    fs::write(&note_path, stamped.as_bytes())?;
                }
                println!("Stamped {} missing <!--c:ULID--> card ID(s) in {}", count, note_path.display());
                Ok(())
            }
        },
        Commands::Epub { action } => match action {
            EpubCommand::Inspect { epub_path, page } => run_epub_inspect(&epub_path, page),
        },
    }
}

fn run_index(vault_path: &Path, report_out: Option<&Path>, cfg: &NotaConfig) -> Result<()> {
    let index = VaultIndex::scan(vault_path, &cfg.vault)?;
    let report_md = index.generate_markdown_report();

    let total_notes = index.notes.len();
    let mocs = index.notes.values().filter(|n| n.is_moc).count();
    let wikilinks = index.resolved_links.iter().filter(|l| !l.is_embed).count();
    let embeds = index.resolved_links.iter().filter(|l| l.is_embed).count();
    let unresolved_targets = index.unresolved_targets.len();
    let unresolved_occurrences: usize = index.unresolved_targets.values().map(|v| v.len()).sum();
    let exact_unresolved = index.exact_unresolved_targets.len();
    let exact_unresolved_occ: usize = index
        .exact_unresolved_targets
        .values()
        .map(|v| v.len())
        .sum();
    let orphans = index.orphan_notes().len();
    let exact_orphans = index.exact_orphan_notes().len();
    let total_tags_key = index
        .notes
        .values()
        .filter(|n| n.frontmatter.has_tags_key)
        .count();
    let empty_tags = index
        .notes
        .values()
        .filter(|n| n.frontmatter.has_tags_key && n.frontmatter.tags.is_empty())
        .count();
    let placeholder_aliases = index
        .notes
        .values()
        .filter(|n| n.frontmatter.has_placeholder_alias)
        .count();
    let title_mismatches = index
        .notes
        .values()
        .filter(|n| n.title_mismatch().is_some())
        .count();

    println!("=== Nota Vault Index Summary ===");
    println!(
        "Active notes:           {} (+ {} template, {} Excalidraw = {} total .md; {} MOCs)",
        total_notes,
        index.template_files.len(),
        index.excalidraw_files.len(),
        total_notes + index.template_files.len() + index.excalidraw_files.len(),
        mocs
    );
    println!(
        "Assets:                 {} files ({:.2} MB)",
        index.assets.len(),
        index.assets.values().sum::<u64>() as f64 / (1024.0 * 1024.0)
    );
    println!("Wikilinks:              {} (+ {} embeds = {} total)", wikilinks, embeds, wikilinks + embeds);
    println!(
        "Unresolved targets:     {} exact ({} occurrences) | {} case+alias folded ({} occurrences)",
        exact_unresolved, exact_unresolved_occ, unresolved_targets, unresolved_occurrences
    );
    println!(
        "Orphan notes:           {} exact | {} case+alias folded",
        exact_orphans, orphans
    );
    println!(
        "Metadata lint:          {}/{} empty tag lists, {} placeholder [\"[]\"] aliases, {} title!=filename",
        empty_tags, total_tags_key, placeholder_aliases, title_mismatches
    );

    if let Some(out_path) = report_out {
        fs::write(out_path, &report_md)
            .with_context(|| format!("Failed to write report to {}", out_path.display()))?;
        println!("Report written to:      {}", out_path.display());
    }

    Ok(())
}

fn run_logic_check(vault_path: &Path, cfg: &NotaConfig) -> Result<()> {
    let index = VaultIndex::scan(vault_path, &cfg.vault)?;

    // Also verify Typing Transformer rules file if present
    let rules_path = Path::new("typing-rules.conf");
    if rules_path.exists() {
        let engine = TypingRuleEngine::from_conf_file(rules_path)?;
        println!(
            "Loaded Typing Transformer config: {} rules active",
            engine.rules.len()
        );
    }

    let mut truth_tables_checked = 0usize;
    let mut truth_table_issues = 0usize;
    let mut statements_checked = 0usize;
    let mut statement_errors = Vec::new();
    let mut trees_checked = 0usize;
    let mut tree_issues = Vec::new();

    for note in index.notes.values() {
        // 1. Lint all truth tables in the note
        for table in &note.body.tables {
            let report = lint_markdown_table(&table.headers, &table.rows, table.start_line);
            if report.is_truth_table {
                truth_tables_checked += 1;
                if !report.issues.is_empty() {
                    truth_table_issues += report.issues.len();
                    println!(
                        "\n[Truth Table Lint] {}:{} (atoms: {})",
                        note.rel_path,
                        report.start_line,
                        report.atoms.join(" ")
                    );
                    for issue in &report.issues {
                        match issue {
                            TableIssueKind::NonAlphabeticalAtomHeader { written, canonical } => {
                                println!(
                                    "  - Non-alphabetical atomic header `| {} |` -> expected canonical `| {} |`",
                                    written.join(" "),
                                    canonical.join(" ")
                                );
                            }
                            TableIssueKind::EmptyOrIncompleteTable => {
                                println!("  - Empty truth table stub (no filled valuation rows)");
                            }
                            TableIssueKind::RowCountMismatch { expected, actual } => {
                                println!(
                                    "  - Row count mismatch: expected {} rows for {} variables, found {}",
                                    expected,
                                    report.atoms.len(),
                                    actual
                                );
                            }
                            TableIssueKind::DuplicateValuationRow {
                                row_index,
                                valuation,
                            } => {
                                println!(
                                    "  - Row {}: duplicate valuation `{}`",
                                    row_index, valuation
                                );
                            }
                            TableIssueKind::NonCanonicalTruthSymbol {
                                row_index,
                                col_index,
                                raw,
                            } => {
                                println!(
                                    "  - Row {}, Col {}: non-canonical truth symbol `{}` (expected D or Y)",
                                    row_index, col_index, raw
                                );
                            }
                            TableIssueKind::UnparseableColumnHeader { col_index, header } => {
                                println!(
                                    "  - Col {}: header `{}` could not be parsed as a formula",
                                    col_index + 1,
                                    header
                                );
                            }
                            TableIssueKind::CellValueError {
                                row_index,
                                col_index,
                                header,
                                expected,
                                actual,
                            } => {
                                println!(
                                    "  - Row {}, Col {} (`{}`): evaluated to `{}`, but table has `{}`",
                                    row_index, col_index, header, expected, actual
                                );
                            }
                        }
                    }
                }
            }
        }

        // 2. Check metalogical statements in logic notes
        let is_logic_note = note.rel_path.to_ascii_lowercase().contains("mantık")
            || note.rel_path.to_ascii_lowercase().contains("logic")
            || matches!(
                note.file_stem.as_str(),
                "Alıştırma"
                    | "Eşdeğerlik"
                    | "içerme"
                    | "Önerme"
                    | "Önerme Kümesi"
                    | "Modus Ponens"
                    | "Modus Tollens"
                    | "Truth Table"
            );

        if is_logic_note {
            let abs_path = vault_path.join(&note.rel_path);
            let raw = fs::read_to_string(&abs_path)?;
            let body = &raw[note.frontmatter.body_byte_offset..];
            for (idx, line) in body.lines().enumerate() {
                let line_no = note.frontmatter.body_start_line + idx;
                let trimmed = line.trim();
                if let Some(candidate) = extract_logic_line_candidate(trimmed) {
                    statements_checked += 1;
                    match parse_metalogic_stmt(&candidate) {
                        Ok(stmt) => {
                            if let MetalogicStmt::Equivalence(lhs, rhs) = stmt {
                                if let Ok(false) = are_equivalent(&lhs, &rhs) {
                                    statement_errors.push(format!(
                                        "{}:{}: Equivalence `{}` === `{}` is NOT tautologically equivalent!",
                                        note.rel_path, line_no, lhs, rhs
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            statement_errors.push(format!(
                                "{}:{}: Failed to parse `{}` -> {}",
                                note.rel_path, line_no, candidate, err
                            ));
                        }
                    }
                }
            }

            // 3. Lint ASCII Truth Trees (e.g. in `Truth Table.md`)
            if note.file_stem == "Truth Table" {
                let tree_reports = lint_ascii_trees_in_note(
                    &raw,
                    note.frontmatter.body_byte_offset,
                    note.frontmatter.body_start_line,
                );
                trees_checked += tree_reports.len();
                for tr in tree_reports {
                    for issue in tr.issues {
                        tree_issues.push(format!(
                            "{}:{}: [Goal: {}] {}",
                            note.rel_path, issue.line, tr.goal, issue.message
                        ));
                    }
                }
            }
        }
    }

    if !statement_errors.is_empty() {
        println!("\n[Formula / Equivalence Issues]");
        for err in &statement_errors {
            println!("  - {}", err);
        }
    }

    if !tree_issues.is_empty() {
        println!("\n[Truth Tree (Analytic Tableau) Issues]");
        for issue in &tree_issues {
            println!("  - {}", issue);
        }
    }

    println!("\n=== Nota Logic Summary ===");
    println!(
        "Truth tables checked:   {} ({} issues flagged)",
        truth_tables_checked, truth_table_issues
    );
    println!(
        "Logic statements:       {} ({} formula/equivalence issues)",
        statements_checked,
        statement_errors.len()
    );
    println!(
        "Truth trees checked:    {} ({} tree issues)",
        trees_checked,
        tree_issues.len()
    );

    Ok(())
}

fn extract_logic_line_candidate(trimmed: &str) -> Option<String> {
    if trimmed.is_empty()
        || trimmed.starts_with('|')
        || trimmed.starts_with('#')
        || trimmed.starts_with("---")
        || trimmed.contains("[[")
    {
        return None;
    }
    // Strip list bullet or checkbox prefix (`- [x] `, `- [ ] `, `- `, `1. `)
    let mut s = trimmed;
    if let Some(r) = s
        .strip_prefix("- [x] ")
        .or_else(|| s.strip_prefix("- [ ] "))
        .or_else(|| s.strip_prefix("- "))
    {
        s = r.trim();
    } else {
        let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 {
            if let Some(r) = s[digits..].strip_prefix(". ") {
                s = r.trim();
            }
        }
    }

    // Must contain a metalogical operator (`===`, `|=`, `∴`) or start with `{` and end with `}`
    let is_metalogic = s.contains("===")
        || s.contains("|=")
        || s.contains('∴')
        || (s.starts_with('{') && (s.ends_with('}') || s.contains("=>")));
    if !is_metalogic {
        return None;
    }
    // Exclude prose explanation lines like `Eşdeğerlik: ===` or `İçerme: |=`
    if s.starts_with("Eşdeğerlik:") || s.starts_with("İçerme:") || s.contains("iddiamız") {
        return None;
    }
    Some(s.to_string())
}

fn run_logic_tree(statement: &str) -> Result<()> {
    let stmt = parse_metalogic_stmt(statement)?;
    let mut tree = match stmt {
        MetalogicStmt::Entailment {
            premises,
            conclusion,
            ..
        } => TruthTree::from_entailment(&premises, &conclusion),
        MetalogicStmt::Equivalence(lhs, rhs) => {
            let iff = nota_logic::Formula::binary(nota_logic::BinOp::Iff, lhs, rhs);
            TruthTree::from_entailment(&[], &iff)
        }
        MetalogicStmt::FormulaSet(fs) => TruthTree::from_formulas(&fs),
        MetalogicStmt::Single(f) => TruthTree::from_formulas(&[f]),
        MetalogicStmt::NonEquivalence(lhs, rhs) => {
            let niff = nota_logic::Formula::binary(nota_logic::BinOp::NonIff, lhs, rhs);
            TruthTree::from_formulas(&[niff])
        }
    };
    tree.solve_alpha_first(100)?;
    println!("```tree\n{}```", tree.to_tree_dsl());
    Ok(())
}

fn run_logic_table(formula_str: &str, cfg: &NotaConfig) -> Result<()> {
    let stmt = parse_metalogic_stmt(formula_str)?;
    match stmt {
        MetalogicStmt::Single(f) => {
            let md = generate_markdown_truth_table(
                &[f],
                cfg.logic.packed_variable_columns,
                &cfg.logic.true_symbol,
                &cfg.logic.false_symbol,
            )?;
            print!("{}", md);
        }
        MetalogicStmt::Equivalence(lhs, rhs) => {
            let iff = nota_logic::Formula::binary(nota_logic::BinOp::Iff, lhs.clone(), rhs.clone());
            let md = generate_markdown_truth_table(
                &[lhs.clone(), rhs.clone(), iff],
                cfg.logic.packed_variable_columns,
                &cfg.logic.true_symbol,
                &cfg.logic.false_symbol,
            )?;
            print!("{}", md);
            println!("Equivalent: {}", are_equivalent(&lhs, &rhs)?);
        }
        MetalogicStmt::Entailment {
            premises,
            conclusion,
            ..
        } => {
            let mut cols = premises.clone();
            cols.push(conclusion.clone());
            let md = generate_markdown_truth_table(
                &cols,
                cfg.logic.packed_variable_columns,
                &cfg.logic.true_symbol,
                &cfg.logic.false_symbol,
            )?;
            print!("{}", md);
            let res = check_entailment(&premises, &conclusion)?;
            println!("Valid: {}", res.is_valid);
        }
        MetalogicStmt::FormulaSet(fs) => {
            let md = generate_markdown_truth_table(
                &fs,
                cfg.logic.packed_variable_columns,
                &cfg.logic.true_symbol,
                &cfg.logic.false_symbol,
            )?;
            print!("{}", md);
            let res = check_consistency(&fs)?;
            println!("Consistent: {}", res.is_consistent);
        }
        MetalogicStmt::NonEquivalence(lhs, rhs) => {
            let f = parse_formula(formula_str)?;
            let md = generate_markdown_truth_table(
                &[lhs, rhs, f],
                cfg.logic.packed_variable_columns,
                &cfg.logic.true_symbol,
                &cfg.logic.false_symbol,
            )?;
            print!("{}", md);
        }
    }
    Ok(())
}

fn run_cards_check(vault_path: &Path, cfg: &NotaConfig) -> Result<()> {
    let index = VaultIndex::scan(vault_path, &cfg.vault)?;
    let mut all_cards = Vec::new();
    let mut id_map: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();

    for note in index.notes.values() {
        let abs_path = vault_path.join(&note.rel_path);
        let raw = fs::read_to_string(&abs_path)?;
        let default_deck = note
            .frontmatter
            .deck
            .as_deref()
            .unwrap_or("default");
        let opt_in = note.frontmatter.flashcards
            || note
                .frontmatter
                .tags
                .iter()
                .any(|t| t == "flashcards" || t.starts_with("flashcards/"));

        let cards = extract_cards_from_note(
            &note.rel_path,
            &raw,
            note.frontmatter.body_byte_offset,
            note.frontmatter.body_start_line,
            default_deck,
            opt_in,
        );
        for c in cards {
            id_map
                .entry(c.id.clone())
                .or_default()
                .push((c.note_path.clone(), c.line));
            all_cards.push(c);
        }
    }

    let duplicates: Vec<_> = id_map.iter().filter(|(_, locs)| locs.len() > 1).collect();
    let events = load_revlog(&cfg.srs.revlog_path)?;
    let materialized = materialize_schedule_from_events(&events);

    println!("=== Nota Spaced Repetition Check ===");
    println!("Cards discovered:       {}", all_cards.len());
    println!("Duplicate card IDs:     {}", duplicates.len());
    println!(
        "Revlog events:          {} ({} scheduled cards materialized)",
        events.len(),
        materialized.cards.len()
    );

    for (dup_id, locs) in duplicates {
        println!("  [Duplicate ID `{}`]: {:?}", dup_id, locs);
    }

    Ok(())
}

fn run_epub_inspect(epub_path: &Path, page: Option<usize>) -> Result<()> {
    let book = EpubBook::open(epub_path)?;
    println!("=== EPUB Book Inspection ===");
    println!("File:                   {}", book.file_name);
    println!("Title:                  {}", book.title);
    println!("Authors:                {}", book.authors.join(", "));
    println!("BLAKE3 Hash:            {}", book.blake3_hash);
    println!("Spine Chapters:         {}", book.chapters.len());
    println!("Canonical Pages:        {}", book.total_pages());

    for ch in &book.chapters {
        println!(
            "  - [Ch {}] {} (pp. {}–{}, {} blocks)",
            ch.spine_index + 1,
            ch.title,
            ch.start_page,
            ch.end_page,
            ch.blocks.len()
        );
    }

    if let Some(p) = page {
        if let Some(cp) = book.resolve_page(p) {
            println!(
                "\nResolved Page {}: Chapter '{}' (offset {}): \"{}...\"",
                cp.page_number, cp.chapter_title, cp.char_offset_in_chapter, cp.preview_snippet
            );
        } else {
            println!(
                "\nPage {} is out of range (1..={})",
                p,
                book.total_pages()
            );
        }
    }

    Ok(())
}
