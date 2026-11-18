//! The Data Editor menu bar, laid out like SPSS Statistics. Items Statik does not have are shown
//! disabled (grey), as the spec requires.
//!
//! [`menu_bar`] only draws and reports what was chosen as a [`MenuAction`]; the app
//! ([`crate::app::StatikApp::handle`]) acts on it (file dialogs, jobs, commands). To wire a new
//! item, replace its `off(...)` entry with an `item(...)` returning the action.
use crate::state::{AppState, Command, DialogKind};
use egui::{Button, Response, Ui, WidgetInfo, WidgetType};

/// What the user picked in the menu.
#[derive(Clone, Debug)]
pub enum MenuAction {
    /// A command needing no further input.
    Cmd(Command),
    /// File > Open > Data... (shows the file dialog).
    OpenData,
    /// File > Import Data > CSV Data...
    ImportCsv,
    /// File > Import Data > Excel...
    ImportExcel,
    /// File > Save (becomes Save As... when the data has no `.sav` file yet).
    Save,
    /// File > Save As...
    SaveAs,
    /// Window > Data Editor.
    ShowDataEditor,
    /// Help > About Statik.
    About,
}

/// Top-level menus, in SPSS order.
pub const TOP_MENUS: [&str; 11] = [
    "File",
    "Edit",
    "View",
    "Data",
    "Transform",
    "Analyze",
    "Graphs",
    "Utilities",
    "Extensions",
    "Window",
    "Help",
];

/// Give a menu entry its plain accessible name: without this the shortcut text or the submenu
/// arrow becomes part of it ("Save Ctrl+S", "Regression ⏵").
fn named(resp: &Response, label: &str) {
    let enabled = resp.enabled();
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
}

/// A submenu.
fn sub(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let r = ui.menu_button(label, add).response;
    named(&r, label);
}

/// An enabled item; closes the menu when clicked.
fn item(
    ui: &mut Ui,
    label: &str,
    shortcut: Option<&str>,
    out: &mut Option<MenuAction>,
    action: impl FnOnce() -> MenuAction,
) {
    let mut b = Button::new(label);
    if let Some(s) = shortcut {
        b = b.shortcut_text(s);
    }
    let r = ui.add(b);
    named(&r, label);
    if r.clicked() {
        *out = Some(action());
        ui.close();
    }
}

/// Disabled items.
fn off(ui: &mut Ui, labels: &[&str]) {
    for l in labels {
        ui.add_enabled(false, Button::new(*l));
    }
}

/// Disabled submenus (shown with their arrow, cannot be opened).
fn sub_off(ui: &mut Ui, labels: &[&str]) {
    for l in labels {
        let r = ui
            .add_enabled_ui(false, |ui| ui.menu_button(*l, |_| {}).response)
            .inner;
        named(&r, l);
    }
}

fn dialog(ui: &mut Ui, label: &str, kind: DialogKind, out: &mut Option<MenuAction>) {
    item(ui, label, None, out, || {
        MenuAction::Cmd(Command::OpenDialog(kind))
    });
}

/// Draw the menu bar. `enabled` is false while a job runs (every menu is greyed out).
pub fn menu_bar(ui: &mut Ui, state: &AppState, enabled: bool) -> Option<MenuAction> {
    let mut out = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            ui.menu_button("File", |ui| file_menu(ui, &mut out));
            ui.menu_button("Edit", |ui| edit_menu(ui, state, &mut out));
            ui.menu_button("View", |ui| view_menu(ui, state, &mut out));
            ui.menu_button("Data", data_menu);
            ui.menu_button("Transform", |ui| transform_menu(ui, &mut out));
            ui.menu_button("Analyze", |ui| analyze_menu(ui, &mut out));
            ui.menu_button("Graphs", |ui| {
                off(ui, &["Chart Builder...", "Graphboard Template Chooser..."]);
                ui.separator();
                sub_off(ui, &["Legacy Dialogs"]);
            });
            ui.menu_button("Utilities", |ui| {
                off(
                    ui,
                    &[
                        "Variables...",
                        "OMS Control Panel...",
                        "OMS Identifiers...",
                        "Scoring Wizard",
                    ],
                );
                ui.separator();
                off(
                    ui,
                    &[
                        "Data File Comments...",
                        "Define Variable Sets...",
                        "Use Variable Sets...",
                    ],
                );
                ui.separator();
                off(ui, &["Run Script..."]);
            });
            ui.menu_button("Extensions", |ui| {
                off(
                    ui,
                    &["Extension Hub...", "Install Local Extension Bundle..."],
                );
                ui.separator();
                off(ui, &["Custom Dialogs", "Utilities"]);
            });
            ui.menu_button("Window", |ui| {
                off(ui, &["Split", "Minimize All Windows"]);
                ui.separator();
                item(ui, "Data Editor", None, &mut out, || {
                    MenuAction::ShowDataEditor
                });
                let mut shown = state.show_output;
                if ui.checkbox(&mut shown, "Output").clicked() {
                    out = Some(MenuAction::Cmd(if shown {
                        Command::ShowOutput
                    } else {
                        Command::HideOutput
                    }));
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                off(ui, &["Topics", "Tutorial", "Command Syntax Reference"]);
                ui.separator();
                item(ui, "About Statik", None, &mut out, || MenuAction::About);
            });
        });
    });
    out
}

