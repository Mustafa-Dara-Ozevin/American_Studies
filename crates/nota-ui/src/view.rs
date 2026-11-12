//! GPUI (`gpui-kit` = `gpui` + `gpui-component`) view layer for Nota.
//! `NotaApp` (in `lib.rs`) remains the deterministic, headless-testable controller;
//! `NotaWorkspaceView` renders it and routes widget events back through `Message`.

use crate::*;
use gpui_kit::component::{
    button::Button,
    menu::{DropdownMenu as _, PopupMenuItem},
    h_flex,
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    resizable::{h_resizable, resizable_panel},
    text::TextView,
    v_flex, ActiveTheme as _, RopeExt as _, Selectable as _, Sizable as _, Theme, ThemeMode,
};
use gpui_kit::prelude::*;
use gpui_kit::*;
use nota_logic::{parse_metalogic_stmt, MetalogicStmt, NodeOrigin, TruthTree};
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Mutex;

const NORD_BG: u32 = 0x2e3440;
const NORD_NODE: u32 = 0x3b4252;
const NORD_NODE_SEL: u32 = 0x434c5e;
const NORD_BORDER: u32 = 0x4c566a;
const NORD_TEXT: u32 = 0xeceff4;
const NORD_CYAN: u32 = 0x88c0d0;
const NORD_PURPLE: u32 = 0x8875cc;
const NORD_RED: u32 = 0xbf616a;
const NORD_GREEN: u32 = 0xa3be8c;
const NORD_ORANGE: u32 = 0xd08770;
const NORD_YELLOW: u32 = 0xebcb8b;

/// Link clicks from `TextView` (which requires `Send + Sync` handlers) are queued here
/// and drained by the view on the next frame.
static PENDING_LINKS: Mutex<Vec<String>> = Mutex::new(Vec::new());

type Getter = fn(&NotaApp) -> String;

/// Obsidian-style table editor state for the Markdown table under the source cursor.
struct TableEdit {
    /// First / last line (0-based, inclusive) of the table in the note text.
    start: usize,
    end: usize,
    /// Trimmed cell text; row 0 is the header row.
    model: Vec<Vec<String>>,
    cells: Vec<Vec<Entity<InputState>>>,
}

#[derive(Clone, Copy)]
enum TableOp {
    AddRowBelow,
    AddColRight,
    DeleteRow,
    DeleteCol,
    Align,
    AutoFixTruthTable,
}

fn is_table_row(l: &str) -> bool {
    l.trim_start().starts_with('|')
}

fn is_table_sep(l: &str) -> bool {
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    !t.trim().is_empty()
        && t.split('|').all(|c| {
            let c = c.trim();
            !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')
        })
}

fn split_table_row(l: &str) -> Vec<String> {
    let t = l.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = if t.ends_with("\\|") { t } else { t.strip_suffix('|').unwrap_or(t) };
    t.replace("\\|", "\u{0}")
        .split('|')
        .map(|c| c.replace('\u{0}', "|").trim().to_string())
        .collect()
}

/// Parses table lines (header, separator, body rows) into a rectangular trimmed model.
fn parse_table_model(lines: &[String]) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(split_table_row(&lines[0]));
    for l in lines.iter().skip(2) {
        rows.push(split_table_row(l));
    }
    let n = rows[0].len().max(1);
    for r in rows.iter_mut() {
        r.resize(n, String::new());
    }
    rows
}

pub struct NotaWorkspaceView {
    app: NotaApp,
    editor: Entity<TextareaState>,
    bindings: Vec<(Entity<InputState>, Getter)>,
    filter_in: Entity<InputState>,
    new_note_in: Entity<InputState>,
    rename_in: Entity<InputState>,
    symbol_in: Entity<InputState>,
    tree_problem_in: Entity<InputState>,
    tree_left_in: Entity<InputState>,
    tree_right_in: Entity<InputState>,
    epub_jump_in: Entity<InputState>,
    epub_quote_in: Entity<InputState>,
    srs_search_in: Entity<InputState>,
    moc_title_in: Entity<InputState>,
    moc_tag_in: Entity<InputState>,
    commit_in: Entity<InputState>,
    graph_bounds: Rc<Cell<Bounds<Pixels>>>,
    graph_hits: Vec<(f32, f32, String)>,
    collapsed: HashSet<&'static str>,
    table_edit: Option<TableEdit>,
}

fn make_input(
    bindings: &mut Vec<(Entity<InputState>, Getter)>,
    window: &mut Window,
    cx: &mut Context<NotaWorkspaceView>,
    placeholder: &'static str,
    getter: Getter,
    to_msg: fn(String) -> Message,
    init: String,
) -> Entity<InputState> {
    let st = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(init)
    });
    cx.subscribe_in(
        &st,
        window,
        move |this: &mut NotaWorkspaceView, state, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::Change) {
                let v = state.read(cx).value().to_string();
                this.dispatch(to_msg(v), window, cx);
            }
        },
    )
    .detach();
    bindings.push((st.clone(), getter));
    st
}

