//! Analyze ▸ Descriptive Statistics ▸ Descriptives... with the Options... sub-dialog.
use super::frame::{checks_off, group, radios_off, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{Ui, vec2};
use procedures::descriptives::{DescriptivesOptions, run_descriptives};

#[derive(Clone, Debug)]
pub struct DescriptivesDialog {
    pub picker: VarPicker,
    /// "Save standardized values as variables".
    pub save_z: bool,
    /// Options...: Kurtosis and Skewness. The engine reports both together, so the two
    /// checkboxes share this flag.
    pub skew_kurt: bool,
}

impl Default for DescriptivesDialog {
    fn default() -> Self {
        DescriptivesDialog {
            picker: VarPicker::new(vec![Target::list("Variable(s):", 12)]),
            save_z: false,
            skew_kurt: false,
        }
    }
}

impl ProcDialog for DescriptivesDialog {
    fn title(&self) -> &str {
        "Descriptives"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 260.0));
            self.picker.target_ui(ui, ds, 0, 210.0);
            a = side_buttons(
                ui,
                &[
                    ("Options...", true),
                    ("Style...", false),
                    ("Bootstrap...", false),
                ],
            );
        });
        ui.checkbox(&mut self.save_z, "Save standardized values as variables");
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(0).is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let o = DescriptivesOptions {
            vars: resolve(ds, self.picker.vars(0), Eligible::Numeric)?,
            skew_kurt: self.skew_kurt,
            save_z: self.save_z,
        };
        Ok(RunSpec::write("Descriptives", move |ds| {
            run_descriptives(ds, &o)
        }))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let stats = if self.skew_kurt {
            "STATISTICS=MEAN STDDEV MIN MAX KURTOSIS SKEWNESS"
        } else {
            "STATISTICS=MEAN STDDEV MIN MAX"
        };
        Syntax::new(format!(
            "DESCRIPTIVES VARIABLES={}",
            var_list(&current_names(ds, self.picker.vars(0)))
        ))
        .subcommand_if(self.save_z, "SAVE")
        .subcommand(stats)
        .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Descriptives shows the number of valid cases, minimum, maximum, mean and standard \
         deviation of each selected variable, and optionally skewness and kurtosis. It can \
         also save the standardized values (z scores) of each variable as a new variable."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name != "Options..." {
            return;
        }
        ui.horizontal(|ui| {
            checks_off(ui, &[("Mean", true), ("Sum", false)]);
        });
        group(ui, "Dispersion", |ui| {
            egui::Grid::new("desc_disp").show(ui, |ui| {
                checks_off(ui, &[("Std. deviation", true), ("Minimum", true)]);
                ui.end_row();
                checks_off(ui, &[("Variance", false), ("Maximum", true)]);
                ui.end_row();
                checks_off(ui, &[("Range", false), ("S.E. mean", false)]);
                ui.end_row();
            });
        });
        group(ui, "Distribution", |ui| {
            ui.checkbox(&mut self.skew_kurt, "Kurtosis");
            ui.checkbox(&mut self.skew_kurt, "Skewness");
        });
        group(ui, "Display Order", |ui| {
            radios_off(
                ui,
                &[
                    ("Variable list", true),
                    ("Alphabetic", false),
                    ("Ascending means", false),
                    ("Descending means", false),
                ],
            );
        });
    }
}
