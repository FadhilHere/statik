//! The Variable View grid, as in SPSS: one row per variable with Name, Type, Width, Decimals,
//! Label, Values, Missing, Columns, Align, Measure and Role, plus empty rows after the last
//! variable (typing a name there creates a variable). Type, Values and Missing open the small
//! SPSS dialogs "Variable Type", "Value Labels" and "Missing Values". Every change is one undo
//! step (see [`crate::edit`]).
//!
//! Align, Role and the Columns of numeric variables are not part of the data model; they live
//! in [`crate::editor::EditorState`] and are not saved (see `KNOWN_GAPS.md`).
use crate::edit::{format_number, parse_number};
use crate::editor::{Align, Role, VarEdit, VarField};
use crate::state::{AppState, Message};
use dataset::{Measure, MissingSpec, OrderedValue, VarId, VarType, Variable};
use egui::text::{CCursor, CCursorRange};
use egui::{
    Color32, Id, Key, Label, Modifiers, Pos2, Rect, RichText, Sense, Stroke, TextEdit, Ui, pos2,
    vec2,
};
use egui_extras::{Column as TCol, TableBuilder};

/// Height of a variable row.
pub const ROW_H: f32 = 22.0;

/// Column titles and initial widths.
pub const COLUMNS: [(&str, f32); 11] = [
    ("Name", 90.0),
    ("Type", 95.0),
    ("Width", 50.0),
    ("Decimals", 60.0),
    ("Label", 140.0),
    ("Values", 115.0),
    ("Missing", 100.0),
    ("Columns", 55.0),
    ("Align", 90.0),
    ("Measure", 105.0),
    ("Role", 90.0),
];

/// An open Variable View dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum VarDialog {
    Type(TypeDialog),
    Values(ValuesDialog),
    Missing(MissingDialog),
}

/// "Variable Type".
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDialog {
    pub var: usize,
    pub string: bool,
    pub width: String,
    pub decimals: String,
    pub chars: String,
}

/// A value-label key.
#[derive(Clone, Debug, PartialEq)]
pub enum LabelKey {
    Num(f64),
    Text(String),
}

/// "Value Labels".
#[derive(Clone, Debug, PartialEq)]
pub struct ValuesDialog {
    pub var: usize,
    pub numeric: bool,
    pub decimals: usize,
    pub value: String,
    pub label: String,
    pub list: Vec<(LabelKey, String)>,
    pub selected: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingMode {
    None,
    Discrete,
    Range,
}

/// "Missing Values".
#[derive(Clone, Debug, PartialEq)]
pub struct MissingDialog {
    pub var: usize,
    pub numeric: bool,
    pub mode: MissingMode,
    pub discrete: [String; 3],
    pub low: String,
    pub high: String,
    pub extra: String,
}

/// Text of the Type cell.
pub fn type_text(v: &Variable) -> &'static str {
    match v.var_type {
        VarType::String { .. } => "String",
        VarType::Numeric => match v.format_type {
            _ if v.has_date_format() => "Date",
            _ if v.has_time_format() => "Time",
            Some(3) => "Comma",
            Some(4) => "Dollar",
            Some(5) => "Dot",
            Some(17) => "Scientific Notation",
            Some(16) => "Restricted Numeric",
            _ => "Numeric",
        },
    }
}

/// Text of the Values cell: `None` or `{1.00, Male}...`.
pub fn values_text(v: &Variable) -> String {
    match v.var_type {
        VarType::Numeric => match v.value_labels.iter().next() {
            None => "None".into(),
            Some((k, l)) => format!("{{{}, {l}}}...", format_number(k.0, v.decimals)),
        },
        VarType::String { .. } => match v.string_value_labels.iter().next() {
            None => "None".into(),
            Some((k, l)) => format!("{{{k}, {l}}}..."),
        },
    }
}

/// Text of the Missing cell: `None`, `9.00, 99.00`, `1.00 - 5.00, 9.00`.
pub fn missing_text(v: &Variable) -> String {
    let f = |x: f64| format_number(x, v.decimals);
    match v.var_type {
        VarType::String { .. } if v.string_missing.is_empty() => "None".into(),
        VarType::String { .. } => v.string_missing.join(", "),
        VarType::Numeric => match &v.missing {
            MissingSpec::None => "None".into(),
            MissingSpec::Discrete(l) => l.iter().map(|&x| f(x)).collect::<Vec<_>>().join(", "),
            MissingSpec::Range { lo, hi, discrete } => {
                let mut s = format!("{} - {}", f(*lo), f(*hi));
                if let Some(d) = discrete {
                    s.push_str(&format!(", {}", f(*d)));
                }
                s
            }
        },
    }
}

