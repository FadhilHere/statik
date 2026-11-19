//! Transform ▸ Compute Variable..., with the Type & Label... and If... sub-dialogs.
//!
//! Layout as in SPSS: Target Variable and "Type & Label..." above the variable list on the
//! left, "=" and the Numeric Expression box on the right, the calculator keypad, Function
//! group / Functions and Special Variables lists with a description of the selected function,
//! and "If..." with the condition below.
//!
//! - Double-clicking a variable (or ▶) inserts its name at the cursor; double-clicking a
//!   function (or ▲) inserts its template, e.g. `ABS(?)`, with the `?` selected.
//! - OK first parses the expression (and the condition) here: a syntax error is reported with
//!   its character position and the cursor is put there. An existing target asks "Change
//!   existing variable?" (Yes/No). The computation runs on the worker thread; it can be undone.
use super::expr::{ExprBox, char_index, keypad, var_list};
use super::frame::{group, radios_off};
use super::var_picker::{arrow_button, list_frame, var_row};
use super::{DialogAction, ProcDialog, RunSpec};
use crate::edit::EditOp;
use crate::state::{LOG_TITLE, ProcOutput};
use dataset::{Dataset, VarType, validate_name};
use egui::{Button, Response, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use transform::{ComputeSpec, compute, parse};

/// Sub-dialog button labels.
pub const TYPE_LABEL: &str = "Type & Label...";
pub const IF: &str = "If...";

/// Question asked before an existing variable is overwritten.
pub const CHANGE_EXISTING: &str = "Change existing variable?";

/// One entry of "Functions and Special Variables".
pub struct FuncInfo {
    /// As listed (SPSS capitalisation), e.g. "Abs".
    pub name: &'static str,
    /// Inserted text; the first `?` is selected.
    pub template: &'static str,
    pub group: &'static str,
    pub help: &'static str,
}

/// Function groups; "All" lists every function alphabetically.
pub const GROUPS: [&str; 5] = [
    "All",
    "Arithmetic",
    "Statistical",
    "Missing Values",
    "Miscellaneous",
];

pub const FUNCS: [FuncInfo; 18] = [
    FuncInfo {
        name: "Abs",
        template: "ABS(?)",
        group: "Arithmetic",
        help: "ABS(numexpr). Numeric. Returns the absolute value of numexpr, which must be numeric.",
    },
    FuncInfo {
        name: "Exp",
        template: "EXP(?)",
        group: "Arithmetic",
        help: "EXP(numexpr). Numeric. Returns e raised to the power numexpr, where e is the base of the natural logarithms.",
    },
    FuncInfo {
        name: "Lg10",
        template: "LG10(?)",
        group: "Arithmetic",
        help: "LG10(numexpr). Numeric. Returns the base-10 logarithm of numexpr, which must be numeric and greater than 0.",
    },
    FuncInfo {
        name: "Ln",
        template: "LN(?)",
        group: "Arithmetic",
        help: "LN(numexpr). Numeric. Returns the base-e (natural) logarithm of numexpr, which must be numeric and greater than 0.",
    },
    FuncInfo {
        name: "Mod",
        template: "MOD(?,?)",
        group: "Arithmetic",
        help: "MOD(numexpr, modulus). Numeric. Returns the remainder when numexpr is divided by modulus. Both arguments must be numeric, and modulus must not be 0.",
    },
    FuncInfo {
        name: "Rnd",
        template: "RND(?)",
        group: "Arithmetic",
        help: "RND(numexpr[, mult]). Numeric. Returns numexpr rounded to the nearest integer, or to the nearest multiple of mult. Halves are rounded away from 0.",
    },
    FuncInfo {
        name: "Sqrt",
        template: "SQRT(?)",
        group: "Arithmetic",
        help: "SQRT(numexpr). Numeric. Returns the positive square root of numexpr, which must be numeric and not negative.",
    },
    FuncInfo {
        name: "Trunc",
        template: "TRUNC(?)",
        group: "Arithmetic",
        help: "TRUNC(numexpr[, mult]). Numeric. Returns numexpr truncated toward 0 to an integer, or to a multiple of mult.",
    },
    FuncInfo {
        name: "Max",
        template: "MAX(?,?)",
        group: "Statistical",
        help: "MAX(value, value[, ...]). Numeric. Returns the largest valid value of its arguments.",
    },
    FuncInfo {
        name: "Mean",
        template: "MEAN(?,?)",
        group: "Statistical",
        help: "MEAN(numexpr, numexpr[, ...]). Numeric. Returns the arithmetic mean of its arguments that have valid, nonmissing values. MEAN.n requires at least n valid values.",
    },
    FuncInfo {
        name: "Min",
        template: "MIN(?,?)",
        group: "Statistical",
        help: "MIN(value, value[, ...]). Numeric. Returns the smallest valid value of its arguments.",
    },
    FuncInfo {
        name: "Sd",
        template: "SD(?,?)",
        group: "Statistical",
        help: "SD(numexpr, numexpr[, ...]). Numeric. Returns the sample standard deviation of its arguments that have valid values (at least two).",
    },
    FuncInfo {
        name: "Sum",
        template: "SUM(?,?)",
        group: "Statistical",
        help: "SUM(numexpr, numexpr[, ...]). Numeric. Returns the sum of its arguments that have valid, nonmissing values.",
    },
    FuncInfo {
        name: "Variance",
        template: "VARIANCE(?,?)",
        group: "Statistical",
        help: "VARIANCE(numexpr, numexpr[, ...]). Numeric. Returns the sample variance of its arguments that have valid values (at least two).",
    },
    FuncInfo {
        name: "Missing",
        template: "MISSING(?)",
        group: "Missing Values",
        help: "MISSING(variable). Logical. Returns 1 or true if variable has a system-missing or user-missing value.",
    },
    FuncInfo {
        name: "Nvalid",
        template: "NVALID(?,?)",
        group: "Missing Values",
        help: "NVALID(variable[, ...]). Numeric. Returns a count of the arguments that have valid, nonmissing values.",
    },
    FuncInfo {
        name: "Sysmis",
        template: "SYSMIS(?)",
        group: "Missing Values",
        help: "SYSMIS(numvar). Logical. Returns 1 or true if the value of numvar is system-missing.",
    },
    FuncInfo {
        name: "Lag",
        template: "LAG(?)",
        group: "Miscellaneous",
        help: "LAG(variable). Numeric. Returns the value of variable for the previous case. For the first case it returns system-missing.",
    },
];

#[derive(Clone, Debug, Default)]
pub struct ComputeDialog {
    pub target: String,
    pub expr: ExprBox,
    /// Type & Label: "Label:" text, or "Use expression as label".
    pub label: String,
    pub use_expr_label: bool,
    /// If: "Include if case satisfies condition".
    pub use_if: bool,
    pub cond: ExprBox,
    /// Selected function group (index into [`GROUPS`]) and function name.
    pub group: usize,
    pub func: Option<&'static str>,
    /// Selected variable in the main list / in the If list.
    pub var_sel: Option<String>,
    pub if_var_sel: Option<String>,
}

/// Expression text on one line (line breaks typed in the box become spaces).
fn one_line(s: &str) -> String {
    s.replace("\r\n", " ")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

/// A syntax error found before running: the message and where to put the cursor.
struct CheckError {
    message: String,
    /// (in the condition, character index)
    at: Option<(bool, usize)>,
}

impl From<String> for CheckError {
    fn from(message: String) -> Self {
        CheckError { message, at: None }
    }
}

/// Parse `text`; a syntax error names its character position (1-based).
fn parse_at(text: &str, what: &str, in_cond: bool) -> Result<(), CheckError> {
    parse(text).map(|_| ()).map_err(|e| {
        let i = char_index(text, e.pos);
        CheckError {
            message: format!(
                "The {what} has a syntax error at character {}:\n{}.",
                i + 1,
                e.message
            ),
            at: Some((in_cond, i)),
        }
    })
}

impl ComputeDialog {
    /// Functions of the selected group, in list order.
    pub fn functions(&self) -> Vec<&'static FuncInfo> {
        let g = GROUPS.get(self.group).copied().unwrap_or("All");
        let mut f: Vec<&FuncInfo> = FUNCS
            .iter()
            .filter(|f| g == "All" || f.group == g)
            .collect();
        f.sort_by_key(|f| f.name);
        f
    }

    /// Description of the selected function (empty when none is selected).
    pub fn func_help(&self) -> String {
        self.func
            .and_then(|n| FUNCS.iter().find(|f| f.name == n))
            .map(|f| f.help.to_string())
            .unwrap_or_default()
    }

    /// The variable label to set: the expression, the typed label, or none (an existing
    /// target keeps its label).
    fn label_value(&self) -> Option<String> {
        if self.use_expr_label {
            Some(one_line(&self.expr.text))
        } else {
            let l = self.label.trim();
            (!l.is_empty()).then(|| l.to_string())
        }
    }

    /// The target as it is spelled in the data (an existing variable), else as typed.
    fn target_name(&self, ds: &Dataset) -> String {
        let t = self.target.trim();
        ds.find(t)
            .map(|id| ds.var(id).name.clone())
            .unwrap_or_else(|| t.to_string())
    }

    /// Everything that can be checked before running. `Ok(true)` = the target exists.
    fn check_inner(&self, ds: &Dataset) -> Result<bool, CheckError> {
        let t = self.target.trim();
        if t.is_empty() {
            return Err("Enter a target variable name.".to_string().into());
        }
        if validate_name(t).is_err() {
            return Err(format!(
                "The target variable name {t} is invalid. A name must begin with a letter, may \
                 contain letters, digits and _ . $ # @, may not be longer than 64 bytes and may \
                 not be a reserved word (ALL, AND, BY, EQ, GE, GT, LE, LT, NE, NOT, OR, TO, WITH)."
            )
            .into());
        }
        if self.expr.text.trim().is_empty() {
            return Err("Enter a numeric expression.".to_string().into());
        }
        parse_at(&self.expr.text, "numeric expression", false)?;
        if self.use_if {
            if self.cond.text.trim().is_empty() {
                return Err("Enter a condition in If..., or choose Include all cases."
                    .to_string()
                    .into());
            }
            parse_at(&self.cond.text, "If condition", true)?;
        }
        match ds.find(t) {
            Some(id) if matches!(ds.var(id).var_type, VarType::String { .. }) => Err(format!(
                "The target variable {} is a string variable. Only numeric targets can be \
                 computed.",
                ds.var(id).name
            )
            .into()),
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    /// Check before OK: on a syntax error the cursor is put at the error. `Ok(true)` = the
    /// target exists (ask before changing it). `Err((message, in_condition))`: the error is in
    /// the If condition when `in_condition` (the caller opens the If sub-dialog).
    pub fn check(&mut self, ds: &Dataset) -> Result<bool, (String, bool)> {
        self.check_inner(ds).map_err(|e| {
            let in_cond = match e.at {
                Some((false, i)) => {
                    self.expr.select(i, i);
                    false
                }
                Some((true, i)) => {
                    self.cond.select(i, i);
                    true
                }
                None => false,
            };
            (e.message, in_cond)
        })
    }

    fn spec(&self) -> ComputeSpec {
        ComputeSpec {
            target: self.target.trim().to_string(),
            expression: self.expr.text.clone(),
            condition: self.use_if.then(|| self.cond.text.clone()),
            label: self.label_value(),
        }
    }
}

/// Run Compute and build the undo op: a new variable is removed again; an existing one gets
/// its old values (and label) back.
fn run_compute(ds: &mut Dataset, spec: &ComputeSpec) -> ProcOutput {
    let before = ds
        .find(spec.target.trim())
        .map(|id| (ds.var(id).clone(), ds.column(id).clone()));
    let r = match compute(ds, spec) {
        Ok(r) => r,
        Err(e) => return ProcOutput::failed(format!("Compute Variable: {e}.")),
    };
    let at = r.target.0;
    let var = ds.var(r.target).clone();
    let col = ds.column(r.target).clone();
    let op = match before {
        None => EditOp::InsertVar {
            at,
            var: Box::new(var),
            col,
        },
        Some((old_var, old_col)) => {
            let mut ops = vec![EditOp::SetBlock {
                row: 0,
                col: at,
                old: vec![old_col],
                new: vec![col],
            }];
            if old_var.label != var.label {
                ops.push(EditOp::ChangeVarProp {
                    at,
                    old: Box::new(old_var),
                    new: Box::new(var),
                    cols: None,
                    weight: false,
                });
            }
            if ops.len() == 1 {
                ops.pop().expect("one op")
            } else {
                EditOp::Group(ops)
            }
        }
    };
    ProcOutput::undoable(Vec::new(), op)
}

/// A small button with a painted ▲ (insert the selected function).
fn up_button(ui: &mut Ui, enabled: bool, name: &str) -> bool {
    let resp = ui.add_enabled(enabled, Button::new("").min_size(vec2(26.0, 20.0)));
    let r = resp.rect.shrink2(vec2(8.0, 6.0));
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            r.left_bottom(),
            r.right_bottom(),
            pos2(r.center().x, r.top()),
        ],
        color,
        Stroke::NONE,
    ));
    let n = name.to_string();
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &n));
    resp.clicked()
}

