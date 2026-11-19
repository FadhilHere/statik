//! The Data View grid, as in SPSS: variable names as column headers (tooltip = label), case
//! numbers on the left, grey "var" columns after the last variable and empty rows after the
//! last case (typing there creates `VAR00001` / new cases), system-missing shown as `.`,
//! numbers with the variable's decimals, and value labels when View > Value Labels is on.
//!
//! Only the visible rows **and** columns are laid out and painted: the grid is one
//! `ScrollArea::show_viewport` whose content size covers all cases and variables, and each frame
//! draws the cells inside the viewport plus the header row and case-number column pinned to its
//! top and left edges. (`egui_extras::TableBuilder` virtualises rows only and cannot pin the
//! case numbers while scrolling sideways, so it is not used here; the Variable View uses it.)
//!
//! Keyboard (when no text field has focus): arrows / Enter / Tab move (Shift extends or goes
//! back), typing starts editing, F2 edits the current value, Delete clears, Ctrl+C / Ctrl+X /
//! Ctrl+V copy, cut and paste tab-separated text (egui's clipboard events).
use crate::edit::display_value;
use crate::editor::{Align, CellEdit, SelKind, View};
use crate::state::{AppState, Command};
use dataset::{Column, VarId};
use egui::text::{CCursor, CCursorRange};
use egui::{
    Align2, Color32, Event, FontId, Id, Key, Modifiers, Pos2, Rect, ScrollArea, Sense, Stroke,
    TextEdit, Ui, Vec2, ViewportCommand, WidgetInfo, WidgetType, pos2, vec2,
};

/// Height of a case row.
pub const ROW_H: f32 = 22.0;
/// Height of the variable-name header row.
pub const HEADER_H: f32 = 24.0;
const CHAR_W: f32 = 8.0;

/// Pixel width of a column showing `chars` characters (Variable View "Columns").
pub fn col_width(chars: usize) -> f32 {
    (chars.max(5) as f32 * CHAR_W + 12.0).min(800.0)
}
/// Width of an empty "var" column (the SPSS default of 8 characters).
fn empty_col_w() -> f32 {
    col_width(8)
}

struct Palette {
    header_bg: Color32,
    grid: Color32,
    selected: Color32,
    cursor: Color32,
    text: Color32,
    grey: Color32,
    cell_bg: Color32,
}

fn palette(ui: &Ui) -> Palette {
    if ui.visuals().dark_mode {
        Palette {
            header_bg: Color32::from_gray(48),
            grid: Color32::from_gray(70),
            selected: Color32::from_rgb(40, 70, 110),
            cursor: Color32::from_rgb(120, 170, 240),
            text: Color32::from_gray(225),
            grey: Color32::from_gray(120),
            cell_bg: Color32::from_gray(30),
        }
    } else {
        Palette {
            header_bg: Color32::from_rgb(240, 240, 240),
            grid: Color32::from_rgb(212, 212, 212),
            selected: Color32::from_rgb(204, 224, 245),
            cursor: Color32::from_rgb(47, 110, 186),
            text: Color32::from_gray(20),
            grey: Color32::from_gray(150),
            cell_bg: Color32::WHITE,
        }
    }
}

/// Something the user did in this frame; applied after drawing.
enum Act {
    Cell {
        r: usize,
        c: usize,
        extend: bool,
    },
    DragTo {
        r: usize,
        c: usize,
    },
    Edit {
        r: usize,
        c: usize,
    },
    Header {
        c: usize,
        extend: bool,
    },
    OpenVar(usize),
    Case {
        r: usize,
        extend: bool,
    },
    Cmd(Command),
    Copy,
    Cut,
    RequestPaste,
    /// Commit the cell being edited and move the cursor by `(rows, cols)`.
    Commit(isize, isize),
    Cancel,
}

