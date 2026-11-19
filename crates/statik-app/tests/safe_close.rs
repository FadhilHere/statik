//! The "Save contents of data editor to ...?" question (Task 7), at every entry point that would
//! drop unsaved data: closing the window, File ▸ Exit, File ▸ New, File ▸ Open, Import CSV and
//! Import Excel. File dialogs are replaced by a scripted picker ([`StatikApp::with_picker`]).
mod common;
use egui::{ViewportCommand, ViewportEvent, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use statik_app::app::{Guarded, PickRequest};
use statik_app::dialogs::import::ImportDialog;
use statik_app::menu::MenuAction;
use statik_app::{AppState, Command, StatikApp};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

/// A fresh, empty temporary directory.
fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("statik-close-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn skripsi() -> AppState {
    let mut s = AppState::new();
    s.apply(Command::ImportCsv(
        fixture("data/skripsi60.csv"),
        Default::default(),
    ))
    .unwrap();
    // These tests are about the data; the import's log output counts as saved here (the
    // output question has its own tests below).
    s.output_dirty = false;
    s
}

fn set_first_cell(s: &mut AppState, v: f64) {
    s.doc
        .ds_mut()
        .set_numeric(dataset::VarId(0), 0, v)
        .unwrap();
}

fn dirty_skripsi() -> AppState {
    let mut s = skripsi();
    set_first_cell(&mut s, 42.0);
    assert!(s.doc.dirty());
    s
}

fn harness(app: StatikApp) -> Harness<'static, StatikApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut StatikApp| app.show(ui), app);
    h.run();
    h
}

// ---------------------------------------------------------------------------------------------
// "Save contents of data editor to ...?"
// ---------------------------------------------------------------------------------------------

/// A picker answering from a script, recording what was asked.
#[derive(Clone, Default)]
struct Script {
    answers: Arc<Mutex<VecDeque<Option<PathBuf>>>>,
    asked: Arc<Mutex<Vec<PickRequest>>>,
}

impl Script {
    fn new(answers: Vec<Option<PathBuf>>) -> Self {
        Script {
            answers: Arc::new(Mutex::new(answers.into())),
            asked: Arc::default(),
        }
    }
    fn asked(&self) -> Vec<PickRequest> {
        self.asked.lock().unwrap().clone()
    }
}

fn app_with(state: AppState, script: &Script) -> Harness<'static, StatikApp> {
    let sc = script.clone();
    let app = StatikApp::new(state).with_picker(Box::new(move |req: &PickRequest| {
        sc.asked.lock().unwrap().push(req.clone());
        sc.answers.lock().unwrap().pop_front().flatten()
    }));
    harness(app)
}

const PROMPT: &str = "Save contents of data editor to skripsi60.csv?";

fn request_close(h: &mut Harness<'_, StatikApp>) -> Vec<ViewportCommand> {
    h.input_mut()
        .viewports
        .entry(ViewportId::ROOT)
        .or_default()
        .events
        .push(ViewportEvent::Close);
    h.step();
    let cmds = root_commands(h);
    h.run();
    cmds
}

fn root_commands(h: &Harness<'_, StatikApp>) -> Vec<ViewportCommand> {
    h.output()
        .viewport_output
        .get(&ViewportId::ROOT)
        .map(|v| v.commands.clone())
        .unwrap_or_default()
}

fn handle(h: &mut Harness<'_, StatikApp>, a: MenuAction) {
    let ctx = h.ctx.clone();
    h.state_mut().handle(a, &ctx);
    h.run();
}

/// The save at exit logged a SAVE command to the output, so the output question follows.
fn answer_no_to_the_output_question(h: &mut Harness<'_, StatikApp>) {
    assert!(!h.state().closing());
    assert!(h.state().output_prompt());
    h.get_by_label("Save contents of output viewer to Output1?");
    h.get_by_label("No").click();
    h.run();
}

