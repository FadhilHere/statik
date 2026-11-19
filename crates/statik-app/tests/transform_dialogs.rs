//! Transform and import dialogs (egui_kittest): Compute Variable (functions, parse errors,
//! existing targets, If, undo), Recode into Different Variables, Read CSV File / Read Excel
//! File, and the Compute Variable snapshot.
mod common;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use statik_app::dialogs::import::ImportDialog;
use statik_app::{AppState, Command, DialogKind, StatikApp};
use std::path::{Path, PathBuf};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn harness_sized(state: AppState, w: f32, h: f32) -> Harness<'static, StatikApp> {
    Harness::builder()
        .with_size(egui::vec2(w, h))
        .with_theme(egui::Theme::Light)
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(120)
        .build_ui_state(
            |ui, app: &mut StatikApp| app.show(ui),
            StatikApp::new(state),
        )
}

fn skripsi() -> AppState {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("data/skripsi60.csv")))
        .unwrap();
    s
}

/// skripsi60 after Linear Regression y on x1 x2 x3 with Save ▸ Unstandardized residuals.
fn with_residuals() -> AppState {
    let mut s = skripsi();
    assert!(s.dialogs.open(DialogKind::LinearRegression));
    let r = &mut s.dialogs.dialogs.regression;
    r.picker.targets[0].vars = vec!["y".into()];
    r.picker.targets[1].vars = vec!["x1".into(), "x2".into(), "x3".into()];
    r.save.residual = true;
    assert!(statik_app::dialogs::ok(&mut s));
    let job = s.pending_job.take().unwrap();
    s.finish(job.run_inline()).unwrap();
    assert!(s.doc.ds().find("RES_1").is_some());
    s.show_output = false;
    s
}

fn harness(state: AppState) -> Harness<'static, StatikApp> {
    let mut h = harness_sized(state, 1024.0, 760.0);
    h.run();
    h
}

fn open_menu(h: &mut Harness<'_, StatikApp>, top: &str, path: &[&str]) {
    h.get_by_label(top).click();
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

/// The widget labelled `label`, preferring widgets over plain labels; the last one wins.
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

fn enabled(h: &Harness<'_, StatikApp>, label: &str) -> bool {
    !node(h, label).accesskit_node().is_disabled()
}

/// Double-click `label` (after a pause, so it is not counted as a triple click).
fn double_click(h: &mut Harness<'_, StatikApp>, label: &str) {
    h.run_steps(45);
    let n = node(h, label);
    n.click();
    n.click();
    h.run();
}

/// The text field (one- or multi-line) labelled `field`.
fn field<'a>(h: &'a Harness<'_, StatikApp>, field: &'a str) -> egui_kittest::Node<'a> {
    h.query_all_by_label(field)
        .rfind(|n| {
            matches!(
                n.accesskit_node().role(),
                Role::TextInput | Role::MultilineTextInput
            )
        })
        .unwrap_or_else(|| panic!("no text field labelled {field:?}"))
}

fn type_into(h: &mut Harness<'_, StatikApp>, name: &str, text: &str) {
    field(h, name).click();
    h.run();
    field(h, name).type_text(text);
    h.run();
}

fn compute<'a>(h: &'a Harness<'_, StatikApp>) -> &'a statik_app::dialogs::compute::ComputeDialog {
    &h.state().state.dialogs.dialogs.compute
}

/// Click OK and run the queued job.
fn ok_and_wait(h: &mut Harness<'_, StatikApp>) {
    node(h, "OK").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
}

fn column(s: &AppState, name: &str) -> Vec<f64> {
    let ds = s.doc.ds();
    ds.numeric(ds.find(name).unwrap()).to_vec()
}