fn measure_label(m: Measure) -> &'static str {
    match m {
        Measure::Scale => "Scale",
        Measure::Ordinal => "Ordinal",
        Measure::Nominal => "Nominal",
    }
}

// ----- icons (drawn, not images) -----

fn icon_rect(ui: &mut Ui) -> Rect {
    ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover()).0
}

/// Measure icon: a ruler (Scale), rising bars (Ordinal), three dots (Nominal).
pub fn measure_icon(ui: &mut Ui, m: Measure) {
    let r = icon_rect(ui);
    paint_measure_icon(ui.painter(), r, m);
}

/// Paint the measure icon of [`measure_icon`] into a 14×14 rect (variable lists in dialogs).
pub fn paint_measure_icon(p: &egui::Painter, r: Rect, m: Measure) {
    let dark = Color32::from_gray(60);
    match m {
        Measure::Scale => {
            let body = Rect::from_min_max(
                pos2(r.left() + 1.0, r.center().y - 3.0),
                pos2(r.right() - 1.0, r.center().y + 3.0),
            );
            p.rect_filled(body, 1.0, Color32::from_rgb(242, 193, 78));
            p.rect_stroke(body, 1.0, Stroke::new(1.0, dark), egui::StrokeKind::Inside);
            for i in 1..4 {
                let x = body.left() + i as f32 * body.width() / 4.0;
                p.vline(x, body.top()..=body.top() + 3.0, Stroke::new(1.0, dark));
            }
        }
        Measure::Ordinal => {
            for (i, (h, c)) in [
                (5.0, (90, 160, 90)),
                (8.0, (70, 130, 200)),
                (11.0, (200, 90, 80)),
            ]
            .iter()
            .enumerate()
            {
                let x = r.left() + 1.5 + i as f32 * 4.2;
                let bar = Rect::from_min_max(
                    pos2(x, r.bottom() - 1.5 - h),
                    pos2(x + 3.2, r.bottom() - 1.5),
                );
                p.rect_filled(bar, 0.0, Color32::from_rgb(c.0, c.1, c.2));
            }
        }
        Measure::Nominal => {
            let c = r.center();
            for (d, col) in [
                ((-3.5, 2.5), (200, 90, 80)),
                ((3.5, 2.5), (70, 130, 200)),
                ((0.0, -3.5), (90, 160, 90)),
            ] {
                p.circle_filled(
                    c + vec2(d.0, d.1),
                    2.8,
                    Color32::from_rgb(col.0, col.1, col.2),
                );
            }
        }
    }
}

/// Align icon: three lines aligned left, right or centred.
pub fn align_icon(ui: &mut Ui, a: Align) {
    let r = icon_rect(ui).shrink(2.0);
    let p = ui.painter();
    let s = Stroke::new(1.5, Color32::from_gray(70));
    for (i, w) in [1.0, 0.6, 0.85].iter().enumerate() {
        let len = r.width() * w;
        let y = r.top() + 2.0 + i as f32 * 4.0;
        let x0 = match a {
            Align::Left => r.left(),
            Align::Right => r.right() - len,
            Align::Center => r.center().x - len / 2.0,
        };
        p.hline(x0..=x0 + len, y, s);
    }
}

/// Role icon.
pub fn role_icon(ui: &mut Ui, role: Role) {
    let r = icon_rect(ui).shrink(1.5);
    let p = ui.painter();
    let c = r.center();
    let s = Stroke::new(1.5, Color32::from_gray(70));
    let blue = Color32::from_rgb(70, 130, 200);
    let arrow = |from: Pos2, to: Pos2| {
        p.line_segment([from, to], Stroke::new(1.5, blue));
        let d = (to - from).normalized();
        let n = vec2(-d.y, d.x);
        p.line_segment([to, to - d * 3.5 + n * 3.0], Stroke::new(1.5, blue));
        p.line_segment([to, to - d * 3.5 - n * 3.0], Stroke::new(1.5, blue));
    };
    match role {
        Role::Input => {
            p.vline(r.right() - 1.0, r.y_range(), s);
            arrow(pos2(r.left(), c.y), pos2(r.right() - 3.0, c.y));
        }
        Role::Target => {
            p.circle_stroke(c, 5.0, Stroke::new(1.5, Color32::from_rgb(200, 90, 80)));
            p.circle_filled(c, 2.0, Color32::from_rgb(200, 90, 80));
        }
        Role::Both => {
            arrow(pos2(r.left(), c.y - 2.5), pos2(r.right(), c.y - 2.5));
            arrow(pos2(r.right(), c.y + 2.5), pos2(r.left(), c.y + 2.5));
        }
        Role::None => {
            p.circle_stroke(c, 5.0, s);
            p.line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], s);
        }
        Role::Partition => {
            p.rect_stroke(r, 1.0, s, egui::StrokeKind::Inside);
            p.vline(c.x, r.y_range(), s);
            p.hline(r.x_range(), c.y, s);
        }
        Role::Split => {
            p.rect_filled(
                Rect::from_min_max(r.min, pos2(c.x - 1.5, r.bottom())),
                1.0,
                blue,
            );
            p.rect_filled(
                Rect::from_min_max(pos2(c.x + 1.5, r.top()), r.max),
                1.0,
                Color32::from_rgb(90, 160, 90),
            );
        }
    }
}

