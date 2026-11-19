//! Variable View and its dialogs in a headless window (egui_kittest).
mod common;
use dataset::{MissingSpec, OrderedValue, VarId, VarType};
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use statik_app::editor::View;
use statik_app::{AppState, Command, StatikApp};
use std::path::Path;

fn harness(state: AppState) -> Harness<'static, StatikApp> {
    Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_theme(egui::Theme::Light)
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

fn has_label(h: &Harness<'_, StatikApp>, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn var_view() -> Harness<'static, StatikApp> {
    let mut s = skripsi();
    s.editor.view = View::Variable;
    let mut h = harness(s);
    h.run();
    h
}

fn type_into(h: &mut Harness<'_, StatikApp>, field: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, field).click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, field)
        .type_text(text);
    h.run();
}

#[test]
fn variable_view_lists_five_variables() {
    let _render = common::render_lock();
    let mut h = harness(skripsi());
    h.run();
    h.get_by_label("Variable View").click();
    h.run();
    assert_eq!(h.state().state.editor.view, View::Variable);
    for c in [
        "Name", "Type", "Width", "Decimals", "Label", "Values", "Missing", "Columns", "Align",
        "Measure", "Role",
    ] {
        assert!(has_label(&h, c), "column {c}");
    }
    for name in ["x1", "x2", "x3", "y", "grp"] {
        assert!(has_label(&h, name), "variable row {name}");
    }
    // Exactly five rows show a type: the rows after them are empty.
    assert_eq!(h.query_all_by_label("Numeric").count(), 5);
    h.get_by_label("Data View").click();
    h.run();
    assert_eq!(h.state().state.editor.view, View::Data);
}

#[test]
fn rename_in_name_cell() {
    let _render = common::render_lock();
    let mut h = var_view();
    h.get_by_label("x1").click();
    h.run();
    // The whole name is selected when editing starts, so typing replaces it.
    h.get_by_role(Role::TextInput).type_text("income");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().state.doc.ds().var(VarId(0)).name, "income");
    // An invalid name: error message, old name kept.
    h.get_by_label("income").click();
    h.run();
    h.get_by_role(Role::TextInput).type_text("1bad");
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().state.doc.ds().var(VarId(0)).name, "income");
    let m = h.state().state.message().expect("error message");
    assert!(m.text.contains("1bad"), "{m:?}");
}

#[test]
fn value_labels_dialog_adds_a_label() {
    let _render = common::render_lock();
    let mut h = var_view();
    h.get_by_label("Values of grp").click();
    h.run();
    assert!(has_label(&h, "Value Labels"));
    type_into(&mut h, "Value:", "1");
    type_into(&mut h, "Label:", "Control");
    h.get_by_label("Add").click();
    h.run();
    assert!(has_label(&h, "1 = \"Control\""));
    h.get_by_label("OK").click();
    h.run();
    let v = h.state().state.doc.ds().var(VarId(4)).clone();
    assert_eq!(
        v.value_labels
            .get(&OrderedValue::new(1.0))
            .map(String::as_str),
        Some("Control")
    );
    assert!(!has_label(&h, "Value Labels"), "dialog closed");
    // Undoable as one step.
    h.state_mut().state.apply(Command::Undo).unwrap();
    assert!(
        h.state()
            .state
            .doc
            .ds()
            .var(VarId(4))
            .value_labels
            .is_empty()
    );
}

#[test]
fn missing_values_dialog_sets_discrete_values() {
    let _render = common::render_lock();
    let mut h = var_view();
    h.get_by_label("Missing of y").click();
    h.run();
    assert!(has_label(&h, "Missing Values"));
    h.get_by_label("Discrete missing values").click();
    h.run();
    type_into(&mut h, "Discrete value 1", "99");
    type_into(&mut h, "Discrete value 2", "98");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().state.doc.ds().var(VarId(3)).missing,
        MissingSpec::Discrete(vec![99.0, 98.0])
    );
    assert!(has_label(&h, "99.00, 98.00"));
}

#[test]
fn variable_type_dialog_changes_to_string() {
    let _render = common::render_lock();
    let mut h = var_view();
    h.get_by_label("Type of x1").click();
    h.run();
    assert!(has_label(&h, "Variable Type"));
    assert!(h.get_by_label("Comma").accesskit_node().is_disabled());
    assert!(h.get_by_label("Date").accesskit_node().is_disabled());
    h.get_by_label("String").click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    let ds = h.state().state.doc.ds();
    assert!(matches!(ds.var(VarId(0)).var_type, VarType::String { .. }));
    assert!(has_label(&h, "String"));
    // Undo restores the numeric column exactly.
    h.state_mut().state.apply(Command::Undo).unwrap();
    assert_eq!(h.state().state.doc.ds().numeric(VarId(0))[0], 24.25);
}
