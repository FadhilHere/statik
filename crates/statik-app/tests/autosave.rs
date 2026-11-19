//! Autosave and crash recovery (Task 7).
mod common;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use statik_app::autosave::{self, Autosave};
use statik_app::jobs::{Job, JobResult};
use statik_app::{AppState, Command, StatikApp};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

/// A fresh, empty temporary directory.
fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("statik-autosave-{name}-{}", std::process::id()));
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

fn first_cell(ds: &dataset::Dataset) -> f64 {
    ds.numeric(dataset::VarId(0))[0]
}

fn files(dir: &Path) -> Vec<PathBuf> {
    autosave::recovery_files(dir)
}

// ---------------------------------------------------------------------------------------------
// Autosave
// ---------------------------------------------------------------------------------------------

#[test]
fn autosave_writes_only_when_dirty_and_changed_and_keeps_three() {
    let dir = tmp_dir("tick");
    let t0 = Instant::now();
    let mut a = Autosave::new(Some(dir.clone()), t0);
    let i = autosave::INTERVAL;

    // A clean document is never autosaved.
    let clean = skripsi();
    assert_eq!(a.tick(&clean, t0 + 2 * i, || {}), None);
    a.wait();
    assert!(files(&dir).is_empty());

    let mut s = dirty_skripsi();
    let before = (s.doc.path.clone(), s.doc.dirty(), s.outputs.len(), s.doc.rev());
    // Not yet due: the time left is returned, so the UI can schedule a repaint.
    let mut a = Autosave::new(Some(dir.clone()), t0);
    let left = a.tick(&s, t0 + Duration::from_secs(60), || {});
    assert_eq!(left, Some(i - Duration::from_secs(60)));
    a.wait();
    assert!(files(&dir).is_empty());

    // Due: one file with the current data; the document is untouched.
    a.tick(&s, t0 + i, || {});
    a.wait();
    let f = files(&dir);
    assert_eq!(f.len(), 1);
    let (ds, _) = sav_io::read_sav(&f[0]).unwrap();
    assert_eq!(ds.n_cases(), 60);
    assert_eq!(first_cell(&ds), 42.0);
    assert_eq!(
        before,
        (s.doc.path.clone(), s.doc.dirty(), s.outputs.len(), s.doc.rev())
    );
    assert!(s.messages.is_empty());
    assert!(a.note().is_none());

    // Same revision: no new file, however long it waits.
    a.tick(&s, t0 + 3 * i, || {});
    a.wait();
    assert_eq!(files(&dir).len(), 1);

    // Every new revision gets a new file; only the 3 newest are kept, newest first.
    for k in 0..4 {
        set_first_cell(&mut s, 100.0 + k as f64);
        a.tick(&s, t0 + (4 + k) * i, || {});
        a.wait();
    }
    let f = files(&dir);
    assert_eq!(f.len(), autosave::KEEP);
    assert_eq!(autosave::KEEP, 3);
    let (ds, _) = sav_io::read_sav(&f[0]).unwrap();
    assert_eq!(first_cell(&ds), 103.0);

    // Once the document is saved (clean), its recovery files are removed.
    let out = dir.join("saved");
    std::fs::create_dir_all(&out).unwrap();
    s.apply(Command::SaveAs(out.join("x.sav"))).unwrap();
    assert!(!s.doc.dirty());
    a.tick(&s, t0 + 9 * i, || {});
    a.wait();
    assert!(files(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autosave_failure_is_a_note_not_a_message() {
    let dir = tmp_dir("fail");
    // The recovery "directory" is below a plain file, so it cannot be created.
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let t0 = Instant::now();
    let mut a = Autosave::new(Some(blocker.join("recovery")), t0);
    let mut s = dirty_skripsi();
    a.tick(&s, t0 + autosave::INTERVAL, || {});
    a.wait();
    let note = a.note().expect("a status note").to_string();
    assert!(note.starts_with("Autosave failed"), "{note}");
    assert!(s.messages.is_empty());
    assert!(s.doc.dirty());
    // A later failure keeps the one note (no repeated messages).
    set_first_cell(&mut s, 7.0);
    a.tick(&s, t0 + 2 * autosave::INTERVAL, || {});
    a.wait();
    assert_eq!(a.note(), Some(note.as_str()));
    assert!(s.messages.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn autosave_runs_while_a_procedure_is_busy() {
    let _render = common::render_lock();
    let dir = tmp_dir("busy");
    let mut auto = Autosave::new(Some(dir.clone()), Instant::now());
    auto.interval = Duration::ZERO;
    let app = StatikApp::new(dirty_skripsi()).with_autosave(auto);
    let mut h = harness(app);
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let job = Job::new("Running Slow...", move || {
        let _ = rx.recv();
        JobResult::Failed("stopped".into())
    });
    assert!(h.state_mut().state.queue_job(job));
    h.step();
    assert!(h.state().busy().is_some());
    h.state_mut().autosave.wait();
    assert_eq!(files(&dir).len(), 1);
    assert!(h.state().busy().is_some(), "the procedure is still running");
    tx.send(()).unwrap();
    h.state_mut().wait_for_job();
    h.state_mut().state.messages.clear();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recovery_files_newer_than_the_clean_exit_marker_are_offered() {
    let dir = tmp_dir("marker");
    assert_eq!(autosave::pending_recovery(&dir), None);
    let ds = skripsi().doc.ds().clone();
    let p1 = autosave::write_recovery(&dir, &ds).unwrap();
    let p2 = autosave::write_recovery(&dir, &ds).unwrap();
    assert_ne!(p1, p2);
    // The newest is offered.
    assert_eq!(autosave::pending_recovery(&dir), Some(p2.clone()));
    // A clean exit removes them and writes the marker.
    autosave::mark_clean_exit(&dir).unwrap();
    assert!(files(&dir).is_empty());
    assert_eq!(autosave::pending_recovery(&dir), None);
    // A file older than the marker (left from before a clean exit) is not offered.
    let old = dir.join("autosave-0000000000000001.sav");
    sav_io::write_sav(&ds, &old).unwrap();
    assert_eq!(files(&dir), vec![old]);
    assert_eq!(autosave::pending_recovery(&dir), None);
    // One written after the clean exit is.
    let p3 = autosave::write_recovery(&dir, &ds).unwrap();
    assert_eq!(autosave::pending_recovery(&dir), Some(p3));
    let _ = std::fs::remove_dir_all(&dir);
}

fn harness(app: StatikApp) -> Harness<'static, StatikApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut StatikApp| app.show(ui), app);
    h.run();
    h
}

#[test]
fn recover_opens_the_newest_file_as_untitled_dirty_data() {
    let _render = common::render_lock();
    let dir = tmp_dir("recover");
    let mut s = dirty_skripsi();
    autosave::write_recovery(&dir, s.doc.ds()).unwrap();
    set_first_cell(&mut s, 77.0);
    let newest = autosave::write_recovery(&dir, s.doc.ds()).unwrap();
    let pending = autosave::pending_recovery(&dir).unwrap();
    assert_eq!(pending, newest);

    let mut app =
        StatikApp::new(AppState::new()).with_autosave(Autosave::new(Some(dir.clone()), Instant::now()));
    app.offer_recovery(pending);
    let mut h = harness(app);
    h.get_by_label("Statik closed unexpectedly. Recover data?");
    h.get_by_label("Discard");
    h.get_by_label("Recover").click();
    h.run();
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    assert!(st.messages.is_empty(), "{:?}", st.messages);
    assert_eq!(st.doc.path, None);
    assert!(st.doc.dirty());
    assert_eq!(st.doc.ds().n_cases(), 60);
    assert_eq!(first_cell(st.doc.ds()), 77.0);
    assert!(st.title().starts_with("*Untitled"), "{}", st.title());
    assert!(h.query_by_label("Recover").is_none());
    // The recovered data is autosaved again right away (the old files stay until then).
    h.state_mut().autosave.wait();
    assert!(!files(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_deletes_the_recovery_files() {
    let _render = common::render_lock();
    let dir = tmp_dir("discard");
    let p = autosave::write_recovery(&dir, skripsi().doc.ds()).unwrap();
    let mut app =
        StatikApp::new(AppState::new()).with_autosave(Autosave::new(Some(dir.clone()), Instant::now()));
    app.offer_recovery(p);
    let mut h = harness(app);
    h.get_by_label("Discard").click();
    h.run();
    assert!(files(&dir).is_empty());
    assert!(h.query_by_label("Recover").is_none());
    assert!(!h.state().state.doc.dirty());
    let _ = std::fs::remove_dir_all(&dir);
}
