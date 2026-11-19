//! Transform ▸ Recode into Different Variables..., with the Old and New Values... sub-dialog.
//!
//! As in SPSS: numeric variables move into the "Numeric Variable -> Output Variable" list
//! (shown as `x --> x_cat`, or `x --> ?` until named); the Output Variable Name/Label apply to
//! the selected input with Change. Old and New Values... holds the rules (Old --> New list with
//! Add / Change / Remove); OK is enabled once every input has an output name and there is at
//! least one rule. The new variables are created on the worker thread and can be undone.
use super::frame::{group, named_text};
use super::var_picker::{Eligible, Side, Target, VarPicker, list_frame, var_row};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::edit::{EditOp, parse_number};
use crate::state::{LOG_TITLE, ProcOutput};
use crate::syntax::value;
use dataset::{Dataset, validate_name};
use egui::{Button, Ui, vec2};
use transform::{NewValue, OldValue, RecodeSpec, recode};

/// The sub-dialog button.
pub const OLD_NEW: &str = "Old and New Values...";

/// Output variable of one input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutSpec {
    pub input: String,
    pub name: String,
    pub label: String,
}

impl OutSpec {
    pub fn new(input: &str, name: &str, label: &str) -> Self {
        OutSpec {
            input: input.into(),
            name: name.into(),
            label: label.into(),
        }
    }
}

/// Old Value choice in the sub-dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OldKind {
    #[default]
    Value,
    SysMis,
    Missing,
    Range,
    Lowest,
    Highest,
    Else,
}

/// New Value choice in the sub-dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NewKind {
    #[default]
    Value,
    SysMis,
    Copy,
}

#[derive(Clone, Debug)]
pub struct RecodeDialog {
    pub picker: VarPicker,
    pub outputs: Vec<OutSpec>,
    /// The Output Variable Name / Label fields, and the input they were loaded for.
    pub name_edit: String,
    pub label_edit: String,
    edit_for: Option<String>,
    pub rules: Vec<(OldValue, NewValue)>,
    // Old and New Values sub-dialog fields.
    pub old_kind: OldKind,
    pub old_value: String,
    pub range_lo: String,
    pub range_hi: String,
    pub lowest_hi: String,
    pub highest_lo: String,
    pub new_kind: NewKind,
    pub new_value: String,
    pub rule_sel: Option<usize>,
}

impl Default for RecodeDialog {
    fn default() -> Self {
        RecodeDialog {
            picker: VarPicker::new(vec![Target::list(
                "Numeric Variable -> Output Variable:",
                10,
            )]),
            outputs: Vec::new(),
            name_edit: String::new(),
            label_edit: String::new(),
            edit_for: None,
            rules: Vec::new(),
            old_kind: OldKind::Value,
            old_value: String::new(),
            range_lo: String::new(),
            range_hi: String::new(),
            lowest_hi: String::new(),
            highest_lo: String::new(),
            new_kind: NewKind::Value,
            new_value: String::new(),
            rule_sel: None,
        }
    }
}

/// Left side of a rule as the dialog lists it.
fn old_text(o: &OldValue) -> String {
    match *o {
        OldValue::Value(v) => value(v),
        OldValue::Range(a, b) => format!("{} thru {}", value(a), value(b)),
        OldValue::Lowest(v) => format!("Lowest thru {}", value(v)),
        OldValue::Highest(v) => format!("{} thru Highest", value(v)),
        OldValue::SysMis => "SYSMIS".into(),
        OldValue::Missing => "MISSING".into(),
        OldValue::Else => "ELSE".into(),
    }
}

fn new_text(n: &NewValue) -> String {
    match *n {
        NewValue::Value(v) => value(v),
        NewValue::SysMis => "SYSMIS".into(),
        NewValue::Copy => "Copy".into(),
    }
}

/// "1 thru 3 --> 1", as in the Old --> New list.
pub fn rule_text(r: &(OldValue, NewValue)) -> String {
    format!("{} --> {}", old_text(&r.0), new_text(&r.1))
}