/// Something the user did; applied after drawing.
enum Act {
    Select(usize),
    StartEdit(usize, VarField, String),
    Commit,
    Cancel,
    Open(VarDialog),
    Align(usize, Align),
    Measure(usize, Measure),
    Role(usize, Role),
}

/// Commit the Variable View cell being edited, if any.
pub fn commit_var_edit(state: &mut AppState) {
    let Some(e) = state.editor.var_edit.take() else {
        return;
    };
    let t = e.text.trim().to_string();
    let Some(v) = state.doc.ds().try_var(VarId(e.row)).cloned() else {
        if e.field == VarField::Name && !t.is_empty() {
            state.create_variable(e.row, &t);
        }
        return;
    };
    let bad = |state: &mut AppState, text: &str| {
        state.push_message(Message::warning("Invalid Value", text));
    };
    let numeric = v.var_type == VarType::Numeric;
    match e.field {
        VarField::Name => {
            if t != v.name {
                state.rename_variable(e.row, &t);
            }
        }
        VarField::Width => match (t.parse::<usize>(), numeric) {
            (Ok(w @ 1..=40), true) => {
                state.change_variable(e.row, |v| {
                    v.width = w;
                    v.decimals = v.decimals.min(w.saturating_sub(1));
                });
            }
            (Ok(w @ 1..=32767), false) => {
                if (VarType::String { width: w }) != v.var_type {
                    state.set_variable_type(e.row, VarType::String { width: w }, w, 0);
                }
            }
            (_, true) => bad(
                state,
                "The width of a numeric variable must be a whole number from 1 to 40.",
            ),
            (_, false) => bad(
                state,
                "The width of a string variable must be a whole number from 1 to 32767.",
            ),
        },
        VarField::Decimals if numeric => match t.parse::<usize>() {
            Ok(d @ 0..=16) => {
                state.change_variable(e.row, |v| {
                    v.decimals = d;
                    if v.width <= d {
                        v.width = (d + 1).min(40);
                    }
                });
            }
            _ => bad(
                state,
                "The number of decimal places must be a whole number from 0 to 16.",
            ),
        },
        VarField::Decimals => {}
        VarField::Label => {
            state.change_variable(e.row, |v| v.label = (!t.is_empty()).then(|| t.clone()));
        }
        VarField::Columns => match t.parse::<usize>() {
            Ok(n @ 1..=255) if numeric => state.editor.set_numeric_columns(&v.name, n),
            Ok(n @ 1..=255) => {
                state.change_variable(e.row, |v| v.width = n);
            }
            _ => bad(state, "Columns must be a whole number from 1 to 255."),
        },
    }
}

