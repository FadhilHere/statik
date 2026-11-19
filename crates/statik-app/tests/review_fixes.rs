//! Fixes from the final Plan 3 review: pending edits at Ctrl+S, new output brought into view,
//! keyboard blocked under modal questions, Value Labels "Change" onto an existing value.
mod common;
use dataset::{OrderedValue, VarId};
use egui::accesskit::Role;
use egui::{Event, Id, Key, LayerId, Modifiers, Order, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use statik_app::app::PickRequest;
use statik_app::autosave::{self, Autosave, Instance};
use statik_app::editor::View;
use statik_app::menu::MenuAction;
use statik_app::output::ItemRef;
use statik_app::{AppState, Command, Message, ProcOutput, StatikApp, run_procedure};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("statik-fixes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn skripsi() -> AppState {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("data/skripsi60.csv")))
        .unwrap();
    s
}

/// skripsi60 saved as `<dir>/d.sav` (clean, Save goes to it directly).
fn saved_skripsi(dir: &Path) -> (AppState, PathBuf) {
    let mut s = skripsi();
    let p = dir.join("d.sav");
    s.apply(Command::SaveAs(p.clone())).unwrap();
    assert!(!s.doc.dirty());
    (s, p)
}

/// The app with a picker that records what it was asked and always cancels.
fn harness_with(state: AppState) -> (Harness<'static, StatikApp>, Arc<Mutex<Vec<PickRequest>>>) {
    harness_app(StatikApp::new(state))
}

fn harness_app(app: StatikApp) -> (Harness<'static, StatikApp>, Arc<Mutex<Vec<PickRequest>>>) {
    let asked: Arc<Mutex<Vec<PickRequest>>> = Arc::default();
    let a = asked.clone();
    let app = app.with_picker(Box::new(move |r: &PickRequest| {
        a.lock().unwrap().push(r.clone());
        None
    }));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut StatikApp| app.show(ui), app);
    h.run();
    (h, asked)
}

fn cell(s: &AppState, r: usize, c: usize) -> f64 {
    s.doc.ds().numeric(VarId(c))[r]
}

// ---------------------------------------------------------------------------------------------
// I1: Ctrl+S with an unfinished edit
// ---------------------------------------------------------------------------------------------