/// Commit the cell being edited, if any (e.g. before switching to the Variable View). Returns
/// false if the text was rejected (a message is queued and the old value kept).
pub fn commit_edit(state: &mut AppState) -> bool {
    let Some(e) = state.editor.editing.take() else {
        return true;
    };
    if e.text == original_text(state, e.row, e.col) {
        return true; // unchanged: keep the stored value (it may have more decimals than shown)
    }
    let before = state.messages.len();
    state.set_cell_text(e.row, e.col, &e.text);
    state.messages.len() == before
}

/// The text a cell's editor starts with on F2 / double click.
fn original_text(state: &AppState, row: usize, col: usize) -> String {
    let ds = state.doc.ds();
    if row >= ds.n_cases() || col >= ds.n_vars() {
        return String::new();
    }
    match ds.column(VarId(col)) {
        Column::Numeric(v) if v[row].is_nan() => String::new(),
        _ => display_value(ds, col, row, false),
    }
}

/// Draw the Data View and handle its input.
pub fn data_view(ui: &mut Ui, state: &mut AppState) {
    let ctx = ui.ctx().clone();
    let pal = palette(ui);
    let font = FontId::proportional(13.0);
    let (n_vars, n_cases) = (state.doc.ds().n_vars(), state.doc.ds().n_cases());

    // Column geometry: x of each variable column's left edge, relative to the first column.
    let mut xs = Vec::with_capacity(n_vars + 1);
    let mut x = 0.0;
    for j in 0..n_vars {
        xs.push(x);
        x += col_width(state.editor.columns(state.doc.ds().var(VarId(j))));
    }
    xs.push(x);
    let data_w = x;
    let avail = ui.available_size();
    let row_digits = (n_cases + 100).to_string().len();
    let rownum_w = (row_digits as f32 * CHAR_W + 16.0).max(44.0);
    let extra_cols = (((avail.x - rownum_w - data_w).max(0.0) / empty_col_w()).ceil() as usize)
        .max(1)
        .max((state.editor.cursor.1 + 1).saturating_sub(n_vars));
    let extra_rows = (((avail.y - HEADER_H - n_cases as f32 * ROW_H).max(0.0) / ROW_H).ceil()
        as usize)
        .max(1)
        .max((state.editor.cursor.0 + 1).saturating_sub(n_cases));
    let total_cols = n_vars + extra_cols;
    let total_rows = n_cases + extra_rows;
    let col_x = |c: usize| -> f32 {
        rownum_w
            + if c < n_vars {
                xs[c]
            } else {
                data_w + (c - n_vars) as f32 * empty_col_w()
            }
    };
    let col_w = |c: usize| -> f32 {
        if c < n_vars {
            xs[c + 1] - xs[c]
        } else {
            empty_col_w()
        }
    };
    let content = vec2(col_x(total_cols), HEADER_H + total_rows as f32 * ROW_H);

    // Keyboard and clipboard, when the grid owns the keyboard.
    let mut acts: Vec<Act> = Vec::new();
    let page = ((avail.y - HEADER_H) / ROW_H).floor().max(1.0) as isize;
    let keys_active = ui.is_enabled()
        && state.message().is_none()
        && state.editor.editing.is_none()
        && !ctx.text_edit_focused();
    if keys_active {
        // Focus moved to another widget (e.g. Tab) belongs back to the grid.
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        keyboard(ui, state, page, n_cases, n_vars);
    }

    ScrollArea::both()
        .id_salt("data_view")
        .auto_shrink([false, false])
        .animated(false)
        .content_margin(0.0)
        .scroll_source(egui::scroll_area::ScrollSource {
            drag: egui::scroll_area::DragScroll::Never,
            ..Default::default()
        })
        .show_viewport(ui, |ui, vp| {
            let origin = ui.max_rect().min;
            ui.set_min_size(content);
            let painter = ui.painter().clone();
            let at = |p: Vec2| origin + p;

            // Visible range (the pinned header / case numbers cover the top / left edge).
            let r0 = ((vp.min.y / ROW_H).floor().max(0.0) as usize).min(total_rows);
            let r1 = (((vp.max.y - HEADER_H) / ROW_H).ceil().max(0.0) as usize + 1).min(total_rows);
            let first_col = |px: f32| -> usize {
                // Last column whose left edge is <= px.
                let mut lo = 0usize;
                let mut hi = total_cols;
                while lo + 1 < hi {
                    let mid = (lo + hi) / 2;
                    if col_x(mid) <= px { lo = mid } else { hi = mid }
                }
                lo
            };
            let c0 = first_col(vp.min.x + rownum_w);
            let c1 = (first_col(vp.max.x) + 1).min(total_cols);
            let cell_rect = |r: usize, c: usize| -> Rect {
                Rect::from_min_size(
                    at(vec2(col_x(c), HEADER_H + r as f32 * ROW_H)),
                    vec2(col_w(c), ROW_H),
                )
            };
            let hit = |p: Pos2| -> (usize, usize) {
                let q = p - origin;
                let r = ((q.y - HEADER_H) / ROW_H).floor().max(0.0) as usize;
                (r.min(total_rows - 1), first_col(q.x.max(rownum_w)))
            };
            let shift = ui.input(|i| i.modifiers.shift);
            let ds = state.doc.ds();
            let editor = &state.editor;

            // Cells.
            painter.rect_filled(
                Rect::from_min_max(
                    at(vec2(col_x(c0), HEADER_H + r0 as f32 * ROW_H)),
                    at(vec2(col_x(c1), HEADER_H + r1 as f32 * ROW_H)),
                ),
                0.0,
                pal.cell_bg,
            );
            for c in c0..c1 {
                let var = (c < n_vars).then(|| ds.var(VarId(c)));
                let align = var.map(|v| editor.align(v)).unwrap_or(Align::Right);
                for r in r0..r1 {
                    let rect = cell_rect(r, c);
                    let selected = editor.is_selected(r, c);
                    if selected {
                        painter.rect_filled(rect, 0.0, pal.selected);
                    }
                    let text = if r < n_cases && c < n_vars {
                        display_value(ds, c, r, state.value_labels)
                    } else {
                        String::new()
                    };
                    if !text.is_empty() {
                        let room = rect.width() - 8.0;
                        let mut galley =
                            painter.layout_no_wrap(text.clone(), font.clone(), pal.text);
                        let mut align = align;
                        if galley.size().x > room {
                            match ds.column(VarId(c)) {
                                // A number too wide for the column: E notation, else asterisks (SPSS).
                                Column::Numeric(v) if text.parse::<f64>().is_ok() => {
                                    let x = v[r];
                                    let fitted = (0..=2)
                                        .rev()
                                        .map(|d| format!("{:.*E}", d, x))
                                        .chain(std::iter::once(
                                            "*".repeat(((room / 7.0) as usize).max(1)),
                                        ))
                                        .map(|t| painter.layout_no_wrap(t, font.clone(), pal.text))
                                        .find(|g| g.size().x <= room);
                                    if let Some(g) = fitted {
                                        galley = g;
                                    }
                                }
                                // Long text or a value label: keep its start visible.
                                _ => align = Align::Left,
                            }
                        }
                        let pos = match align {
                            Align::Left => {
                                pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0)
                            }
                            Align::Right => pos2(
                                rect.right() - 4.0 - galley.size().x,
                                rect.center().y - galley.size().y / 2.0,
                            ),
                            Align::Center => rect.center() - galley.size() / 2.0,
                        };
                        painter
                            .with_clip_rect(rect.shrink(1.0))
                            .galley(pos, galley, pal.text);
                    }
                    let resp =
                        ui.interact(rect, Id::new(("dv_cell", r, c)), Sense::CLICK | Sense::DRAG);
                    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
                    if resp.drag_started() {
                        acts.push(Act::Cell {
                            r,
                            c,
                            extend: shift,
                        });
                    } else if resp.dragged() {
                        if let Some(p) = resp.interact_pointer_pos() {
                            let (r, c) = hit(p);
                            acts.push(Act::DragTo { r, c });
                        }
                    } else if resp.double_clicked() {
                        acts.push(Act::Edit { r, c });
                    } else if resp.clicked() {
                        acts.push(Act::Cell {
                            r,
                            c,
                            extend: shift,
                        });
                    } else if resp.secondary_clicked() && !selected {
                        acts.push(Act::Cell {
                            r,
                            c,
                            extend: false,
                        });
                    }
                    resp.context_menu(|ui| {
                        clip_items(ui, &mut acts);
                        ui.separator();
                        if ui.button("Insert Variable").clicked() {
                            acts.push(Act::Cmd(Command::InsertVariable));
                        }
                        if ui.button("Insert Cases").clicked() {
                            acts.push(Act::Cmd(Command::InsertCases));
                        }
                    });
                }
            }
            // Grid lines.
            let grid = Stroke::new(1.0, pal.grid);
            let (x_a, x_b) = (at(vec2(col_x(c0), 0.0)).x, at(vec2(col_x(c1), 0.0)).x);
            for r in r0..=r1 {
                let y = at(vec2(0.0, HEADER_H + r as f32 * ROW_H)).y - 0.5;
                painter.hline(x_a..=x_b, y, grid);
            }
            let (y_a, y_b) = (
                at(vec2(0.0, HEADER_H + r0 as f32 * ROW_H)).y,
                at(vec2(0.0, HEADER_H + r1 as f32 * ROW_H)).y,
            );
            for c in c0..=c1 {
                painter.vline(at(vec2(col_x(c), 0.0)).x - 0.5, y_a..=y_b, grid);
            }
            // Current cell.
            let (cr, cc) = editor.cursor;
            if (r0..r1).contains(&cr) && (c0..c1).contains(&cc) {
                painter.rect_stroke(
                    cell_rect(cr, cc).shrink(1.0),
                    0.0,
                    Stroke::new(2.0, pal.cursor),
                    egui::StrokeKind::Inside,
                );
            }

            // Case numbers, pinned to the left edge.
            let left = origin.x + vp.min.x;
            for r in r0..r1 {
                let rect = Rect::from_min_size(
                    pos2(left, at(vec2(0.0, HEADER_H + r as f32 * ROW_H)).y),
                    vec2(rownum_w, ROW_H),
                );
                let sel = editor.sel_kind == SelKind::Cases && editor.is_selected(r, 0);
                painter.rect_filled(rect, 0.0, if sel { pal.selected } else { pal.header_bg });
                painter.rect_stroke(rect, 0.0, grid, egui::StrokeKind::Inside);
                let label = (r + 1).to_string();
                let color = if r < n_cases { pal.text } else { pal.grey };
                painter.text(
                    rect.right_center() - vec2(5.0, 0.0),
                    Align2::RIGHT_CENTER,
                    &label,
                    font.clone(),
                    color,
                );
                let resp = ui.interact(rect, Id::new(("dv_case", r)), Sense::CLICK);
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &label));
                if resp.clicked() || (resp.secondary_clicked() && !sel) {
                    acts.push(Act::Case {
                        r,
                        extend: shift && resp.clicked(),
                    });
                }
                resp.context_menu(|ui| {
                    clip_items(ui, &mut acts);
                    ui.separator();
                    if ui.button("Insert Cases").clicked() {
                        acts.push(Act::Cmd(Command::InsertCases));
                    }
                });
            }

            // Variable names, pinned to the top edge.
            let top = origin.y + vp.min.y;
            for c in c0..c1 {
                let x0 = at(vec2(col_x(c), 0.0)).x;
                let rect = Rect::from_min_size(pos2(x0, top), vec2(col_w(c), HEADER_H));
                let sel = editor.sel_kind == SelKind::Vars && editor.is_selected(0, c);
                painter.rect_filled(rect, 0.0, if sel { pal.selected } else { pal.header_bg });
                painter.rect_stroke(rect, 0.0, grid, egui::StrokeKind::Inside);
                let var = (c < n_vars).then(|| ds.var(VarId(c)));
                let (name, color) = match var {
                    Some(v) => (v.name.as_str(), pal.text),
                    None => ("var", pal.grey),
                };
                painter.with_clip_rect(rect.shrink(1.0)).text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    name,
                    font.clone(),
                    color,
                );
                let mut resp = ui.interact(rect, Id::new(("dv_head", c)), Sense::CLICK);
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, name));
                if let Some(label) = var.and_then(|v| v.label.as_deref()) {
                    resp = resp.on_hover_text(label);
                }
                if resp.double_clicked() && c < n_vars {
                    acts.push(Act::OpenVar(c));
                } else if resp.clicked() || (resp.secondary_clicked() && !sel) {
                    acts.push(Act::Header {
                        c,
                        extend: shift && resp.clicked(),
                    });
                }
                resp.context_menu(|ui| {
                    clip_items(ui, &mut acts);
                    ui.separator();
                    if ui.button("Insert Variable").clicked() {
                        acts.push(Act::Cmd(Command::InsertVariable));
                    }
                    ui.separator();
                    for l in ["Sort Ascending", "Sort Descending"] {
                        ui.add_enabled(false, egui::Button::new(l));
                    }
                    ui.separator();
                    for l in ["Variable Information...", "Descriptive Statistics"] {
                        ui.add_enabled(false, egui::Button::new(l));
                    }
                });
            }
            // Corner.
            let corner = Rect::from_min_size(pos2(left, top), vec2(rownum_w, HEADER_H));
            painter.rect_filled(corner, 0.0, pal.header_bg);
            painter.rect_stroke(corner, 0.0, grid, egui::StrokeKind::Inside);
            ui.interact(corner, Id::new("dv_corner"), Sense::hover());

            // The cell being edited.
            let mut cursor_rect = None;
            if state.editor.scroll_to_cursor {
                let (r, c) = state.editor.cursor;
                let rect = cell_rect(r, c);
                cursor_rect = Some(Rect::from_min_max(
                    rect.min - vec2(rownum_w, HEADER_H),
                    rect.max,
                ));
            }
            if let Some(edit) = &mut state.editor.editing {
                let visible = (r0..r1).contains(&edit.row) && (c0..c1).contains(&edit.col);
                if visible {
                    edit_cell(ui, edit, cell_rect(edit.row, edit.col), &pal, &mut acts);
                } else if edit.focused {
                    acts.push(Act::Commit(0, 0)); // scrolled away
                }
            }
            if let Some(rect) = cursor_rect {
                ui.scroll_to_rect(rect, None);
                state.editor.scroll_to_cursor = false;
            }
        });

    for a in acts {
        apply(ui, state, a);
    }
}