impl NotaWorkspaceView {
    pub fn new(app: NotaApp, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_text = app.current_tab().map(|t| t.text()).unwrap_or_default();
        let editor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Type Markdown or symbolic logic (e.g. ->, <->, !->, |-, ===)...")
                .default_value(initial_text)
        });
        cx.subscribe_in(
            &editor,
            window,
            |this: &mut NotaWorkspaceView, _state, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.on_editor_change(window, cx);
                }
            },
        )
        .detach();

        cx.observe(&editor, |_, _, cx| cx.notify()).detach();

        let mut b = Vec::new();
        let filter_in = make_input(&mut b, window, cx, "Filter notes / search...", |a| a.filter_query.clone(), Message::FilterQueryChanged, app.filter_query.clone());
        let new_note_in = make_input(&mut b, window, cx, "New note title...", |a| a.new_note_title.clone(), Message::NewNoteTitleChanged, String::new());
        let rename_in = make_input(&mut b, window, cx, "Note title...", |a| a.rename_input.clone(), Message::RenameActiveNoteChanged, app.rename_input.clone());
        let symbol_in = make_input(&mut b, window, cx, "Search symbols/triggers (e.g. xor, :tf)...", |a| a.symbol_palette_query.clone(), Message::SymbolPaletteQueryChanged, String::new());
        let tree_problem_in = make_input(&mut b, window, cx, "Formula or argument (e.g. p -> q, q -> r ∴ p -> r)...", |a| a.tree_session.problem_input.clone(), Message::TreeProblemInputChanged, app.tree_session.problem_input.clone());
        let tree_left_in = make_input(&mut b, window, cx, "Left / stacked formula(s), comma-separated...", |a| a.tree_session.user_left_input.clone(), Message::TreeUserLeftInputChanged, String::new());
        let tree_right_in = make_input(&mut b, window, cx, "Right branch formula(s) (β)...", |a| a.tree_session.user_right_input.clone(), Message::TreeUserRightInputChanged, String::new());
        let epub_jump_in = make_input(&mut b, window, cx, "Page", |a| a.epub_jump_input.clone(), Message::EpubJumpPageInputChanged, app.epub_jump_input.clone());
        let epub_quote_in = make_input(&mut b, window, cx, "Quote passage to cite with page link...", |a| a.epub_quote_input.clone(), Message::EpubQuoteInputChanged, String::new());
        let srs_search_in = make_input(&mut b, window, cx, "Search cards / deck / note...", |a| a.srs_browser_search.clone(), Message::SrsBrowserSearchChanged, String::new());
        let moc_title_in = make_input(&mut b, window, cx, "MOC title...", |a| a.moc_title_input.clone(), Message::MocTitleInputChanged, app.moc_title_input.clone());
        let moc_tag_in = make_input(&mut b, window, cx, "Tag/keyword filter (optional)...", |a| a.moc_tag_filter.clone(), Message::MocTagFilterChanged, String::new());
        let commit_in = make_input(&mut b, window, cx, "Commit message...", |a| a.commit_message_input.clone(), Message::CommitMessageChanged, String::new());

        Self {
            app,
            editor,
            bindings: b,
            filter_in,
            new_note_in,
            rename_in,
            symbol_in,
            tree_problem_in,
            tree_left_in,
            tree_right_in,
            epub_jump_in,
            epub_quote_in,
            srs_search_in,
            moc_title_in,
            moc_tag_in,
            commit_in,
            graph_bounds: Rc::new(Cell::new(Bounds::default())),
            graph_hits: Vec::new(),
            collapsed: HashSet::new(),
            table_edit: None,
        }
    }

    /// Applies a `Message` to the controller, then pushes any programmatic state back into widgets.
    pub fn dispatch(&mut self, msg: Message, window: &mut Window, cx: &mut Context<Self>) {
        let theme_toggle = matches!(msg, Message::ToggleTheme);
        self.app.update(msg);
        if theme_toggle {
            let mode = if self.app.dark_mode {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            Theme::change(mode, None, cx);
        }
        self.sync_inputs(window, cx);
        cx.notify();
    }

    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (st, getter) in self.bindings.clone() {
            let want = getter(&self.app);
            let cur = st.read(cx).value().to_string();
            if cur != want {
                st.update(cx, |s, cx| s.set_value(want, window, cx));
            }
        }
        let (want_text, want_cursor) = match self.app.current_tab() {
            Some(t) => (t.text(), t.content.cursor_byte_offset()),
            None => (String::new(), 0),
        };
        let cur = self.editor.read(cx).value().to_string();
        if cur != want_text {
            self.editor.update(cx, |s, cx| {
                s.set_value(want_text, window, cx);
                let pos = s.text().offset_to_position(want_cursor);
                s.set_cursor_position(pos, window, cx);
            });
        }
    }

    fn on_editor_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, cursor) = {
            let s = self.editor.read(cx);
            (s.value().to_string(), s.cursor())
        };
        let rules = self.app.typing_rules.clone();
        let tt = self.app.typing_transformer_enabled;
        let mut replace = None;
        if let Some(tab) = self.app.current_tab_mut() {
            replace = tab.sync_from_gpui_editor(&text, cursor, &rules, tt);
            if replace.is_some() {
                if let Some(m) = tab.last_matched_rule.clone() {
                    self.app.status_banner = format!("Typing Transformer: {}", m);
                }
            }
        }
        if let Some((new_text, cur)) = replace {
            self.editor.update(cx, |s, cx| {
                s.set_value(new_text, window, cx);
                let pos = s.text().offset_to_position(cur);
                s.set_cursor_position(pos, window, cx);
            });
        }
        cx.notify();
    }

    /// Detects the Markdown table under the source cursor and keeps the grid editor in sync.
    fn ensure_table_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source_visible = matches!(
            self.app.center_mode,
            CenterPaneMode::SourceOnly
                | CenterPaneMode::SplitSourceAndPreview
                | CenterPaneMode::SplitNoteAndEpub
        );
        if !source_visible || self.app.current_tab().is_none() {
            self.table_edit = None;
            return;
        }
        let (n_lines, cur_line) = {
            let st = self.editor.read(cx);
            let rope = st.text();
            (rope.lines_len(), rope.offset_to_point(st.cursor()).row)
        };
        let line_at = |i: usize, cx: &Context<Self>| -> String {
            self.editor.read(cx).text().slice_line(i).to_string()
        };
        if cur_line >= n_lines || !is_table_row(&line_at(cur_line, cx)) {
            self.table_edit = None;
            return;
        }
        let mut s = cur_line;
        while s > 0 && is_table_row(&line_at(s - 1, cx)) {
            s -= 1;
        }
        let mut e = cur_line;
        while e + 1 < n_lines && is_table_row(&line_at(e + 1, cx)) {
            e += 1;
        }
        if e < s + 1 || !is_table_sep(&line_at(s + 1, cx)) {
            self.table_edit = None;
            return;
        }
        let lines: Vec<String> = (s..=e).map(|i| line_at(i, cx)).collect();
        let model = parse_table_model(&lines);
        let same = self
            .table_edit
            .as_ref()
            .is_some_and(|t| t.start == s && t.end == e && t.model == model);
        if !same {
            self.table_edit = Some(self.build_table_edit(s, e, model, window, cx));
        }
    }

    fn build_table_edit(
        &mut self,
        start: usize,
        end: usize,
        model: Vec<Vec<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TableEdit {
        let mut cells = Vec::new();
        for (r, row) in model.iter().enumerate() {
            let mut cell_row = Vec::new();
            for (c, val) in row.iter().enumerate() {
                let v = val.clone();
                let st = cx.new(|cx| InputState::new(window, cx).default_value(v));
                cx.subscribe_in(
                    &st,
                    window,
                    move |this: &mut NotaWorkspaceView, _s, ev: &InputEvent, window, cx| match ev {
                        InputEvent::Change => this.on_table_cell_change(r, c, window, cx),
                        InputEvent::PressEnter { .. } => this.table_focus(r + 1, c, window, cx),
                        _ => {}
                    },
                )
                .detach();
                cell_row.push(st);
            }
            cells.push(cell_row);
        }
        TableEdit {
            start,
            end,
            model,
            cells,
        }
    }

    fn table_values(&self, cx: &Context<Self>) -> Option<Vec<Vec<String>>> {
        let t = self.table_edit.as_ref()?;
        Some(
            t.cells
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|e| e.read(cx).value().to_string().trim().to_string())
                        .collect()
                })
                .collect(),
        )
    }

    fn on_table_cell_change(&mut self, r: usize, c: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(rows) = self.table_values(cx) {
            self.apply_table_model(rows, Some((r, c)), window, cx);
        }
    }

    fn table_focus(&mut self, r: usize, c: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.table_edit.as_ref() else {
            return;
        };
        if r >= t.cells.len() {
            // Enter on the last row appends a new row (like Obsidian).
            if let Some(mut rows) = self.table_values(cx) {
                let n = rows[0].len();
                rows.push(vec![String::new(); n]);
                let target = rows.len() - 1;
                self.apply_table_model(rows, Some((target, c)), window, cx);
            }
            return;
        }
        if let Some(cell) = t.cells.get(r).and_then(|row| row.get(c)).cloned() {
            cell.update(cx, |s, cx| s.focus(window, cx));
        }
    }

    fn table_op(&mut self, op: TableOp, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut rows) = self.table_values(cx) else {
            return;
        };
        let (mut fr, mut fc) = (rows.len() - 1, 0usize);
        if let Some(t) = self.table_edit.as_ref() {
            'find: for (r, row) in t.cells.iter().enumerate() {
                for (c, cell) in row.iter().enumerate() {
                    if cell.read(cx).focus_handle(cx).is_focused(window) {
                        fr = r;
                        fc = c;
                        break 'find;
                    }
                }
            }
        }
        let n = rows[0].len();
        match op {
            TableOp::AddRowBelow => {
                rows.insert(fr + 1, vec![String::new(); n]);
                fr += 1;
            }
            TableOp::AddColRight => {
                for r in rows.iter_mut() {
                    r.insert(fc + 1, String::new());
                }
                fc += 1;
            }
            TableOp::DeleteRow => {
                if fr >= 1 {
                    rows.remove(fr);
                    fr = fr.min(rows.len() - 1);
                }
            }
            TableOp::DeleteCol => {
                if n > 1 {
                    for r in rows.iter_mut() {
                        r.remove(fc);
                    }
                    fc = fc.min(n - 2);
                }
            }
            TableOp::Align => {}
            TableOp::AutoFixTruthTable => {
                if !rows.is_empty() {
                    let headers = rows[0].clone();
                    let body = rows[1..].to_vec();
                    let true_sym = self.app.config.logic.true_symbol.clone();
                    let false_sym = self.app.config.logic.false_symbol.clone();
                    if let Ok(fixed_md) =
                        auto_fix_markdown_truth_table(&headers, &body, &true_sym, &false_sym)
                    {
                        let lines: Vec<String> = fixed_md.lines().map(|l| l.to_string()).collect();
                        if lines.len() >= 2 {
                            rows = parse_table_model(&lines);
                            fr = 0;
                            fc = 0;
                            self.app.status_banner =
                                "Auto-fixed Truth Table in Source Table Editor".to_string();
                        }
                    }
                }
            }
        }
        self.apply_table_model(rows, Some((fr, fc)), window, cx);
    }

    /// Writes the grid model back into the note as an aligned Markdown table.
    fn apply_table_model(
        &mut self,
        mut rows: Vec<Vec<String>>,
        focus: Option<(usize, usize)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((s, e)) = self.table_edit.as_ref().map(|t| (t.start, t.end)) else {
            return;
        };
        let ncols = rows.first().map(|r| r.len()).unwrap_or(0);
        if ncols == 0 {
            return;
        }
        for r in rows.iter_mut() {
            r.resize(ncols, String::new());
            for c in r.iter_mut() {
                *c = c.trim().to_string();
            }
        }
        let esc = |row: &Vec<String>| -> Vec<String> {
            row.iter().map(|c| c.replace('|', "\\|")).collect()
        };
        let headers = esc(&rows[0]);
        let body: Vec<Vec<String>> = rows[1..].iter().map(esc).collect();
        let table_md = format_aligned_markdown_table(&headers, &body);

        let text = self.editor.read(cx).value().to_string();
        let mut lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            return;
        }
        let new_lines: Vec<&str> = table_md.lines().collect();
        let new_len = new_lines.len();
        let e_clamped = e.min(lines.len() - 1);
        lines.splice(s..=e_clamped, new_lines);
        let new_text = lines.join("\n");
        let cursor_byte: usize = lines[..s.min(lines.len())].iter().map(|l| l.len() + 1).sum();

        let rules = self.app.typing_rules.clone();
        if let Some(tab) = self.app.current_tab_mut() {
            tab.sync_from_gpui_editor(&new_text, cursor_byte, &rules, false);
        }
        self.editor.update(cx, |st, cx| {
            st.set_value(new_text.clone(), window, cx);
            let pos = st.text().offset_to_position(cursor_byte);
            st.set_cursor_position(pos, window, cx);
        });

        let new_end = s + new_len.saturating_sub(1);
        let same_dims = self.table_edit.as_ref().is_some_and(|t| {
            t.cells.len() == rows.len() && t.cells.first().map(|r| r.len()) == Some(ncols)
        });
        if same_dims {
            if let Some(t) = self.table_edit.as_mut() {
                t.model = rows;
                t.end = new_end;
            }
        } else {
            let te = self.build_table_edit(s, new_end, rows, window, cx);
            self.table_edit = Some(te);
        }
        if let Some((r, c)) = focus {
            if let Some(cell) = self
                .table_edit
                .as_ref()
                .and_then(|t| t.cells.get(r).and_then(|row| row.get(c)))
                .cloned()
            {
                cell.update(cx, |s, cx| s.focus(window, cx));
            }
        }
        cx.notify();
    }

    fn table_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = self.table_edit.as_ref()?;
        let border = cx.theme().border;
        let head_bg = cx.theme().table_head;
        let ncols = t.cells.first().map(|r| r.len()).unwrap_or(0);
        let nrows = t.cells.len();

        let headers = t.model.first().cloned().unwrap_or_default();
        let body = if t.model.len() > 1 {
            t.model[1..].to_vec()
        } else {
            Vec::new()
        };
        let lint = lint_markdown_table(&headers, &body, t.start + 1);

        let view = cx.entity();
        let mut ops: Vec<(String, TableOp)> = vec![
            ("Add row below".into(), TableOp::AddRowBelow),
            ("Add column right".into(), TableOp::AddColRight),
            ("Delete row".into(), TableOp::DeleteRow),
            ("Delete column".into(), TableOp::DeleteCol),
            ("Align table".into(), TableOp::Align),
        ];
        if lint.is_truth_table {
            ops.push((
                "✨ Auto-Fix Truth Table".into(),
                TableOp::AutoFixTruthTable,
            ));
        }
        let menu = Button::new("table-menu")
            .label("Table ▾")
            .small()
            .dropdown_menu(move |menu, _w, _cx| {
                let mut menu = menu;
                for (label, op) in ops.iter() {
                    let view = view.clone();
                    let op = *op;
                    menu = menu.item(PopupMenuItem::new(label.clone()).on_click(
                        move |_ev, window, cx| {
                            view.update(cx, |this, cx| this.table_op(op, window, cx));
                        },
                    ));
                }
                menu
            });

        let mut grid = v_flex().border_1().border_color(border).rounded_md();
        for (r, row) in t.cells.iter().enumerate() {
            let mut hr = h_flex().border_b_1().border_color(border);
            if r == 0 {
                hr = hr.bg(head_bg);
            }
            for (c, cell) in row.iter().enumerate() {
                hr = hr.child(
                    div()
                        .flex_1()
                        .min_w(px(110.))
                        .border_r_1()
                        .border_color(border)
                        .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                            if ev.keystroke.key == "tab" {
                                let (nr, nc) = if ev.keystroke.modifiers.shift {
                                    if c > 0 {
                                        (r, c - 1)
                                    } else if r > 0 {
                                        (r - 1, ncols.saturating_sub(1))
                                    } else {
                                        (0, 0)
                                    }
                                } else if c + 1 < ncols {
                                    (r, c + 1)
                                } else {
                                    (r + 1, 0)
                                };
                                this.table_focus(nr, nc, window, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .child(Input::new(cell)),
                );
            }
            grid = grid.child(hr);
        }

        let mut header_bar = h_flex().items_center().gap_2().child(small(format!(
            "📋 Table editor — lines {}–{} ({}×{}) · Tab: next cell · Enter: next row (adds a row at the end)",
            t.start + 1,
            t.end + 1,
            nrows.saturating_sub(1),
            ncols
        )));
        if lint.is_truth_table {
            let badge = if lint.issues.is_empty() {
                format!("✅ Valid Truth Table ({})", lint.atoms.join(" "))
            } else {
                format!("⚠️ {} truth table issue(s)", lint.issues.len())
            };
            header_bar = header_bar.child(small(badge));
            if !lint.issues.is_empty() {
                let view2 = cx.entity();
                header_bar = header_bar.child(
                    Button::new("src-tbl-autofix")
                        .label("✨ Auto-Fix")
                        .small()
                        .on_click(move |_ev, window, cx| {
                            view2.update(cx, |this, cx| {
                                this.table_op(TableOp::AutoFixTruthTable, window, cx)
                            });
                        }),
                );
            }
        }
        header_bar = header_bar.child(div().flex_1()).child(menu);

        Some(
            v_flex()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(border)
                .child(header_bar)
                .child(
                    div()
                        .id("table-grid")
                        .max_h(px(230.))
                        .overflow_scroll()
                        .child(grid),
                )
                .into_any_element(),
        )
    }

    fn drain_links(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let links: Vec<String> = match PENDING_LINKS.lock() {
            Ok(mut g) => std::mem::take(&mut *g),
            Err(_) => Vec::new(),
        };
        for l in links {
            self.dispatch(Message::OpenNote(l), window, cx);
        }
    }
}

