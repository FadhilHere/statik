//! Analyze ▸ Descriptive Statistics ▸ Frequencies... with the Charts... sub-dialog.
use super::frame::{checks_off, group, radios_off, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{Ui, vec2};
use procedures::frequencies::{FrequenciesOptions, run_frequencies};

#[derive(Clone, Debug)]
pub struct FrequenciesDialog {
    pub picker: VarPicker,
    /// Charts...: Histograms (otherwise None).
    pub histogram: bool,
}

impl Default for FrequenciesDialog {
    fn default() -> Self {
        FrequenciesDialog {
            picker: VarPicker::new(vec![Target::list("Variable(s):", 12)]),
            histogram: false,
        }
    }
}

impl ProcDialog for FrequenciesDialog {
    fn title(&self) -> &str {
        "Frequencies"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 260.0));
            self.picker.target_ui(ui, ds, 0, 210.0);
            a = side_buttons(
                ui,
                &[
                    ("Statistics...", false),
                    ("Charts...", true),
                    ("Format...", false),
                    ("Style...", false),
                    ("Bootstrap...", false),
                ],
            );
        });
        checks_off(ui, &[("Display frequency tables", true)]);
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(0).is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let o = FrequenciesOptions {
            vars: resolve(ds, self.picker.vars(0), Eligible::Numeric)?,
            histogram: self.histogram,
        };
        Ok(RunSpec::read("Frequencies", move |ds| {
            run_frequencies(ds, &o)
        }))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        Syntax::new(format!(
            "FREQUENCIES VARIABLES={}",
            var_list(&current_names(ds, self.picker.vars(0)))
        ))
        .subcommand_if(self.histogram, "HISTOGRAM")
        .subcommand("ORDER=ANALYSIS")
        .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Frequencies shows, for each selected variable, the number of valid and missing cases \
         and a frequency table with the count, percent, valid percent and cumulative percent \
         of every value. Charts can add a histogram of each variable."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name != "Charts..." {
            return;
        }
        group(ui, "Chart Type", |ui| {
            ui.radio_value(&mut self.histogram, false, "None");
            radios_off(ui, &[("Bar charts", false), ("Pie charts", false)]);
            ui.radio_value(&mut self.histogram, true, "Histograms:");
            ui.indent("normal", |ui| {
                checks_off(ui, &[("Show normal curve on histogram", false)]);
            });
        });
        group(ui, "Chart Values", |ui| {
            radios_off(ui, &[("Frequencies", true), ("Percentages", false)]);
        });
    }
}