/// Cut / Copy / Paste / Clear entries of the context menus.
fn clip_items(ui: &mut Ui, acts: &mut Vec<Act>) {
    if ui.button("Cut").clicked() {
        acts.push(Act::Cut);
    }
    if ui.button("Copy").clicked() {
        acts.push(Act::Copy);
    }
    if ui.button("Paste").clicked() {
        acts.push(Act::RequestPaste);
    }
    if ui.button("Clear").clicked() {
        acts.push(Act::Cmd(Command::Clear));
    }
}

/// The text field over the cell being edited.
fn edit_cell(ui: &mut Ui, edit: &mut CellEdit, rect: Rect, pal: &Palette, acts: &mut Vec<Act>) {
    let id = Id::new("data_view_cell_edit");
    if edit.focused {
        // Enter / Tab commit and move, Escape cancels; in typing mode up/down also commit and
        // move, as in SPSS. Taken before the text field sees them.
        let typing = edit.from_typing;
        let act = ui.input_mut(|i| {
            let shift = i.modifiers.shift;
            let back = if shift { -1 } else { 1 };
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                Some(Act::Cancel)
            } else if i.consume_key(Modifiers::SHIFT, Key::Enter)
                || i.consume_key(Modifiers::NONE, Key::Enter)
            {
                Some(Act::Commit(back, 0))
            } else if i.consume_key(Modifiers::SHIFT, Key::Tab)
                || i.consume_key(Modifiers::NONE, Key::Tab)
            {
                Some(Act::Commit(0, back))
            } else if typing && i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                Some(Act::Commit(-1, 0))
            } else if typing && i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                Some(Act::Commit(1, 0))
            } else {
                None
            }
        });
        if let Some(a) = act {
            acts.push(a);
            return;
        }
    }
    ui.painter().rect_filled(rect, 0.0, pal.cell_bg);
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        0.0,
        Stroke::new(2.0, pal.cursor),
        egui::StrokeKind::Inside,
    );
    let resp = ui.put(
        rect.shrink2(vec2(3.0, 2.0)),
        TextEdit::singleline(&mut edit.text)
            .id(id)
            .frame(egui::Frame::NONE)
            .margin(0.0)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Body),
    );
    if !edit.focused {
        resp.request_focus();
        let mut st = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        let end = CCursor::new(edit.text.chars().count());
        st.cursor.set_char_range(Some(CCursorRange::one(end)));
        st.store(ui.ctx(), id);
        edit.focused = true;
        return;
    }
    if resp.lost_focus() {
        acts.push(Act::Commit(0, 0)); // clicked elsewhere
    }
}

