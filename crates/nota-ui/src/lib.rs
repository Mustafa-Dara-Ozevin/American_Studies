//! `nota-ui`: Complete GPUI (`gpui-kit`) Desktop Application for Nota.
//! Includes File Explorer, Turkish Full-Text Search, Source Editor with 61-rule Typing Transformer,
//! Rendered Markdown/Table/Tree Preview, Side-by-Side EPUB Split-View Reader with Deep Page Linking,
//! Interactive Truth Tree (Analytic Tableau) Workbench, FSRS v6 Flashcard Review, and Manual Git Commit Bar.

use chrono::Utc;
use nota_core::{
    normalize_lookup_key, parse_frontmatter, parse_note_body, NotaConfig, TypingRuleEngine,
    VaultIndex,
};
use nota_editor::{
    expand_note_template, rename_note_and_update_links, InteractiveTruthTreeSession, NotaEditorTab,
};
use nota_epub::EpubBook;
use nota_logic::{
    auto_fix_markdown_truth_table, classify_decomposition, format_aligned_markdown_table,
    generate_markdown_truth_table, lint_markdown_table, parse_formula, search_symbol_palette,
    DecompositionRule, NodeContinuation,
};
use nota_site::{build_static_site, export_note_standalone_html, export_note_typst};
use nota_srs::{
    build_daily_review_queue, compute_srs_statistics, extract_cards_from_note,
    filter_cards_for_browser, generate_logic_flashcards_for_input, import_obsidian_sr_to_schedule,
    load_revlog, materialize_schedule_from_events, stamp_missing_card_ids, CardBrowserFilter,
    CardScheduleEntry, CardState, Flashcard, FsrsScheduler, ReviewEvent, ScheduleSnapshot,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod view;
pub use view::{run_desktop_app, NotaWorkspaceView};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CenterPaneMode {
    SourceOnly,
    RenderedPreview,
    SplitSourceAndPreview,
    SplitNoteAndEpub,
    TruthTreeWorkbench,
    FlashcardReview,
    GraphAndMocView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SrsSubTab {
    ReviewQueue,
    CardBrowser,
    Statistics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeftSidebarTab {
    Explorer,
    Search,
    Tasks,
    LinkHealth,
    Books,
}

#[derive(Debug, Clone)]
pub enum Message {
    // Navigation & Tabs
    SelectLeftTab(LeftSidebarTab),
    SetCenterMode(CenterPaneMode),
    OpenNote(String),
    SwitchTab(usize),
    CloseTab(usize),
    CreateNewNoteFromInput,
    NewNoteTitleChanged(String),
    CreateMissingNote(String),
    RenameActiveNoteChanged(String),
    ExecuteRenameActiveNote,

    // Editor & Typing Transformer & Phase 2 Note Tools
    EditorAction(nota_editor::Action),
    ToggleTypingTransformer,
    InsertSymbol(String),
    SymbolPaletteQueryChanged(String),
    CompleteWikilink(String),
    SaveActiveNote,
    UndoEdit,
    RedoEdit,
    ToggleTaskInActiveNote(usize),
    InsertTruthTableForInput,
    NormalizeOperatorsInActiveNote,
    ConvertAsciiTreesInActiveNote,
    ConvertHtmlTablesInActiveNote,
    CleanMetadataInActiveNote,
    AutoFixTruthTableInActiveNote(usize),
    AlignMarkdownTableInActiveNote(usize),

    // Search & Filters
    FilterQueryChanged(String),
    ToggleRightSidebar,
    ToggleLeftSidebar,
    ToggleTheme,

    // Truth Tree Workbench
    TreeProblemInputChanged(String),
    LoadTreeFromProblemInput,
    SelectTreeNode(usize),
    DecomposeSelectedTreeNode,
    SolveFullTruthTree,
    ToggleTreePracticeMode,
    ToggleTreeUserBranchMode,
    TreeUserLeftInputChanged(String),
    TreeUserRightInputChanged(String),
    InsertTreeIntoActiveNote,

    // EPUB Split View & Page Linking
    OpenEpubFile(PathBuf),
    EpubPrevPage,
    EpubNextPage,
    EpubJumpPageInputChanged(String),
    EpubExecuteJumpPage,
    EpubSelectChapter(usize),
    EpubQuoteInputChanged(String),
    EpubInsertQuoteCitationToNote,
    EpubCopyPageWikilinkToNote,

    // Spaced Repetition (FSRS v6, Card Browser & Stats)
    SelectSrsSubTab(SrsSubTab),
    SetSrsBrowserFilter(CardBrowserFilter),
    SrsBrowserSearchChanged(String),
    ToggleSrsSiblingBurying,
    ToggleSuspendSrsCard(String),
    StampCardIdsInActiveNote,
    ImportObsidianSrComments,
    RefreshSrsQueue,
    GenerateLogicCardsFromProblemInput,
    RevealSrsAnswer,
    RateSrsCard(u8),

    // Graph View & MOC Generator
    ToggleGraphLocalMode,
    ToggleGraphOrphans,
    ToggleGraphUnresolved,
    ToggleGraphOnlyMocs,
    MocTitleInputChanged(String),
    MocTagFilterChanged(String),
    GenerateMocNote,

    // Export & Static Site Build
    ExportActiveNoteHtml,
    ExportActiveNoteTypst,
    BuildStaticSite,

    // Git Status & Manual Commit
    RefreshGitStatus,
    CommitMessageChanged(String),
    ExecuteGitCommit,
}

pub struct NotaApp {
    pub config: NotaConfig,
    pub vault_root: PathBuf,
    pub index: VaultIndex,
    pub typing_rules: TypingRuleEngine,
    pub typing_transformer_enabled: bool,

    // UI layout state
    pub dark_mode: bool,
    pub left_tab: LeftSidebarTab,
    pub center_mode: CenterPaneMode,
    pub show_right_sidebar: bool,
    pub show_left_sidebar: bool,
    pub filter_query: String,
    pub symbol_palette_query: String,
    pub new_note_title: String,
    pub rename_input: String,
    pub status_banner: String,

    // Open note tabs
    pub tabs: Vec<NotaEditorTab>,
    pub active_tab: usize,

    // Truth Tree Workbench state
    pub tree_session: InteractiveTruthTreeSession,

    // EPUB Split-View Reader state
    pub available_epubs: Vec<PathBuf>,
    pub active_epub: Option<EpubBook>,
    pub epub_current_page: usize,
    pub epub_jump_input: String,
    pub epub_quote_input: String,

    // FSRS Spaced Repetition state
    pub srs_sub_tab: SrsSubTab,
    pub srs_browser_filter: CardBrowserFilter,
    pub srs_browser_search: String,
    pub srs_bury_siblings: bool,
    pub srs_all_cards: Vec<Flashcard>,
    pub srs_cards: Vec<Flashcard>,
    pub srs_events: Vec<ReviewEvent>,
    pub srs_schedule: ScheduleSnapshot,
    pub srs_current_idx: usize,
    pub srs_answer_revealed: bool,

    // Graph View & MOC state
    pub graph_local_mode: bool,
    pub graph_include_orphans: bool,
    pub graph_include_unresolved: bool,
    pub graph_only_mocs: bool,
    pub moc_title_input: String,
    pub moc_tag_filter: String,

    // Git state
    pub git_branch: String,
    pub git_dirty_count: usize,
    pub commit_message_input: String,
}

impl NotaApp {
    pub fn load(config: NotaConfig, initial_note: Option<&str>) -> anyhow::Result<Self> {
        let vault_root = config.vault.path.clone();
        let index = VaultIndex::scan(&vault_root, &config.vault)?;
        let rules_path = Path::new("typing-rules.conf");
        let typing_rules = if rules_path.exists() {
            TypingRuleEngine::from_conf_file(rules_path).unwrap_or_default()
        } else {
            TypingRuleEngine::default()
        };

        let tree_session = InteractiveTruthTreeSession::from_problem_str(
            "p -> q, r -> s, ~q v ~s ∴ ~p v ~r",
            false,
        )?;

        let available_epubs = discover_epub_files(&config);
        let srs_events = load_revlog(&config.srs.revlog_path).unwrap_or_default();
        let srs_schedule = materialize_schedule_from_events(&srs_events);

        let (git_branch, git_dirty_count) = query_git_status();

        let mut app = Self {
            config,
            vault_root,
            index,
            typing_rules,
            typing_transformer_enabled: true,
            dark_mode: true,
            left_tab: LeftSidebarTab::Explorer,
            center_mode: CenterPaneMode::SplitSourceAndPreview,
            show_right_sidebar: true,
            show_left_sidebar: true,
            filter_query: String::new(),
            symbol_palette_query: String::new(),
            new_note_title: String::new(),
            rename_input: String::new(),
            status_banner: "Ready — 61 Typing Transformer logic rules active".to_string(),
            tabs: Vec::new(),
            active_tab: 0,
            tree_session,
            available_epubs,
            active_epub: None,
            epub_current_page: 1,
            epub_jump_input: "1".to_string(),
            epub_quote_input: String::new(),
            srs_sub_tab: SrsSubTab::ReviewQueue,
            srs_browser_filter: CardBrowserFilter::All,
            srs_browser_search: String::new(),
            srs_bury_siblings: true,
            srs_all_cards: Vec::new(),
            srs_cards: Vec::new(),
            srs_events,
            srs_schedule,
            srs_current_idx: 0,
            srs_answer_revealed: false,
            graph_local_mode: false,
            graph_include_orphans: true,
            graph_include_unresolved: false,
            graph_only_mocs: false,
            moc_title_input: "Custom_MOC".to_string(),
            moc_tag_filter: String::new(),
            git_branch,
            git_dirty_count,
            commit_message_input: String::new(),
        };

        app.rebuild_srs_cards();

        let target_note = initial_note
            .map(|s| s.to_string())
            .or_else(|| {
                if app.index.notes.contains_key("Sembolik Mantık Giriş.md") {
                    Some("Sembolik Mantık Giriş.md".to_string())
                } else {
                    app.index.notes.keys().next().cloned()
                }
            });

        if let Some(rel) = target_note {
            app.open_note_tab(&rel);
        }

        Ok(app)
    }

    pub fn open_note_tab(&mut self, rel_path: &str) {
        let normalized = if rel_path.ends_with(".md") {
            rel_path.to_string()
        } else {
            format!("{}.md", rel_path)
        };

        if let Some(existing_idx) = self.tabs.iter().position(|t| t.rel_path == normalized) {
            self.active_tab = existing_idx;
            self.rename_input = self.tabs[existing_idx].title.clone();
            return;
        }

        let abs_path = self.vault_root.join(&normalized);
        let raw_text = fs::read_to_string(&abs_path).unwrap_or_default();
        let tab = NotaEditorTab::new(normalized.clone(), &raw_text);
        self.rename_input = tab.title.clone();
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn current_tab(&self) -> Option<&NotaEditorTab> {
        self.tabs.get(self.active_tab)
    }

    pub fn current_tab_mut(&mut self) -> Option<&mut NotaEditorTab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectLeftTab(tab) => {
                self.left_tab = tab;
            }
            Message::SetCenterMode(mode) => {
                self.center_mode = mode;
            }
            Message::OpenNote(rel_path) => {
                // Check if this is an EPUB link `Book.epub#p=42`
                if rel_path.to_ascii_lowercase().contains(".epub") {
                    let (book_part, frag) = match rel_path.split_once('#') {
                        Some((b, f)) => (b, Some(f)),
                        None => (rel_path.as_str(), None),
                    };
                    let target = nota_core::parse_epub_link_target(book_part, frag);
                    if let Some(epub_path) = self.find_epub_path(&target.book_file) {
                        if let Ok(book) = EpubBook::open(&epub_path) {
                            let page = target.page.unwrap_or(1).clamp(1, book.total_pages().max(1));
                            self.active_epub = Some(book);
                            self.epub_current_page = page;
                            self.epub_jump_input = page.to_string();
                            self.center_mode = CenterPaneMode::SplitNoteAndEpub;
                            self.status_banner =
                                format!("Opened {} at canonical page {}", target.book_file, page);
                            return;
                        }
                    }
                }
                self.open_note_tab(&rel_path);
            }
            Message::SwitchTab(idx) => {
                if idx < self.tabs.len() {
                    self.active_tab = idx;
                    self.rename_input = self.tabs[idx].title.clone();
                }
            }
            Message::CloseTab(idx) => {
                if idx < self.tabs.len() && self.tabs.len() > 1 {
                    self.tabs.remove(idx);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
            }
            Message::NewNoteTitleChanged(val) => {
                self.new_note_title = val;
            }
            Message::CreateNewNoteFromInput => {
                let title = self.new_note_title.trim().to_string();
                if !title.is_empty() {
                    self.create_note_with_template(&title);
                    self.new_note_title.clear();
                }
            }
            Message::CreateMissingNote(target_name) => {
                self.create_note_with_template(&target_name);
            }
            Message::RenameActiveNoteChanged(val) => {
                self.rename_input = val;
            }
            Message::ExecuteRenameActiveNote => {
                let new_stem = self.rename_input.trim().to_string();
                if let Some(cur) = self.current_tab() {
                    let old_rel = cur.rel_path.clone();
                    let new_rel = format!("{}.md", new_stem);
                    if !new_stem.is_empty() && old_rel != new_rel {
                        let all_paths: Vec<String> = self.index.notes.keys().cloned().collect();
                        if let Ok(updated_count) = rename_note_and_update_links(
                            &self.vault_root,
                            &old_rel,
                            &new_rel,
                            &all_paths,
                        ) {
                            if let Some(tab) = self.current_tab_mut() {
                                tab.rel_path = new_rel.clone();
                                tab.title = new_stem;
                            }
                            if let Ok(reindexed) =
                                VaultIndex::scan(&self.vault_root, &self.config.vault)
                            {
                                self.index = reindexed;
                            }
                            self.status_banner = format!(
                                "Renamed to {} and updated wikilinks in {} notes",
                                new_rel, updated_count
                            );
                        }
                    }
                }
            }
            Message::EditorAction(action) => {
                let rules = self.typing_rules.clone();
                let tt_on = self.typing_transformer_enabled;
                if let Some(tab) = self.current_tab_mut() {
                    tab.apply_editor_action(action, &rules, tt_on);
                    if let Some(ref matched) = tab.last_matched_rule {
                        self.status_banner = format!("Typing Transformer: {}", matched);
                    }
                }
            }
            Message::ToggleTypingTransformer => {
                self.typing_transformer_enabled = !self.typing_transformer_enabled;
                self.status_banner = format!(
                    "Typing Transformer: {}",
                    if self.typing_transformer_enabled {
                        "ON"
                    } else {
                        "OFF"
                    }
                );
            }
            Message::InsertSymbol(sym) => {
                if let Some(tab) = self.current_tab_mut() {
                    tab.insert_block_at_cursor(&sym);
                }
            }
            Message::SymbolPaletteQueryChanged(q) => {
                self.symbol_palette_query = q;
            }
            Message::CompleteWikilink(stem) => {
                let exists = self
                    .index
                    .notes
                    .values()
                    .any(|n| normalize_lookup_key(&n.file_stem) == normalize_lookup_key(&stem));
                if !exists {
                    self.create_note_with_template(&stem);
                }
                if let Some(tab) = self.current_tab_mut() {
                    tab.complete_wikilink(&stem);
                }
            }
            Message::SaveActiveNote => {
                let vault_root = self.vault_root.clone();
                if let Some(tab) = self.current_tab_mut() {
                    if let Err(err) = tab.save_atomic(&vault_root) {
                        self.status_banner = format!("Save error: {}", err);
                        return;
                    }
                    let saved_path = tab.rel_path.clone();
                    if let Ok(reindexed) = VaultIndex::scan(&self.vault_root, &self.config.vault) {
                        self.index = reindexed;
                    }
                    let (branch, dirty) = query_git_status();
                    self.git_branch = branch;
                    self.git_dirty_count = dirty;
                    self.status_banner = format!("Saved {}", saved_path);
                }
            }
            Message::UndoEdit => {
                if let Some(tab) = self.current_tab_mut() {
                    tab.undo();
                }
            }
            Message::RedoEdit => {
                if let Some(tab) = self.current_tab_mut() {
                    tab.redo();
                }
            }
            Message::ToggleTaskInActiveNote(line_no) => {
                if let Some(tab) = self.current_tab_mut() {
                    let current = tab.text();
                    let mut lines: Vec<String> = current.lines().map(|s| s.to_string()).collect();
                    if line_no > 0 && line_no <= lines.len() {
                        let line = &mut lines[line_no - 1];
                        if line.contains("[ ]") {
                            *line = line.replacen("[ ]", "[x]", 1);
                        } else if line.contains("[x]") {
                            *line = line.replacen("[x]", "[ ]", 1);
                        } else if line.contains("[X]") {
                            *line = line.replacen("[X]", "[ ]", 1);
                        }
                        let new_text = lines.join("\n");
                        tab.content = nota_editor::Content::with_text(&new_text);
                        tab.is_dirty = true;
                    }
                }
            }
            Message::InsertTruthTableForInput => {
                let input = self.tree_session.problem_input.clone();
                if let Ok(f) = parse_formula(&input) {
                    if let Ok(md_table) = generate_markdown_truth_table(
                        &[f],
                        self.config.logic.packed_variable_columns,
                        &self.config.logic.true_symbol,
                        &self.config.logic.false_symbol,
                    ) {
                        if let Some(tab) = self.current_tab_mut() {
                            tab.insert_block_at_cursor(&format!("\n{}\n", md_table));
                            self.status_banner =
                                "Inserted canonical alphabetical Truth Table into note".to_string();
                        }
                    }
                }
            }
            Message::NormalizeOperatorsInActiveNote => {
                if let Some(tab) = self.current_tab_mut() {
                    let count = tab.normalize_ascii_operators_in_note();
                    self.status_banner = format!(
                        "Normalized {} ASCII logic operator(s) to Unicode (Ctrl+Z to undo)",
                        count
                    );
                }
            }
            Message::ConvertAsciiTreesInActiveNote => {
                if let Some(tab) = self.current_tab_mut() {
                    let count = tab.convert_ascii_trees_in_note();
                    self.status_banner = format!(
                        "Converted {} ASCII Truth Tree(s) into ```tree fenced blocks",
                        count
                    );
                }
            }
            Message::ConvertHtmlTablesInActiveNote => {
                if let Some(tab) = self.current_tab_mut() {
                    let count = tab.convert_html_tables_in_note();
                    self.status_banner = format!(
                        "Converted {} HTML <table> block(s) into aligned Markdown pipe tables",
                        count
                    );
                }
            }
            Message::CleanMetadataInActiveNote => {
                if let Some(tab) = self.current_tab_mut() {
                    let changes = tab.clean_metadata_and_highlights_in_note();
                    if changes.is_empty() {
                        self.status_banner = "No metadata or highlight issues to clean".to_string();
                    } else {
                        self.status_banner = format!("Cleaned: {}", changes.join("; "));
                    }
                }
            }
            Message::AutoFixTruthTableInActiveNote(table_idx) => {
                let true_sym = self.config.logic.true_symbol.clone();
                let false_sym = self.config.logic.false_symbol.clone();
                if let Some(tab) = self.current_tab_mut() {
                    let raw = tab.text();
                    let fm = parse_frontmatter(&raw);
                    let parsed = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
                    if let Some(tbl) = parsed.tables.get(table_idx) {
                        if let Ok(fixed_md) = auto_fix_markdown_truth_table(
                            &tbl.headers,
                            &tbl.rows,
                            &true_sym,
                            &false_sym,
                        ) {
                            tab.replace_table_lines(tbl.start_line, tbl.end_line, &fixed_md);
                            self.status_banner = format!(
                                "Auto-fixed Truth Table at line {} (canonical alphabetical atoms & evaluated rows)",
                                tbl.start_line
                            );
                        }
                    }
                }
            }
            Message::AlignMarkdownTableInActiveNote(table_idx) => {
                if let Some(tab) = self.current_tab_mut() {
                    let raw = tab.text();
                    let fm = parse_frontmatter(&raw);
                    let parsed = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
                    if let Some(tbl) = parsed.tables.get(table_idx) {
                        let aligned = format_aligned_markdown_table(&tbl.headers, &tbl.rows);
                        tab.replace_table_lines(tbl.start_line, tbl.end_line, &aligned);
                        self.status_banner =
                            format!("Auto-aligned Markdown table at line {}", tbl.start_line);
                    }
                }
            }
            Message::FilterQueryChanged(q) => {
                self.filter_query = q;
            }
            Message::ToggleLeftSidebar => {
                self.show_left_sidebar = !self.show_left_sidebar;
            }
            Message::ToggleRightSidebar => {
                self.show_right_sidebar = !self.show_right_sidebar;
            }
            Message::ToggleTheme => {
                self.dark_mode = !self.dark_mode;
            }
            Message::TreeProblemInputChanged(val) => {
                self.tree_session.problem_input = val;
            }
            Message::LoadTreeFromProblemInput => {
                let input = self.tree_session.problem_input.clone();
                let practice = self.tree_session.practice_mode;
                match InteractiveTruthTreeSession::from_problem_str(&input, practice) {
                    Ok(sess) => {
                        self.tree_session = sess;
                        self.status_banner = format!("Initialized Truth Tree for `{}`", input);
                    }
                    Err(err) => {
                        self.status_banner = format!("Formula parse error: {}", err);
                    }
                }
            }
            Message::SelectTreeNode(id) => {
                self.tree_session.selected_node = Some(id);
            }
            Message::DecomposeSelectedTreeNode => {
                if let Err(err) = self.tree_session.decompose_selected() {
                    self.status_banner = format!("Truth Tree step error: {}", err);
                }
            }
            Message::SolveFullTruthTree => {
                if let Err(err) = self.tree_session.solve_all() {
                    self.status_banner = format!("Truth Tree solve error: {}", err);
                }
            }
            Message::ToggleTreePracticeMode => {
                self.tree_session.practice_mode = !self.tree_session.practice_mode;
            }
            Message::ToggleTreeUserBranchMode => {
                self.tree_session.user_branch_mode = !self.tree_session.user_branch_mode;
            }
            Message::TreeUserLeftInputChanged(val) => {
                self.tree_session.user_left_input = val;
            }
            Message::TreeUserRightInputChanged(val) => {
                self.tree_session.user_right_input = val;
            }
            Message::InsertTreeIntoActiveNote => {
                let dsl = format!("\n```tree\n{}```\n", self.tree_session.tree.to_tree_dsl());
                if let Some(tab) = self.current_tab_mut() {
                    tab.insert_block_at_cursor(&dsl);
                    self.status_banner = "Inserted ```tree block into active note".to_string();
                }
            }
            Message::OpenEpubFile(path) => {
                match EpubBook::open(&path) {
                    Ok(book) => {
                        self.status_banner = format!(
                            "Opened EPUB '{}' ({} canonical pages)",
                            book.title,
                            book.total_pages()
                        );
                        self.active_epub = Some(book);
                        self.epub_current_page = 1;
                        self.epub_jump_input = "1".to_string();
                        self.center_mode = CenterPaneMode::SplitNoteAndEpub;
                    }
                    Err(err) => {
                        self.status_banner = format!("Failed to open EPUB: {}", err);
                    }
                }
            }
            Message::EpubPrevPage => {
                if self.epub_current_page > 1 {
                    self.epub_current_page -= 1;
                    self.epub_jump_input = self.epub_current_page.to_string();
                }
            }
            Message::EpubNextPage => {
                if let Some(ref book) = self.active_epub {
                    if self.epub_current_page < book.total_pages() {
                        self.epub_current_page += 1;
                        self.epub_jump_input = self.epub_current_page.to_string();
                    }
                }
            }
            Message::EpubJumpPageInputChanged(val) => {
                self.epub_jump_input = val;
            }
            Message::EpubExecuteJumpPage => {
                if let (Some(ref book), Ok(p)) =
                    (&self.active_epub, self.epub_jump_input.trim().parse::<usize>())
                {
                    self.epub_current_page = p.clamp(1, book.total_pages().max(1));
                    self.epub_jump_input = self.epub_current_page.to_string();
                }
            }
            Message::EpubSelectChapter(spine_idx) => {
                if let Some(ref book) = self.active_epub {
                    if let Some(ch) = book.chapters.get(spine_idx) {
                        self.epub_current_page = ch.start_page;
                        self.epub_jump_input = ch.start_page.to_string();
                    }
                }
            }
            Message::EpubQuoteInputChanged(val) => {
                self.epub_quote_input = val;
            }
            Message::EpubInsertQuoteCitationToNote => {
                let citation_info = self.active_epub.as_ref().map(|book| {
                    let quote = if self.epub_quote_input.trim().is_empty() {
                        book.resolve_page(self.epub_current_page)
                            .map(|p| p.preview_snippet.clone())
                            .unwrap_or_default()
                    } else {
                        self.epub_quote_input.clone()
                    };
                    (
                        book.title.clone(),
                        book.format_quote_citation(&quote, self.epub_current_page, None),
                    )
                });
                if let Some((book_title, citation)) = citation_info {
                    let page = self.epub_current_page;
                    if let Some(tab) = self.current_tab_mut() {
                        tab.insert_block_at_cursor(&format!("\n{}\n", citation));
                        self.status_banner = format!(
                            "Inserted quote & page link to {} (s. {})",
                            book_title, page
                        );
                    }
                }
            }
            Message::EpubCopyPageWikilinkToNote => {
                let link_opt = self.active_epub.as_ref().map(|book| {
                    format!(
                        "[[{}#p={}|{}, s. {}]]",
                        book.file_name, self.epub_current_page, book.title, self.epub_current_page
                    )
                });
                if let Some(link) = link_opt {
                    if let Some(tab) = self.current_tab_mut() {
                        tab.insert_block_at_cursor(&link);
                        self.status_banner = format!("Inserted {}", link);
                    }
                }
            }
            Message::SelectSrsSubTab(sub) => {
                self.srs_sub_tab = sub;
            }
            Message::SetSrsBrowserFilter(filter) => {
                self.srs_browser_filter = filter;
            }
            Message::SrsBrowserSearchChanged(q) => {
                self.srs_browser_search = q;
            }
            Message::ToggleSrsSiblingBurying => {
                self.srs_bury_siblings = !self.srs_bury_siblings;
                self.rebuild_srs_cards();
                self.status_banner = format!(
                    "Sibling burying: {}",
                    if self.srs_bury_siblings { "ON" } else { "OFF" }
                );
            }
            Message::ToggleSuspendSrsCard(card_id) => {
                let now = Utc::now();
                let today = now.format("%Y-%m-%d").to_string();
                let entry = self
                    .srs_schedule
                    .cards
                    .entry(card_id.clone())
                    .or_insert_with(|| CardScheduleEntry {
                        note: String::new(),
                        due: today,
                        state: CardState::New,
                        stability: 1.0,
                        difficulty: 5.0,
                        reps: 0,
                        lapses: 0,
                        last_review: now,
                    });
                entry.state = if entry.state == CardState::Suspended {
                    CardState::Review
                } else {
                    CardState::Suspended
                };
                if let Ok(json) = serde_json::to_string_pretty(&self.srs_schedule) {
                    let _ = fs::write(&self.config.srs.schedule_path, json.as_bytes());
                }
                self.rebuild_srs_cards();
                self.status_banner = format!("Toggled suspend state for card {}", card_id);
            }
            Message::StampCardIdsInActiveNote => {
                if let Some(tab) = self.current_tab_mut() {
                    let cur = tab.text();
                    let (stamped, count) = stamp_missing_card_ids(&cur);
                    if count > 0 {
                        tab.content = nota_editor::Content::with_text(&stamped);
                        tab.is_dirty = true;
                    }
                    self.status_banner =
                        format!("Stamped {} missing <!--c:ULID--> card ID(s) in note", count);
                }
            }
            Message::ImportObsidianSrComments => {
                let count = import_obsidian_sr_to_schedule(
                    &self.srs_all_cards,
                    &mut self.srs_schedule,
                    Utc::now(),
                );
                if let Ok(json) = serde_json::to_string_pretty(&self.srs_schedule) {
                    let _ = fs::write(&self.config.srs.schedule_path, json.as_bytes());
                }
                self.rebuild_srs_cards();
                self.status_banner =
                    format!("Imported {} Obsidian SR schedule comment(s)", count);
            }
            Message::RefreshSrsQueue => {
                self.rebuild_srs_cards();
            }
            Message::GenerateLogicCardsFromProblemInput => {
                if let Ok(generated) =
                    generate_logic_flashcards_for_input(&self.tree_session.problem_input)
                {
                    let mut block = String::from("\n");
                    for (front, back) in generated {
                        let one_line_back = back.replace('\n', " <br> ");
                        block.push_str(&format!("{}::{}\n", front, one_line_back));
                    }
                    if let Some(tab) = self.current_tab_mut() {
                        tab.insert_block_at_cursor(&block);
                        self.status_banner =
                            "Generated logic & truth tree flashcards into active note".to_string();
                    }
                }
            }
            Message::RevealSrsAnswer => {
                self.srs_answer_revealed = true;
            }
            Message::RateSrsCard(rating) => {
                if let Some(card) = self.srs_cards.get(self.srs_current_idx).cloned() {
                    if let Ok(scheduler) =
                        FsrsScheduler::new(self.config.srs.desired_retention as f32)
                    {
                        let prev = self.srs_schedule.cards.get(&card.id);
                        if let Ok(ev) = scheduler.record_review(
                            &card.id,
                            &card.note_path,
                            prev,
                            rating,
                            Utc::now(),
                        ) {
                            self.srs_events.push(ev.clone());
                            self.srs_schedule =
                                materialize_schedule_from_events(&self.srs_events);
                            let _ = append_revlog_and_save_schedule(
                                &self.config.srs.revlog_path,
                                &self.config.srs.schedule_path,
                                &ev,
                                &self.srs_schedule,
                            );
                            self.srs_answer_revealed = false;
                            if self.srs_current_idx + 1 < self.srs_cards.len() {
                                self.srs_current_idx += 1;
                            } else {
                                self.srs_current_idx = 0;
                            }
                            self.status_banner = format!(
                                "Recorded FSRS rating {} for card {} (next due: {})",
                                rating, ev.card, ev.due
                            );
                        }
                    }
                }
            }
            Message::ToggleGraphLocalMode => {
                self.graph_local_mode = !self.graph_local_mode;
            }
            Message::ToggleGraphOrphans => {
                self.graph_include_orphans = !self.graph_include_orphans;
            }
            Message::ToggleGraphUnresolved => {
                self.graph_include_unresolved = !self.graph_include_unresolved;
            }
            Message::ToggleGraphOnlyMocs => {
                self.graph_only_mocs = !self.graph_only_mocs;
            }
            Message::MocTitleInputChanged(val) => {
                self.moc_title_input = val;
            }
            Message::MocTagFilterChanged(val) => {
                self.moc_tag_filter = val;
            }
            Message::GenerateMocNote => {
                let title = if self.moc_title_input.trim().is_empty() {
                    "Generated_MOC".to_string()
                } else {
                    self.moc_title_input.trim().to_string()
                };
                let filter_opt = if self.moc_tag_filter.trim().is_empty() {
                    None
                } else {
                    Some(self.moc_tag_filter.trim())
                };
                let moc_md = self.index.generate_moc_markdown(&title, filter_opt);
                let rel_path = format!("{}.md", title.trim_end_matches(".md"));
                let abs_path = self.vault_root.join(&rel_path);
                let _ = fs::write(&abs_path, moc_md.as_bytes());
                if let Ok(reindexed) = VaultIndex::scan(&self.vault_root, &self.config.vault) {
                    self.index = reindexed;
                }
                self.open_note_tab(&rel_path);
                self.center_mode = CenterPaneMode::SplitSourceAndPreview;
                self.status_banner = format!("Generated MOC note {}", rel_path);
            }
            Message::ExportActiveNoteHtml => {
                if let Some(tab) = self.current_tab() {
                    let raw = tab.text();
                    let fm = parse_frontmatter(&raw);
                    let body = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
                    let temp_note = nota_core::IndexedNote {
                        rel_path: tab.rel_path.clone(),
                        file_name: tab.rel_path.clone(),
                        file_stem: tab.title.clone(),
                        slug: nota_core::slugify_file_path(&tab.rel_path),
                        is_moc: tab.title.to_ascii_lowercase().contains("moc"),
                        frontmatter: fm,
                        body,
                        raw_len_bytes: raw.len(),
                    };
                    let html = export_note_standalone_html(&temp_note, &raw, &self.index);
                    let out_dir = &self.config.site.out_dir;
                    let _ = fs::create_dir_all(out_dir);
                    let dest = out_dir.join(format!("{}.html", temp_note.slug));
                    if fs::write(&dest, html.as_bytes()).is_ok() {
                        self.status_banner = format!("Exported HTML to {}", dest.display());
                    }
                }
            }
            Message::ExportActiveNoteTypst => {
                if let Some(tab) = self.current_tab() {
                    let raw = tab.text();
                    let fm = parse_frontmatter(&raw);
                    let body = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
                    let temp_note = nota_core::IndexedNote {
                        rel_path: tab.rel_path.clone(),
                        file_name: tab.rel_path.clone(),
                        file_stem: tab.title.clone(),
                        slug: nota_core::slugify_file_path(&tab.rel_path),
                        is_moc: tab.title.to_ascii_lowercase().contains("moc"),
                        frontmatter: fm,
                        body,
                        raw_len_bytes: raw.len(),
                    };
                    let typ = export_note_typst(&temp_note, &raw);
                    let out_dir = &self.config.site.out_dir;
                    let _ = fs::create_dir_all(out_dir);
                    let dest = out_dir.join(format!("{}.typ", temp_note.slug));
                    if fs::write(&dest, typ.as_bytes()).is_ok() {
                        self.status_banner = format!("Exported Typst PDF source to {}", dest.display());
                    }
                }
            }
            Message::BuildStaticSite => {
                let out_dir = self.config.site.out_dir.clone();
                match build_static_site(&self.index, &self.config.site, &out_dir) {
                    Ok(summary) => {
                        self.status_banner = format!(
                            "Built site to {}: {} pages, {} tag pages, {} redirects, {} assets",
                            summary.out_dir.display(),
                            summary.pages_built,
                            summary.tag_pages_built,
                            summary.alias_redirects_built,
                            summary.assets_copied
                        );
                    }
                    Err(err) => {
                        self.status_banner = format!("Site build error: {}", err);
                    }
                }
            }
            Message::RefreshGitStatus => {
                let (b, d) = query_git_status();
                self.git_branch = b;
                self.git_dirty_count = d;
            }
            Message::CommitMessageChanged(msg) => {
                self.commit_message_input = msg;
            }
            Message::ExecuteGitCommit => {
                let msg = if self.commit_message_input.trim().is_empty() {
                    format!("Nota sync: {} reviews", self.srs_events.len())
                } else {
                    self.commit_message_input.trim().to_string()
                };
                match execute_git_commit(&msg) {
                    Ok(summary) => {
                        self.commit_message_input.clear();
                        let (b, d) = query_git_status();
                        self.git_branch = b;
                        self.git_dirty_count = d;
                        self.status_banner = summary;
                    }
                    Err(err) => {
                        self.status_banner = format!("Git commit error: {}", err);
                    }
                }
            }
        }
    }

    fn create_note_with_template(&mut self, title: &str) {
        let clean_title = title.trim().trim_end_matches(".md");
        if clean_title.is_empty() {
            return;
        }
        let rel_path = format!("{}.md", clean_title);
        let abs_path = self.vault_root.join(&rel_path);
        if !abs_path.exists() {
            let tpl_path = self.config.vault.templates_dir.join("Basic Note.md");
            let tpl_raw = fs::read_to_string(&tpl_path).unwrap_or_else(|_| {
                "---\ntitle: \"{{title}}\"\ntags: []\naliases: []\n---\n\n# {{title}}\n\n"
                    .to_string()
            });
            let expanded = expand_note_template(&tpl_raw, clean_title);
            let _ = fs::write(&abs_path, expanded.as_bytes());
            if let Ok(reindexed) = VaultIndex::scan(&self.vault_root, &self.config.vault) {
                self.index = reindexed;
            }
        }
        self.open_note_tab(&rel_path);
        self.status_banner = format!("Created and opened {}", rel_path);
    }

    fn find_epub_path(&self, book_file: &str) -> Option<PathBuf> {
        let key = normalize_lookup_key(book_file);
        for p in &self.available_epubs {
            if let Some(fname) = p.file_name() {
                if normalize_lookup_key(&fname.to_string_lossy()) == key {
                    return Some(p.clone());
                }
            }
        }
        None
    }

    fn rebuild_srs_cards(&mut self) {
        let mut all_cards = Vec::new();
        for note in self.index.notes.values() {
            let abs = self.vault_root.join(&note.rel_path);
            let Ok(raw) = fs::read_to_string(&abs) else {
                continue;
            };
            let deck = note.frontmatter.deck.as_deref().unwrap_or("default");
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
                deck,
                opt_in,
            );
            all_cards.extend(cards);
        }
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.srs_cards = build_daily_review_queue(
            &all_cards,
            &self.srs_schedule,
            &today,
            None,
            self.srs_bury_siblings,
            self.config.srs.new_cards_per_day,
            self.config.srs.max_reviews_per_day,
        );
        self.srs_all_cards = all_cards;
        self.srs_current_idx = 0;
    }
}

fn discover_epub_files(config: &NotaConfig) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in &config.vault.books_dirs {
        if dir.exists() {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension()
                        .map(|e| e.to_string_lossy().eq_ignore_ascii_case("epub"))
                        .unwrap_or(false)
                    {
                        out.push(p);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

fn append_revlog_and_save_schedule(
    revlog_path: &Path,
    schedule_path: &Path,
    event: &ReviewEvent,
    snapshot: &ScheduleSnapshot,
) -> anyhow::Result<()> {
    if let Some(parent) = revlog_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut line = serde_json::to_string(event)?;
    line.push('\n');
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(revlog_path)?;
    f.write_all(line.as_bytes())?;

    let sched_json = serde_json::to_string_pretty(snapshot)?;
    fs::write(schedule_path, sched_json.as_bytes())?;
    Ok(())
}

fn query_git_status() -> (String, usize) {
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "main".to_string());

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);

    (branch, dirty)
}

fn execute_git_commit(message: &str) -> anyhow::Result<String> {
    let add_out = Command::new("git")
        .args(["add", "content", "reviews", "nota.toml", "typing-rules.conf", "books/catalog.toml"])
        .output()?;
    if !add_out.status.success() {
        return Err(anyhow::anyhow!(
            "git add failed: {}",
            String::from_utf8_lossy(&add_out.stderr)
        ));
    }
    let commit_out = Command::new("git")
        .args(["commit", "-m", message])
        .output()?;
    if !commit_out.status.success() {
        return Err(anyhow::anyhow!(
            "{}",
            String::from_utf8_lossy(&commit_out.stderr).trim()
        ));
    }
    Ok(format!("Committed: {}", message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nota_app_state_transitions_and_views() {
        let cfg = NotaConfig::load_from_path(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("nota.toml"),
        )
        .unwrap_or_default();
        let vault_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("content");
        let mut cfg = cfg;
        cfg.vault.path = vault_root;

        let mut app = NotaApp::load(cfg, Some("Sembolik Mantık Giriş.md")).unwrap();
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].rel_path, "Sembolik Mantık Giriş.md");

        // Switch to Truth Tree Workbench and solve the tree
        app.update(Message::SetCenterMode(CenterPaneMode::TruthTreeWorkbench));
        app.update(Message::SolveFullTruthTree);
        assert!(app
            .tree_session
            .tree
            .to_tree_dsl()
            .contains("verdict: valid"));

        // Switch to another note tab
        app.update(Message::OpenNote("Mantık Devreleri.md".to_string()));
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active_tab, 1);
    }
}
