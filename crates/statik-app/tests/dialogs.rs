//! Analyze dialogs (egui_kittest): open from the menu, move variables with the arrow buttons,
//! sub-dialogs, OK / Paste / Reset, stale names, and the 800×600 snapshot.
mod common;
use egui::Modifiers;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use output_model::{HeaderNode, OutputItem};
use statik_app::{AppState, Command, DialogKind, StatikApp};
use std::path::Path;

fn harness_sized(state: AppState, w: f32, h: f32) -> Harness<'static, StatikApp> {
    Harness::builder()
        .with_size(egui::vec2(w, h))
        .with_theme(egui::Theme::Light)
        // egui's double-click limit is 0.3 s; kittest's default step is 0.25 s.
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(120)
        .build_ui_state(
            |ui, app: &mut StatikApp| app.show(ui),
            StatikApp::new(state),
        )
}

fn skripsi() -> AppState {
    let mut s = AppState::new();
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/skripsi60.csv");
    s.apply(Command::Open(p)).unwrap();
    s
}

fn harness() -> Harness<'static, StatikApp> {
    let mut h = harness_sized(skripsi(), 1024.0, 760.0);
    h.run();
    h
}

/// Analyze ▸ path..., clicking the last item.
fn open_menu(h: &mut Harness<'_, StatikApp>, path: &[&str]) {
    h.get_by_label("Analyze").click();
    h.run();
    for (i, p) in path.iter().enumerate() {
        if i + 1 < path.len() {
            h.get_by_label(p).hover();
        } else {
            h.get_by_label(p).click();
        }
        h.run();
    }
}

/// The widget labelled `label`: Data View headers and window titles (plain labels) are
/// skipped when a widget has the same text; dialogs are drawn last, so the last one wins.
fn node<'a>(h: &'a Harness<'_, StatikApp>, label: &'a str) -> egui_kittest::Node<'a> {
    let all: Vec<_> = h.query_all_by_label(label).collect();
    assert!(!all.is_empty(), "no node labelled {label:?}");
    let widgets: Vec<_> = all
        .iter()
        .filter(|n| n.accesskit_node().role() != Role::Label)
        .cloned()
        .collect();
    widgets
        .last()
        .cloned()
        .unwrap_or_else(|| *all.last().unwrap())
}

fn click(h: &mut Harness<'_, StatikApp>, label: &str) {
    node(h, label).click();
    h.run();
}

fn ctrl_click(h: &mut Harness<'_, StatikApp>, label: &str) {
    node(h, label).click_modifiers(Modifiers::COMMAND);
    h.run();
}

fn enabled(h: &Harness<'_, StatikApp>, label: &str) -> bool {
    !node(h, label).accesskit_node().is_disabled()
}

/// Move one variable with the arrow button of `target`.
fn move_var(h: &mut Harness<'_, StatikApp>, var: &str, target: &str) {
    click(h, var);
    click(h, &format!("Move to {target}"));
}

fn type_into(h: &mut Harness<'_, StatikApp>, field: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, field).click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, field)
        .type_text(text);
    h.run();
}

fn clear_field(h: &mut Harness<'_, StatikApp>, field: &str) {
    let n = h.get_by_role_and_label(Role::TextInput, field);
    n.click();
    h.run();
    for _ in 0..6 {
        h.key_press(egui::Key::Backspace);
        h.run();
    }
}

/// Click OK, run the job, and return the new output entry.
fn press_ok(h: &mut Harness<'_, StatikApp>) -> statik_app::OutputEntry {
    let before = h.state().state.outputs.len();
    node(h, "OK").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let outs = &h.state().state.outputs;
    assert_eq!(
        outs.len(),
        before + 1,
        "no output; messages: {:?}",
        h.state().state.messages
    );
    assert!(h.state().state.dialogs.open.is_none(), "dialog still open");
    assert!(h.state().state.show_output);
    outs.last().unwrap().clone()
}

fn first_table(e: &statik_app::OutputEntry) -> String {
    e.items
        .iter()
        .find_map(|i| match i {
            OutputItem::Table(t) => Some(t.title.clone()),
            _ => None,
        })
        .expect("a table")
}

fn labels(nodes: &[HeaderNode], out: &mut Vec<String>) {
    for n in nodes {
        out.push(n.label.clone());
        labels(&n.children, out);
    }
}

