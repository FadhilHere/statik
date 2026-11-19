//! Analyze ▸ Compare Means ▸ Independent-Samples T Test... with Define Groups... and
//! Options... sub-dialogs.
use super::frame::{ci_options, labeled_text, parse_ci, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve, resolve_one};
use crate::edit::parse_number;
use crate::syntax::{Syntax, fraction, value, var_list};
use dataset::Dataset;
use egui::{RadioButton, Ui, vec2};
use procedures::ttest_ind::{IndependentOptions, run_independent};

const TEST: usize = 0;
const GROUP: usize = 1;

#[derive(Clone, Debug)]
pub struct IndependentDialog {
    pub picker: VarPicker,
    /// Define Groups...: the two group values as typed.
    pub group1: String,
    pub group2: String,
    pub conf: String,
}

impl Default for IndependentDialog {
    fn default() -> Self {
        IndependentDialog {
            picker: VarPicker::new(vec![
                Target::list("Test Variable(s):", 8),
                // The engine compares numeric group codes, so the grouping variable is numeric.
                Target::single("Grouping Variable:"),
            ]),
            group1: String::new(),
            group2: String::new(),
            conf: "95".into(),
        }
    }
}

impl IndependentDialog {
    fn groups(&self) -> Option<(f64, f64)> {
        Some((
            parse_number(self.group1.trim())?,
            parse_number(self.group2.trim())?,
        ))
    }
    /// "(1 2)", or "(? ?)" before the groups are defined.
    fn suffix(&self) -> String {
        match self.groups() {
            Some((a, b)) => format!("({} {})", value(a), value(b)),
            None => "(? ?)".into(),
        }
    }
}

impl ProcDialog for IndependentDialog {
    fn title(&self) -> &str {
        "Independent-Samples T Test"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        self.picker.targets[GROUP].suffix = Some(self.suffix());
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 270.0));
            ui.vertical(|ui| {
                self.picker.target_ui(ui, ds, TEST, 210.0);
                self.picker.target_ui(ui, ds, GROUP, 210.0);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    let on = !self.picker.vars(GROUP).is_empty();
                    if ui
                        .add_enabled(on, egui::Button::new("Define Groups..."))
                        .clicked()
                    {
                        a = DialogAction::Sub("Define Groups...");
                    }
                });
            });
            a = a.or(side_buttons(
                ui,
                &[("Options...", true), ("Bootstrap...", false)],
            ));
        });
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(TEST).is_empty()
            && !self.picker.vars(GROUP).is_empty()
            && self.groups().is_some()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let g = self
            .picker
            .vars(GROUP)
            .first()
            .ok_or("A grouping variable is required.")?;
        let (group1, group2) = self.groups().ok_or("Define the two groups first.")?;
        let o = IndependentOptions {
            test_vars: resolve(ds, self.picker.vars(TEST), Eligible::Numeric)?,
            grouping: resolve_one(ds, g, Eligible::Numeric)?,
            group1,
            group2,
            conf: parse_ci(&self.conf)? / 100.0,
        };
        Ok(RunSpec::read("T-Test", move |ds| run_independent(ds, &o)))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let g = current_names(ds, self.picker.vars(GROUP)).join(" ");
        let conf = parse_ci(&self.conf).unwrap_or(95.0);
        Syntax::new(format!("T-TEST GROUPS={g}{}", self.suffix()))
            .subcommand("MISSING=ANALYSIS")
            .subcommand(format!(
                "VARIABLES={}",
                var_list(&current_names(ds, self.picker.vars(TEST)))
            ))
            .subcommand(format!("CRITERIA=CI({})", fraction(conf, 2)))
            .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Independent-Samples T Test compares the means of two groups of cases, defined by two \
         values of the grouping variable. It shows group statistics, Levene's test for \
         equality of variances, and the t test with and without assuming equal variances, \
         with the mean difference and its confidence interval."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        match name {
            "Define Groups..." => {
                let _ = ui.radio(true, "Use specified values");
                ui.indent("groups", |ui| {
                    labeled_text(ui, "Group 1:", &mut self.group1, 80.0);
                    labeled_text(ui, "Group 2:", &mut self.group2, 80.0);
                });
                ui.add_enabled(false, RadioButton::new(false, "Cut point:"));
            }
            "Options..." => ci_options(ui, &mut self.conf),
            _ => {}
        }
    }

    fn sub_continue(&mut self, name: &str) -> Result<(), String> {
        match name {
            "Define Groups..." if self.groups().is_none() => {
                Err("Enter a numeric value for Group 1 and for Group 2.".into())
            }
            "Options..." => parse_ci(&self.conf).map(|_| ()),
            _ => Ok(()),
        }
    }
}
