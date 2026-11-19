//! Data View and Variable View in a headless window (egui_kittest).
use dataset::{Column, Dataset, OrderedValue, VarId, Variable};
use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use statik_app::editor::View;
use statik_app::{AppState, Command, Doc, StatikApp};
use std::path::Path;
use std::time::Instant;

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

#[test]
fn data_view_shows_headers_and_first_row() {
    let mut h = harness(skripsi());
    h.run();
    for name in ["x1", "x2", "x3", "y", "grp"] {
        assert!(has_label(&h, name), "header {name}");
    }
    // Row number 1 and the first case (24.25, 42.36, 4.68, 62.53, 1).
    assert!(has_label(&h, "1"));
    for v in ["24.25", "42.36", "4.68", "62.53"] {
        assert!(has_label(&h, v), "cell {v}");
    }
    // The grey "var" column after the last variable, and the bottom tabs.
    assert!(has_label(&h, "var"));
    assert!(has_label(&h, "Data View"));
    assert!(has_label(&h, "Variable View"));
}

#[test]
fn typing_into_a_cell_and_enter_moves_down() {
    let mut h = harness(skripsi());
    h.run();
    h.get_by_label("24.25").click();
    h.run();
    h.event(Event::Text("7".into()));
    h.run();
    h.event(Event::Text("5".into()));
    h.run();
    h.key_press(Key::Enter);
    h.run();
    let s = &h.state().state;
    assert_eq!(s.doc.ds().numeric(VarId(0))[0], 75.0);
    assert_eq!(s.editor.cursor, (1, 0));
    assert!(s.can_undo());
    assert!(h.state().state.doc.dirty());
    // Ctrl+Z undoes it.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().state.doc.ds().numeric(VarId(0))[0], 24.25);
    // Invalid text: a message, and the old value is kept.
    h.get_by_label("24.25").click();
    h.run();
    h.event(Event::Text("x".into()));
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().state.message().is_some());
    assert_eq!(h.state().state.doc.ds().numeric(VarId(0))[0], 24.25);
}

#[test]
fn arrows_and_tab_move_the_cursor() {
    let mut h = harness(skripsi());
    h.run();
    h.get_by_label("24.25").click();
    h.run();
    h.key_press(Key::ArrowDown);
    h.key_press(Key::ArrowDown);
    h.run();
    h.key_press(Key::Tab);
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    h.key_press(Key::ArrowUp);
    h.run();
    assert_eq!(h.state().state.editor.cursor, (1, 2));
}

#[test]
fn copy_and_paste_with_clipboard_events() {
    let mut h = harness(skripsi());
    h.run();
    h.get_by_label("24.25").click();
    h.run();
    h.event(Event::Copy);
    h.step();
    let copied: Vec<String> = h
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, ["24.25\r\n"]);
    h.event(Event::Paste("1\t2\n3\t4".into()));
    h.run();
    let ds = h.state().state.doc.ds();
    assert_eq!(ds.numeric(VarId(0))[..2], [1.0, 3.0]);
    assert_eq!(ds.numeric(VarId(1))[..2], [2.0, 4.0]);
}

#[test]
fn double_click_header_opens_its_variable_row() {
    // A short step: kittest's default 0.25 s per frame is too slow for a double click (0.3 s).
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            |ui, app: &mut StatikApp| app.show(ui),
            StatikApp::new(skripsi()),
        );
    h.run();
    let x3 = h.get_by_label("x3");
    x3.click();
    x3.click();
    h.run();
    let e = &h.state().state.editor;
    assert_eq!(e.view, View::Variable);
    assert_eq!(e.var_row, 2);
}

#[test]
fn system_missing_dot_and_value_labels() {
    let mut s = AppState::new();
    let mut ds = Dataset::new();
    let mut v = Variable::numeric("sex");
    v.decimals = 0;
    v.value_labels.insert(OrderedValue::new(1.0), "Male".into());
    ds.add_variable(v, Column::Numeric(vec![1.0, f64::NAN]))
        .unwrap();
    s.doc = Doc::new(ds, None);
    let mut h = harness(s);
    h.run();
    assert!(has_label(&h, "."));
    assert!(!has_label(&h, "Male"));
    h.state_mut()
        .state
        .apply(Command::ToggleValueLabels)
        .unwrap();
    h.run();
    assert!(has_label(&h, "Male"));
}

#[test]
fn large_dataset_scrolls_fast() {
    // 100,000 cases x 50 variables; only the visible cells are drawn.
    let mut ds = Dataset::new();
    for j in 0..50 {
        let col: Vec<f64> = (0..100_000).map(|i| (i * 50 + j) as f64).collect();
        ds.add_variable(Variable::numeric(&format!("v{j}")), Column::Numeric(col))
            .unwrap();
    }
    let mut s = AppState::new();
    s.doc = Doc::new(ds, None);
    let mut h = harness(s);
    h.run();
    // Jump to the middle, then scroll one row per frame.
    let frames = 20;
    let mut total = std::time::Duration::ZERO;
    for i in 0..frames {
        h.state_mut().state.editor.jump_to(50_000 + i, 3);
        let t = Instant::now();
        h.step();
        total += t.elapsed();
    }
    h.run();
    let row = format!("{}", 50_000 + frames); // row numbers are 1-based
    assert!(has_label(&h, &row), "row {row} visible");
    let per_frame = total / frames as u32;
    eprintln!("large_dataset_scrolls_fast: {per_frame:?} per frame (100000 x 50)");
    if !cfg!(debug_assertions) {
        assert!(per_frame.as_millis() < 50, "{per_frame:?} per frame");
    }
}