/// Draw the Variable View (and its open dialog) and handle its input.
pub fn variable_view(ui: &mut Ui, state: &mut AppState) {
    let n_vars = state.doc.ds().n_vars();
    let avail_h = ui.available_height();
    let total = (n_vars + 1).max((avail_h / ROW_H) as usize);
    let mut acts: Vec<Act> = Vec::new();

    egui::ScrollArea::horizontal()
        .id_salt("variable_view_h")
        .show(ui, |ui| {
            let mut tb = TableBuilder::new(ui)
                .id_salt("variable_view")
                .striped(false)
                .resizable(true)
                .auto_shrink([false, false])
                .animate_scrolling(false)
                .sense(Sense::click())
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(TCol::exact(36.0));
            for (_, w) in COLUMNS {
                tb = tb.column(TCol::initial(w).at_least(30.0).clip(true));
            }
            if state.editor.scroll_to_var_row {
                tb = tb.scroll_to_row(state.editor.var_row, None);
                state.editor.scroll_to_var_row = false;
            }
            tb.header(24.0, |mut h| {
                h.col(|_| {});
                for (name, _) in COLUMNS {
                    h.col(|ui| {
                        ui.strong(name);
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_H, total, |mut row| {
                    let i = row.index();
                    row.set_selected(i == state.editor.var_row);
                    let v = state.doc.ds().try_var(VarId(i)).cloned();
                    row.col(|ui| {
                        let grey = v.is_none();
                        let mut t = RichText::new((i + 1).to_string());
                        if grey {
                            t = t.color(Color32::from_gray(150));
                        }
                        if ui.add(Label::new(t).sense(Sense::click())).clicked() {
                            acts.push(Act::Select(i));
                        }
                    });
                    let Some(v) = v else {
                        row.col(|ui| text_cell(ui, state, i, VarField::Name, "", &mut acts));
                        for _ in 1..COLUMNS.len() {
                            row.col(|_| {});
                        }
                        return;
                    };
                    let numeric = v.var_type == VarType::Numeric;
                    row.col(|ui| text_cell(ui, state, i, VarField::Name, &v.name, &mut acts));
                    row.col(|ui| {
                        ui.label(type_text(&v));
                        dots(
                            ui,
                            &format!("Type of {}", v.name),
                            || Act::Open(VarDialog::Type(type_dialog(i, &v))),
                            &mut acts,
                        );
                    });
                    let width = match v.var_type {
                        VarType::String { width } => width,
                        VarType::Numeric => v.width,
                    };
                    row.col(|ui| {
                        text_cell(ui, state, i, VarField::Width, &width.to_string(), &mut acts)
                    });
                    row.col(|ui| {
                        if numeric {
                            text_cell(
                                ui,
                                state,
                                i,
                                VarField::Decimals,
                                &v.decimals.to_string(),
                                &mut acts,
                            );
                        } else {
                            ui.add_enabled(false, Label::new("0"));
                        }
                    });
                    row.col(|ui| {
                        text_cell(
                            ui,
                            state,
                            i,
                            VarField::Label,
                            v.label.as_deref().unwrap_or(""),
                            &mut acts,
                        )
                    });
                    row.col(|ui| {
                        ui.add(Label::new(values_text(&v)).truncate());
                        dots(
                            ui,
                            &format!("Values of {}", v.name),
                            || Act::Open(VarDialog::Values(values_dialog(i, &v))),
                            &mut acts,
                        );
                    });
                    row.col(|ui| {
                        ui.add(Label::new(missing_text(&v)).truncate());
                        dots(
                            ui,
                            &format!("Missing of {}", v.name),
                            || Act::Open(VarDialog::Missing(missing_dialog(i, &v))),
                            &mut acts,
                        );
                    });
                    let cols = state.editor.columns(&v);
                    row.col(|ui| {
                        text_cell(
                            ui,
                            state,
                            i,
                            VarField::Columns,
                            &cols.to_string(),
                            &mut acts,
                        )
                    });
                    row.col(|ui| {
                        let cur = state.editor.align(&v);
                        align_icon(ui, cur);
                        egui::ComboBox::from_id_salt(("vv_align", i))
                            .selected_text(cur.label())
                            .width(60.0)
                            .show_ui(ui, |ui| {
                                for a in Align::ALL {
                                    ui.horizontal(|ui| {
                                        align_icon(ui, a);
                                        if ui.selectable_label(a == cur, a.label()).clicked() {
                                            acts.push(Act::Align(i, a));
                                        }
                                    });
                                }
                            });
                    });
                    row.col(|ui| {
                        measure_icon(ui, v.measure);
                        egui::ComboBox::from_id_salt(("vv_measure", i))
                            .selected_text(measure_label(v.measure))
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                for m in [Measure::Scale, Measure::Ordinal, Measure::Nominal] {
                                    // SPSS does not offer Scale for string variables.
                                    let ok = numeric || m != Measure::Scale;
                                    ui.horizontal(|ui| {
                                        measure_icon(ui, m);
                                        let r = ui.add_enabled(
                                            ok,
                                            egui::Button::selectable(
                                                m == v.measure,
                                                measure_label(m),
                                            ),
                                        );
                                        if r.clicked() {
                                            acts.push(Act::Measure(i, m));
                                        }
                                    });
                                }
                            });
                    });
                    row.col(|ui| {
                        let cur = state.editor.role(&v);
                        role_icon(ui, cur);
                        egui::ComboBox::from_id_salt(("vv_role", i))
                            .selected_text(cur.label())
                            .width(64.0)
                            .show_ui(ui, |ui| {
                                for r in Role::ALL {
                                    ui.horizontal(|ui| {
                                        role_icon(ui, r);
                                        if ui.selectable_label(r == cur, r.label()).clicked() {
                                            acts.push(Act::Role(i, r));
                                        }
                                    });
                                }
                            });
                    });
                    if row.response().clicked() {
                        acts.push(Act::Select(i));
                    }
                });
            });
        });

    let enabled = ui.is_enabled();
    for a in acts {
        // Disabled (a job runs): the field losing focus must not change the data meanwhile.
        if !enabled && matches!(a, Act::Commit) {
            continue;
        }
        match a {
            Act::Select(i) => {
                commit_var_edit(state);
                state.editor.var_row = i;
            }
            Act::StartEdit(i, field, text) => {
                commit_var_edit(state);
                state.editor.var_row = i;
                state.editor.var_edit = Some(VarEdit {
                    row: i,
                    field,
                    text,
                    focused: false,
                });
            }
            Act::Commit => commit_var_edit(state),
            Act::Cancel => state.editor.var_edit = None,
            Act::Open(d) => {
                commit_var_edit(state);
                state.editor.var_row = dialog_var(&d);
                state.editor.var_dialog = Some(d);
            }
            Act::Align(i, a) => {
                if let Some(v) = state.doc.ds().try_var(VarId(i)) {
                    let name = v.name.clone();
                    state.editor.set_align(&name, a);
                }
            }
            Act::Measure(i, m) => {
                state.change_variable(i, |v| v.measure = m);
            }
            Act::Role(i, r) => {
                if let Some(v) = state.doc.ds().try_var(VarId(i)) {
                    let name = v.name.clone();
                    state.editor.set_role(&name, r);
                }
            }
        }
    }

    if state.editor.var_dialog.is_some() {
        show_dialog(ui, state);
    }
}