#[test]
fn compute_abs_res_from_function_list_and_undo() {
    let _render = common::render_lock();
    let mut h = harness(with_residuals());
    let n_vars = h.state().state.doc.ds().n_vars();
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    assert_eq!(h.state().state.dialogs.open, Some(DialogKind::Compute));
    node(&h, "Compute Variable");
    assert!(!enabled(&h, "OK"));
    type_into(&mut h, "Target Variable:", "ABS_RES");
    // Function group ▸ Arithmetic, double-click Abs: "ABS(?)" with the ? selected.
    click(&mut h, "Arithmetic");
    click(&mut h, "Abs");
    let help = &compute(&h).func_help();
    assert!(help.contains("absolute value"), "{help}");
    double_click(&mut h, "Abs");
    assert_eq!(compute(&h).expr.text, "ABS(?)");
    assert_eq!(compute(&h).expr.selection(), (4, 5));
    // Double-clicking a variable replaces the selected ?.
    double_click(&mut h, "Unstandardized Residual [RES_1]");
    assert_eq!(compute(&h).expr.text, "ABS(RES_1)");
    assert!(enabled(&h, "OK"));
    ok_and_wait(&mut h);

    let st = &h.state().state;
    assert!(st.dialogs.open.is_none(), "messages: {:?}", st.messages);
    assert_eq!(st.doc.ds().n_vars(), n_vars + 1);
    let res = column(st, "RES_1");
    let abs = column(st, "ABS_RES");
    assert!(
        res.iter()
            .zip(&abs)
            .all(|(r, a)| (r.abs() - a).abs() < 1e-12)
    );
    assert!(res.iter().any(|r| *r < 0.0));
    let log = st.outputs.last().unwrap();
    assert_eq!(log.title, "Log");
    assert_eq!(log.log, "COMPUTE ABS_RES=ABS(RES_1).\nEXECUTE.");
    assert!(st.doc.dirty());
    // The new column is in the Data View.
    h.state_mut().state.show_output = false;
    h.run();
    assert!(h.query_by_label("ABS_RES").is_some());

    // Undo removes the new variable; Redo brings it back.
    assert!(h.state().state.can_undo());
    h.state_mut().state.apply(Command::Undo).unwrap();
    assert!(h.state().state.doc.ds().find("ABS_RES").is_none());
    assert_eq!(h.state().state.doc.ds().n_vars(), n_vars);
    h.state_mut().state.apply(Command::Redo).unwrap();
    assert_eq!(column(&h.state().state, "ABS_RES"), abs);
}

#[test]
fn compute_parse_error_shows_character_position_and_changes_nothing() {
    let _render = common::render_lock();
    let mut h = harness(skripsi());
    let n_vars = h.state().state.doc.ds().n_vars();
    let outs = h.state().state.outputs.len();
    let rev = h.state().state.doc.rev();
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    type_into(&mut h, "Target Variable:", "z");
    // The * is at byte 9 but character 7 (é takes two bytes).
    type_into(&mut h, "Numeric Expression:", "'éé' + * 2");
    click(&mut h, "OK");
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    let m = st.message().expect("an error message").clone();
    assert!(m.text.contains("character 8"), "{}", m.text);
    assert_eq!(st.doc.ds().n_vars(), n_vars);
    assert_eq!(st.outputs.len(), outs);
    assert_eq!(st.doc.rev(), rev);
    assert_eq!(st.dialogs.open, Some(DialogKind::Compute));
    h.state_mut().state.dismiss_message();
    h.run();
    // The cursor was put at the error.
    assert_eq!(compute(&h).expr.selection(), (7, 7));

    // An unknown variable is reported by the run; the data stays as it was.
    let d = &mut h.state_mut().state.dialogs.dialogs.compute;
    d.expr.text = "nosuch + 1".into();
    h.run();
    ok_and_wait(&mut h);
    let st = &h.state().state;
    let m = st.message().expect("an error message");
    assert!(m.text.contains("nosuch"), "{}", m.text);
    assert_eq!(st.doc.ds().n_vars(), n_vars);
    assert_eq!(st.doc.rev(), rev);
}

#[test]
fn compute_existing_target_asks_and_undo_restores_old_values() {
    let _render = common::render_lock();
    let mut h = harness(skripsi());
    let old = column(&h.state().state, "x1");
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    type_into(&mut h, "Target Variable:", "x1");
    type_into(&mut h, "Numeric Expression:", "x1 * 2");
    click(&mut h, "Type & Label...");
    type_into(&mut h, "Label:", "Doubled");
    click(&mut h, "Continue");
    click(&mut h, "OK");
    node(&h, "Change existing variable?");
    // No: nothing runs, the dialog stays open.
    click(&mut h, "No");
    assert!(h.state().state.pending_job.is_none());
    assert_eq!(h.state().state.dialogs.open, Some(DialogKind::Compute));
    click(&mut h, "OK");
    click(&mut h, "Yes");
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    let new = column(st, "x1");
    assert!(
        old.iter()
            .zip(&new)
            .all(|(a, b)| (a * 2.0 - b).abs() < 1e-12)
    );
    let x1 = st.doc.ds().find("x1").unwrap();
    assert_eq!(st.doc.ds().var(x1).label.as_deref(), Some("Doubled"));
    assert_eq!(
        st.outputs.last().unwrap().log,
        "COMPUTE x1=x1 * 2.\nVARIABLE LABELS x1 'Doubled'.\nEXECUTE."
    );
    h.state_mut().state.apply(Command::Undo).unwrap();
    let st = &h.state().state;
    assert_eq!(column(st, "x1"), old);
    assert_eq!(st.doc.ds().var(x1).label, None);
    assert_eq!(st.doc.ds().var(x1).name, "x1");
}