/// Keyboard and clipboard input while no text field has focus.
fn keyboard(ui: &mut Ui, state: &mut AppState, page: isize, n_cases: usize, n_vars: usize) {
    let mut moves: Vec<(isize, isize, bool)> = Vec::new();
    let (mut text, mut f2, mut delete, mut home) = (String::new(), false, false, false);
    let (mut copy, mut cut, mut paste) = (false, false, None::<String>);
    ui.input_mut(|i| {
        let keys = [
            (Key::ArrowUp, -1, 0),
            (Key::ArrowDown, 1, 0),
            (Key::ArrowLeft, 0, -1),
            (Key::ArrowRight, 0, 1),
            (Key::PageUp, -page, 0),
            (Key::PageDown, page, 0),
        ];
        for (key, dr, dc) in keys {
            while i.consume_key(Modifiers::SHIFT, key) {
                moves.push((dr, dc, true));
            }
            while i.consume_key(Modifiers::NONE, key) {
                moves.push((dr, dc, false));
            }
        }
        while i.consume_key(Modifiers::SHIFT, Key::Enter) {
            moves.push((-1, 0, false));
        }
        while i.consume_key(Modifiers::NONE, Key::Enter) {
            moves.push((1, 0, false));
        }
        while i.consume_key(Modifiers::SHIFT, Key::Tab) {
            moves.push((0, -1, false));
        }
        while i.consume_key(Modifiers::NONE, Key::Tab) {
            moves.push((0, 1, false));
        }
        home = i.consume_key(Modifiers::COMMAND, Key::Home);
        f2 = i.consume_key(Modifiers::NONE, Key::F2);
        delete = i.consume_key(Modifiers::NONE, Key::Delete);
        for e in &i.events {
            match e {
                Event::Text(t) => text.push_str(t),
                Event::Copy => copy = true,
                Event::Cut => cut = true,
                Event::Paste(s) => paste = Some(s.clone()),
                _ => {}
            }
        }
        i.events.retain(|e| {
            !matches!(
                e,
                Event::Text(_) | Event::Copy | Event::Cut | Event::Paste(_)
            )
        });
    });
    let ed = &mut state.editor;
    if home {
        ed.jump_to(0, 0);
    }
    for (dr, dc, extend) in moves {
        let (r, c) = ed.cursor;
        let r = (r as isize + dr).clamp(0, n_cases.max(r) as isize) as usize;
        let c = (c as isize + dc).clamp(0, n_vars.max(c) as isize) as usize;
        if extend {
            ed.cursor = (r, c);
            ed.sel_kind = SelKind::Cells;
        } else {
            ed.select_cells((r, c), (r, c));
        }
        ed.scroll_to_cursor = true;
    }
    let ctx = ui.ctx().clone();
    if copy || cut {
        ctx.copy_text(state.copy_selection());
    }
    if cut || delete {
        state.clear_selection();
    }
    if let Some(s) = paste {
        state.paste_at_cursor(&s);
    }
    let (row, col) = state.editor.cursor;
    if f2 {
        let t = original_text(state, row, col);
        state.editor.editing = Some(CellEdit {
            row,
            col,
            text: t,
            from_typing: false,
            focused: false,
        });
    } else if !text.is_empty() {
        state.editor.select_cells((row, col), (row, col));
        state.editor.editing = Some(CellEdit {
            row,
            col,
            text,
            from_typing: true,
            focused: false,
        });
    }
}