/// Paste closes the dialog and appends syntax starting with `prefix`; returns the new text.
fn paste(h: &mut Harness<'_, StatikApp>, prefix: &str) -> String {
    let before = h.state().state.syntax.text.clone();
    let outs = h.state().state.outputs.len();
    click(h, "Paste");
    let st = &h.state().state;
    assert!(st.dialogs.open.is_none());
    assert!(st.syntax.show);
    assert_eq!(st.outputs.len(), outs, "Paste must not run");
    let new = st.syntax.text[before.len()..].trim().to_string();
    assert!(new.starts_with(prefix), "{new}");
    node(h, statik_app::syntax::SYNTAX_TITLE);
    new
}

/// Close the Syntax window and the Output window so they do not cover the next dialog.
fn tidy(h: &mut Harness<'_, StatikApp>) {
    h.state_mut().state.syntax.show = false;
    h.state_mut().state.show_output = false;
    h.run();
}

#[test]
fn linear_regression_full_run() {
    let _render = common::render_lock();
    let mut h = harness();
    open_menu(&mut h, &["Regression", "Linear..."]);
    assert_eq!(
        h.state().state.dialogs.open,
        Some(DialogKind::LinearRegression)
    );
    node(&h, "Linear Regression");
    // The menus are disabled while the dialog is open (modal).
    assert!(h.get_by_label("Analyze").accesskit_node().is_disabled());
    assert!(!enabled(&h, "OK"));
    assert!(!enabled(&h, "Paste"));
    move_var(&mut h, "y", "Dependent");
    // The selection is in Dependent now: its arrow points back, the others are disabled.
    assert!(enabled(&h, "Remove from Dependent"));
    assert!(h.query_by_label("Move to Dependent").is_none());
    assert!(!enabled(&h, "Move to Independent(s)"));
    assert!(!enabled(&h, "OK"));
    // Ctrl+click selects several.
    click(&mut h, "x1");
    ctrl_click(&mut h, "x2");
    ctrl_click(&mut h, "x3");
    // The single Dependent box is full: its arrow is disabled.
    assert!(!enabled(&h, "Move to Dependent"));
    click(&mut h, "Move to Independent(s)");
    assert_eq!(
        h.state().state.dialogs.dialogs.regression.picker.vars(1),
        ["x1", "x2", "x3"]
    );
    assert!(enabled(&h, "OK"));
    assert!(!enabled(&h, "Move to Selection Variable"));

    click(&mut h, "Statistics...");
    node(&h, "Linear Regression: Statistics");
    assert!(!enabled(&h, "Estimates"));
    click(&mut h, "Collinearity diagnostics");
    click(&mut h, "Durbin-Watson");
    click(&mut h, "Continue");
    assert!(h.state().state.dialogs.sub.is_none());

    click(&mut h, "Plots...");
    assert!(!enabled(&h, "*ZRESID"));
    move_var(&mut h, "*SRESID", "Y");
    move_var(&mut h, "*ZPRED", "X");
    click(&mut h, "Histogram");
    click(&mut h, "Normal probability plot");
    click(&mut h, "Continue");

    click(&mut h, "Save...");
    // Residuals ▸ Unstandardized (the second "Unstandardized").
    h.get_all_by_label("Unstandardized").nth(1).unwrap().click();
    h.run();
    click(&mut h, "Continue");

    let e = press_ok(&mut h);
    assert_eq!(e.title, "Regression");
    assert_eq!(first_table(&e), "Variables Entered/Removed");
    assert_eq!(
        e.log,
        "REGRESSION\n  /MISSING LISTWISE\n  /STATISTICS COEFF OUTS R ANOVA COLLIN TOL\n  \
         /CRITERIA=PIN(.05) POUT(.10)\n  /NOORIGIN\n  /DEPENDENT y\n  /METHOD=ENTER x1 x2 x3\n  \
         /SCATTERPLOT=(*SRESID ,*ZPRED)\n  /RESIDUALS DURBIN HISTOGRAM(ZRESID) NORMPROB(ZRESID)\n  \
         /SAVE RESID."
    );
    let charts = e
        .items
        .iter()
        .filter(|i| matches!(i, OutputItem::Chart(_)))
        .count();
    assert_eq!(charts, 3);
    assert!(
        e.items
            .iter()
            .any(|i| matches!(i, OutputItem::Table(t) if t.title == "Collinearity Diagnostics"))
    );
    // /SAVE RESID added RES_1 and marked the data changed.
    assert!(h.state().state.doc.ds().find("RES_1").is_some());
    assert!(h.state().state.doc.dirty());
}