#[test]
fn compute_if_condition_and_paste() {
    let _render = common::render_lock();
    let mut h = harness(skripsi());
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    type_into(&mut h, "Target Variable:", "hi");
    type_into(&mut h, "Numeric Expression:", "1");
    click(&mut h, "If...");
    node(&h, "Compute Variable: If Cases");
    // The condition box is disabled until "Include if" is chosen.
    h.get_by_role_and_label(Role::RadioButton, "Include if case satisfies condition:")
        .click();
    h.run();
    type_into(&mut h, "Include if case satisfies condition:", "grp = 2");
    click(&mut h, "Continue");
    assert!(compute(&h).use_if);
    click(&mut h, "Paste");
    let st = &h.state().state;
    assert!(st.dialogs.open.is_none());
    assert_eq!(st.syntax.text.trim(), "IF (grp = 2) hi=1.\nEXECUTE.");
    // Reopen and run: only cases of group 2 get 1, the rest are system-missing.
    h.state_mut().state.syntax.show = false;
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    ok_and_wait(&mut h);
    let st = &h.state().state;
    let grp = column(st, "grp");
    let hi = column(st, "hi");
    for (g, v) in grp.iter().zip(&hi) {
        if *g == 2.0 {
            assert_eq!(*v, 1.0);
        } else {
            assert!(v.is_nan());
        }
    }
    assert_eq!(
        st.outputs.last().unwrap().log,
        "IF (grp = 2) hi=1.\nEXECUTE."
    );
}

#[test]
fn recode_into_different_variable_and_undo() {
    let _render = common::render_lock();
    let mut h = harness(skripsi());
    let n_vars = h.state().state.doc.ds().n_vars();
    open_menu(&mut h, "Transform", &["Recode into Different Variables..."]);
    node(&h, "Recode into Different Variables");
    assert!(!enabled(&h, "OK"));
    click(&mut h, "x3");
    click(&mut h, "Move to Numeric Variable -> Output Variable");
    node(&h, "x3 --> ?");
    assert!(!enabled(&h, "Change"));
    type_into(&mut h, "Name:", "x3_cat");
    type_into(&mut h, "Label:", "Kategori x3");
    click(&mut h, "Change");
    node(&h, "x3 --> x3_cat");
    // No rules yet: OK stays disabled.
    assert!(!enabled(&h, "OK"));

    click(&mut h, "Old and New Values...");
    node(&h, "Recode into Different Variables: Old and New Values");
    type_into(&mut h, "Range from", "1");
    type_into(&mut h, "Range through", "3");
    type_into(&mut h, "New Value", "1");
    click(&mut h, "Add");
    node(&h, "1 thru 3 --> 1");
    click(&mut h, "All other values");
    field(&h, "New Value").click();
    h.run();
    for _ in 0..3 {
        h.key_press(egui::Key::Backspace);
        h.run();
    }
    type_into(&mut h, "New Value", "2");
    click(&mut h, "Add");
    node(&h, "ELSE --> 2");
    click(&mut h, "Continue");
    assert!(enabled(&h, "OK"));
    ok_and_wait(&mut h);

    let st = &h.state().state;
    assert!(st.dialogs.open.is_none(), "messages: {:?}", st.messages);
    assert_eq!(st.doc.ds().n_vars(), n_vars + 1);
    let x3 = column(st, "x3");
    let cat = column(st, "x3_cat");
    for (x, c) in x3.iter().zip(&cat) {
        let want = if (1.0..=3.0).contains(x) { 1.0 } else { 2.0 };
        assert_eq!(*c, want, "x3 = {x}");
    }
    let id = st.doc.ds().find("x3_cat").unwrap();
    assert_eq!(st.doc.ds().var(id).label.as_deref(), Some("Kategori x3"));
    assert_eq!(
        st.outputs.last().unwrap().log,
        "RECODE x3 (1 thru 3=1) (ELSE=2) INTO x3_cat.\nVARIABLE LABELS x3_cat 'Kategori x3'.\nEXECUTE."
    );
    h.state_mut().state.apply(Command::Undo).unwrap();
    assert!(h.state().state.doc.ds().find("x3_cat").is_none());
    assert_eq!(h.state().state.doc.ds().n_vars(), n_vars);
}