/// One row of a plain text list.
fn text_row(ui: &mut Ui, text: &str, selected: bool) -> Response {
    var_row(ui, None, text, selected, true)
}

impl ProcDialog for ComputeDialog {
    fn title(&self) -> &str {
        "Compute Variable"
    }

    fn size(&self) -> egui::Vec2 {
        vec2(720.0, 520.0)
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            // Left: target, Type & Label..., variables.
            ui.vertical(|ui| {
                ui.set_width(200.0);
                let l = ui.label("Target Variable:");
                ui.add(egui::TextEdit::singleline(&mut self.target).desired_width(190.0))
                    .labelled_by(l.id);
                if ui.button(TYPE_LABEL).clicked() {
                    a = DialogAction::Sub(TYPE_LABEL);
                }
                ui.add_space(4.0);
                if let Some(n) = var_list(
                    ui,
                    ds,
                    "compute_vars",
                    vec2(200.0, 250.0),
                    &mut self.var_sel,
                ) {
                    self.expr.insert(&n, None);
                }
            });
            // Middle: "=" and the insert-variable arrow.
            ui.vertical(|ui| {
                ui.add_space(18.0);
                ui.label(egui::RichText::new("=").strong());
                ui.add_space(110.0);
                let sel = self.var_sel.clone().filter(|n| ds.find(n).is_some());
                if arrow_button(ui, true, sel.is_some(), "Insert variable")
                    && let Some(n) = sel
                {
                    self.expr.insert(&n, None);
                }
            });
            // Right: expression, keypad, functions.
            ui.vertical(|ui| {
                let l = ui.label("Numeric Expression:");
                self.expr.show(ui, "compute_expr", 4, 420.0, l.id);
                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(200.0);
                        keypad(ui, &mut self.expr);
                        ui.add_space(6.0);
                        let help = self.func_help();
                        ui.add(egui::Label::new(egui::RichText::new(help).small()).wrap());
                    });
                    ui.vertical(|ui| {
                        ui.set_width(210.0);
                        ui.label("Function group:");
                        list_frame(ui, "compute_groups", 5.0 * 18.0 + 2.0, |ui| {
                            for (i, g) in GROUPS.iter().enumerate() {
                                if text_row(ui, g, self.group == i).clicked() {
                                    self.group = i;
                                }
                            }
                        });
                        let funcs = self.functions();
                        let sel = self.func.and_then(|n| funcs.iter().find(|f| f.name == n));
                        if up_button(ui, sel.is_some(), "Insert function")
                            && let Some(f) = sel
                        {
                            self.expr.insert_template(f.template);
                        }
                        ui.label("Functions and Special Variables:");
                        let mut insert = None;
                        list_frame(ui, "compute_funcs", 7.0 * 18.0 + 2.0, |ui| {
                            for f in &funcs {
                                let r = text_row(ui, f.name, self.func == Some(f.name));
                                if r.double_clicked() {
                                    self.func = Some(f.name);
                                    insert = Some(f.template);
                                } else if r.clicked() {
                                    self.func = Some(f.name);
                                }
                            }
                        });
                        if let Some(t) = insert {
                            self.expr.insert_template(t);
                        }
                    });
                });
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(IF).clicked() {
                a = DialogAction::Sub(IF);
            }
            let text = if self.use_if {
                one_line(&self.cond.text)
            } else {
                "(optional case selection condition)".to_string()
            };
            ui.label(text);
        });
        a
    }

    fn ready(&self) -> bool {
        !self.target.trim().is_empty()
            && !self.expr.text.trim().is_empty()
            && (!self.use_if || !self.cond.text.trim().is_empty())
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        self.check_inner(ds).map_err(|e| e.message)?;
        let spec = self.spec();
        Ok(RunSpec {
            title: LOG_TITLE,
            run: Box::new(move |ds: &mut Dataset| run_compute(ds, &spec)),
        })
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let t = self.target_name(ds);
        let expr = one_line(&self.expr.text);
        let mut lines = vec![if self.use_if {
            format!("IF ({}) {t}={expr}.", one_line(&self.cond.text))
        } else {
            format!("COMPUTE {t}={expr}.")
        }];
        if let Some(l) = self.label_value() {
            lines.push(format!("VARIABLE LABELS {t} '{}'.", l.replace('\'', "''")));
        }
        lines.push("EXECUTE.".into());
        lines.join("\n")
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Compute Variable computes values for a numeric variable from a numeric expression, \
         for all cases or only for the cases that satisfy a condition (If...). Type a new \
         variable name as the target to create a variable, or the name of an existing numeric \
         variable to replace its values. Double-click a variable or a function to insert it into \
         the expression; system-missing results are left empty. Edit ▸ Undo reverses it."
    }

    fn sub_title(&self, name: &str) -> String {
        match name {
            TYPE_LABEL => "Type and Label".into(),
            IF => "If Cases".into(),
            other => other.trim_end_matches("...").into(),
        }
    }

    fn sub_ui_ds(&mut self, ui: &mut Ui, name: &str, ds: &Dataset) {
        match name {
            TYPE_LABEL => {
                group(ui, "Label", |ui| {
                    ui.horizontal(|ui| {
                        let r = ui.radio(!self.use_expr_label, "Label:");
                        if r.clicked() {
                            self.use_expr_label = false;
                        }
                        let t = ui
                            .add(egui::TextEdit::singleline(&mut self.label).desired_width(220.0))
                            .labelled_by(r.id);
                        if t.gained_focus() || t.changed() {
                            self.use_expr_label = false;
                        }
                    });
                    if ui
                        .radio(self.use_expr_label, "Use expression as label")
                        .clicked()
                    {
                        self.use_expr_label = true;
                    }
                });
                group(ui, "Type", |ui| {
                    radios_off(ui, &[("Numeric", true), ("String", false)]);
                    ui.add_enabled_ui(false, |ui| {
                        let mut w = "8".to_string();
                        super::frame::labeled_text(ui, "Width:", &mut w, 40.0);
                    });
                });
            }
            IF => {
                ui.horizontal_top(|ui| {
                    if let Some(n) = var_list(
                        ui,
                        ds,
                        "compute_if_vars",
                        vec2(180.0, 240.0),
                        &mut self.if_var_sel,
                    ) && self.use_if
                    {
                        self.cond.insert(&n, None);
                    }
                    ui.vertical(|ui| {
                        ui.add_space(100.0);
                        let sel = self.if_var_sel.clone().filter(|n| ds.find(n).is_some());
                        if arrow_button(
                            ui,
                            true,
                            self.use_if && sel.is_some(),
                            "Insert variable into condition",
                        ) && let Some(n) = sel
                        {
                            self.cond.insert(&n, None);
                        }
                    });
                    ui.vertical(|ui| {
                        if ui.radio(!self.use_if, "Include all cases").clicked() {
                            self.use_if = false;
                        }
                        let r = ui.radio(self.use_if, "Include if case satisfies condition:");
                        if r.clicked() {
                            self.use_if = true;
                        }
                        ui.add_enabled_ui(self.use_if, |ui| {
                            self.cond.show(ui, "compute_cond", 4, 340.0, r.id);
                            ui.add_space(4.0);
                            keypad(ui, &mut self.cond);
                        });
                    });
                });
            }
            _ => {}
        }
    }

    fn sub_continue(&mut self, name: &str) -> Result<(), String> {
        if name != IF || !self.use_if {
            return Ok(());
        }
        if self.cond.text.trim().is_empty() {
            return Err("Enter a condition, or choose Include all cases.".into());
        }
        parse_at(&self.cond.text, "condition", true).map_err(|e| {
            if let Some((_, i)) = e.at {
                self.cond.select(i, i);
            }
            e.message
        })
    }
}
