//! Analyze ▸ Regression ▸ Linear... ("Linear Regression"), with the Statistics..., Plots...
//! and Save... sub-dialogs.
use super::frame::{checks_off, group, side_buttons};
use super::var_picker::{Eligible, Target, VarPicker, arrow_button, list_frame, var_row};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve, resolve_one};
use crate::syntax::{Syntax, var_list};
use dataset::Dataset;
use egui::{Ui, Vec2, vec2};
use procedures::regression::{RegressionOptions, SaveFlags, run_regression};

const DEP: usize = 0;
const IND: usize = 1;

/// The Plots... source list; only `*SRESID` (Y) against `*ZPRED` (X) is supported.
pub const PLOT_VARS: [&str; 7] = [
    "DEPENDNT", "*ZPRED", "*ZRESID", "*DRESID", "*ADJPRED", "*SRESID", "*SDRESID",
];

#[derive(Clone, Debug)]
pub struct RegressionDialog {
    pub picker: VarPicker,
    pub collinearity: bool,
    pub durbin_watson: bool,
    pub plot_hist: bool,
    pub plot_pp: bool,
    /// Y and X of the scatterplot (`Some("*SRESID")`, `Some("*ZPRED")`).
    pub plot_y: Option<&'static str>,
    pub plot_x: Option<&'static str>,
    /// Selected item in the Plots... dialog: a source item, or "Y"/"X" for a filled box.
    pub plot_sel: Option<&'static str>,
    pub save: SaveFlags,
}

impl Default for RegressionDialog {
    fn default() -> Self {
        RegressionDialog {
            picker: VarPicker::new(vec![
                Target::single("Dependent:"),
                Target::list("Independent(s):", 5),
                Target::single("Selection Variable:").disabled(),
                Target::single("Case Labels:").disabled(),
                Target::single("WLS Weight:").disabled(),
            ]),
            collinearity: false,
            durbin_watson: false,
            plot_hist: false,
            plot_pp: false,
            plot_y: None,
            plot_x: None,
            plot_sel: None,
            save: SaveFlags::default(),
        }
    }
}

impl RegressionDialog {
    fn scatter(&self) -> bool {
        self.plot_y.is_some() && self.plot_x.is_some()
    }