#[test]
fn recode_syntax_for_special_values() {
    use statik_app::dialogs::ProcDialog;
    use statik_app::dialogs::recode::{OutSpec, RecodeDialog};
    use transform::{NewValue, OldValue};
    let s = skripsi();
    let mut d = RecodeDialog::default();
    d.picker.targets[0].vars = vec!["x1".into(), "x2".into()];
    d.outputs = vec![
        OutSpec::new("x1", "a", ""),
        OutSpec::new("x2", "b", "It's b"),
    ];
    d.rules = vec![
        (OldValue::Lowest(2.0), NewValue::Value(1.0)),
        (OldValue::Highest(8.5), NewValue::Copy),
        (OldValue::SysMis, NewValue::SysMis),
        (OldValue::Missing, NewValue::Value(9.0)),
        (OldValue::Value(5.0), NewValue::SysMis),
        (OldValue::Else, NewValue::Copy),
    ];
    assert!(d.ready());
    assert_eq!(
        d.syntax(s.doc.ds()),
        "RECODE x1 x2 (LOWEST thru 2=1) (8.5 thru HIGHEST=COPY) (SYSMIS=SYSMIS) (MISSING=9) \
         (5=SYSMIS) (ELSE=COPY) INTO a b.\nVARIABLE LABELS b 'It''s b'.\nEXECUTE."
    );
}

#[test]
fn import_csv_dialog_reads_indonesian_semicolon_file() {
    let _render = common::render_lock();
    let mut h = harness(AppState::new());
    h.state_mut()
        .state
        .open_import_csv(fixture("data/indo_semicolon.csv"));
    h.run();
    node(&h, "Read CSV File");
    // Preview: detected names and the first rows as in the file.
    node(&h, "Nilai_Ujian");
    node(&h, "78,5");
    let Some(ImportDialog::Csv(d)) = &h.state().state.import else {
        panic!("no CSV dialog")
    };
    let p = d.preview.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(p.delimiter, b';');
    assert!(p.decimal_comma);
    // Without the header line the names become VAR00001...; the preview follows the option.
    click(&mut h, "First line contains variable names");
    node(&h, "VAR00001");
    assert!(h.query_by_label("Nilai_Ujian").is_none());
    click(&mut h, "First line contains variable names");
    node(&h, "Nilai_Ujian");

    ok_and_wait(&mut h);
    let st = &h.state().state;
    assert!(st.import.is_none());
    let ds = st.doc.ds();
    assert_eq!(ds.n_vars(), 3);
    assert_eq!(ds.n_cases(), 5);
    for i in 0..3 {
        assert_eq!(
            ds.var(dataset::VarId(i)).var_type,
            dataset::VarType::Numeric
        );
    }
    assert_eq!(column(st, "Nilai_Ujian")[0], 78.5);
    assert!(column(st, "Nilai_Ujian")[4].is_nan());
    assert!(st.needs_save_as());
    let log = &st.outputs.last().unwrap().log;
    assert!(log.contains("SET DECIMAL COMMA."), "{log}");
    assert!(log.contains("/DELIMITERS=\";\""), "{log}");
    assert!(log.contains("/FIRSTCASE=2"), "{log}");
}

#[test]
fn import_csv_dialog_options_override_detection() {
    let _render = common::render_lock();
    let mut h = harness(AppState::new());
    h.state_mut()
        .state
        .open_import_csv(fixture("data/indo_semicolon.csv"));
    h.run();
    // A decimal point makes "78,5" text: Nilai_Ujian becomes a string variable.
    if let Some(ImportDialog::Csv(d)) = &mut h.state_mut().state.import {
        d.decimal = statik_app::dialogs::import::Decimal::Period;
    }
    h.run();
    ok_and_wait(&mut h);
    let ds = h.state().state.doc.ds();
    let id = ds.find("Nilai_Ujian").unwrap();
    assert!(matches!(
        ds.var(id).var_type,
        dataset::VarType::String { .. }
    ));
}