fn dialog_var(d: &VarDialog) -> usize {
    match d {
        VarDialog::Type(t) => t.var,
        VarDialog::Values(v) => v.var,
        VarDialog::Missing(m) => m.var,
    }
}

/// The small "..." button that opens a dialog; `name` is its accessible name.
fn dots(ui: &mut Ui, name: &str, act: impl FnOnce() -> Act, acts: &mut Vec<Act>) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let r = ui.add(egui::Button::new("...").small());
        let n = name.to_string();
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &n));
        if r.clicked() {
            acts.push(act());
        }
    });
}

/// A text cell: a label that starts editing when clicked, or the text field while editing.
fn text_cell(
    ui: &mut Ui,
    state: &mut AppState,
    row: usize,
    field: VarField,
    text: &str,
    acts: &mut Vec<Act>,
) {
    let editing = state
        .editor
        .var_edit
        .as_mut()
        .filter(|e| e.row == row && e.field == field);
    let Some(e) = editing else {
        let r = ui.add(Label::new(text).sense(Sense::click()).truncate());
        if r.clicked() {
            acts.push(Act::StartEdit(row, field, text.to_string()));
        }
        return;
    };
    let id = Id::new("variable_view_edit");
    if e.focused {
        let act = ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                Some(Act::Cancel)
            } else if i.consume_key(Modifiers::NONE, Key::Enter)
                || i.consume_key(Modifiers::NONE, Key::Tab)
            {
                Some(Act::Commit)
            } else {
                None
            }
        });
        if let Some(a) = act {
            acts.push(a);
            return;
        }
    }
    let resp = ui.add(
        TextEdit::singleline(&mut e.text)
            .id(id)
            .desired_width(f32::INFINITY)
            .margin(vec2(2.0, 0.0)),
    );
    if !e.focused {
        resp.request_focus();
        let mut st = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        let end = CCursor::new(e.text.chars().count());
        st.cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
        st.store(ui.ctx(), id);
        e.focused = true;
    } else if resp.lost_focus() {
        acts.push(Act::Commit);
    }
}

// ----- dialogs -----

fn type_dialog(var: usize, v: &Variable) -> TypeDialog {
    let (string, chars) = match v.var_type {
        VarType::String { width } => (true, width),
        VarType::Numeric => (false, v.width.max(8)),
    };
    TypeDialog {
        var,
        string,
        width: if string {
            "8".into()
        } else {
            v.width.to_string()
        },
        decimals: if string {
            "2".into()
        } else {
            v.decimals.to_string()
        },
        chars: chars.to_string(),
    }
}

