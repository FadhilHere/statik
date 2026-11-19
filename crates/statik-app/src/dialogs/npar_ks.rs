//! Analyze ▸ Nonparametric Tests ▸ Legacy Dialogs ▸ 1-Sample K-S...
use super::frame::{checks_off, group, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{Ui, vec2};
use procedures::npar_ks::{NparKsOptions, run_npar_ks};

#[derive(Clone, Debug)]
pub struct KsDialog {
    pub picker: VarPicker,
}

impl Default for KsDialog {
    fn default() -> Self {
        KsDialog {
            picker: VarPicker::new(vec![Target::list("Test Variable List:", 10)]),
        }
    }
}

impl ProcDialog for KsDialog {
    fn title(&self) -> &str {
        "One-Sample Kolmogorov-Smirnov Test"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 240.0));
            self.picker.target_ui(ui, ds, 0, 210.0);
            a = side_buttons(ui, &[("Exact...", false), ("Options...", false)]);
        });
        group(ui, "Test Distribution", |ui| {
            ui.horizontal(|ui| {
                checks_off(
                    ui,
                    &[
                        ("Normal", true),
                        ("Uniform", false),
                        ("Poisson", false),
                        ("Exponential", false),
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
        let o = NparKsOptions {
            vars: resolve(ds, self.picker.vars(0), Eligible::Numeric)?,
        };
        Ok(RunSpec::read("NPar Tests", move |ds| run_npar_ks(ds, &o)))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        Syntax::new("NPAR TESTS")
            .subcommand(format!(
                "K-S(NORMAL)={}",
                var_list(&current_names(ds, self.picker.vars(0)))
            ))
            .subcommand("MISSING ANALYSIS")
            .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "The One-Sample Kolmogorov-Smirnov test compares the distribution of each test \
         variable with a normal distribution whose mean and standard deviation are estimated \
         from the data (Lilliefors significance correction). A small significance value \
         suggests the variable is not normally distributed."
    }
}