#[test]
fn import_excel_dialog() {
    let _render = common::render_lock();
    let mut h = harness(AppState::new());
    h.state_mut()
        .state
        .open_import_excel(fixture("data/sample.xlsx"));
    // The sheet names are read in the background.
    h.state_mut().state.poll_import(true);
    h.run();
    node(&h, "Read Excel File");
    let Some(ImportDialog::Excel(d)) = &h.state().state.import else {
        panic!("no Excel dialog")
    };
    assert!(!d.sheets.is_empty());
    assert!(d.header);
    let first_sheet = d.sheets[0].clone();
    ok_and_wait(&mut h);
    let st = &h.state().state;
    assert!(st.import.is_none(), "messages: {:?}", st.messages);
    assert!(st.doc.ds().n_vars() > 0);
    let log = &st.outputs.last().unwrap().log;
    assert!(log.contains("/TYPE=XLSX"), "{log}");
    assert!(
        log.contains(&format!("/SHEET=name '{first_sheet}'")),
        "{log}"
    );
    assert!(log.contains("/READNAMES=ON"), "{log}");
}

#[test]
fn compute_dialog_fits_800x600() {
    let _render = common::render_lock();
    let mut h = harness_sized(with_residuals(), 800.0, 600.0);
    h.run();
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    type_into(&mut h, "Target Variable:", "ABS_RES");
    type_into(&mut h, "Numeric Expression:", "ABS(RES_1)");
    click(&mut h, "Arithmetic");
    click(&mut h, "Abs");
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    for l in [
        "OK",
        "Paste",
        "Reset",
        "Cancel",
        "Help",
        "Compute Variable",
        "If...",
        "Type & Label...",
    ] {
        let r = node(&h, l).rect();
        assert!(screen.contains_rect(r), "{l} at {r:?} is clipped");
    }
    h.snapshot("dialog_compute_variable");
}

#[test]
fn import_excel_unreadable_workbook_closes_with_message() {
    let mut s = AppState::new();
    s.open_import_excel(fixture("data/indo_semicolon.csv"));
    assert!(s.import.is_some());
    s.poll_import(true);
    assert!(s.import.is_none());
    let m = s.message().expect("an error message");
    assert!(m.text.contains("Cannot read"), "{}", m.text);
}

#[test]
fn compute_condition_error_opens_if_with_cursor() {
    let _render = common::render_lock();
    let mut h = harness(skripsi());
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    let d = &mut h.state_mut().state.dialogs.dialogs.compute;
    d.target = "z".into();
    d.expr.text = "1".into();
    d.use_if = true;
    d.cond.text = "grp = = 2".into();
    h.run();
    click(&mut h, "OK");
    let st = &h.state().state;
    assert!(
        st.message().unwrap().text.contains("character 7"),
        "{:?}",
        st.message()
    );
    assert_eq!(st.dialogs.sub, Some("If..."));
    assert!(st.pending_job.is_none());
    h.state_mut().state.dismiss_message();
    h.run();
    node(&h, "Compute Variable: If Cases");
    assert_eq!(compute(&h).cond.selection(), (6, 6));
}

#[test]
fn transform_conflicting_with_an_edit_adds_no_log() {
    let mut s = skripsi();
    assert!(s.dialogs.open(DialogKind::Compute));
    let d = &mut s.dialogs.dialogs.compute;
    d.target = "z".into();
    d.expr.text = "x1 + 1".into();
    assert!(statik_app::dialogs::ok(&mut s));
    let job = s.pending_job.take().unwrap();
    // The data is edited while the job runs.
    s.doc
        .ds_mut()
        .set_numeric(dataset::VarId(0), 0, 99.0)
        .unwrap();
    let outs = s.outputs.len();
    s.finish(job.run_inline()).unwrap();
    assert_eq!(s.outputs.len(), outs);
    assert!(s.doc.ds().find("z").is_none());
    assert_eq!(s.message().unwrap().title, "Data changed");
    assert!(!s.can_undo());
}