#[test]
fn ctrl_s_saves_the_value_still_being_typed_into_a_cell() {
    let _render = common::render_lock();
    let dir = tmp_dir("ctrl-s-cell");
    let (s, path) = saved_skripsi(&dir);
    let (mut h, _) = harness_with(s);
    h.get_by_label("24.25").click();
    h.run();
    h.event(Event::Text("5".into()));
    h.run();
    assert!(
        h.state().state.editor.editing.is_some(),
        "the cell editor is open"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    assert!(st.messages.is_empty(), "{:?}", st.messages);
    assert_eq!(cell(st, 0, 0), 5.0);
    assert!(!st.doc.dirty());
    assert!(!st.title().starts_with('*'), "{}", st.title());
    let (ds, _) = sav_io::read_sav(&path).unwrap();
    assert_eq!(
        ds.numeric(VarId(0))[0],
        5.0,
        "the typed value is in the file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ctrl_s_saves_a_variable_view_field_being_typed() {
    let _render = common::render_lock();
    let dir = tmp_dir("ctrl-s-var");
    let (mut s, path) = saved_skripsi(&dir);
    s.editor.view = View::Variable;
    let (mut h, _) = harness_with(s);
    h.get_by_label("x1").click();
    h.run();
    h.get_by_role(Role::TextInput).type_text("income");
    h.run();
    assert!(h.state().state.editor.var_edit.is_some());
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    assert!(st.messages.is_empty(), "{:?}", st.messages);
    assert!(!st.doc.dirty());
    assert!(!st.title().starts_with('*'), "{}", st.title());
    let (ds, _) = sav_io::read_sav(&path).unwrap();
    assert!(ds.find("income").is_some(), "the new name is in the file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_command_commits_a_pending_cell_edit() {
    let dir = tmp_dir("save-cmd");
    let (mut s, path) = saved_skripsi(&dir);
    s.editor.editing = Some(statik_app::editor::CellEdit {
        row: 0,
        col: 1,
        text: "7".into(),
        from_typing: true,
        focused: true,
    });
    s.apply(Command::Save).unwrap();
    assert!(s.editor.editing.is_none());
    assert!(!s.doc.dirty());
    let (ds, _) = sav_io::read_sav(&path).unwrap();
    assert_eq!(ds.numeric(VarId(1))[0], 7.0);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// I2: new output is brought into view
// ---------------------------------------------------------------------------------------------

fn trivial(s: &mut AppState, title: &str) {
    run_procedure(
        s,
        title,
        |_| ProcOutput::output(vec![output_model::OutputItem::Title(String::from("T"))]),
        format!("{}.", title.to_uppercase()),
    );
}

#[test]
fn new_output_is_selected_and_scrolled_to() {
    let mut s = skripsi();
    trivial(&mut s, "Frequencies");
    trivial(&mut s, "Descriptives");
    let e = s.outputs.len() - 1;
    assert_eq!(s.outputs[e].title, "Descriptives");
    assert_eq!(s.viewer.selected, Some(ItemRef::entry(e)));
    assert_eq!(s.viewer.scroll_to, Some(ItemRef::entry(e)));
    assert!(s.show_output);
}

#[test]
fn a_finished_procedure_raises_the_output_window() {
    let _render = common::render_lock();
    let mut s = skripsi();
    s.show_output = true;
    s.syntax.show = true;
    let (mut h, _) = harness_with(s);
    let out = ViewportId::from_hash_of(statik_app::output::OUTPUT_VIEWPORT);
    // Embedded (kittest): the viewer is an inner window. Put the Syntax window above it.
    let out_layer = LayerId::new(Order::Middle, Id::new(out));
    let syntax_layer = LayerId::new(Order::Middle, Id::new(statik_app::syntax::SYNTAX_TITLE));
    h.ctx.move_to_top(syntax_layer);
    h.run();
    let top = || h.ctx.memory(|m| m.areas().top_layer_id(Order::Middle));
    assert_eq!(top(), Some(syntax_layer));
    let job = h.state().state.procedure_job(
        "Frequencies",
        |_| ProcOutput::output(vec![output_model::OutputItem::Title("T".into())]),
        "FREQUENCIES.".into(),
    );
    h.state_mut().state.queue_job(job);
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let top = h.ctx.memory(|m| m.areas().top_layer_id(Order::Middle));
    assert_eq!(top, Some(out_layer), "the viewer is brought to the front");
    assert!(!h.state().state.raise_output);
}

// ---------------------------------------------------------------------------------------------
// I4 / M1: no Data View keyboard (or shortcuts) under a modal question
// ---------------------------------------------------------------------------------------------

/// Delete, typing and Ctrl+Z/Ctrl+Y change nothing while the question is up.
fn assert_keys_blocked(h: &mut Harness<'static, StatikApp>) {
    let before = cell(&h.state().state, 0, 1);
    let undo = h.state().state.can_undo();
    h.key_press(Key::Delete);
    h.run();
    h.event(Event::Text("y".into()));
    h.run();
    h.event(Event::Text("n".into()));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
    h.run();
    let st = &h.state().state;
    assert_eq!(cell(st, 0, 1), before, "Delete / undo changed the data");
    assert!(st.editor.editing.is_none(), "typing opened a cell editor");
    assert_eq!(st.can_undo(), undo);
}

/// skripsi with one undoable edit (cell (0,0) := 1) and the cursor on cell (0,1).
fn edited_skripsi() -> AppState {
    let mut s = skripsi();
    s.set_cell_text(0, 0, "1");
    assert!(s.can_undo());
    s.editor.jump_to(0, 1);
    s
}

#[test]
fn keys_are_blocked_under_the_save_changes_question() {
    let _render = common::render_lock();
    let (mut h, _) = harness_with(edited_skripsi());
    let ctx = h.ctx.clone();
    h.state_mut().handle(MenuAction::Cmd(Command::New), &ctx);
    h.run();
    h.get_by_label("Save contents of data editor to skripsi60.csv?");
    assert_keys_blocked(&mut h);
    assert_eq!(cell(&h.state().state, 0, 0), 1.0);
}

#[test]
fn keys_are_blocked_under_the_recovery_question() {
    let _render = common::render_lock();
    let root = tmp_dir("recovery-keys");
    let inst = Instance::create(&root).unwrap();
    autosave::write_recovery(inst.dir(), skripsi().doc.ds()).unwrap();
    drop(inst);
    let orphan = autosave::find_orphans(&root).into_iter().next().unwrap();
    let mut app =
        StatikApp::new(edited_skripsi()).with_autosave(Autosave::new(None, Instant::now()));
    app.offer_recovery(orphan);
    let (mut h, _) = harness_app(app);
    h.get_by_label("Statik closed unexpectedly. Recover data?");
    assert_keys_blocked(&mut h);
    assert_eq!(cell(&h.state().state, 0, 0), 1.0, "Ctrl+Z did not undo");
    drop(h);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn keys_are_blocked_under_a_message() {
    let _render = common::render_lock();
    let (mut h, _) = harness_with(edited_skripsi());
    h.state_mut()
        .state
        .push_message(Message::info("Note", "Something happened."));
    h.run();
    assert_keys_blocked(&mut h);
}

#[test]
fn shortcuts_are_ignored_while_a_variable_view_dialog_is_open() {
    let _render = common::render_lock();
    let mut s = edited_skripsi();
    s.editor.view = View::Variable;
    let (mut h, asked) = harness_with(s);
    h.get_by_label("Values of grp").click();
    h.run();
    assert!(h.state().state.editor.var_dialog.is_some());
    h.key_press_modifiers(Modifiers::COMMAND, Key::O);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert!(
        asked.lock().unwrap().is_empty(),
        "{:?}",
        asked.lock().unwrap()
    );
    let app = h.state();
    assert!(app.save_prompt().is_none());
    assert!(app.busy().is_none());
    assert_eq!(cell(&app.state, 0, 0), 1.0);
    assert!(app.state.editor.var_dialog.is_some());
}

// ---------------------------------------------------------------------------------------------
// M2: Value Labels "Change" onto a value that already has a label
// ---------------------------------------------------------------------------------------------

fn type_into(h: &mut Harness<'_, StatikApp>, field: &str, text: &str) {
    let n = h.get_by_role_and_label(Role::TextInput, field);
    n.click();
    h.run();
    for _ in 0..12 {
        h.key_press(Key::Backspace);
    }
    h.run();
    h.get_by_role_and_label(Role::TextInput, field)
        .type_text(text);
    h.run();
}

#[test]
fn value_labels_change_onto_an_existing_value_replaces_it() {
    let _render = common::render_lock();
    let mut s = skripsi();
    s.editor.view = View::Variable;
    let (mut h, _) = harness_with(s);
    h.get_by_label("Values of grp").click();
    h.run();
    for (v, l) in [("1", "Control"), ("2", "Treatment")] {
        type_into(&mut h, "Value:", v);
        type_into(&mut h, "Label:", l);
        h.get_by_label("Add").click();
        h.run();
    }
    // Select 1 = "Control" and change it to 2 = "Both".
    h.get_by_label("1 = \"Control\"").click();
    h.run();
    type_into(&mut h, "Value:", "2");
    type_into(&mut h, "Label:", "Both");
    h.get_by_label("Change").click();
    h.run();
    assert!(h.query_by_label("2 = \"Both\"").is_some());
    assert!(
        h.query_by_label("1 = \"Control\"").is_none(),
        "old entry removed"
    );
    assert!(
        h.query_by_label("2 = \"Treatment\"").is_none(),
        "label replaced"
    );
    h.get_by_label("OK").click();
    h.run();
    let v = h.state().state.doc.ds().var(VarId(4)).clone();
    assert_eq!(v.value_labels.len(), 1, "{:?}", v.value_labels);
    assert_eq!(
        v.value_labels
            .get(&OrderedValue::new(2.0))
            .map(String::as_str),
        Some("Both")
    );
}

#[test]
fn value_labels_change_with_a_bad_value_keeps_the_entry() {
    let _render = common::render_lock();
    let mut s = skripsi();
    s.editor.view = View::Variable;
    let (mut h, _) = harness_with(s);
    h.get_by_label("Values of grp").click();
    h.run();
    type_into(&mut h, "Value:", "1");
    type_into(&mut h, "Label:", "Control");
    h.get_by_label("Add").click();
    h.run();
    h.get_by_label("1 = \"Control\"").click();
    h.run();
    type_into(&mut h, "Value:", "abc");
    h.get_by_label("Change").click();
    h.run();
    assert!(h.state().state.message().is_some());
    assert!(h.query_by_label("1 = \"Control\"").is_some(), "kept");
}

// ---------------------------------------------------------------------------------------------
// M6: a message is shown in one window only
// ---------------------------------------------------------------------------------------------

#[test]
fn messages_show_in_the_output_window_only_when_it_has_the_focus() {
    use egui::ViewportClass;
    use statik_app::output::messages_in_output;
    assert!(messages_in_output(ViewportClass::Immediate, Some(true)));
    assert!(!messages_in_output(ViewportClass::Immediate, Some(false)));
    assert!(!messages_in_output(ViewportClass::Immediate, None));
    // Embedded in the Data Editor: the Data Editor's box is the one shown.
    assert!(!messages_in_output(
        ViewportClass::EmbeddedWindow,
        Some(true)
    ));
}

#[test]
fn an_embedded_viewer_leaves_messages_to_the_data_editor() {
    let _render = common::render_lock();
    let mut s = skripsi();
    s.show_output = true;
    s.push_message(Message::info("Note", "Shown once."));
    let (mut h, _) = harness_with(s);
    h.run();
    assert_eq!(h.query_all_by_label("Shown once.").count(), 1);
    assert!(!h.state().state.viewer.hosts_messages);
}