/// "(1 thru 3=1)", as in RECODE syntax.
fn rule_syntax(r: &(OldValue, NewValue)) -> String {
    let old = match r.0 {
        OldValue::Value(v) => value(v),
        OldValue::Range(a, b) => format!("{} thru {}", value(a), value(b)),
        OldValue::Lowest(v) => format!("LOWEST thru {}", value(v)),
        OldValue::Highest(v) => format!("{} thru HIGHEST", value(v)),
        OldValue::SysMis => "SYSMIS".into(),
        OldValue::Missing => "MISSING".into(),
        OldValue::Else => "ELSE".into(),
    };
    let new = match r.1 {
        NewValue::Value(v) => value(v),
        NewValue::SysMis => "SYSMIS".into(),
        NewValue::Copy => "COPY".into(),
    };
    format!("({old}={new})")
}

fn num(s: &str) -> Option<f64> {
    parse_number(s)
}

impl RecodeDialog {
    fn output(&self, input: &str) -> Option<&OutSpec> {
        self.outputs
            .iter()
            .find(|o| o.input.eq_ignore_ascii_case(input) && !o.name.trim().is_empty())
    }

    /// The input selected in the output list, when exactly one is.
    fn selected_input(&self) -> Option<String> {
        match (self.picker.side, self.picker.selected.as_slice()) {
            (Side::Target(0), [one]) => Some(one.clone()),
            _ => None,
        }
    }

    /// The rule described by the sub-dialog fields, if they are complete and valid.
    pub fn pending_rule(&self) -> Option<(OldValue, NewValue)> {
        let old = match self.old_kind {
            OldKind::Value => OldValue::Value(num(&self.old_value)?),
            OldKind::SysMis => OldValue::SysMis,
            OldKind::Missing => OldValue::Missing,
            OldKind::Range => {
                let (a, b) = (num(&self.range_lo)?, num(&self.range_hi)?);
                if a > b {
                    return None;
                }
                OldValue::Range(a, b)
            }
            OldKind::Lowest => OldValue::Lowest(num(&self.lowest_hi)?),
            OldKind::Highest => OldValue::Highest(num(&self.highest_lo)?),
            OldKind::Else => OldValue::Else,
        };
        let new = match self.new_kind {
            NewKind::Value => NewValue::Value(num(&self.new_value)?),
            NewKind::SysMis => NewValue::SysMis,
            NewKind::Copy => NewValue::Copy,
        };
        Some((old, new))
    }

    /// Add a rule; a rule for the same old value is replaced; ELSE stays last.
    fn add_rule(&mut self, r: (OldValue, NewValue)) {
        if let Some(e) = self.rules.iter_mut().find(|e| e.0 == r.0) {
            e.1 = r.1;
        } else {
            self.rules.push(r);
        }
        self.rules.sort_by_key(|(o, _)| matches!(o, OldValue::Else));
    }

    fn clear_fields(&mut self) {
        for s in [
            &mut self.old_value,
            &mut self.range_lo,
            &mut self.range_hi,
            &mut self.lowest_hi,
            &mut self.highest_lo,
            &mut self.new_value,
        ] {
            s.clear();
        }
    }

    /// Show rule `i` in the fields (clicking it in the list).
    fn load_rule(&mut self, i: usize) {
        let Some(&(old, new)) = self.rules.get(i) else {
            return;
        };
        self.clear_fields();
        match old {
            OldValue::Value(v) => (self.old_kind, self.old_value) = (OldKind::Value, value(v)),
            OldValue::Range(a, b) => {
                self.old_kind = OldKind::Range;
                self.range_lo = value(a);
                self.range_hi = value(b);
            }
            OldValue::Lowest(v) => (self.old_kind, self.lowest_hi) = (OldKind::Lowest, value(v)),
            OldValue::Highest(v) => (self.old_kind, self.highest_lo) = (OldKind::Highest, value(v)),
            OldValue::SysMis => self.old_kind = OldKind::SysMis,
            OldValue::Missing => self.old_kind = OldKind::Missing,
            OldValue::Else => self.old_kind = OldKind::Else,
        }
        match new {
            NewValue::Value(v) => (self.new_kind, self.new_value) = (NewKind::Value, value(v)),
            NewValue::SysMis => self.new_kind = NewKind::SysMis,
            NewValue::Copy => self.new_kind = NewKind::Copy,
        }
    }