#[test]
fn regression_paste_cancel_restore_persist_and_reset() {
    let _render = common::render_lock();
    let mut h = harness();
    open_menu(&mut h, &["Regression", "Linear..."]);
    move_var(&mut h, "y", "Dependent");
    // Double-click moves to the first target that accepts the variable (Independent(s)).
    // (Wait first: a click within 0.6 s of the arrow click would count as a triple click.)
    h.run_steps(45);
    let x1 = node(&h, "x1");
    x1.click();
    x1.click();
    h.run();
    assert_eq!(
        h.state().state.dialogs.dialogs.regression.picker.vars(1),
        ["x1"]
    );
    // Sub-dialog Cancel restores the settings from before it opened.
    click(&mut h, "Statistics...");
    click(&mut h, "Durbin-Watson");
    click(&mut h, "Cancel");
    assert!(!h.state().state.dialogs.dialogs.regression.durbin_watson);
    // Help shows a message.
    click(&mut h, "Help");
    let m = h.state().state.message().cloned().unwrap();
    assert_eq!(m.title, "Linear Regression");
    assert!(m.text.contains("dependent variable"), "{}", m.text);
    h.state_mut().state.dismiss_message();
    h.run();
    let s = paste(&mut h, "REGRESSION");
    assert!(s.contains("/DEPENDENT y\n  /METHOD=ENTER x1."), "{s}");
    // Cancel keeps the settings; reopening shows them again (SPSS remembers them).
    tidy(&mut h);
    open_menu(&mut h, &["Regression", "Linear..."]);
    assert!(enabled(&h, "OK"));
    assert_eq!(
        h.state().state.dialogs.dialogs.regression.picker.vars(0),
        ["y"]
    );
    click(&mut h, "Cancel");
    assert!(h.state().state.dialogs.open.is_none());
    open_menu(&mut h, &["Regression", "Linear..."]);
    click(&mut h, "Reset");
    let r = &h.state().state.dialogs.dialogs.regression;
    assert!(r.picker.vars(0).is_empty() && r.picker.vars(1).is_empty());
    assert!(!enabled(&h, "OK"));
    // y is back in the source list.
    assert!(enabled(&h, "y"));
}

#[test]
fn deleted_variable_is_reported_and_nothing_runs() {
    let _render = common::render_lock();
    let mut h = harness();
    open_menu(&mut h, &["Regression", "Linear..."]);
    move_var(&mut h, "y", "Dependent");
    move_var(&mut h, "x1", "Independent(s)");
    click(&mut h, "Cancel");
    // x1 is deleted in the Data Editor while the dialog keeps its name.
    assert!(h.state_mut().state.delete_variables(0..1));
    h.run();
    open_menu(&mut h, &["Regression", "Linear..."]);
    let outs = h.state().state.outputs.len();
    click(&mut h, "OK");
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    assert_eq!(st.outputs.len(), outs);
    let m = st.message().expect("message");
    assert!(
        m.text.contains("Variable x1 no longer exists"),
        "{}",
        m.text
    );
    assert_eq!(st.dialogs.open, Some(DialogKind::LinearRegression));
}

#[test]
fn edited_variables_are_mapped_by_name_at_ok() {
    let _render = common::render_lock();
    let mut h = harness();
    open_menu(&mut h, &["Regression", "Linear..."]);
    move_var(&mut h, "y", "Dependent");
    move_var(&mut h, "x3", "Independent(s)");
    click(&mut h, "Cancel");
    // A new variable before x1 moves every VarId; a string variable is not movable.
    assert!(h.state_mut().state.insert_variable(0));
    let ds = h.state_mut().state.doc.ds_mut();
    let id = ds.find("VAR00001").unwrap();
    ds.set_type(id, dataset::VarType::String { width: 8 })
        .unwrap();
    ds.var_mut(ds.find("x2").unwrap()).label = Some("Motivation".into());
    h.run();
    open_menu(&mut h, &["Regression", "Linear..."]);
    // Labels show as "Label [name]"; string variables cannot move to numeric targets.
    node(&h, "Motivation [x2]");
    click(&mut h, "VAR00001");
    assert!(!enabled(&h, "Move to Independent(s)"));
    let e = press_ok(&mut h);
    let coef = e
        .items
        .iter()
        .find_map(|i| match i {
            OutputItem::Table(t) if t.title == "Coefficients" => Some(t.clone()),
            _ => None,
        })
        .unwrap();
    let mut rows = vec![];
    labels(&coef.row_headers, &mut rows);
    assert!(rows.iter().any(|r| r == "x3"), "{rows:?}");
    assert!(
        !rows.iter().any(|r| r == "VAR00001" || r == "x2"),
        "{rows:?}"
    );
}