// ───────────────────────────── helpers ─────────────────────────────

fn msg_btn(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    msg: Message,
    selected: bool,
    cx: &mut Context<NotaWorkspaceView>,
) -> Button {
    Button::new(id)
        .label(label)
        .small()
        .selected(selected)
        .on_click(cx.listener(move |this, _, window, cx| {
            this.dispatch(msg.clone(), window, cx);
        }))
}

/// A button that opens a dropdown menu of `(label, message, checked)` entries.
fn menu_btn(
    id: &'static str,
    label: impl Into<SharedString>,
    items: Vec<(String, Message, bool)>,
    cx: &mut Context<NotaWorkspaceView>,
) -> AnyElement {
    let view = cx.entity();
    let label: SharedString = format!("{} ▾", label.into()).into();
    Button::new(id)
        .label(label)
        .small()
        .dropdown_menu(move |menu, _window, _cx| {
            let mut menu = menu;
            for (text, msg, checked) in items.iter() {
                let view = view.clone();
                let msg = msg.clone();
                menu = menu.item(
                    PopupMenuItem::new(text.clone())
                        .checked(*checked)
                        .on_click(move |_ev, window, cx| {
                            let msg = msg.clone();
                            view.update(cx, |this, cx| this.dispatch(msg, window, cx));
                        }),
                );
            }
            menu
        })
        .into_any_element()
}

fn mi(label: impl Into<String>, msg: Message) -> (String, Message, bool) {
    (label.into(), msg, false)
}

fn mc(label: impl Into<String>, msg: Message, checked: bool) -> (String, Message, bool) {
    (label.into(), msg, checked)
}

fn heading(s: impl Into<SharedString>) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .child(s.into())
}

fn small(s: impl Into<SharedString>) -> Div {
    div().text_xs().child(s.into())
}

fn scroll_area(id: &'static str) -> Stateful<Div> {
    div().id(id).flex_1().min_h_0().overflow_y_scroll()
}

fn encode_link(s: &str) -> String {
    s.replace(' ', "%20").replace('(', "%28").replace(')', "%29")
}

fn decode_link(s: &str) -> String {
    s.replace("%20", " ").replace("%28", "(").replace("%29", ")")
}

/// Converts `[[Target|Alias]]` to clickable Markdown links and protects logic operators
/// (`<->`, `<-`, `~`) from being parsed as HTML / strikethrough.
fn md_preprocess(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find("[[") {
        out.push_str(&rest[..a]);
        let after = &rest[a + 2..];
        if let Some(b) = after.find("]]") {
            let inner = &after[..b];
            let (target, alias) = inner.split_once('|').unwrap_or((inner, inner));
            out.push_str(&format!(
                "[{}](nota://{})",
                alias.trim(),
                encode_link(target.trim())
            ));
            rest = &after[b + 2..];
        } else {
            out.push_str("[[");
            rest = after;
        }
    }
    out.push_str(rest);
    out.replace('<', "\\<").replace('~', "\\~")
}

fn md_element(seg: usize, text: &str) -> AnyElement {
    TextView::markdown(("note-md", seg), md_preprocess(text))
        .selectable(true)
        .on_link_click(|url, _ev, window, _cx| {
            let u = url.to_string();
            if let Some(t) = u.strip_prefix("nota://") {
                let mut target = decode_link(t);
                if !target.to_ascii_lowercase().contains(".epub") {
                    if let Some((p, _)) = target.split_once('#') {
                        target = p.to_string();
                    }
                }
                if let Ok(mut g) = PENDING_LINKS.lock() {
                    g.push(target);
                }
                window.refresh();
            }
        })
        .into_any_element()
}

// ───────────────────────── truth tree diagram ─────────────────────────

enum TreeEnd {
    Branch(usize, usize),
    Closed(usize, usize),
    Open(Option<String>),
}