    /// Output names in input order (for syntax and the run).
    fn out_names(&self) -> Vec<String> {
        self.picker
            .vars(0)
            .iter()
            .map(|i| {
                self.output(i)
                    .map(|o| o.name.trim().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn output_list(&mut self, ui: &mut Ui, ds: &Dataset) {
        let inputs = self.picker.vars(0).to_vec();
        let mut clicked: Option<(String, bool)> = None;
        list_frame(ui, "recode_outputs", 10.0 * 18.0 + 2.0, |ui| {
            for n in &inputs {
                let var = ds.find(n).map(|id| ds.var(id));
                let name = var.map_or(n.as_str(), |v| v.name.as_str());
                let out = self.output(n).map_or("?", |o| o.name.trim());
                let text = format!("{name} --> {out}");
                let sel = self.picker.side == Side::Target(0)
                    && self
                        .picker
                        .selected
                        .iter()
                        .any(|s| s.eq_ignore_ascii_case(n));
                let r = var_row(ui, var, &text, sel, true);
                if r.double_clicked() {
                    clicked = Some((n.clone(), true));
                } else if r.clicked() {
                    clicked = Some((n.clone(), false));
                }
            }
        });
        if let Some((n, double)) = clicked {
            self.picker.side = Side::Target(0);
            self.picker.selected = vec![n];
            if double {
                self.picker.remove(0);
            }
        }
    }

    fn output_group(&mut self, ui: &mut Ui) {
        let sel = self.selected_input();
        if sel != self.edit_for {
            // Load the fields for the newly selected input (empty for none).
            let o = sel.as_deref().and_then(|s| self.output(s)).cloned();
            self.name_edit = o.as_ref().map(|o| o.name.clone()).unwrap_or_default();
            self.label_edit = o.map(|o| o.label).unwrap_or_default();
            self.edit_for = sel.clone();
        }
        group(ui, "Output Variable", |ui| {
            ui.add_enabled_ui(sel.is_some(), |ui| {
                super::frame::labeled_text(ui, "Name:", &mut self.name_edit, 120.0);
                super::frame::labeled_text(ui, "Label:", &mut self.label_edit, 120.0);
                let name = self.name_edit.trim();
                let valid = validate_name(name).is_ok();
                if !name.is_empty() && !valid {
                    ui.colored_label(ui.visuals().error_fg_color, "Invalid variable name");
                }
                if ui
                    .add_enabled(valid, Button::new("Change").min_size(vec2(70.0, 0.0)))
                    .clicked()
                    && let Some(input) = &sel
                {
                    let spec = OutSpec::new(input, name, self.label_edit.trim());
                    self.outputs
                        .retain(|o| !o.input.eq_ignore_ascii_case(input));
                    self.outputs.push(spec);
                }
            });
        });
    }

    fn old_new_ui(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(280.0);
                group(ui, "Old Value", |ui| {
                    let k = &mut self.old_kind;
                    ui.horizontal(|ui| {
                        ui.radio_value(k, OldKind::Value, "Value:");
                        let r = named_text(ui, &mut self.old_value, 90.0, "Old Value");
                        if r.gained_focus() || r.changed() {
                            *k = OldKind::Value;
                        }
                    });
                    ui.radio_value(k, OldKind::SysMis, "System-missing");
                    ui.radio_value(k, OldKind::Missing, "System- or user-missing");
                    ui.radio_value(k, OldKind::Range, "Range:");
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        let a = named_text(ui, &mut self.range_lo, 70.0, "Range from");
                        ui.label("through");
                        let b = named_text(ui, &mut self.range_hi, 70.0, "Range through");
                        if a.gained_focus() || a.changed() || b.gained_focus() || b.changed() {
                            *k = OldKind::Range;
                        }
                    });
                    ui.radio_value(k, OldKind::Lowest, "Range, LOWEST through value:");
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        let r = named_text(ui, &mut self.lowest_hi, 90.0, "LOWEST through");
                        if r.gained_focus() || r.changed() {
                            *k = OldKind::Lowest;
                        }
                    });
                    ui.radio_value(k, OldKind::Highest, "Range, value through HIGHEST:");
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        let r = named_text(ui, &mut self.highest_lo, 90.0, "through HIGHEST");
                        if r.gained_focus() || r.changed() {
                            *k = OldKind::Highest;
                        }
                    });
                    ui.radio_value(k, OldKind::Else, "All other values");
                });
            });
            ui.vertical(|ui| {
                ui.set_width(300.0);
                group(ui, "New Value", |ui| {
                    let k = &mut self.new_kind;
                    ui.horizontal(|ui| {
                        ui.radio_value(k, NewKind::Value, "Value:");
                        let r = named_text(ui, &mut self.new_value, 90.0, "New Value");
                        if r.gained_focus() || r.changed() {
                            *k = NewKind::Value;
                        }
                    });
                    ui.radio_value(k, NewKind::SysMis, "System-missing");
                    ui.radio_value(k, NewKind::Copy, "Copy old value(s)");
                });
                ui.label("Old --> New:");
                let pending = self.pending_rule();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.add_space(20.0);
                        let w = vec2(64.0, 0.0);
                        if ui
                            .add_enabled(pending.is_some(), Button::new("Add").min_size(w))
                            .clicked()
                            && let Some(r) = pending
                        {
                            self.add_rule(r);
                            self.rule_sel = None;
                            self.clear_fields();
                        }
                        let sel = self.rule_sel.filter(|&i| i < self.rules.len());
                        if ui
                            .add_enabled(
                                pending.is_some() && sel.is_some(),
                                Button::new("Change").min_size(w),
                            )
                            .clicked()
                            && let (Some(r), Some(i)) = (pending, sel)
                        {
                            self.rules.remove(i);
                            self.add_rule(r);
                            self.rule_sel = None;
                            self.clear_fields();
                        }
                        if ui
                            .add_enabled(sel.is_some(), Button::new("Remove").min_size(w))
                            .clicked()
                            && let Some(i) = sel
                        {
                            self.rules.remove(i);
                            self.rule_sel = None;
                        }
                    });
                    let mut clicked = None;
                    list_frame(ui, "recode_rules", 7.0 * 18.0 + 2.0, |ui| {
                        for (i, r) in self.rules.iter().enumerate() {
                            if var_row(ui, None, &rule_text(r), self.rule_sel == Some(i), true)
                                .clicked()
                            {
                                clicked = Some(i);
                            }
                        }
                    });
                    if let Some(i) = clicked {
                        self.rule_sel = Some(i);
                        self.load_rule(i);
                    }
                });
                if self.old_kind == OldKind::Range
                    && let (Some(a), Some(b)) = (num(&self.range_lo), num(&self.range_hi))
                    && a > b
                {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        "The first value of the range is greater than the second.",
                    );
                }
                super::frame::checks_off(
                    ui,
                    &[
                        ("Output variables are strings", false),
                        ("Convert numeric strings to numbers ('5'->5)", false),
                    ],
                );
            });
        });
    }
}

