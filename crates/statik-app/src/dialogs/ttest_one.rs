//! Analyze ▸ Compare Means ▸ One-Sample T Test... with the Options... sub-dialog.
use super::frame::{ci_options, labeled_text, parse_ci, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::edit::parse_number;
use crate::syntax::{Syntax, fraction, value, var_list};
use dataset::Dataset;
use egui::{Ui, vec2};
use procedures::ttest_one::{OneSampleOptions, run_one_sample};

#[derive(Clone, Debug)]
pub struct OneSampleDialog {
    pub picker: VarPicker,
    pub test_value: String,
    /// Options...: confidence interval percentage.
    pub conf: String,
}

impl Default for OneSampleDialog {
    fn default() -> Self {
        OneSampleDialog {
            picker: VarPicker::new(vec![Target::list("Test Variable(s):", 10)]),
            test_value: "0".into(),
            conf: "95".into(),
        }
    }
}

impl ProcDialog for OneSampleDialog {
    fn title(&self) -> &str {
        "One-Sample T Test"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 260.0));
            ui.vertical(|ui| {
                self.picker.target_ui(ui, ds, 0, 210.0);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    labeled_text(ui, "Test Value:", &mut self.test_value, 70.0);
                });
            });
            a = side_buttons(ui, &[("Options...", true), ("Bootstrap...", false)]);
        });
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(0).is_empty() && parse_number(self.test_value.trim()).is_some()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let o = OneSampleOptions {
            vars: resolve(ds, self.picker.vars(0), Eligible::Numeric)?,
            test_value: parse_number(self.test_value.trim())
                .ok_or("The test value must be a number.")?,
            conf: parse_ci(&self.conf)? / 100.0,
        };
        Ok(RunSpec::read("T-Test", move |ds| run_one_sample(ds, &o)))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let tv = parse_number(self.test_value.trim()).unwrap_or(0.0);
        let conf = parse_ci(&self.conf).unwrap_or(95.0);
        Syntax::new("T-TEST")
            .subcommand(format!("TESTVAL={}", value(tv)))
            .subcommand("MISSING=ANALYSIS")
            .subcommand(format!(
                "VARIABLES={}",
                var_list(&current_names(ds, self.picker.vars(0)))
            ))
            .subcommand(format!("CRITERIA=CI({})", fraction(conf, 2)))
            .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "One-Sample T Test tests whether the mean of each test variable differs from the test \
         value. It shows the mean, standard deviation and standard error of each variable, \
         and the t statistic, degrees of freedom, significance, mean difference and its \
         confidence interval."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name == "Options..." {
            ci_options(ui, &mut self.conf);
        }
    }

    fn sub_continue(&mut self, name: &str) -> Result<(), String> {
        if name == "Options..." {
            parse_ci(&self.conf)?;
        }
        Ok(())
    }
}
