//! Analyze ▸ Nonparametric Tests ▸ Legacy Dialogs ▸ Runs...
use super::frame::{checks_off, group, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{Ui, vec2};
use procedures::npar_runs::{RunsOptions, run_runs};

#[derive(Clone, Debug)]
pub struct RunsDialog {
    pub picker: VarPicker,
}

impl Default for RunsDialog {
    fn default() -> Self {
        RunsDialog {
            picker: VarPicker::new(vec![Target::list("Test Variable List:", 10)]),
        }
    }
}

impl ProcDialog for RunsDialog {
    fn title(&self) -> &str {
        "Runs Test"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 240.0));
            self.picker.target_ui(ui, ds, 0, 210.0);
            a = side_buttons(ui, &[("Exact...", false), ("Options...", false)]);
        });
        group(ui, "Cut Point", |ui| {
            ui.horizontal(|ui| {
                checks_off(
                    ui,
                    &[
                        ("Median", true),
                        ("Mode", false),
                        ("Mean", false),
                        ("Custom:", false),
                    ],
                );
            });
        });
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(0).is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let o = RunsOptions {
            vars: resolve(ds, self.picker.vars(0), Eligible::Numeric)?,
        };
        Ok(RunSpec::read("NPar Tests", move |ds| run_runs(ds, &o)))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        Syntax::new("NPAR TESTS")
            .subcommand(format!(
                "RUNS(MEDIAN)={}",
                var_list(&current_names(ds, self.picker.vars(0)))
            ))
            .subcommand("MISSING ANALYSIS")
            .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "The Runs test checks whether the order of the values of a variable is random. Each \
         value is classed as below or at-or-above the median (the cut point), and the number \
         of runs of consecutive values on the same side is compared with the number expected \
         by chance."
    }
}