fn values_dialog(var: usize, v: &Variable) -> ValuesDialog {
    let numeric = v.var_type == VarType::Numeric;
    let list = if numeric {
        v.value_labels
            .iter()
            .map(|(k, l)| (LabelKey::Num(k.0), l.clone()))
            .collect()
    } else {
        v.string_value_labels
            .iter()
            .map(|(k, l)| (LabelKey::Text(k.clone()), l.clone()))
            .collect()
    };
    ValuesDialog {
        var,
        numeric,
        decimals: v.decimals,
        value: String::new(),
        label: String::new(),
        list,
        selected: None,
    }
}

fn missing_dialog(var: usize, v: &Variable) -> MissingDialog {
    let f = |x: f64| format_number(x, v.decimals);
    let mut d = MissingDialog {
        var,
        numeric: v.var_type == VarType::Numeric,
        mode: MissingMode::None,
        discrete: Default::default(),
        low: String::new(),
        high: String::new(),
        extra: String::new(),
    };
    if d.numeric {
        match &v.missing {
            MissingSpec::None => {}
            MissingSpec::Discrete(l) => {
                d.mode = MissingMode::Discrete;
                for (slot, x) in d.discrete.iter_mut().zip(l) {
                    *slot = f(*x);
                }
            }
            MissingSpec::Range { lo, hi, discrete } => {
                d.mode = MissingMode::Range;
                d.low = f(*lo);
                d.high = f(*hi);
                d.extra = discrete.map(f).unwrap_or_default();
            }
        }
    } else if !v.string_missing.is_empty() {
        d.mode = MissingMode::Discrete;
        for (slot, s) in d.discrete.iter_mut().zip(&v.string_missing) {
            *slot = s.clone();
        }
    }
    d
}

fn label_key_text(k: &LabelKey, decimals: usize) -> String {
    match k {
        LabelKey::Num(x) => format_number(*x, decimals),
        LabelKey::Text(s) => s.clone(),
    }
}

/// A single-line text field with an accessible name.
fn field(ui: &mut Ui, name: &str, text: &mut String, width: f32) -> egui::Response {
    let r = ui.add(TextEdit::singleline(text).desired_width(width));
    let n = name.to_string();
    ui.ctx().accesskit_node_builder(r.id, |b| b.set_label(n));
    r
}

/// A labelled field on one row: `label` shown left of the field and used as its name.
fn labelled_field(ui: &mut Ui, label: &str, text: &mut String, width: f32) {
    ui.horizontal(|ui| {
        ui.add_sized([110.0, 18.0], Label::new(label));
        field(ui, label, text, width);
    });
}

/// OK / Cancel / Help row. Returns `Some(true)` for OK, `Some(false)` for Cancel.
fn ok_cancel(ui: &mut Ui) -> Option<bool> {
    let mut out = None;
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            out = Some(true);
        }
        if ui.button("Cancel").clicked() {
            out = Some(false);
        }
        ui.add_enabled(false, egui::Button::new("Help"));
    });
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        out = Some(false);
    }
    out
}