/// Run Recode; the undo op removes the new variables again.
fn run_recode(ds: &mut Dataset, spec: &RecodeSpec) -> ProcOutput {
    let ids = match recode(ds, spec) {
        Ok(ids) => ids,
        Err(e) => return ProcOutput::failed(format!("Recode into Different Variables: {e}.")),
    };
    let mut ops: Vec<EditOp> = ids
        .iter()
        .map(|&id| EditOp::InsertVar {
            at: id.0,
            var: Box::new(ds.var(id).clone()),
            col: ds.column(id).clone(),
        })
        .collect();
    let op = if ops.len() == 1 {
        ops.pop().expect("one op")
    } else {
        EditOp::Group(ops)
    };
    ProcOutput::undoable(Vec::new(), op)
}

impl ProcDialog for RecodeDialog {
    fn title(&self) -> &str {
        "Recode into Different Variables"
    }

    fn size(&self) -> egui::Vec2 {
        vec2(660.0, 420.0)
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(190.0, 280.0));
            ui.vertical(|ui| {
                ui.add_space(90.0);
                self.picker.arrow_ui(ui, ds, 0);
            });
            ui.vertical(|ui| {
                ui.set_width(210.0);
                ui.label(self.picker.targets[0].label);
                self.output_list(ui, ds);
                ui.add_space(6.0);
                if ui.button(OLD_NEW).clicked() {
                    a = DialogAction::Sub(OLD_NEW);
                }
                ui.horizontal(|ui| {
                    ui.add_enabled(false, Button::new("If..."));
                    ui.label("(optional case selection condition)");
                });
            });
            ui.vertical(|ui| {
                ui.set_width(190.0);
                self.output_group(ui);
            });
        });
        a
    }

    fn ready(&self) -> bool {
        let inputs = self.picker.vars(0);
        !inputs.is_empty()
            && inputs.iter().all(|i| self.output(i).is_some())
            && !self.rules.is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let ids = resolve(ds, self.picker.vars(0), Eligible::Numeric)?;
        let mut pairs = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (id, input) in ids.into_iter().zip(self.picker.vars(0)) {
            let o = self
                .output(input)
                .ok_or_else(|| format!("Enter an output variable name for {input}."))?;
            let name = o.name.trim();
            if validate_name(name).is_err() {
                return Err(format!("{name} is not a valid variable name."));
            }
            if ds.find(name).is_some()
                || seen.iter().any(|s| s.to_lowercase() == name.to_lowercase())
            {
                return Err(format!(
                    "The output variable name {name} is already in use. Choose a new name."
                ));
            }
            seen.push(name.to_string());
            let label = o.label.trim();
            pairs.push((
                id,
                name.to_string(),
                (!label.is_empty()).then(|| label.to_string()),
            ));
        }
        if self.rules.is_empty() {
            return Err("Add at least one rule in Old and New Values.".into());
        }
        let spec = RecodeSpec {
            pairs,
            rules: self.rules.clone(),
        };
        Ok(RunSpec {
            title: LOG_TITLE,
            run: Box::new(move |ds: &mut Dataset| run_recode(ds, &spec)),
        })
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let inputs = current_names(ds, self.picker.vars(0)).join(" ");
        let outs = self.out_names();
        let rules: Vec<String> = self.rules.iter().map(rule_syntax).collect();
        let mut lines = vec![format!(
            "RECODE {inputs} {} INTO {}.",
            rules.join(" "),
            outs.join(" ")
        )];
        for (input, out) in self.picker.vars(0).iter().zip(&outs) {
            if let Some(o) = self.output(input)
                && !o.label.trim().is_empty()
            {
                lines.push(format!(
                    "VARIABLE LABELS {out} '{}'.",
                    o.label.trim().replace('\'', "''")
                ));
            }
        }
        lines.push("EXECUTE.".into());
        lines.join("\n")
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Recode into Different Variables creates new numeric variables from the values of \
         existing ones. Move the variables to the list, give each an output name (Change), and \
         define the rules in Old and New Values...: a value, a range, system-missing or all \
         other values becomes a new value, system-missing, or a copy of the old value. Values \
         that no rule matches become system-missing. Edit ▸ Undo removes the new variables."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name == OLD_NEW {
            self.old_new_ui(ui);
        }
    }
}