fn prompt_buttons_shown(h: &Harness<'_, StatikApp>) {
    h.get_by_label(PROMPT);
    for b in ["Yes", "No", "Cancel"] {
        h.get_by_label(b);
    }
}

#[test]
fn close_when_clean_closes_without_asking() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(skripsi(), &sc);
    let cmds = request_close(&mut h);
    assert!(!cmds.contains(&ViewportCommand::CancelClose));
    assert!(h.state().closing());
    assert!(h.query_by_label(PROMPT).is_none());
}

#[test]
fn close_when_dirty_asks_and_cancel_keeps_the_app_open() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(dirty_skripsi(), &sc);
    let cmds = request_close(&mut h);
    assert!(cmds.contains(&ViewportCommand::CancelClose), "{cmds:?}");
    assert_eq!(h.state().save_prompt(), Some(Guarded::Exit));
    prompt_buttons_shown(&h);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.query_by_label(PROMPT).is_none());
    assert!(!h.state().closing());
    assert!(!h.state().state.exit_requested);
    assert!(h.state().state.doc.dirty());
    assert!(sc.asked().is_empty());
    // Escape cancels too.
    request_close(&mut h);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_label(PROMPT).is_none());
    assert!(!h.state().closing());
}

#[test]
fn close_when_dirty_no_closes_without_saving() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(dirty_skripsi(), &sc);
    request_close(&mut h);
    h.get_by_label("No").click();
    h.step();
    assert!(root_commands(&h).contains(&ViewportCommand::Close));
    h.run();
    assert!(h.state().closing());
    assert!(h.state().state.doc.dirty());
    assert!(sc.asked().is_empty());
    // The repeated close request now goes through.
    let cmds = request_close(&mut h);
    assert!(!cmds.contains(&ViewportCommand::CancelClose));
}

#[test]
fn close_yes_saves_the_sav_file_then_closes() {
    let _render = common::render_lock();
    let dir = tmp_dir("yes-sav");
    let path = dir.join("data.sav");
    std::fs::copy(fixture("sav/basic.sav"), &path).unwrap();
    let mut s = AppState::new();
    s.apply(Command::Open(path.clone())).unwrap();
    s.doc.touch();
    let sc = Script::default();
    let mut h = app_with(s, &sc);
    request_close(&mut h);
    h.get_by_label("Save contents of data editor to data.sav?");
    h.get_by_label("Yes").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    assert!(sc.asked().is_empty(), "a .sav document is saved in place");
    assert!(!h.state().state.doc.dirty());
    answer_no_to_the_output_question(&mut h);
    assert!(h.state().closing());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_yes_on_imported_data_asks_for_a_file_name() {
    let _render = common::render_lock();
    let dir = tmp_dir("yes-csv");
    let sc = Script::new(vec![Some(dir.join("skripsi"))]);
    let mut h = app_with(dirty_skripsi(), &sc);
    request_close(&mut h);
    h.get_by_label("Yes").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    assert!(matches!(sc.asked()[..], [PickRequest::SaveData { .. }]));
    assert!(dir.join("skripsi.sav").exists());
    assert!(!h.state().state.doc.dirty());
    answer_no_to_the_output_question(&mut h);
    assert!(h.state().closing());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_yes_then_cancelled_save_as_keeps_the_app_open() {
    let _render = common::render_lock();
    let sc = Script::new(vec![None]);
    let mut h = app_with(dirty_skripsi(), &sc);
    request_close(&mut h);
    h.get_by_label("Yes").click();
    h.run();
    assert_eq!(sc.asked().len(), 1);
    assert!(h.state().busy().is_none());
    assert!(!h.state().closing());
    assert!(h.state().state.doc.dirty());
    assert!(h.query_by_label(PROMPT).is_none());
}

#[test]
fn close_yes_with_a_failed_save_keeps_the_app_open() {
    let _render = common::render_lock();
    let dir = tmp_dir("yes-fail");
    let sc = Script::new(vec![Some(dir.join("no").join("such").join("dir").join("x.sav"))]);
    let mut h = app_with(dirty_skripsi(), &sc);
    request_close(&mut h);
    h.get_by_label("Yes").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    assert!(!h.state().closing());
    assert!(h.state().state.doc.dirty());
    h.get_by_label("Error");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_exit_when_dirty_asks() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(dirty_skripsi(), &sc);
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("Exit").click();
    h.run();
    prompt_buttons_shown(&h);
    assert_eq!(h.state().save_prompt(), Some(Guarded::Exit));
    assert!(!h.state().closing());
    h.get_by_label("No").click();
    h.run();
    assert!(h.state().closing());
}

#[test]
fn file_exit_when_clean_closes() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(skripsi(), &sc);
    let ctx = h.ctx.clone();
    h.state_mut().dispatch(Command::Exit, &ctx);
    h.step();
    assert!(root_commands(&h).contains(&ViewportCommand::Close));
    assert!(h.state().closing());
    assert!(!h.state().state.exit_requested);
}

#[test]
fn new_when_dirty_asks_no_replaces_cancel_keeps() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(dirty_skripsi(), &sc);
    handle(&mut h, MenuAction::Cmd(Command::New));
    prompt_buttons_shown(&h);
    assert_eq!(h.state().save_prompt(), Some(Guarded::New));
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().state.doc.ds().n_cases(), 60);
    assert!(h.state().state.doc.dirty());

    handle(&mut h, MenuAction::Cmd(Command::New));
    h.get_by_label("No").click();
    h.run();
    assert_eq!(h.state().state.doc.ds().n_vars(), 0);
    assert!(!h.state().state.doc.dirty());
    assert!(!h.state().closing());
}