fn apply(ui: &mut Ui, state: &mut AppState, a: Act) {
    match a {
        Act::Cell { r, c, extend } => {
            if !commit_edit(state) {
                return;
            }
            let ed = &mut state.editor;
            if extend {
                ed.cursor = (r, c);
                ed.sel_kind = SelKind::Cells;
            } else {
                ed.select_cells((r, c), (r, c));
            }
        }
        Act::DragTo { r, c } => {
            state.editor.cursor = (r, c);
            state.editor.sel_kind = SelKind::Cells;
        }
        Act::Edit { r, c } => {
            if !commit_edit(state) {
                return;
            }
            let text = original_text(state, r, c);
            state.editor.select_cells((r, c), (r, c));
            state.editor.editing = Some(CellEdit {
                row: r,
                col: c,
                text,
                from_typing: false,
                focused: false,
            });
        }
        Act::Header { c, extend } => {
            if !commit_edit(state) {
                return;
            }
            let ed = &mut state.editor;
            if extend && ed.sel_kind == SelKind::Vars {
                ed.cursor = (0, c);
            } else {
                ed.select_vars(c, c);
            }
        }
        Act::Case { r, extend } => {
            if !commit_edit(state) {
                return;
            }
            let ed = &mut state.editor;
            if extend && ed.sel_kind == SelKind::Cases {
                ed.cursor = (r, 0);
            } else {
                ed.select_cases(r, r);
            }
        }
        Act::OpenVar(c) => {
            commit_edit(state);
            state.editor.view = View::Variable;
            state.editor.var_row = c;
            state.editor.scroll_to_var_row = true;
        }
        Act::Cmd(cmd) => {
            commit_edit(state);
            if let Err(e) = state.apply(cmd) {
                state.push_message(crate::state::Message::error(e));
            }
        }
        Act::Copy | Act::Cut => {
            ui.ctx().copy_text(state.copy_selection());
            if matches!(a, Act::Cut) {
                state.clear_selection();
            }
        }
        Act::RequestPaste => ui.ctx().send_viewport_cmd(ViewportCommand::RequestPaste),
        Act::Commit(dr, dc) => {
            let Some(e) = state.editor.editing.as_ref() else {
                return;
            };
            let (r, c) = (e.row, e.col);
            if commit_edit(state) {
                let r = (r as isize + dr).max(0) as usize;
                let c = (c as isize + dc).max(0) as usize;
                state.editor.select_cells((r, c), (r, c));
                state.editor.scroll_to_cursor = true;
            }
        }
        Act::Cancel => state.editor.editing = None,
    }
}