#[test]
fn descriptive_statistics_dialogs() {
    let _render = common::render_lock();
    let mut h = harness();

    open_menu(&mut h, &["Descriptive Statistics", "Frequencies..."]);
    node(&h, "Frequencies");
    assert!(!enabled(&h, "OK"));
    assert!(!enabled(&h, "Statistics..."));
    move_var(&mut h, "grp", "Variable(s)");
    click(&mut h, "Charts...");
    click(&mut h, "Histograms:");
    click(&mut h, "Continue");
    let s = paste(&mut h, "FREQUENCIES");
    assert_eq!(
        s,
        "FREQUENCIES VARIABLES=grp\n  /HISTOGRAM\n  /ORDER=ANALYSIS."
    );
    tidy(&mut h);
    open_menu(&mut h, &["Descriptive Statistics", "Frequencies..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Statistics");
    assert_eq!(e.log, s);
    assert!(e.items.iter().any(|i| matches!(i, OutputItem::Chart(_))));
    tidy(&mut h);

    open_menu(&mut h, &["Descriptive Statistics", "Descriptives..."]);
    move_var(&mut h, "x1", "Variable(s)");
    click(&mut h, "Save standardized values as variables");
    click(&mut h, "Options...");
    click(&mut h, "Skewness");
    click(&mut h, "Continue");
    let s = paste(&mut h, "DESCRIPTIVES");
    assert_eq!(
        s,
        "DESCRIPTIVES VARIABLES=x1\n  /SAVE\n  /STATISTICS=MEAN STDDEV MIN MAX KURTOSIS SKEWNESS."
    );
    tidy(&mut h);
    open_menu(&mut h, &["Descriptive Statistics", "Descriptives..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Descriptive Statistics");
    assert!(h.state().state.doc.ds().find("Zx1").is_some());
    tidy(&mut h);

    open_menu(&mut h, &["Descriptive Statistics", "Explore..."]);
    move_var(&mut h, "y", "Dependent List");
    assert!(!enabled(&h, "Move to Factor List"));
    click(&mut h, "Plots...");
    click(&mut h, "Normality plots with tests");
    click(&mut h, "Continue");
    let s = paste(&mut h, "EXAMINE");
    assert!(
        s.contains("/PLOT NPPLOT") && s.contains("/STATISTICS DESCRIPTIVES"),
        "{s}"
    );
    tidy(&mut h);
    open_menu(&mut h, &["Descriptive Statistics", "Explore..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Case Processing Summary");
    assert!(
        e.items
            .iter()
            .any(|i| matches!(i, OutputItem::Table(t) if t.title == "Tests of Normality"))
    );
}

#[test]
fn compare_means_dialogs() {
    let _render = common::render_lock();
    let mut h = harness();

    open_menu(&mut h, &["Compare Means", "One-Sample T Test..."]);
    move_var(&mut h, "y", "Test Variable(s)");
    clear_field(&mut h, "Test Value:");
    assert!(!enabled(&h, "OK"), "no test value");
    type_into(&mut h, "Test Value:", "60");
    let s = paste(&mut h, "T-TEST");
    assert_eq!(
        s,
        "T-TEST\n  /TESTVAL=60\n  /MISSING=ANALYSIS\n  /VARIABLES=y\n  /CRITERIA=CI(.95)."
    );
    tidy(&mut h);
    open_menu(&mut h, &["Compare Means", "One-Sample T Test..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "One-Sample Statistics");
    tidy(&mut h);

    open_menu(&mut h, &["Compare Means", "Independent-Samples T Test..."]);
    move_var(&mut h, "y", "Test Variable(s)");
    assert!(!enabled(&h, "Define Groups..."));
    move_var(&mut h, "grp", "Grouping Variable");
    node(&h, "grp(? ?)");
    assert!(!enabled(&h, "OK"), "groups not defined");
    click(&mut h, "Define Groups...");
    type_into(&mut h, "Group 1:", "1");
    type_into(&mut h, "Group 2:", "2");
    click(&mut h, "Continue");
    node(&h, "grp(1 2)");
    click(&mut h, "Options...");
    clear_field(&mut h, "Confidence Interval Percentage:");
    type_into(&mut h, "Confidence Interval Percentage:", "90");
    click(&mut h, "Continue");
    let s = paste(&mut h, "T-TEST GROUPS=grp(1 2)");
    assert_eq!(
        s,
        "T-TEST GROUPS=grp(1 2)\n  /MISSING=ANALYSIS\n  /VARIABLES=y\n  /CRITERIA=CI(.90)."
    );
    tidy(&mut h);
    open_menu(&mut h, &["Compare Means", "Independent-Samples T Test..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Group Statistics");
    tidy(&mut h);

    open_menu(&mut h, &["Compare Means", "Paired-Samples T Test..."]);
    assert!(!enabled(&h, "OK"));
    click(&mut h, "x1");
    click(&mut h, "Move to Paired Variables");
    assert!(!enabled(&h, "OK"), "incomplete pair");
    // x1 is still in the source list (variables can be used in several pairs).
    click(&mut h, "x2");
    click(&mut h, "Move to Paired Variables");
    node(&h, "Pair 1: x1, x2");
    assert!(enabled(&h, "OK"));
    let s = paste(&mut h, "T-TEST PAIRS=x1 WITH x2 (PAIRED)");
    assert_eq!(
        s,
        "T-TEST PAIRS=x1 WITH x2 (PAIRED)\n  /CRITERIA=CI(.9500)\n  /MISSING=ANALYSIS."
    );
    tidy(&mut h);
    open_menu(&mut h, &["Compare Means", "Paired-Samples T Test..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Paired Samples Statistics");
    tidy(&mut h);

    open_menu(&mut h, &["Compare Means", "One-Way ANOVA..."]);
    move_var(&mut h, "y", "Dependent List");
    move_var(&mut h, "x1", "Dependent List");
    move_var(&mut h, "grp", "Factor");
    click(&mut h, "Options...");
    click(&mut h, "Descriptive");
    click(&mut h, "Homogeneity of variance test");
    click(&mut h, "Continue");
    let s = paste(&mut h, "ONEWAY");
    assert_eq!(
        s,
        "ONEWAY y x1 BY grp\n  /STATISTICS DESCRIPTIVES HOMOGENEITY\n  /MISSING ANALYSIS."
    );
    tidy(&mut h);
    open_menu(&mut h, &["Compare Means", "One-Way ANOVA..."]);
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Descriptives");
    let titles = e
        .items
        .iter()
        .filter(|i| matches!(i, OutputItem::Title(_)))
        .count();
    assert_eq!(titles, 1);
    let anovas = e
        .items
        .iter()
        .filter(|i| matches!(i, OutputItem::Table(t) if t.title == "ANOVA"))
        .count();
    assert_eq!(anovas, 2);
}

#[test]
fn nonparametric_legacy_dialogs() {
    let _render = common::render_lock();
    let mut h = harness();
    open_menu(
        &mut h,
        &["Nonparametric Tests", "Legacy Dialogs", "1-Sample K-S..."],
    );
    node(&h, "One-Sample Kolmogorov-Smirnov Test");
    move_var(&mut h, "y", "Test Variable List");
    let s = paste(&mut h, "NPAR TESTS");
    assert_eq!(s, "NPAR TESTS\n  /K-S(NORMAL)=y\n  /MISSING ANALYSIS.");
    tidy(&mut h);
    open_menu(
        &mut h,
        &["Nonparametric Tests", "Legacy Dialogs", "1-Sample K-S..."],
    );
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "One-Sample Kolmogorov-Smirnov Test");
    tidy(&mut h);

    open_menu(
        &mut h,
        &["Nonparametric Tests", "Legacy Dialogs", "Runs..."],
    );
    move_var(&mut h, "x1", "Test Variable List");
    let s = paste(&mut h, "NPAR TESTS");
    assert_eq!(s, "NPAR TESTS\n  /RUNS(MEDIAN)=x1\n  /MISSING ANALYSIS.");
    // Two pastes are both kept in the Syntax window.
    assert!(h.state().state.syntax.text.contains("K-S(NORMAL)"));
    tidy(&mut h);
    open_menu(
        &mut h,
        &["Nonparametric Tests", "Legacy Dialogs", "Runs..."],
    );
    let e = press_ok(&mut h);
    assert_eq!(first_table(&e), "Runs Test");
}

#[test]
fn linear_regression_dialog_fits_800x600() {
    let _render = common::render_lock();
    let mut h = harness_sized(skripsi(), 800.0, 600.0);
    h.run();
    open_menu(&mut h, &["Regression", "Linear..."]);
    move_var(&mut h, "y", "Dependent");
    move_var(&mut h, "x1", "Independent(s)");
    move_var(&mut h, "x2", "Independent(s)");
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    for l in [
        "OK",
        "Paste",
        "Reset",
        "Cancel",
        "Help",
        "Linear Regression",
        "Statistics...",
    ] {
        let r = node(&h, l).rect();
        assert!(screen.contains_rect(r), "{l} at {r:?} is clipped");
    }
    h.snapshot("dialog_linear_regression");
}