#[test]
fn new_when_clean_does_not_ask() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(skripsi(), &sc);
    handle(&mut h, MenuAction::Cmd(Command::New));
    assert!(h.query_by_label(PROMPT).is_none());
    assert_eq!(h.state().state.doc.ds().n_vars(), 0);
}

#[test]
fn open_when_dirty_asks_before_the_file_dialog() {
    let _render = common::render_lock();
    let sc = Script::new(vec![Some(fixture("sav/basic.sav"))]);
    let mut h = app_with(dirty_skripsi(), &sc);
    handle(&mut h, MenuAction::OpenData);
    prompt_buttons_shown(&h);
    assert_eq!(h.state().save_prompt(), Some(Guarded::Open));
    assert!(sc.asked().is_empty());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(sc.asked().is_empty());

    handle(&mut h, MenuAction::OpenData);
    h.get_by_label("No").click();
    h.run();
    h.state_mut().wait_for_job();
    h.run();
    assert_eq!(sc.asked(), vec![PickRequest::OpenData]);
    assert!(h.state().state.title().starts_with("basic.sav"), "{}", h.state().state.title());
    assert!(!h.state().state.doc.dirty());
}

#[test]
fn open_when_clean_goes_straight_to_the_file_dialog() {
    let _render = common::render_lock();
    let sc = Script::new(vec![None]);
    let mut h = app_with(skripsi(), &sc);
    handle(&mut h, MenuAction::OpenData);
    assert!(h.query_by_label(PROMPT).is_none());
    assert_eq!(sc.asked(), vec![PickRequest::OpenData]);
}