fn show_dialog(ui: &mut Ui, state: &mut AppState) {
    let ctx = ui.ctx().clone();
    let Some(mut d) = state.editor.var_dialog.take() else {
        return;
    };
    let name = state
        .doc
        .ds()
        .try_var(VarId(dialog_var(&d)))
        .map(|v| v.name.clone());
    let Some(name) = name else {
        return; // the variable is gone
    };
    let mut result = None;
    egui::Modal::new(Id::new("variable_view_dialog")).show(&ctx, |ui| {
        ui.set_min_width(340.0);
        match &mut d {
            VarDialog::Type(t) => {
                ui.strong("Variable Type");
                ui.label(format!("Variable: {name}"));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.radio_value(&mut t.string, false, "Numeric");
                        for l in [
                            "Comma",
                            "Dot",
                            "Scientific notation",
                            "Date",
                            "Dollar",
                            "Custom currency",
                        ] {
                            ui.add_enabled(false, egui::RadioButton::new(false, l));
                        }
                        ui.radio_value(&mut t.string, true, "String");
                        ui.add_enabled(
                            false,
                            egui::RadioButton::new(
                                false,
                                "Restricted Numeric (integer with leading zeros)",
                            ),
                        );
                    });
                    ui.vertical(|ui| {
                        if t.string {
                            labelled_field(ui, "Characters:", &mut t.chars, 50.0);
                        } else {
                            labelled_field(ui, "Width:", &mut t.width, 50.0);
                            labelled_field(ui, "Decimal Places:", &mut t.decimals, 50.0);
                        }
                    });
                });
            }
            VarDialog::Values(v) => {
                ui.strong("Value Labels");
                ui.label(format!("Variable: {name}"));
                ui.separator();
                labelled_field(ui, "Value:", &mut v.value, 120.0);
                labelled_field(ui, "Label:", &mut v.label, 200.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if ui.button("Add").clicked() {
                            values_add(state, v);
                        }
                        if ui
                            .add_enabled(v.selected.is_some(), egui::Button::new("Change"))
                            .clicked()
                        {
                            values_change(state, v);
                        }
                        if ui
                            .add_enabled(v.selected.is_some(), egui::Button::new("Remove"))
                            .clicked()
                            && let Some(s) = v.selected.take()
                        {
                            v.list.remove(s);
                        }
                    });
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_min_size(vec2(220.0, 110.0));
                        egui::ScrollArea::vertical()
                            .max_height(110.0)
                            .show(ui, |ui| {
                                let mut pick = None;
                                for (k, (key, label)) in v.list.iter().enumerate() {
                                    let text = format!(
                                        "{} = \"{label}\"",
                                        label_key_text(key, v.decimals)
                                    );
                                    if ui.selectable_label(v.selected == Some(k), text).clicked() {
                                        pick = Some(k);
                                    }
                                }
                                if let Some(k) = pick {
                                    v.selected = Some(k);
                                    v.value = label_key_text(&v.list[k].0, v.decimals);
                                    v.label = v.list[k].1.clone();
                                }
                            });
                    });
                });
            }
            VarDialog::Missing(m) => {
                ui.strong("Missing Values");
                ui.label(format!("Variable: {name}"));
                ui.separator();
                ui.radio_value(&mut m.mode, MissingMode::None, "No missing values");
                ui.radio_value(
                    &mut m.mode,
                    MissingMode::Discrete,
                    "Discrete missing values",
                );
                ui.add_enabled_ui(m.mode == MissingMode::Discrete, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        for (k, s) in m.discrete.iter_mut().enumerate() {
                            field(ui, &format!("Discrete value {}", k + 1), s, 60.0);
                        }
                    });
                });
                let numeric = m.numeric;
                ui.add_enabled_ui(numeric, |ui| {
                    ui.radio_value(
                        &mut m.mode,
                        MissingMode::Range,
                        "Range plus one optional discrete missing value",
                    );
                });
                ui.add_enabled_ui(numeric && m.mode == MissingMode::Range, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label("Low:");
                        field(ui, "Low:", &mut m.low, 60.0);
                        ui.label("High:");
                        field(ui, "High:", &mut m.high, 60.0);
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label("Discrete value:");
                        field(ui, "Discrete value:", &mut m.extra, 60.0);
                    });
                });
            }
        }
        result = ok_cancel(ui);
    });
    match result {
        Some(true) => {
            if !dialog_ok(state, &d) {
                state.editor.var_dialog = Some(d); // invalid input: keep the dialog open
            }
        }
        Some(false) => {}
        None => state.editor.var_dialog = Some(d),
    }
}