fn tree_block(
    tree: &TruthTree,
    start: usize,
    clickable: bool,
    selected: Option<usize>,
    cx: &mut Context<NotaWorkspaceView>,
) -> (AnyElement, f32) {
    let mut lines: Vec<(usize, String)> = Vec::new();
    let mut cur = start;
    let end = loop {
        let n = &tree.nodes[cur];
        let origin = match &n.origin {
            NodeOrigin::Premise => "[P]".to_string(),
            NodeOrigin::NegatedConclusion => "[¬C]".to_string(),
            NodeOrigin::InitialFormula => String::new(),
            NodeOrigin::Derived {
                from_node,
                rule_name,
            } => format!("({}: {})", from_node + 1, rule_name),
        };
        let check = if n.checked && !n.formula.is_literal() {
            " ✓"
        } else {
            ""
        };
        lines.push((n.id, format!("{}. {} {}{}", n.id + 1, n.formula, origin, check)));
        match &n.continuation {
            NodeContinuation::Stacked(nx) => cur = *nx,
            NodeContinuation::Branched { left, right } => break TreeEnd::Branch(*left, *right),
            NodeContinuation::ClosedLeaf { contradicts: (a, b) } => break TreeEnd::Closed(*a, *b),
            NodeContinuation::OpenLeaf {
                completed,
                valuation,
            } => {
                let badge = if *completed {
                    let v = valuation
                        .iter()
                        .map(|(k, &val)| format!("{}={}", k, if val { "D" } else { "Y" }))
                        .collect::<Vec<_>>()
                        .join(", ");
                    Some(format!("o ({})", v))
                } else {
                    Some("…".to_string())
                };
                break TreeEnd::Open(badge);
            }
        }
    };

    let (badge, badge_color): (Option<String>, u32) = match &end {
        TreeEnd::Closed(a, b) => (Some(format!("× ({}, {})", a + 1, b + 1)), NORD_RED),
        TreeEnd::Open(b) => (b.clone(), NORD_GREEN),
        TreeEnd::Branch(..) => (None, NORD_TEXT),
    };

    let mut box_w = lines
        .iter()
        .map(|(_, s)| s.chars().count() as f32 * 8.2 + 24.0)
        .fold(160.0_f32, f32::max);
    if let Some(b) = &badge {
        box_w = box_w.max(b.chars().count() as f32 * 8.2 + 24.0);
    }

    let mut bx = v_flex()
        .w(px(box_w))
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgb(NORD_NODE))
        .border_1()
        .border_color(rgb(NORD_BORDER))
        .font_family("monospace")
        .text_sm()
        .text_color(rgb(NORD_TEXT));
    for (id, text) in lines {
        let row = if clickable {
            let is_sel = selected == Some(id);
            div()
                .id(("tree-line", id))
                .cursor_pointer()
                .when(is_sel, |d| d.bg(rgb(NORD_NODE_SEL)))
                .child(text)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.dispatch(Message::SelectTreeNode(id), window, cx);
                }))
                .into_any_element()
        } else {
            div().child(text).into_any_element()
        };
        bx = bx.child(row);
    }
    if let Some(b) = badge {
        bx = bx.child(
            div()
                .w_full()
                .text_center()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(badge_color))
                .child(b),
        );
    }

    match end {
        TreeEnd::Branch(l, r) => {
            let (l_el, lw) = tree_block(tree, l, clickable, selected, cx);
            let (r_el, rw) = tree_block(tree, r, clickable, selected, cx);
            let gap = 36.0_f32;
            let ch = lw + rw + gap;
            let w = box_w.max(ch);
            let left_cx = w / 2.0 - ch / 2.0 + lw / 2.0;
            let right_cx = w / 2.0 + ch / 2.0 - rw / 2.0;
            let conn = canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    for tx in [left_cx, right_cx] {
                        let mut pb = PathBuilder::stroke(px(2.0));
                        pb.move_to(point(bounds.origin.x + px(w / 2.0), bounds.origin.y));
                        pb.line_to(point(
                            bounds.origin.x + px(tx),
                            bounds.origin.y + bounds.size.height,
                        ));
                        if let Ok(p) = pb.build() {
                            window.paint_path(p, rgb(NORD_PURPLE));
                        }
                    }
                },
            )
            .w(px(w))
            .h(px(28.0));
            let el = v_flex()
                .items_center()
                .w(px(w))
                .child(bx)
                .child(conn)
                .child(
                    h_flex()
                        .items_start()
                        .child(div().w(px(lw)).child(l_el))
                        .child(div().w(px(gap)))
                        .child(div().w(px(rw)).child(r_el)),
                )
                .into_any_element();
            (el, w)
        }
        _ => (
            v_flex()
                .items_center()
                .w(px(box_w))
                .child(bx)
                .into_any_element(),
            box_w,
        ),
    }
}

fn tree_diagram(
    tree: &TruthTree,
    clickable: bool,
    selected: Option<usize>,
    id: &'static str,
    cx: &mut Context<NotaWorkspaceView>,
) -> AnyElement {
    if tree.nodes.is_empty() {
        return div().child("(empty tree)").into_any_element();
    }
    let (el, w) = tree_block(tree, 0, clickable, selected, cx);
    v_flex()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(rgb(NORD_BG))
        .border_1()
        .border_color(rgb(NORD_BORDER))
        .child(
            div()
                .text_color(rgb(NORD_CYAN))
                .font_weight(FontWeight::BOLD)
                .child(format!("🌳 {}", tree.goal_description)),
        )
        .child(
            div()
                .id(id)
                .w_full()
                .overflow_x_scroll()
                .child(div().min_w(px(w)).flex().justify_center().child(el)),
        )
        .into_any_element()
}

fn tree_from_dsl(
    content: &str,
    id: &'static str,
    cx: &mut Context<NotaWorkspaceView>,
) -> AnyElement {
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix("problem:") {
            let goal = rest.trim();
            let built = if let Ok(MetalogicStmt::Entailment {
                premises,
                conclusion,
                ..
            }) = parse_metalogic_stmt(goal)
            {
                Some(TruthTree::from_entailment(&premises, &conclusion))
            } else if let Ok(f) = parse_formula(goal) {
                Some(TruthTree::from_formulas(&[f]))
            } else {
                None
            };
            if let Some(mut tree) = built {
                let _ = tree.solve_alpha_first(100);
                return tree_diagram(&tree, false, None, id, cx);
            }
        }
    }
    div()
        .p_2()
        .rounded_md()
        .bg(rgb(NORD_BG))
        .text_color(rgb(NORD_TEXT))
        .font_family("monospace")
        .text_sm()
        .child(content.to_string())
        .into_any_element()
}

// ───────────────────────────── rendering ─────────────────────────────

impl Render for NotaWorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drain_links(window, cx);
        self.ensure_table_editor(window, cx);
        let bg = cx.theme().background;
        let fg = cx.theme().foreground;
        let show_right = self.app.show_right_sidebar;
        let show_left = self.app.show_left_sidebar;

        let top = self.top_toolbar(cx);
        let left = self.left_sidebar(cx);
        let center = self.center(cx);
        let right = self.right_sidebar(cx);
        let bottom = self.status_bar(cx);

        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .p_2()
            .gap_2()
            .child(top)
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("nota-main")
                        .child(
                            resizable_panel()
                                .size(px(270.))
                                .visible(show_left)
                                .child(left),
                        )
                        .child(resizable_panel().child(center))
                        .child(
                            resizable_panel()
                                .size(px(300.))
                                .visible(show_right)
                                .child(right),
                        ),
                ),
            )
            .child(bottom)
    }
}