#[test]
fn import_csv_when_dirty_yes_saves_then_opens_the_import_dialog() {
    let _render = common::render_lock();
    let dir = tmp_dir("import-csv");
    let sc = Script::new(vec![
        Some(dir.join("mine.sav")),
        Some(fixture("data/indo_semicolon.csv")),
    ]);
    let mut h = app_with(dirty_skripsi(), &sc);
    handle(&mut h, MenuAction::ImportCsv);
    prompt_buttons_shown(&h);
    assert_eq!(h.state().save_prompt(), Some(Guarded::ImportCsv));
    h.get_by_label("Yes").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let asked = sc.asked();
    assert!(matches!(asked[0], PickRequest::SaveData { .. }), "{asked:?}");
    assert_eq!(asked[1], PickRequest::ImportCsv);
    assert!(dir.join("mine.sav").exists());
    assert!(matches!(h.state().state.import, Some(ImportDialog::Csv(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_excel_when_dirty_asks_and_no_opens_the_import_dialog() {
    let _render = common::render_lock();
    let sc = Script::new(vec![Some(fixture("data/sample.xlsx"))]);
    let mut h = app_with(dirty_skripsi(), &sc);
    handle(&mut h, MenuAction::ImportExcel);
    prompt_buttons_shown(&h);
    assert_eq!(h.state().save_prompt(), Some(Guarded::ImportExcel));
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().state.import.is_none());
    assert!(sc.asked().is_empty());

    handle(&mut h, MenuAction::ImportExcel);
    h.get_by_label("No").click();
    // The Read Excel File dialog shows a spinner while it reads the sheet names: step, not run.
    h.step();
    assert_eq!(sc.asked(), vec![PickRequest::ImportExcel]);
    assert!(matches!(h.state().state.import, Some(ImportDialog::Excel(_))));
    h.state_mut().state.poll_import(true);
    h.run();
    assert!(matches!(h.state().state.import, Some(ImportDialog::Excel(_))));
}

/// A cell being typed into counts as an unsaved change.
#[test]
fn close_commits_a_pending_cell_edit_first() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut s = skripsi();
    s.editor.editing = Some(statik_app::editor::CellEdit {
        row: 0,
        col: 0,
        text: "5".into(),
        from_typing: true,
        focused: false,
    });
    let mut h = app_with(s, &sc);
    let cmds = request_close(&mut h);
    assert!(cmds.contains(&ViewportCommand::CancelClose), "{cmds:?}");
    h.get_by_label(PROMPT);
}

// ---------------------------------------------------------------------------------------------
// "Save contents of output viewer to Output1?" (at exit only)
// ---------------------------------------------------------------------------------------------

const OUTPUT_PROMPT: &str = "Save contents of output viewer to Output1?";

/// skripsi (clean data) with unsaved output.
fn with_output() -> AppState {
    let mut s = skripsi();
    statik_app::run_procedure(
        &mut s,
        "Frequencies",
        |_| {
            statik_app::ProcOutput::output(vec![output_model::OutputItem::Title(
                "Frequencies".into(),
            )])
        },
        "FREQUENCIES x1.".into(),
    );
    assert!(s.output_dirty);
    s
}

#[test]
fn close_with_unsaved_output_asks_and_no_closes() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(with_output(), &sc);
    let cmds = request_close(&mut h);
    assert!(cmds.contains(&ViewportCommand::CancelClose), "{cmds:?}");
    assert!(h.state().output_prompt());
    h.get_by_label(OUTPUT_PROMPT);
    for b in ["Yes", "No", "Cancel"] {
        h.get_by_label(b);
    }
    h.get_by_label("No").click();
    h.step();
    assert!(root_commands(&h).contains(&ViewportCommand::Close));
    h.run();
    assert!(h.state().closing());
    assert!(sc.asked().is_empty());
}

#[test]
fn close_with_unsaved_output_cancel_keeps_the_app_open() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut h = app_with(with_output(), &sc);
    request_close(&mut h);
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!h.state().closing());
    assert!(!h.state().output_prompt());
    assert!(h.query_by_label(OUTPUT_PROMPT).is_none());
    assert!(h.state().state.output_dirty);
    // Asked again next time.
    request_close(&mut h);
    h.get_by_label(OUTPUT_PROMPT);
}