/// Add the Value / Label fields to the list (a value already in the list gets the new label).
/// False if the input was rejected (a message is queued).
fn values_add(state: &mut AppState, v: &mut ValuesDialog) -> bool {
    let label = v.label.trim().to_string();
    let key = if v.numeric {
        match parse_number(&v.value) {
            Some(x) => LabelKey::Num(x),
            None => {
                state.push_message(Message::warning(
                    "Value Labels",
                    format!(
                        "\"{}\" is not a valid value for a numeric variable.",
                        v.value.trim()
                    ),
                ));
                return false;
            }
        }
    } else {
        LabelKey::Text(v.value.trim_end().to_string())
    };
    if label.is_empty() {
        state.push_message(Message::warning(
            "Value Labels",
            "Enter a label for the value.",
        ));
        return false;
    }
    match v.list.iter().position(|(k, _)| *k == key) {
        Some(p) => v.list[p].1 = label,
        None => v.list.push((key, label)),
    }
    v.list.sort_by(|a, b| match (&a.0, &b.0) {
        (LabelKey::Num(x), LabelKey::Num(y)) => x.total_cmp(y),
        (LabelKey::Text(x), LabelKey::Text(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    });
    v.value.clear();
    v.label.clear();
    v.selected = None;
    true
}

/// Change the selected entry to the Value / Label fields. Changing it to a value that already
/// has a label replaces that label and drops the selected entry (as SPSS does).
fn values_change(state: &mut AppState, v: &mut ValuesDialog) {
    if let Some(s) = v.selected.take() {
        let old = v.list.remove(s);
        if !values_add(state, v) {
            v.list.insert(s, old); // rejected: put it back
            v.selected = Some(s);
        }
    }
}

/// Apply a dialog. False if its input is invalid (a message is queued).
fn dialog_ok(state: &mut AppState, d: &VarDialog) -> bool {
    let bad = |state: &mut AppState, title: &str, text: String| {
        state.push_message(Message::warning(title, text));
        false
    };
    match d {
        VarDialog::Type(t) => {
            if t.string {
                match t.chars.trim().parse::<usize>() {
                    Ok(w @ 1..=32767) => {
                        state.set_variable_type(t.var, VarType::String { width: w }, w, 0);
                        true
                    }
                    _ => bad(
                        state,
                        "Variable Type",
                        "Characters must be a whole number from 1 to 32767.".into(),
                    ),
                }
            } else {
                match (t.width.trim().parse::<usize>(), t.decimals.trim().parse::<usize>()) {
                    (Ok(w @ 1..=40), Ok(dec @ 0..=16)) if dec < w || (w == 1 && dec == 0) => {
                        state.set_variable_type(t.var, VarType::Numeric, w, dec);
                        true
                    }
                    _ => bad(
                        state,
                        "Variable Type",
                        "Width must be from 1 to 40 and Decimal Places from 0 to 16, smaller than the width.".into(),
                    ),
                }
            }
        }
        VarDialog::Values(v) => {
            if v.numeric {
                let map = v
                    .list
                    .iter()
                    .filter_map(|(k, l)| match k {
                        LabelKey::Num(x) => Some((OrderedValue::new(*x), l.clone())),
                        LabelKey::Text(_) => None,
                    })
                    .collect();
                state.change_variable(v.var, |var| var.value_labels = map);
            } else {
                let map = v
                    .list
                    .iter()
                    .filter_map(|(k, l)| match k {
                        LabelKey::Text(s) => Some((s.clone(), l.clone())),
                        LabelKey::Num(_) => None,
                    })
                    .collect();
                state.change_variable(v.var, |var| var.string_value_labels = map);
            }
            true
        }
        VarDialog::Missing(m) => {
            let filled: Vec<&str> = m
                .discrete
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if !m.numeric {
                let strings: Vec<String> = match m.mode {
                    MissingMode::Discrete => filled.iter().map(|s| s.to_string()).collect(),
                    _ => Vec::new(),
                };
                if strings.iter().any(|s| s.len() > 8) {
                    return bad(
                        state,
                        "Missing Values",
                        "Missing values of a string variable can be at most 8 bytes long.".into(),
                    );
                }
                state.set_missing(m.var, MissingSpec::None, strings);
                return true;
            }
            let num = |s: &str| parse_number(s);
            let spec = match m.mode {
                MissingMode::None => MissingSpec::None,
                MissingMode::Discrete => {
                    let mut vals = Vec::new();
                    for s in &filled {
                        match num(s) {
                            Some(x) => vals.push(x),
                            None => {
                                return bad(
                                    state,
                                    "Missing Values",
                                    format!("\"{s}\" is not a valid number."),
                                );
                            }
                        }
                    }
                    if vals.is_empty() {
                        MissingSpec::None
                    } else {
                        MissingSpec::Discrete(vals)
                    }
                }
                MissingMode::Range => {
                    let (Some(lo), Some(hi)) = (num(&m.low), num(&m.high)) else {
                        return bad(
                            state,
                            "Missing Values",
                            "Enter numbers for Low and High.".into(),
                        );
                    };
                    if lo > hi {
                        return bad(
                            state,
                            "Missing Values",
                            "Low must not be greater than High.".into(),
                        );
                    }
                    let extra = m.extra.trim();
                    let discrete = if extra.is_empty() {
                        None
                    } else {
                        match num(extra) {
                            Some(x) => Some(x),
                            None => {
                                return bad(
                                    state,
                                    "Missing Values",
                                    format!("\"{extra}\" is not a valid number."),
                                );
                            }
                        }
                    };
                    MissingSpec::Range { lo, hi, discrete }
                }
            };
            let strings = state
                .doc
                .ds()
                .try_var(VarId(m.var))
                .map(|v| v.string_missing.clone())
                .unwrap_or_default();
            state.set_missing(m.var, spec, strings);
            true
        }
    }
}