    fn plots_ui(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(120.0);
                list_frame(
                    ui,
                    "plot_vars",
                    7.0 * super::var_picker::ROW_H + 2.0,
                    |ui| {
                        for v in PLOT_VARS {
                            let on = v == "*SRESID" || v == "*ZPRED";
                            let used = self.plot_y == Some(v) || self.plot_x == Some(v);
                            if used {
                                continue;
                            }
                            let r = ui
                                .add_enabled_ui(on, |ui| {
                                    var_row(ui, None, v, self.plot_sel == Some(v), on)
                                })
                                .inner;
                            if r.clicked() {
                                self.plot_sel = Some(v);
                            }
                        }
                    },
                );
            });
            ui.vertical(|ui| {
                ui.label("Scatter 1 of 1");
                for (axis, want) in [("Y", "*SRESID"), ("X", "*ZPRED")] {
                    ui.horizontal(|ui| {
                        let slot = if axis == "Y" {
                            self.plot_y
                        } else {
                            self.plot_x
                        };
                        if self.plot_sel == Some(axis) {
                            if arrow_button(ui, false, true, &format!("Remove from {axis}")) {
                                if axis == "Y" {
                                    self.plot_y = None;
                                } else {
                                    self.plot_x = None;
                                }
                                self.plot_sel = None;
                            }
                        } else {
                            let ok = slot.is_none() && self.plot_sel == Some(want);
                            if arrow_button(ui, true, ok, &format!("Move to {axis}")) {
                                if axis == "Y" {
                                    self.plot_y = Some(want);
                                } else {
                                    self.plot_x = Some(want);
                                }
                                self.plot_sel = None;
                            }
                        }
                        ui.label(format!("{axis}:"));
                        ui.vertical(|ui| {
                            ui.set_width(110.0);
                            list_frame(
                                ui,
                                ("plot_axis", axis),
                                super::var_picker::ROW_H + 2.0,
                                |ui| {
                                    if let Some(v) = slot
                                        && var_row(ui, None, v, self.plot_sel == Some(axis), true)
                                            .clicked()
                                    {
                                        self.plot_sel = Some(axis);
                                    }
                                },
                            );
                        });
                    });
                }
                ui.horizontal(|ui| {
                    ui.add_enabled(false, egui::Button::new("Previous"));
                    ui.add_enabled(false, egui::Button::new("Next"));
                });
            });
        });
        group(ui, "Standardized Residual Plots", |ui| {
            ui.checkbox(&mut self.plot_hist, "Histogram");
            ui.checkbox(&mut self.plot_pp, "Normal probability plot");
        });
        checks_off(ui, &[("Produce all partial plots", false)]);
    }

    fn statistics_ui(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            group(ui, "Regression Coefficients", |ui| {
                checks_off(
                    ui,
                    &[
                        ("Estimates", true),
                        ("Confidence intervals", false),
                        ("Covariance matrix", false),
                    ],
                );
            });
            ui.vertical(|ui| {
                checks_off(
                    ui,
                    &[
                        ("Model fit", true),
                        ("R squared change", false),
                        ("Descriptives", false),
                        ("Part and partial correlations", false),
                    ],
                );
                ui.checkbox(&mut self.collinearity, "Collinearity diagnostics");
            });
        });
        group(ui, "Residuals", |ui| {
            ui.checkbox(&mut self.durbin_watson, "Durbin-Watson");
            checks_off(ui, &[("Casewise diagnostics", false)]);
        });
    }

    fn save_ui(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                group(ui, "Predicted Values", |ui| {
                    ui.checkbox(&mut self.save.predicted, "Unstandardized");
                    ui.checkbox(&mut self.save.z_predicted, "Standardized");
                    checks_off(
                        ui,
                        &[("Adjusted", false), ("S.E. of mean predictions", false)],
                    );
                });
                group(ui, "Distances", |ui| {
                    checks_off(
                        ui,
                        &[
                            ("Mahalanobis", false),
                            ("Cook's", false),
                            ("Leverage values", false),
                        ],
                    );
                });
            });
            ui.vertical(|ui| {
                group(ui, "Residuals", |ui| {
                    ui.checkbox(&mut self.save.residual, "Unstandardized");
                    ui.checkbox(&mut self.save.z_residual, "Standardized");
                    ui.checkbox(&mut self.save.studentized, "Studentized");
                    checks_off(ui, &[("Deleted", false), ("Studentized deleted", false)]);
                });
                group(ui, "Influence Statistics", |ui| {
                    checks_off(
                        ui,
                        &[
                            ("DfBeta(s)", false),
                            ("DfFit", false),
                            ("Covariance ratio", false),
                        ],
                    );
                });
            });
        });
    }
}

impl ProcDialog for RegressionDialog {
    fn title(&self) -> &str {
        "Linear Regression"
    }