impl NotaWorkspaceView {
    fn top_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = self.app.center_mode;
        let a = &self.app;
        let quick = vec![
            msg_btn("m-src", "Source", Message::SetCenterMode(CenterPaneMode::SourceOnly), m == CenterPaneMode::SourceOnly, cx),
            msg_btn("m-prev", "Preview", Message::SetCenterMode(CenterPaneMode::RenderedPreview), m == CenterPaneMode::RenderedPreview, cx),
            msg_btn("m-split", "Split", Message::SetCenterMode(CenterPaneMode::SplitSourceAndPreview), m == CenterPaneMode::SplitSourceAndPreview, cx),
        ];
        let workbench = menu_btn(
            "m-bench",
            "Workbench",
            vec![
                mc("EPUB Split-View", Message::SetCenterMode(CenterPaneMode::SplitNoteAndEpub), m == CenterPaneMode::SplitNoteAndEpub),
                mc("Truth Tree Workbench", Message::SetCenterMode(CenterPaneMode::TruthTreeWorkbench), m == CenterPaneMode::TruthTreeWorkbench),
                mc(format!("Flashcard Review ({})", a.srs_cards.len()), Message::SetCenterMode(CenterPaneMode::FlashcardReview), m == CenterPaneMode::FlashcardReview),
                mc("Graph & MOC", Message::SetCenterMode(CenterPaneMode::GraphAndMocView), m == CenterPaneMode::GraphAndMocView),
            ],
            cx,
        );
        let file = menu_btn(
            "m-file",
            "File",
            vec![
                mi("Save", Message::SaveActiveNote),
                mi("Undo", Message::UndoEdit),
                mi("Redo", Message::RedoEdit),
            ],
            cx,
        );
        let export = menu_btn(
            "m-export",
            "Export",
            vec![
                mi("Export note as HTML", Message::ExportActiveNoteHtml),
                mi("Export note as Typst", Message::ExportActiveNoteTypst),
                mi("Build static site", Message::BuildStaticSite),
            ],
            cx,
        );
        let view = menu_btn(
            "m-view",
            "View",
            vec![
                mc("Explorer sidebar", Message::ToggleLeftSidebar, a.show_left_sidebar),
                mc("Inspector sidebar", Message::ToggleRightSidebar, a.show_right_sidebar),
                mc("Dark theme (Nord)", Message::ToggleTheme, a.dark_mode),
            ],
            cx,
        );
        let left_toggle = msg_btn(
            "toggle-left",
            if a.show_left_sidebar { "◀" } else { "▶" },
            Message::ToggleLeftSidebar,
            a.show_left_sidebar,
            cx,
        );
        let right_toggle = msg_btn(
            "toggle-right",
            if a.show_right_sidebar { "▶" } else { "◀" },
            Message::ToggleRightSidebar,
            a.show_right_sidebar,
            cx,
        );
        h_flex()
            .gap_3()
            .items_center()
            .child(left_toggle)
            .child(div().text_xl().font_weight(FontWeight::BOLD).child("NOTA"))
            .child(h_flex().gap_1().children(quick).child(workbench))
            .child(div().flex_1())
            .child(h_flex().gap_1().child(file).child(export).child(view).child(right_toggle))
            .into_any_element()
    }

    fn left_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let lt = self.app.left_tab;
        let nav = h_flex().gap_1().children(vec![
            msg_btn("l-notes", "Notes", Message::SelectLeftTab(LeftSidebarTab::Explorer), lt == LeftSidebarTab::Explorer, cx),
            msg_btn("l-search", "Search", Message::SelectLeftTab(LeftSidebarTab::Search), lt == LeftSidebarTab::Search, cx),
            msg_btn("l-tasks", "Tasks", Message::SelectLeftTab(LeftSidebarTab::Tasks), lt == LeftSidebarTab::Tasks, cx),
            msg_btn("l-links", "Links", Message::SelectLeftTab(LeftSidebarTab::LinkHealth), lt == LeftSidebarTab::LinkHealth, cx),
            msg_btn("l-books", "EPUBs", Message::SelectLeftTab(LeftSidebarTab::Books), lt == LeftSidebarTab::Books, cx),
        ]);

        let q = normalize_lookup_key(&self.app.filter_query);
        let mut items: Vec<AnyElement> = Vec::new();
        match lt {
            LeftSidebarTab::Explorer => {
                items.push(
                    h_flex()
                        .gap_1()
                        .child(div().flex_1().child(Input::new(&self.new_note_in)))
                        .child(msg_btn("new-note", "+", Message::CreateNewNoteFromInput, false, cx))
                        .into_any_element(),
                );
                for (i, note) in self.app.index.notes.values().enumerate() {
                    if !q.is_empty() && !normalize_lookup_key(&note.file_stem).contains(&q) {
                        continue;
                    }
                    let prefix = if note.is_moc { "🗺️ " } else { "📄 " };
                    items.push(
                        msg_btn(("note", i), format!("{}{}", prefix, note.file_stem), Message::OpenNote(note.rel_path.clone()), false, cx)
                            .w_full()
                            .into_any_element(),
                    );
                }
            }
            LeftSidebarTab::Search => {
                for (i, note) in self.app.index.notes.values().enumerate() {
                    let mt = normalize_lookup_key(&note.file_stem).contains(&q);
                    let mg = note.frontmatter.tags.iter().any(|t| normalize_lookup_key(t).contains(&q));
                    if !q.is_empty() && (mt || mg) {
                        items.push(
                            msg_btn(("sr", i), format!("{} [{}]", note.file_stem, note.frontmatter.tags.join(", ")), Message::OpenNote(note.rel_path.clone()), false, cx)
                                .w_full()
                                .into_any_element(),
                        );
                    }
                }
            }
            LeftSidebarTab::Tasks => {
                let mut i = 0usize;
                for note in self.app.index.notes.values() {
                    for task in &note.body.tasks {
                        if !q.is_empty() && !normalize_lookup_key(&task.text).contains(&q) {
                            continue;
                        }
                        let bx = if task.checked { "[x]" } else { "[ ]" };
                        items.push(
                            msg_btn(("tk", i), format!("{} {} ({})", bx, task.text, note.file_stem), Message::OpenNote(note.rel_path.clone()), false, cx)
                                .w_full()
                                .into_any_element(),
                        );
                        i += 1;
                    }
                }
            }
            LeftSidebarTab::LinkHealth => {
                items.push(heading(format!("Unresolved Targets ({}):", self.app.index.unresolved_targets.len())).into_any_element());
                for (i, (target, locs)) in self.app.index.unresolved_targets.iter().enumerate() {
                    if !q.is_empty() && !normalize_lookup_key(target).contains(&q) {
                        continue;
                    }
                    items.push(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(small(format!("[[{}]] ({})", target, locs.len())).flex_1())
                            .child(msg_btn(("mk", i), "+ Create", Message::CreateMissingNote(target.clone()), false, cx))
                            .into_any_element(),
                    );
                }
            }
            LeftSidebarTab::Books => {
                items.push(heading("Local EPUB Library:").into_any_element());
                if self.app.available_epubs.is_empty() {
                    items.push(small("Place .epub files in books/ or content/assets/books/ to open in Split View.").into_any_element());
                }
                for (i, p) in self.app.available_epubs.iter().enumerate() {
                    let f = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    items.push(
                        msg_btn(("bk", i), format!("📖 {}", f), Message::OpenEpubFile(p.clone()), false, cx)
                            .w_full()
                            .into_any_element(),
                    );
                }
            }
        }

        v_flex()
            .size_full()
            .gap_2()
            .pr_2()
            .child(nav)
            .child(Input::new(&self.filter_in))
            .child(scroll_area("left-list").child(v_flex().gap_1().children(items)))
            .into_any_element()
    }

    fn center(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut tabs: Vec<AnyElement> = Vec::new();
        for (idx, tab) in self.app.tabs.iter().enumerate() {
            let dirty = if tab.is_dirty { "*" } else { "" };
            tabs.push(
                h_flex()
                    .gap_0p5()
                    .child(msg_btn(("tab", idx), format!("{}{}", tab.title, dirty), Message::SwitchTab(idx), idx == self.app.active_tab, cx))
                    .child(msg_btn(("tabx", idx), "×", Message::CloseTab(idx), false, cx))
                    .into_any_element(),
            );
        }
        let tab_bar = div()
            .id("tab-bar")
            .w_full()
            .overflow_x_scroll()
            .child(h_flex().gap_1().children(tabs));

        let pane = |e: AnyElement| div().flex_1().min_w_0().h_full().child(e).into_any_element();
        let body: AnyElement = match self.app.center_mode {
            CenterPaneMode::SourceOnly => self.source_pane(cx),
            CenterPaneMode::RenderedPreview => self.preview_pane(cx),
            CenterPaneMode::SplitSourceAndPreview => {
                let (a, b) = (self.source_pane(cx), self.preview_pane(cx));
                h_flex().size_full().gap_2().child(pane(a)).child(pane(b)).into_any_element()
            }
            CenterPaneMode::SplitNoteAndEpub => {
                let (a, b) = (self.source_pane(cx), self.epub_pane(cx));
                h_flex().size_full().gap_2().child(pane(a)).child(pane(b)).into_any_element()
            }
            CenterPaneMode::TruthTreeWorkbench => self.tree_workbench(cx),
            CenterPaneMode::FlashcardReview => self.srs_pane(cx),
            CenterPaneMode::GraphAndMocView => self.graph_pane(cx),
        };
        v_flex()
            .size_full()
            .gap_2()
            .child(tab_bar)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    fn source_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.app.current_tab() else {
            return div().child("No note open").into_any_element();
        };
        let tools = menu_btn(
            "src-tools",
            "Tools",
            vec![
                mi("Normalize ASCII → Unicode", Message::NormalizeOperatorsInActiveNote),
                mi("Convert ASCII Trees", Message::ConvertAsciiTreesInActiveNote),
                mi("Convert HTML <table> → Markdown", Message::ConvertHtmlTablesInActiveNote),
                mi("Clean Metadata", Message::CleanMetadataInActiveNote),
            ],
            cx,
        );
        let syms = [
            "¬", "∧", "∨", "→", "↔", "↛", "↚", "↑", "↓", "⊕", "⊢", "⊨", "≡", "≢", "∴", "∀", "∃",
        ];
        let sym_menu = menu_btn(
            "src-syms",
            "Symbols",
            syms.iter().map(|s| mi(*s, Message::InsertSymbol(s.to_string()))).collect(),
            cx,
        );
        let rename_bar = h_flex()
            .gap_1()
            .items_center()
            .child(div().w(px(200.)).child(Input::new(&self.rename_in)))
            .child(msg_btn("rename", "Rename", Message::ExecuteRenameActiveNote, false, cx))
            .child(div().flex_1())
            .child(sym_menu)
            .child(tools);
        let sym_bar = div();

        let mut col = v_flex().size_full().gap_1().child(rename_bar).child(sym_bar);

        if let Some((_, query)) = tab.active_wikilink_query() {
            let qn = normalize_lookup_key(&query);
            let mut sug: Vec<AnyElement> = vec![small(format!("Link [[{}]]:", query)).into_any_element()];
            let mut count = 0;
            for (i, n) in self.app.index.notes.values().enumerate() {
                if qn.is_empty() || normalize_lookup_key(&n.file_stem).contains(&qn) {
                    sug.push(msg_btn(("wl", i), n.file_stem.clone(), Message::CompleteWikilink(n.file_stem.clone()), false, cx).into_any_element());
                    count += 1;
                    if count >= 5 {
                        break;
                    }
                }
            }
            if !query.trim().is_empty() {
                sug.push(msg_btn("wl-new", format!("+ Create '{}'", query.trim()), Message::CompleteWikilink(query.trim().to_string()), false, cx).into_any_element());
            }
            col = col.child(h_flex().gap_1().items_center().children(sug));
        }

        if let Some(panel) = self.table_panel(cx) {
            col = col.child(panel);
        }

        col.child(
            div()
                .flex_1()
                .min_h_0()
                .child(Textarea::new(&self.editor).h(relative(1.0))),
        )
        .into_any_element()
    }

    fn table_block(&self, idx: usize, raw_rows: &[&str], cx: &mut Context<Self>) -> AnyElement {
        let raw = self.app.current_tab().map(|t| t.text()).unwrap_or_default();
        let fm = parse_frontmatter(&raw);
        let parsed = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
        let Some(table) = parsed.tables.get(idx) else {
            return div().font_family("monospace").text_sm().child(raw_rows.join("\n")).into_any_element();
        };
        let lint = lint_markdown_table(&table.headers, &table.rows, table.start_line);
        let border = cx.theme().border;
        let head_bg = cx.theme().table_head;

        let mut bar = h_flex().gap_2().items_center();
        if lint.is_truth_table {
            let status = if lint.issues.is_empty() {
                format!("✅ Valid Truth Table (atoms: {})", lint.atoms.join(" "))
            } else {
                format!("⚠️ Truth Table has {} issue(s) at line {}", lint.issues.len(), lint.start_line)
            };
            bar = bar.child(small(status));
            if !lint.issues.is_empty() {
                bar = bar.child(msg_btn(("fix", idx), "✨ Auto-Fix Truth Table", Message::AutoFixTruthTableInActiveNote(idx), false, cx));
            }
        }
        bar = bar.child(msg_btn(("align", idx), "Align Table", Message::AlignMarkdownTableInActiveNote(idx), false, cx));

        let cell = |s: &str, header: bool| {
            let mut d = div()
                .flex_1()
                .min_w(px(40.))
                .px_2()
                .py_1()
                .border_r_1()
                .border_color(border)
                .text_sm()
                .child(s.to_string());
            match s.trim() {
                "D" if !header => d = d.text_color(rgb(NORD_GREEN)).font_weight(FontWeight::BOLD),
                "Y" if !header => d = d.text_color(rgb(NORD_RED)).font_weight(FontWeight::BOLD),
                _ => {}
            }
            if header {
                d = d.font_weight(FontWeight::BOLD);
            }
            d
        };

        let mut grid = v_flex().border_1().border_color(border).rounded_md().overflow_hidden();
        grid = grid.child(
            h_flex()
                .bg(head_bg)
                .border_b_1()
                .border_color(border)
                .children(table.headers.iter().map(|h| cell(h, true)).collect::<Vec<_>>()),
        );
        for r in &table.rows {
            grid = grid.child(
                h_flex()
                    .border_b_1()
                    .border_color(border)
                    .children(r.iter().map(|c| cell(c, false)).collect::<Vec<_>>()),
            );
        }
        v_flex().gap_1().child(bar).child(grid).into_any_element()
    }

    fn preview_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.app.current_tab() else {
            return div().child("No note open").into_any_element();
        };
        let raw = tab.text();
        let fm = parse_frontmatter(&raw);
        let parsed = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
        let mut col = v_flex().gap_3().p_3();

        if fm.has_frontmatter {
            let title = fm.title.clone().unwrap_or_else(|| tab.title.clone());
            let tags = if fm.tags.is_empty() { "none".to_string() } else { fm.tags.join(", ") };
            col = col.child(
                v_flex()
                    .gap_0p5()
                    .child(div().text_2xl().font_weight(FontWeight::BOLD).child(title))
                    .child(small(format!("Tags: {} | Aliases: {:?}", tags, fm.clean_aliases))),
            );
        }

        if !parsed.wikilinks.is_empty() {
            let mut links: Vec<AnyElement> = vec![small("Links:").into_any_element()];
            for (i, wl) in parsed.wikilinks.iter().take(10).enumerate() {
                let label = wl.alias.as_deref().unwrap_or(&wl.target);
                let target = if let Some(ref h) = wl.heading { format!("{}#{}", wl.target, h) } else { wl.target.clone() };
                let icon = if wl.epub.is_some() { "📖 " } else { "🔗 " };
                links.push(msg_btn(("pl", i), format!("{}{}", icon, label), Message::OpenNote(target), false, cx).into_any_element());
            }
            col = col.child(h_flex().gap_1().items_center().children(links));
        }

        if !parsed.tasks.is_empty() {
            let mut tasks = v_flex().gap_1().child(heading("Tasks:"));
            for (i, t) in parsed.tasks.iter().enumerate() {
                let mark = if t.checked { "☑" } else { "☐" };
                tasks = tasks.child(msg_btn(("task", i), format!("{} {}", mark, t.text), Message::ToggleTaskInActiveNote(t.line), false, cx));
            }
            col = col.child(tasks);
        }

        let body = &raw[fm.body_byte_offset.min(raw.len())..];
        let lines: Vec<&str> = body.lines().collect();
        let mut md = String::new();
        let mut seg = 0usize;
        let mut tbl = 0usize;
        let mut i = 0usize;
        macro_rules! flush {
            () => {
                if !md.trim().is_empty() {
                    col = col.child(md_element(seg, &md));
                    seg += 1;
                }
                md.clear();
            };
        }
        while i < lines.len() {
            let t = lines[i].trim_start();
            if t.starts_with("```tree") || t.starts_with("```tableau") {
                flush!();
                let mut content = String::new();
                i += 1;
                while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                    content.push_str(lines[i]);
                    content.push('\n');
                    i += 1;
                }
                i += 1;
                let ids: [&'static str; 8] = ["pt0", "pt1", "pt2", "pt3", "pt4", "pt5", "pt6", "pt7"];
                col = col.child(tree_from_dsl(&content, ids[seg % 8], cx));
                seg += 1;
                continue;
            }
            if t.starts_with("```") || t.starts_with("~~~") {
                md.push_str(lines[i]);
                md.push('\n');
                i += 1;
                while i < lines.len() && !lines[i].trim_start().starts_with("```") && !lines[i].trim_start().starts_with("~~~") {
                    md.push_str(lines[i]);
                    md.push('\n');
                    i += 1;
                }
                if i < lines.len() {
                    md.push_str(lines[i]);
                    md.push('\n');
                    i += 1;
                }
                continue;
            }
            if t.starts_with('|') {
                flush!();
                let start = i;
                while i < lines.len() && lines[i].trim_start().starts_with('|') {
                    i += 1;
                }
                col = col.child(self.table_block(tbl, &lines[start..i], cx));
                tbl += 1;
                continue;
            }
            md.push_str(lines[i]);
            md.push('\n');
            i += 1;
        }
        flush!();
        let _ = seg;

        div().id("preview-scroll").size_full().overflow_y_scroll().child(col).into_any_element()
    }

    fn tree_workbench(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.app.tree_session;
        let insert = menu_btn(
            "t-insert",
            "Insert",
            vec![
                mi("```tree block into note", Message::InsertTreeIntoActiveNote),
                mi("Truth table into note", Message::InsertTruthTableForInput),
                mi("Generate SRS cards into note", Message::GenerateLogicCardsFromProblemInput),
            ],
            cx,
        );
        let top = h_flex()
            .gap_1()
            .items_center()
            .child(div().flex_1().child(Input::new(&self.tree_problem_in)))
            .child(msg_btn("t-start", "Start Tree", Message::LoadTreeFromProblemInput, false, cx))
            .child(msg_btn("t-solve", "Solve All (α-First)", Message::SolveFullTruthTree, false, cx))
            .child(insert);
        let mode = h_flex()
            .gap_2()
            .child(msg_btn("t-mode", if s.practice_mode { "Mode: Practice (Manual Step Verification)" } else { "Mode: Auto-Decompose" }, Message::ToggleTreePracticeMode, s.practice_mode, cx))
            .child(msg_btn("t-dec", "Decompose Selected Node", Message::DecomposeSelectedTreeNode, false, cx));

        let mut col = v_flex().size_full().gap_2().child(top).child(mode);
        if s.practice_mode {
            col = col.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(msg_btn("t-rule", if s.user_branch_mode { "Rule Type: Branching (β)" } else { "Rule Type: Stacking (α)" }, Message::ToggleTreeUserBranchMode, false, cx))
                    .child(div().flex_1().child(Input::new(&self.tree_left_in)))
                    .child(div().flex_1().child(Input::new(&self.tree_right_in))),
            );
        }
        if let Some((ok, ref msg)) = s.last_feedback {
            col = col.child(div().text_sm().child(format!("{}{}", if ok { "✅ " } else { "❌ " }, msg)));
        }

        let mut nodes = v_flex().gap_1().child(heading("Click any undecomposed formula node to decompose:"));
        for node in &s.tree.nodes {
            let sel = s.selected_node == Some(node.id);
            let check = if node.checked { "✓" } else { "○" };
            let hint = match classify_decomposition(&node.formula) {
                DecompositionRule::Literal => "Literal".to_string(),
                DecompositionRule::Alpha { rule_name, .. } => format!("α-Stack ({})", rule_name),
                DecompositionRule::Beta { rule_name, .. } => format!("β-Branch ({})", rule_name),
                DecompositionRule::Gamma { rule_name, .. } => format!("γ-Universal ({})", rule_name),
                DecompositionRule::Delta { rule_name, .. } => format!("δ-Existential ({})", rule_name),
            };
            let suffix = match &node.continuation {
                NodeContinuation::ClosedLeaf { contradicts: (a, b) } => format!("  [× Closed: lines {}, {}]", a + 1, b + 1),
                NodeContinuation::OpenLeaf { completed: true, valuation } => {
                    let v = valuation.iter().map(|(k, &val)| format!("{}={}", k, if val { "D" } else { "Y" })).collect::<Vec<_>>().join(", ");
                    format!("  [o Open: {}]", v)
                }
                _ => String::new(),
            };
            nodes = nodes.child(
                msg_btn(("tnode", node.id), format!("{}. {}   [{}]  ({}){}", node.id + 1, node.formula, check, hint, suffix), Message::SelectTreeNode(node.id), sel, cx).w_full(),
            );
        }

        let diagram = tree_diagram(&s.tree, true, s.selected_node, "wb-tree", cx);
        let dsl = div().font_family("monospace").text_xs().child(s.tree.to_tree_dsl());
        col.child(
            h_flex()
                .flex_1()
                .min_h_0()
                .gap_3()
                .items_start()
                .child(div().id("wb-nodes").w(px(340.)).h_full().overflow_y_scroll().child(nodes))
                .child(
                    div()
                        .id("wb-diagram")
                        .flex_1()
                        .h_full()
                        .overflow_y_scroll()
                        .child(v_flex().gap_2().child(diagram).child(heading("Rendered Truth Tree DSL:")).child(dsl)),
                ),
        )
        .into_any_element()
    }

    fn epub_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(book) = self.app.active_epub.as_ref() else {
            let mut col = v_flex()
                .gap_2()
                .p_3()
                .child(heading("📖 EPUB Split-View Reader"))
                .child(small("Select an EPUB from the left 'EPUBs' tab or click any [[Book.epub#p=42]] link in a note."));
            for (i, p) in self.app.available_epubs.iter().enumerate() {
                let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                col = col.child(msg_btn(("eo", i), format!("Open {}", n), Message::OpenEpubFile(p.clone()), false, cx));
            }
            return col.into_any_element();
        };
        let nav = h_flex()
            .gap_1()
            .items_center()
            .child(small(format!("📖 {} ({})", book.title, book.authors.join(", "))))
            .child(msg_btn("ep-prev", "◀ Prev", Message::EpubPrevPage, false, cx))
            .child(div().w(px(70.)).child(Input::new(&self.epub_jump_in)))
            .child(small(format!("/ {}", book.total_pages())))
            .child(msg_btn("ep-go", "Go", Message::EpubExecuteJumpPage, false, cx))
            .child(msg_btn("ep-next", "Next ▶", Message::EpubNextPage, false, cx))
            .child(msg_btn("ep-link", "Link Page in Note", Message::EpubCopyPageWikilinkToNote, false, cx));
        let quote = h_flex()
            .gap_1()
            .child(div().flex_1().child(Input::new(&self.epub_quote_in)))
            .child(msg_btn("ep-quote", "Quote & Link to Active Note", Message::EpubInsertQuoteCitationToNote, false, cx));
        let mut toc: Vec<AnyElement> = vec![small("TOC:").into_any_element()];
        for ch in book.chapters.iter().take(6) {
            toc.push(msg_btn(("toc", ch.spine_index), format!("{} (p.{})", ch.title, ch.start_page), Message::EpubSelectChapter(ch.spine_index), false, cx).into_any_element());
        }

        let mut content = v_flex().gap_2();
        let mut backlinks = Vec::new();
        for note in self.app.index.notes.values() {
            for wl in &note.body.wikilinks {
                if let Some(ref ep) = wl.epub {
                    if normalize_lookup_key(&ep.book_file) == normalize_lookup_key(&book.file_name)
                        && (ep.page == Some(self.app.epub_current_page) || ep.page.is_none())
                    {
                        backlinks.push((note.rel_path.clone(), wl.line));
                    }
                }
            }
        }
        if !backlinks.is_empty() {
            let mut r: Vec<AnyElement> = vec![small(format!("📝 Cited on p.{} by:", self.app.epub_current_page)).into_any_element()];
            for (i, (p, l)) in backlinks.into_iter().enumerate() {
                r.push(msg_btn(("bl", i), format!("{}:{}", p, l), Message::OpenNote(p), false, cx).into_any_element());
            }
            content = content.child(h_flex().gap_1().items_center().children(r));
        }
        if let Some(cp) = book.resolve_page(self.app.epub_current_page) {
            content = content.child(heading(format!("Chapter: {} — Canonical Page {}", cp.chapter_title, cp.page_number)));
            if let Some(ch) = book.chapters.get(cp.spine_index) {
                let slice: String = ch.plain_text.chars().skip(cp.char_offset_in_chapter).take(1200).collect();
                content = content.child(div().text_sm().child(slice));
            }
        }
        v_flex()
            .size_full()
            .gap_2()
            .child(nav)
            .child(h_flex().gap_1().items_center().children(toc))
            .child(quote)
            .child(scroll_area("epub-scroll").child(content))
            .into_any_element()
    }

    fn srs_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = &self.app;
        let srs_tools = menu_btn(
            "s-tools",
            "Tools",
            vec![
                mc("Sibling burying", Message::ToggleSrsSiblingBurying, a.srs_bury_siblings),
                mi("Stamp ULIDs in note", Message::StampCardIdsInActiveNote),
                mi("Import Obsidian SR comments", Message::ImportObsidianSrComments),
                mi("Refresh queue", Message::RefreshSrsQueue),
            ],
            cx,
        );
        let sub = h_flex().gap_1().children(vec![
            msg_btn("s-q", format!("Review Queue ({})", a.srs_cards.len()), Message::SelectSrsSubTab(SrsSubTab::ReviewQueue), a.srs_sub_tab == SrsSubTab::ReviewQueue, cx).into_any_element(),
            msg_btn("s-b", format!("Card Browser ({})", a.srs_all_cards.len()), Message::SelectSrsSubTab(SrsSubTab::CardBrowser), a.srs_sub_tab == SrsSubTab::CardBrowser, cx).into_any_element(),
            msg_btn("s-s", "Statistics & Forecast", Message::SelectSrsSubTab(SrsSubTab::Statistics), a.srs_sub_tab == SrsSubTab::Statistics, cx).into_any_element(),
            srs_tools,
        ]);

        let body: AnyElement = match a.srs_sub_tab {
            SrsSubTab::ReviewQueue => {
                let Some(card) = a.srs_cards.get(a.srs_current_idx) else {
                    return v_flex().gap_3().child(sub).child(small("No due flashcards in queue. Add `Front::Back` or `{{c1::cloze}}` in any note, or click 'Generate SRS Cards' in the Truth Tree Workbench!")).into_any_element();
                };
                let mut b = v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(small(format!("Deck: {} | Source: {}:{} | ID: {}", card.deck, card.note_path, card.line, card.id)))
                            .child(msg_btn("s-jump", "Jump to Note", Message::OpenNote(card.note_path.clone()), false, cx)),
                    )
                    .child(div().text_lg().child(card.front.clone()));
                if a.srs_answer_revealed {
                    b = b
                        .child(small("─── Answer ───"))
                        .child(div().text_base().child(card.back.clone()))
                        .child(h_flex().gap_2().children(vec![
                            msg_btn("r1", "Again (1)", Message::RateSrsCard(1), false, cx),
                            msg_btn("r2", "Hard (2)", Message::RateSrsCard(2), false, cx),
                            msg_btn("r3", "Good (3)", Message::RateSrsCard(3), false, cx),
                            msg_btn("r4", "Easy (4)", Message::RateSrsCard(4), false, cx),
                        ]));
                } else {
                    b = b.child(msg_btn("s-show", "Show Answer", Message::RevealSrsAnswer, false, cx));
                }
                b.into_any_element()
            }
            SrsSubTab::CardBrowser => {
                let f = &a.srs_browser_filter;
                let fname = match f {
                    CardBrowserFilter::All => "All".to_string(),
                    CardBrowserFilter::Due => "Due".to_string(),
                    CardBrowserFilter::New => "New".to_string(),
                    CardBrowserFilter::Suspended => "Suspended".to_string(),
                    CardBrowserFilter::Leeches => "Leeches".to_string(),
                    CardBrowserFilter::Deck(d) => format!("Deck {}", d),
                };
                let filter_bar = menu_btn(
                    "f-menu",
                    format!("Filter: {}", fname),
                    vec![
                        mc("All", Message::SetSrsBrowserFilter(CardBrowserFilter::All), *f == CardBrowserFilter::All),
                        mc("Due", Message::SetSrsBrowserFilter(CardBrowserFilter::Due), *f == CardBrowserFilter::Due),
                        mc("New", Message::SetSrsBrowserFilter(CardBrowserFilter::New), *f == CardBrowserFilter::New),
                        mc("Suspended", Message::SetSrsBrowserFilter(CardBrowserFilter::Suspended), *f == CardBrowserFilter::Suspended),
                        mc("Leeches (≥8 lapses)", Message::SetSrsBrowserFilter(CardBrowserFilter::Leeches), *f == CardBrowserFilter::Leeches),
                    ],
                    cx,
                );
                let today = Utc::now().format("%Y-%m-%d").to_string();
                let filtered = filter_cards_for_browser(&a.srs_all_cards, &a.srs_schedule, &a.srs_browser_filter, &a.srs_browser_search, &today);
                let mut rows = v_flex().gap_1().child(small("Front | Deck | Due | State | Stability | Difficulty | Reps | Lapses | Note"));
                for (i, card) in filtered.into_iter().enumerate() {
                    let entry = a.srs_schedule.cards.get(&card.id);
                    let due = entry.map(|e| e.due.as_str()).unwrap_or("new");
                    let state = entry.map(|e| format!("{:?}", e.state)).unwrap_or_else(|| "New".to_string());
                    let susp = entry.is_some_and(|e| e.state == CardState::Suspended);
                    let line = format!(
                        "{}  |  {}  |  {}  |  {}  |  S:{:.1} D:{:.1}  |  reps:{} lapses:{}  |  {}:{}",
                        card.front, card.deck, due, state,
                        entry.map(|e| e.stability).unwrap_or(0.0),
                        entry.map(|e| e.difficulty).unwrap_or(0.0),
                        entry.map(|e| e.reps).unwrap_or(0),
                        entry.map(|e| e.lapses).unwrap_or(0),
                        card.note_path, card.line
                    );
                    rows = rows.child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(small(line).flex_1())
                            .child(msg_btn(("cs", i), if susp { "Unsuspend" } else { "Suspend" }, Message::ToggleSuspendSrsCard(card.id.clone()), false, cx))
                            .child(msg_btn(("co", i), "Open", Message::OpenNote(card.note_path.clone()), false, cx)),
                    );
                }
                v_flex()
                    .size_full()
                    .gap_2()
                    .child(h_flex().gap_2().items_center().child(filter_bar).child(div().w(px(240.)).child(Input::new(&self.srs_search_in))))
                    .child(scroll_area("srs-browser").child(rows))
                    .into_any_element()
            }
            SrsSubTab::Statistics => {
                let st = compute_srs_statistics(&a.srs_all_cards, &a.srs_schedule, &a.srs_events, Utc::now());
                let mut c = v_flex()
                    .gap_1()
                    .child(heading("📊 FSRS v6 Study Statistics & 30-Day Forecast"))
                    .child(small(format!("Total Cards: {} | Due Today: {} | New: {} | Learning: {} | Review: {} | Suspended: {} | Leeches: {}", st.total_cards, st.due_today, st.new_cards, st.learning_cards, st.review_cards, st.suspended_cards, st.leech_cards)))
                    .child(small(format!("Total Reviews Logged: {} | Historical Retention Rate: {:.1}%", st.total_reviews_logged, st.retention_rate_percent)))
                    .child(heading("Per-Deck Breakdown (Total / Due):"));
                for (deck, (tot, due)) in &st.per_deck_counts {
                    c = c.child(small(format!("  • Deck `{}`: {} total ({} due)", deck, tot, due)));
                }
                c = c.child(heading("30-Day Due Forecast:"));
                for (date, count) in st.forecast_next_30_days.iter().take(30) {
                    c = c.child(div().text_xs().font_family("monospace").child(format!("  {} : {:>3} {}", date, count, "█".repeat((*count).min(30)))));
                }
                scroll_area("srs-stats").child(c).into_any_element()
            }
        };
        v_flex().size_full().gap_2().child(sub).child(div().flex_1().min_h_0().child(body)).into_any_element()
    }

    fn graph_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let focus = if self.app.graph_local_mode { self.app.current_tab().map(|t| t.rel_path.clone()) } else { None };
        let graph = self.app.index.build_graph(
            focus.as_deref(),
            self.app.graph_include_orphans,
            self.app.graph_include_unresolved,
            self.app.graph_only_mocs,
            None,
        );
        let a = &self.app;
        let graph_menu = menu_btn(
            "g-menu",
            "Graph options",
            vec![
                mc("Local neighborhood (current note)", Message::ToggleGraphLocalMode, a.graph_local_mode),
                mc("Show orphans", Message::ToggleGraphOrphans, a.graph_include_orphans),
                mc("Show unresolved", Message::ToggleGraphUnresolved, a.graph_include_unresolved),
                mc("Only MOCs", Message::ToggleGraphOnlyMocs, a.graph_only_mocs),
            ],
            cx,
        );
        let controls = h_flex()
            .gap_2()
            .items_center()
            .child(graph_menu)
            .child(small(format!("{} nodes, {} edges (click any node to open)", graph.nodes.len(), graph.edges.len())));
        let moc = h_flex()
            .gap_1()
            .items_center()
            .child(small("MOC Generator:"))
            .child(div().w(px(180.)).child(Input::new(&self.moc_title_in)))
            .child(div().w(px(220.)).child(Input::new(&self.moc_tag_in)))
            .child(msg_btn("g-gen", "Generate MOC Note", Message::GenerateMocNote, false, cx));

        let nodes: Vec<(f32, f32, f32, u32)> = graph
            .nodes
            .iter()
            .map(|n| {
                let r = if n.is_moc { 7.0 } else { (3.5 + (n.degree as f32).sqrt() * 0.8).min(6.5) };
                let c = if n.is_moc { NORD_PURPLE } else if n.is_unresolved { NORD_ORANGE } else if n.is_orphan { NORD_YELLOW } else { NORD_CYAN };
                (n.x as f32, n.y as f32, r, c)
            })
            .collect();
        let edges: Vec<(usize, usize)> = graph.edges.iter().map(|e| (e.source, e.target)).collect();
        self.graph_hits = graph
            .nodes
            .iter()
            .filter(|n| !n.is_unresolved)
            .map(|n| (n.x as f32, n.y as f32, n.id.clone()))
            .collect();

        let bounds_cell = self.graph_bounds.clone();
        let cv = canvas(
            move |bounds, _, _| {
                bounds_cell.set(bounds);
                bounds
            },
            move |_, bounds, window, _| {
                window.paint_quad(fill(bounds, rgb(NORD_BG)));
                let (w, h) = (bounds.size.width, bounds.size.height);
                let pt = |x: f32, y: f32| point(bounds.origin.x + w * x, bounds.origin.y + h * y);
                for (s, t) in &edges {
                    if let (Some(u), Some(v)) = (nodes.get(*s), nodes.get(*t)) {
                        let mut pb = PathBuilder::stroke(px(1.2));
                        pb.move_to(pt(u.0, u.1));
                        pb.line_to(pt(v.0, v.1));
                        if let Ok(p) = pb.build() {
                            window.paint_path(p, rgba(0x88c0d040));
                        }
                    }
                }
                for (x, y, r, c) in &nodes {
                    let center = pt(*x, *y);
                    let rr = px(*r);
                    let b = Bounds::new(point(center.x - rr, center.y - rr), size(rr * 2.0, rr * 2.0));
                    window.paint_quad(quad(b, rr, rgb(*c), 0.0, rgb(*c), BorderStyle::default()));
                }
            },
        )
        .size_full();

        let bounds_cell = self.graph_bounds.clone();
        let canvas_box = div()
            .flex_1()
            .min_h_0()
            .w_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    let b = bounds_cell.get();
                    if b.size.width <= px(0.) || b.size.height <= px(0.) {
                        return;
                    }
                    let px_x = f32::from(ev.position.x - b.origin.x);
                    let px_y = f32::from(ev.position.y - b.origin.y);
                    let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
                    let hit = this
                        .graph_hits
                        .iter()
                        .find(|(x, y, _)| (px_x - x * bw).powi(2) + (px_y - y * bh).powi(2) <= 100.0)
                        .map(|(_, _, id)| id.clone());
                    if let Some(id) = hit {
                        this.dispatch(Message::OpenNote(id), window, cx);
                    }
                }),
            )
            .child(cv);

        v_flex().size_full().gap_2().child(controls).child(moc).child(canvas_box).into_any_element()
    }

    fn section(
        &self,
        key: &'static str,
        title: String,
        body: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = !self.collapsed.contains(key);
        let head = div()
            .id(format!("sec-{}", key))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.collapsed.remove(key) {
                    this.collapsed.insert(key);
                }
                cx.notify();
            }))
            .child(heading(format!("{} {}", if open { "▾" } else { "▸" }, title)));
        v_flex()
            .gap_1()
            .child(head)
            .when(open, |d| d.child(v_flex().gap_1().children(body)))
            .into_any_element()
    }

    fn right_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.app.current_tab() else {
            return div().child("No active note").into_any_element();
        };
        let raw = tab.text();
        let fm = parse_frontmatter(&raw);
        let parsed = parse_note_body(&raw, fm.body_byte_offset, fm.body_start_line);
        let mut col = v_flex().gap_3().pl_2();

        let outline: Vec<AnyElement> = parsed
            .headings
            .iter()
            .map(|h| {
                let indent = "  ".repeat(h.level.saturating_sub(1) as usize);
                small(format!("{}{} (L{})", indent, h.text, h.line)).into_any_element()
            })
            .collect();
        col = col.child(self.section("outline", format!("Outline ({})", outline.len()), outline, cx));

        let incoming = self.app.index.incoming_links.get(&tab.rel_path).cloned().unwrap_or_default();
        let n_in = incoming.len();
        let back: Vec<AnyElement> = incoming
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(i, src)| msg_btn(("in", i), src.clone(), Message::OpenNote(src), false, cx).w_full().into_any_element())
            .collect();
        col = col.child(self.section("backlinks", format!("Backlinks ({})", n_in), back, cx));

        let related = self.app.index.related_notes(&tab.rel_path, 6);
        if !related.is_empty() {
            let rel: Vec<AnyElement> = related
                .into_iter()
                .enumerate()
                .map(|(i, (rpath, _score, reason))| msg_btn(("rel", i), format!("{} ({})", rpath, reason), Message::OpenNote(rpath), false, cx).w_full().into_any_element())
                .collect();
            col = col.child(self.section("related", "Related Notes".to_string(), rel, cx));
        }

        let mut pal: Vec<AnyElement> = vec![Input::new(&self.symbol_in).into_any_element()];
        for (i, e) in search_symbol_palette(&self.app.symbol_palette_query).into_iter().take(6).enumerate() {
            let label = format!("{}  {}  [{}]", e.symbol, e.name, e.triggers.join(", "));
            pal.push(msg_btn(("pal", i), label, Message::InsertSymbol(e.symbol.clone()), false, cx).w_full().into_any_element());
        }
        col = col.child(self.section("palette", "Logic Symbol Palette".to_string(), pal, cx));

        let mut props: Vec<AnyElement> = Vec::new();
        if fm.has_placeholder_alias {
            props.push(small("⚠️ Has placeholder alias [\"[]\"]").into_any_element());
        }
        if fm.has_tags_key && fm.tags.is_empty() {
            props.push(small("⚠️ Frontmatter `tags:` list is empty").into_any_element());
        }
        for issue in &parsed.highlight_issues {
            props.push(small(format!("⚠️ L{}: {}", issue.line, issue.description)).into_any_element());
        }
        col = col.child(self.section("props", format!("Properties & Lint ({})", props.len()), props, cx));

        div().id("right-scroll").size_full().overflow_y_scroll().child(col).into_any_element()
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let (words, chars, line, col, dirty) = match self.app.current_tab() {
            Some(t) => {
                let (w, c) = t.word_and_char_count();
                let p = t.cursor_position();
                (w, c, p.line, p.column, if t.is_dirty { "Modified*" } else { "Saved" })
            }
            None => (0, 0, 0, 0, "Idle"),
        };
        let mins = (words / 200).max(1);
        h_flex()
            .gap_3()
            .items_center()
            .child(small(format!("{} | {} words ({} chars, ~{}m) | Ln {}, Col {}", dirty, words, chars, mins, line + 1, col + 1)))
            .child(msg_btn("tt", if self.app.typing_transformer_enabled { "TT: ON (61 rules)" } else { "TT: OFF" }, Message::ToggleTypingTransformer, self.app.typing_transformer_enabled, cx))
            .child(small(self.app.status_banner.clone()).flex_1())
            .child(menu_btn(
                "git-menu",
                format!("⎇ {} ({} changed)", self.app.git_branch, self.app.git_dirty_count),
                vec![
                    mi("Refresh git status", Message::RefreshGitStatus),
                    mi("Commit with message →", Message::ExecuteGitCommit),
                ],
                cx,
            ))
            .child(div().w(px(200.)).child(Input::new(&self.commit_in)))
            .into_any_element()
    }
}

pub fn run_desktop_app(config: NotaConfig, initial_note: Option<String>) -> anyhow::Result<()> {
    let app = NotaApp::load(config, initial_note.as_deref())?;
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
            cx.new(|cx| NotaWorkspaceView::new(app, window, cx))
        })
        .expect("failed to open Nota window");
        cx.activate(true);
    });
    Ok(())
}