fn file_menu(ui: &mut Ui, out: &mut Option<MenuAction>) {
    sub(ui, "New", |ui| {
        item(ui, "Data", None, out, || MenuAction::Cmd(Command::New));
        off(ui, &["Syntax", "Output", "Script"]);
    });
    sub(ui, "Open", |ui| {
        item(ui, "Data...", Some("Ctrl+O"), out, || MenuAction::OpenData);
        off(ui, &["Syntax...", "Output...", "Script..."]);
    });
    sub(ui, "Import Data", |ui| {
        sub_off(ui, &["Database"]);
        item(ui, "Excel...", None, out, || MenuAction::ImportExcel);
        item(ui, "CSV Data...", None, out, || MenuAction::ImportCsv);
        off(
            ui,
            &[
                "Text Data...",
                "SAS...",
                "Stata...",
                "dBase...",
                "Lotus...",
                "SYLK...",
                "Cognos TM1...",
            ],
        );
    });
    ui.separator();
    off(ui, &["Close"]);
    item(ui, "Save", Some("Ctrl+S"), out, || MenuAction::Save);
    item(ui, "Save As...", None, out, || MenuAction::SaveAs);
    off(ui, &["Save All Data"]);
    sub_off(ui, &["Export"]);
    off(ui, &["Mark File Read Only", "Revert to Saved File"]);
    ui.separator();
    off(ui, &["Rename Dataset..."]);
    sub_off(ui, &["Display Data File Information"]);
    off(ui, &["Cache Data...", "Stop Processor"]);
    ui.separator();
    off(ui, &["Print Preview", "Print..."]);
    ui.separator();
    sub_off(ui, &["Recently Used Data", "Recently Used Files"]);
    ui.separator();
    item(ui, "Exit", None, out, || MenuAction::Cmd(Command::Exit));
}

fn edit_menu(ui: &mut Ui, state: &AppState, out: &mut Option<MenuAction>) {
    for (label, key, enabled, cmd) in [
        ("Undo", "Ctrl+Z", state.can_undo(), Command::Undo),
        ("Redo", "Ctrl+Y", state.can_redo(), Command::Redo),
    ] {
        let r = ui.add_enabled(enabled, Button::new(label).shortcut_text(key));
        named(&r, label);
        if r.clicked() {
            *out = Some(MenuAction::Cmd(cmd));
            ui.close();
        }
    }
    ui.separator();
    // Implemented with the Data View (Task 3).
    off(
        ui,
        &[
            "Cut",
            "Copy",
            "Copy with Variable Names",
            "Copy with Variable Labels",
            "Paste",
            "Paste Variables...",
            "Clear",
        ],
    );
    ui.separator();
    off(ui, &["Insert Variable", "Insert Cases"]);
    ui.separator();
    off(
        ui,
        &[
            "Search Data Files",
            "Find...",
            "Find Next",
            "Replace...",
            "Go to Case...",
            "Go to Variable...",
        ],
    );
    ui.separator();
    off(ui, &["Options..."]);
}

fn view_menu(ui: &mut Ui, state: &AppState, out: &mut Option<MenuAction>) {
    off(ui, &["Status Bar"]);
    sub_off(ui, &["Toolbars", "Menu Editor..."]);
    off(ui, &["Fonts...", "Grid Lines"]);
    let mut on = state.value_labels;
    if ui.checkbox(&mut on, "Value Labels").clicked() {
        *out = Some(MenuAction::Cmd(Command::ToggleValueLabels));
        ui.close();
    }
    off(ui, &["Mark Imputed Data"]);
    ui.separator();
    off(ui, &["Customize Variable View...", "Variables"]);
}

fn data_menu(ui: &mut Ui) {
    off(
        ui,
        &[
            "Define Variable Properties...",
            "Set Measurement Level for Unknown...",
            "Copy Data Properties...",
            "New Custom Attribute...",
            "Define Date and Time...",
            "Define Multiple Response Sets...",
        ],
    );
    sub_off(ui, &["Validation"]);
    off(
        ui,
        &[
            "Identify Duplicate Cases...",
            "Identify Unusual Cases...",
            "Compare Datasets...",
        ],
    );
    ui.separator();
    off(
        ui,
        &[
            "Sort Cases...",
            "Sort Variables...",
            "Transpose...",
            "Adjust String Widths Across Files",
        ],
    );
    sub_off(ui, &["Merge Files"]);
    off(
        ui,
        &[
            "Restructure...",
            "Rake Weights...",
            "Propensity Score Matching...",
            "Case Control Matching...",
            "Aggregate...",
        ],
    );
    sub_off(ui, &["Orthogonal Design"]);
    off(ui, &["Split into Files", "Copy Dataset"]);
    ui.separator();
    off(ui, &["Split File...", "Select Cases...", "Weight Cases..."]);
}