#[test]
fn close_with_unsaved_output_yes_saves_it_then_closes() {
    let _render = common::render_lock();
    let dir = tmp_dir("output-yes");
    let sc = Script::new(vec![Some(dir.join("hasil"))]);
    let mut h = app_with(with_output(), &sc);
    request_close(&mut h);
    h.get_by_label("Yes").click();
    h.run();
    assert_eq!(
        sc.asked(),
        vec![PickRequest::SaveOutput {
            file_name: "Output1.stko".into()
        }]
    );
    assert!(dir.join("hasil.stko").exists());
    assert!(!h.state().state.output_dirty);
    assert!(h.state().closing());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_with_unsaved_output_yes_then_cancelled_file_dialog_stays() {
    let _render = common::render_lock();
    let sc = Script::new(vec![None]);
    let mut h = app_with(with_output(), &sc);
    request_close(&mut h);
    h.get_by_label("Yes").click();
    h.run();
    assert_eq!(sc.asked().len(), 1);
    assert!(!h.state().closing());
    assert!(h.state().state.output_dirty);
    assert!(h.query_by_label(OUTPUT_PROMPT).is_none());
}

#[test]
fn close_with_unsaved_output_yes_with_a_failed_save_stays() {
    let _render = common::render_lock();
    let dir = tmp_dir("output-fail");
    let sc = Script::new(vec![Some(dir.join("no").join("such").join("x.stko"))]);
    let mut h = app_with(with_output(), &sc);
    request_close(&mut h);
    h.get_by_label("Yes").click();
    h.run();
    assert!(!h.state().closing());
    h.get_by_label("Error");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_exit_asks_about_data_first_then_output() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut s = with_output();
    set_first_cell(&mut s, 42.0);
    let mut h = app_with(s, &sc);
    handle(&mut h, MenuAction::Cmd(Command::Exit));
    h.get_by_label(PROMPT);
    assert!(
        h.query_by_label(OUTPUT_PROMPT).is_none(),
        "one question at a time"
    );
    h.get_by_label("No").click();
    h.run();
    assert!(!h.state().closing());
    h.get_by_label(OUTPUT_PROMPT);
    h.get_by_label("No").click();
    h.run();
    assert!(h.state().closing());
}

#[test]
fn saved_output_is_not_asked_about() {
    let _render = common::render_lock();
    let dir = tmp_dir("output-saved");
    let mut s = with_output();
    s.save_output(&dir.join("o")).unwrap();
    let sc = Script::default();
    let mut h = app_with(s, &sc);
    let cmds = request_close(&mut h);
    assert!(!cmds.contains(&ViewportCommand::CancelClose));
    assert!(h.state().closing());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_and_open_data_do_not_ask_about_output() {
    let _render = common::render_lock();
    let sc = Script::new(vec![None]);
    let mut h = app_with(with_output(), &sc);
    handle(&mut h, MenuAction::Cmd(Command::New));
    assert!(!h.state().output_prompt());
    assert!(h.query_by_label(OUTPUT_PROMPT).is_none());
    assert_eq!(h.state().state.doc.ds().n_vars(), 0);
    handle(&mut h, MenuAction::OpenData);
    assert!(h.query_by_label(OUTPUT_PROMPT).is_none());
    assert_eq!(sc.asked(), vec![PickRequest::OpenData]);
    // The viewer keeps its output.
    assert!(h.state().state.output_dirty);
}

/// The keyboard is ignored under the output question too.
#[test]
fn keys_are_blocked_under_the_output_question() {
    let _render = common::render_lock();
    let sc = Script::default();
    let mut s = with_output();
    s.editor.jump_to(0, 1);
    let before = s.doc.ds().numeric(dataset::VarId(1))[0];
    let mut h = app_with(s, &sc);
    request_close(&mut h);
    h.get_by_label(OUTPUT_PROMPT);
    h.key_press(egui::Key::Delete);
    h.run();
    h.event(egui::Event::Text("y".into()));
    h.run();
    let st = &h.state().state;
    assert_eq!(st.doc.ds().numeric(dataset::VarId(1))[0], before);
    assert!(st.editor.editing.is_none());
}
