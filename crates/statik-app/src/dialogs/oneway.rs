//! Analyze ▸ Compare Means ▸ One-Way ANOVA... with the Options... sub-dialog.
use super::frame::{checks_off, group, missing_group, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve, resolve_one};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{Ui, vec2};
use output_model::OutputItem;
use procedures::oneway::{OneWayOptions, run_oneway};

const DEP: usize = 0;
const FACTOR: usize = 1;

#[derive(Clone, Debug)]
pub struct OneWayDialog {
    pub picker: VarPicker,
    pub descriptives: bool,
    pub homogeneity: bool,
}

impl Default for OneWayDialog {
    fn default() -> Self {
        OneWayDialog {
            picker: VarPicker::new(vec![
                Target::list("Dependent List:", 8),
                Target::single("Factor:"),
            ]),
            descriptives: false,
            homogeneity: false,
        }
    }
}

impl ProcDialog for OneWayDialog {
    fn title(&self) -> &str {
        "One-Way ANOVA"
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 260.0));
            ui.vertical(|ui| {
                self.picker.target_ui(ui, ds, DEP, 210.0);
                self.picker.target_ui(ui, ds, FACTOR, 210.0);
            });
            a = side_buttons(
                ui,
                &[
                    ("Contrasts...", false),
                    ("Post Hoc...", false),
                    ("Options...", true),
                    ("Bootstrap...", false),
                ],
            );
        });
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(DEP).is_empty() && !self.picker.vars(FACTOR).is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let deps = resolve(ds, self.picker.vars(DEP), Eligible::Numeric)?;
        let f = self
            .picker
            .vars(FACTOR)
            .first()
            .ok_or("A factor variable is required.")?;
        let factor = resolve_one(ds, f, Eligible::Numeric)?;
        let (descriptives, homogeneity) = (self.descriptives, self.homogeneity);
        Ok(RunSpec::read("Oneway", move |ds| {
            // One analysis per dependent; one "Oneway" heading for all of them.
            let mut out = Vec::new();
            for dependent in deps {
                let items = run_oneway(
                    ds,
                    &OneWayOptions {
                        dependent,
                        factor,
                        descriptives,
                        homogeneity,
                    },
                );
                let skip_title = !out.is_empty();
                out.extend(
                    items
                        .into_iter()
                        .filter(|i| !(skip_title && matches!(i, OutputItem::Title(_)))),
                );
            }
            out
        }))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let mut stats = vec![];
        if self.descriptives {
            stats.push("DESCRIPTIVES");
        }
        if self.homogeneity {
            stats.push("HOMOGENEITY");
        }
        Syntax::new(format!(
            "ONEWAY {} BY {}",
            var_list(&current_names(ds, self.picker.vars(DEP))),
            current_names(ds, self.picker.vars(FACTOR)).join(" ")
        ))
        .subcommand_if(!stats.is_empty(), format!("STATISTICS {}", stats.join(" ")))
        .subcommand("MISSING ANALYSIS")
        .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "One-Way ANOVA tests whether the means of a dependent variable are equal across the \
         groups defined by a factor variable. Options adds descriptive statistics for each \
         group and Levene's test of homogeneity of variances."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name != "Options..." {
            return;
        }
        group(ui, "Statistics", |ui| {
            ui.checkbox(&mut self.descriptives, "Descriptive");
            checks_off(ui, &[("Fixed and random effects", false)]);
            ui.checkbox(&mut self.homogeneity, "Homogeneity of variance test");
            checks_off(ui, &[("Brown-Forsythe", false), ("Welch", false)]);
        });
        checks_off(ui, &[("Means plot", false)]);
        missing_group(
            ui,
            "Exclude cases analysis by analysis",
            "Exclude cases listwise",
        );
    }
}