fn transform_menu(ui: &mut Ui, out: &mut Option<MenuAction>) {
    dialog(ui, "Compute Variable...", DialogKind::Compute, out);
    off(
        ui,
        &[
            "Programmability Transformation...",
            "Count Values within Cases...",
            "Shift Values...",
        ],
    );
    ui.separator();
    off(ui, &["Recode into Same Variables..."]);
    dialog(
        ui,
        "Recode into Different Variables...",
        DialogKind::RecodeDifferent,
        out,
    );
    off(
        ui,
        &[
            "Automatic Recode...",
            "Create Dummy Variables",
            "Visual Binning...",
            "Optimal Binning...",
        ],
    );
    sub_off(ui, &["Prepare Data for Modeling"]);
    ui.separator();
    off(
        ui,
        &[
            "Rank Cases...",
            "Date and Time Wizard...",
            "Create Time Series...",
            "Replace Missing Values...",
            "Random Number Generators...",
        ],
    );
    ui.separator();
    off(ui, &["Run Pending Transforms"]);
}

fn analyze_menu(ui: &mut Ui, out: &mut Option<MenuAction>) {
    sub_off(ui, &["Reports"]);
    sub(ui, "Descriptive Statistics", |ui| {
        dialog(ui, "Frequencies...", DialogKind::Frequencies, out);
        dialog(ui, "Descriptives...", DialogKind::Descriptives, out);
        dialog(ui, "Explore...", DialogKind::Explore, out);
        off(
            ui,
            &[
                "Crosstabs...",
                "TURF Analysis",
                "Ratio...",
                "P-P Plots...",
                "Q-Q Plots...",
            ],
        );
    });
    sub_off(ui, &["Bayesian Statistics", "Tables"]);
    sub(ui, "Compare Means", |ui| {
        off(ui, &["Means..."]);
        dialog(ui, "One-Sample T Test...", DialogKind::OneSampleT, out);
        dialog(
            ui,
            "Independent-Samples T Test...",
            DialogKind::IndependentT,
            out,
        );
        off(ui, &["Summary Independent-Samples T Test..."]);
        dialog(ui, "Paired-Samples T Test...", DialogKind::PairedT, out);
        dialog(ui, "One-Way ANOVA...", DialogKind::OneWayAnova, out);
    });
    sub_off(
        ui,
        &[
            "General Linear Model",
            "Generalized Linear Models",
            "Mixed Models",
            "Correlate",
        ],
    );
    sub(ui, "Regression", |ui| {
        off(ui, &["Automatic Linear Modeling..."]);
        dialog(ui, "Linear...", DialogKind::LinearRegression, out);
        off(
            ui,
            &[
                "Curve Estimation...",
                "Partial Least Squares...",
                "Binary Logistic...",
                "Multinomial Logistic...",
                "Ordinal...",
                "Probit...",
                "Nonlinear...",
                "Weight Estimation...",
                "2-Stage Least Squares...",
                "Optimal Scaling (CATREG)...",
            ],
        );
    });
    sub_off(
        ui,
        &[
            "Loglinear",
            "Neural Networks",
            "Classify",
            "Dimension Reduction",
            "Scale",
        ],
    );
    sub(ui, "Nonparametric Tests", |ui| {
        off(
            ui,
            &[
                "One Sample...",
                "Independent Samples...",
                "Related Samples...",
            ],
        );
        sub(ui, "Legacy Dialogs", |ui| {
            off(ui, &["Chi-square...", "Binomial..."]);
            dialog(ui, "Runs...", DialogKind::NparRuns, out);
            dialog(ui, "1-Sample K-S...", DialogKind::NparOneSampleKs, out);
            off(
                ui,
                &[
                    "2 Independent Samples...",
                    "K Independent Samples...",
                    "2 Related Samples...",
                    "K Related Samples...",
                ],
            );
        });
    });
    sub_off(ui, &["Forecasting", "Survival", "Multiple Response"]);
    off(ui, &["Missing Value Analysis..."]);
    sub_off(ui, &["Multiple Imputation", "Complex Samples"]);
    off(ui, &["Simulation..."]);
    sub_off(
        ui,
        &[
            "Quality Control",
            "Spatial and Temporal Modeling",
            "Direct Marketing",
        ],
    );
}
