//! Analyze ▸ Descriptive Statistics ▸ Explore... with the Plots... sub-dialog.
use super::frame::{checks_off, group, radios_off, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{RadioButton, Ui, Vec2, vec2};
use procedures::explore::{ExploreOptions, run_explore};

/// Display: Both / Statistics / Plots (Plots alone is not available: the engine always shows
/// the Descriptives table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Both,
    Statistics,
}

#[derive(Clone, Debug)]
pub struct ExploreDialog {
    pub picker: VarPicker,
    pub display: Display,
    /// Plots...: "Normality plots with tests" (Tests of Normality + Normal Q-Q plots).
    pub normality: bool,
}

impl Default for ExploreDialog {
    fn default() -> Self {
        ExploreDialog {
            picker: VarPicker::new(vec![
                Target::list("Dependent List:", 5),
                Target::list("Factor List:", 4).disabled(),
                Target::single("Label Cases by:").disabled(),
            ]),
            display: Display::Both,
            normality: false,
        }
    }
}

impl ExploreDialog {
    fn plots(&self) -> bool {
        self.display == Display::Both && self.normality
    }
}

impl ProcDialog for ExploreDialog {
    fn title(&self) -> &str {
        "Explore"
    }

    fn size(&self) -> Vec2 {
        vec2(600.0, 420.0)
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 270.0));
            ui.vertical(|ui| {
                for i in 0..3 {
                    self.picker.target_ui(ui, ds, i, 210.0);
                }
            });
            let plots = self.display == Display::Both;
            a = side_buttons(
                ui,
                &[
                    ("Statistics...", false),
                    ("Plots...", plots),
                    ("Options...", false),
                    ("Bootstrap...", false),
                ],
            );
        });
        group(ui, "Display", |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.display, Display::Both, "Both");
                ui.radio_value(&mut self.display, Display::Statistics, "Statistics");
                ui.add_enabled(false, RadioButton::new(false, "Plots"));
            });
        });
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(0).is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let o = ExploreOptions {
            dependents: resolve(ds, self.picker.vars(0), Eligible::Numeric)?,
            normality: self.plots(),
            qq_plot: self.plots(),
        };
        Ok(RunSpec::read("Explore", move |ds| run_explore(ds, &o)))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        Syntax::new(format!(
            "EXAMINE VARIABLES={}",
            var_list(&current_names(ds, self.picker.vars(0)))
        ))
        .subcommand(if self.plots() {
            "PLOT NPPLOT"
        } else {
            "PLOT NONE"
        })
        .subcommand_if(self.plots(), "COMPARE GROUPS")
        .subcommand("STATISTICS DESCRIPTIVES")
        .subcommand("CINTERVAL 95")
        .subcommand("MISSING LISTWISE")
        .subcommand("NOTOTAL")
        .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Explore summarizes each dependent variable: a case processing summary and descriptive \
         statistics (mean with its confidence interval, trimmed mean, median, variance, \
         standard deviation, range, skewness, kurtosis). With \"Normality plots with tests\" \
         it adds the Kolmogorov-Smirnov (Lilliefors) and Shapiro-Wilk tests of normality and a \
         normal Q-Q plot."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name != "Plots..." {
            return;
        }
        ui.horizontal_top(|ui| {
            group(ui, "Boxplots", |ui| {
                radios_off(
                    ui,
                    &[
                        ("Factor levels together", false),
                        ("Dependents together", false),
                        ("None", true),
                    ],
                );
            });
            group(ui, "Descriptive", |ui| {
                checks_off(ui, &[("Stem-and-leaf", false), ("Histogram", false)]);
            });
        });
        ui.checkbox(&mut self.normality, "Normality plots with tests");
        group(ui, "Spread vs Level with Levene Test", |ui| {
            radios_off(ui, &[("None", true), ("Power estimation", false)]);
        });
    }
}