    fn size(&self) -> Vec2 {
        vec2(680.0, 540.0)
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 330.0));
            ui.vertical(|ui| {
                self.picker.target_ui(ui, ds, DEP, 220.0);
                group(ui, "Block 1 of 1", |ui| {
                    ui.horizontal(|ui| {
                        ui.add_enabled(false, egui::Button::new("Previous"));
                        ui.add_enabled(false, egui::Button::new("Next"));
                    });
                    self.picker.target_ui(ui, ds, IND, 196.0);
                    ui.horizontal(|ui| {
                        ui.add_space(32.0);
                        ui.label("Method:");
                        egui::ComboBox::from_id_salt("reg_method")
                            .selected_text("Enter")
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                let _ = ui.selectable_label(true, "Enter");
                                for m in ["Stepwise", "Remove", "Backward", "Forward"] {
                                    ui.add_enabled(false, egui::Button::selectable(false, m));
                                }
                            });
                    });
                });
                for i in 2..5 {
                    self.picker.target_ui(ui, ds, i, 220.0);
                }
            });
            a = side_buttons(
                ui,
                &[
                    ("Statistics...", true),
                    ("Plots...", true),
                    ("Save...", true),
                    ("Options...", false),
                    ("Style...", false),
                    ("Bootstrap...", false),
                ],
            );
        });
        a
    }

    fn ready(&self) -> bool {
        !self.picker.vars(DEP).is_empty() && !self.picker.vars(IND).is_empty()
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let dep = self
            .picker
            .vars(DEP)
            .first()
            .ok_or("A dependent variable is required.")?;
        let opt = RegressionOptions {
            dependent: resolve_one(ds, dep, Eligible::Numeric)?,
            independents: resolve(ds, self.picker.vars(IND), Eligible::Numeric)?,
            collinearity: self.collinearity,
            durbin_watson: self.durbin_watson,
            plot_hist: self.plot_hist,
            plot_pp: self.plot_pp,
            plot_zpred_sresid: self.scatter(),
            save: self.save,
        };
        if opt.independents.is_empty() {
            return Err("At least one independent variable is required.".into());
        }
        Ok(RunSpec::write("Regression", move |ds| {
            run_regression(ds, &opt)
        }))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let stats = if self.collinearity {
            "STATISTICS COEFF OUTS R ANOVA COLLIN TOL"
        } else {
            "STATISTICS COEFF OUTS R ANOVA"
        };
        let mut resid = vec![];
        if self.durbin_watson {
            resid.push("DURBIN");
        }
        if self.plot_hist {
            resid.push("HISTOGRAM(ZRESID)");
        }
        if self.plot_pp {
            resid.push("NORMPROB(ZRESID)");
        }
        let s = self.save;
        let save: Vec<&str> = [
            (s.predicted, "PRED"),
            (s.z_predicted, "ZPRED"),
            (s.residual, "RESID"),
            (s.z_residual, "ZRESID"),
            (s.studentized, "SRESID"),
        ]
        .into_iter()
        .filter_map(|(on, k)| on.then_some(k))
        .collect();
        Syntax::new("REGRESSION")
            .subcommand("MISSING LISTWISE")
            .subcommand(stats)
            .subcommand("CRITERIA=PIN(.05) POUT(.10)")
            .subcommand("NOORIGIN")
            .subcommand(format!(
                "DEPENDENT {}",
                var_list(&current_names(ds, self.picker.vars(DEP)))
            ))
            .subcommand(format!(
                "METHOD=ENTER {}",
                var_list(&current_names(ds, self.picker.vars(IND)))
            ))
            .subcommand_if(self.scatter(), "SCATTERPLOT=(*SRESID ,*ZPRED)")
            .subcommand_if(!resid.is_empty(), format!("RESIDUALS {}", resid.join(" ")))
            .subcommand_if(!save.is_empty(), format!("SAVE {}", save.join(" ")))
            .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Linear Regression estimates the coefficients of a linear equation that predicts the \
         dependent variable from one or more independent variables (method Enter: all \
         independents enter in one block). Statistics adds collinearity diagnostics and the \
         Durbin-Watson test, Plots adds residual plots (histogram, normal P-P plot, *SRESID \
         against *ZPRED), and Save adds predicted values and residuals to the data as new \
         variables."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        match name {
            "Statistics..." => self.statistics_ui(ui),
            "Plots..." => self.plots_ui(ui),
            "Save..." => self.save_ui(ui),
            _ => {}
        }
    }

    fn sub_continue(&mut self, name: &str) -> Result<(), String> {
        if name == "Plots..." && self.plot_x.is_some() != self.plot_y.is_some() {
            return Err("A scatterplot needs both a Y and an X variable.".into());
        }
        Ok(())
    }
}
