//! Autosave and crash recovery (Task 7).
mod common;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use statik_app::autosave::{self, Autosave, Instance};
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

fn harness(app: StatikApp) -> Harness<'static, StatikApp> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut StatikApp| app.show(ui), app);
    h.run();
    h
}

// ---------------------------------------------------------------------------------------------
// Per-instance folders and recovery
// ---------------------------------------------------------------------------------------------

/// An instance that "crashed": its folder holds these datasets as autosave files (oldest first)
/// and its lock is released. Returns the folder.
fn crashed_instance(root: &Path, data: &[&dataset::Dataset]) -> PathBuf {
    let inst = Instance::create(root).unwrap();
    for ds in data {
        autosave::write_recovery(inst.dir(), ds).unwrap();
    }
    let dir = inst.dir().to_path_buf();
    drop(inst); // the process ended without a normal exit
    dir
}

fn orphan_dirs(root: &Path) -> Vec<PathBuf> {
    autosave::find_orphans(root)
        .iter()
        .map(|o| o.dir().to_path_buf())
        .collect()
}

#[test]
fn two_instances_keep_to_their_own_folders() {
    let root = tmp_dir("two");
    let t0 = Instant::now();
    let ds = dirty_skripsi();
    let a_inst = Instance::create(&root).unwrap();
    let b_inst = Instance::create(&root).unwrap();
    let (a_dir, b_dir) = (a_inst.dir().to_path_buf(), b_inst.dir().to_path_buf());
    assert_ne!(a_dir, b_dir);
    let mut a = Autosave::for_instance(a_inst, t0);
    let mut b = Autosave::for_instance(b_inst, t0);

    // Both autosave; each prunes only its own folder.
    let mut sa = dirty_skripsi();
    for k in 0..4 {
        set_first_cell(&mut sa, k as f64);
        a.tick(&sa, t0 + (k + 1) * autosave::INTERVAL, || {});
        a.wait();
    }
    b.tick(&ds, t0 + autosave::INTERVAL, || {});
    b.wait();
    assert_eq!(files(&a_dir).len(), autosave::KEEP);
    assert_eq!(files(&b_dir).len(), 1);

    // A running instance is never offered as crashed.
    assert!(orphan_dirs(&root).is_empty());

    // A becomes clean: only A's files go.
    let out = tmp_dir("two-out");
    sa.apply(Command::SaveAs(out.join("a.sav"))).unwrap();
    a.tick(&sa, t0 + 9 * autosave::INTERVAL, || {});
    assert!(files(&a_dir).is_empty());
    assert_eq!(files(&b_dir).len(), 1);

    // A exits normally: its folder is gone, B's is untouched and still not an orphan.
    a.clean_exit();
    assert!(!a_dir.exists());
    assert_eq!(files(&b_dir).len(), 1);
    assert!(orphan_dirs(&root).is_empty());

    // B "crashes" (its lock is released, the folder stays): now it is an orphan.
    drop(b);
    assert_eq!(orphan_dirs(&root), vec![b_dir.clone()]);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn find_orphans_cleans_dead_folders_and_orders_newest_first() {
    let root = tmp_dir("orphans");
    let ds = skripsi().doc.ds().clone();
    let older = crashed_instance(&root, &[&ds]);
    let newer = crashed_instance(&root, &[&ds, &ds]);
    // A dead folder with only a stray temp file from a cut-short write, and one with nothing.
    let stray = Instance::create(&root).unwrap();
    let stray_dir = stray.dir().to_path_buf();
    std::fs::write(stray_dir.join("autosave-1.sav.123-456.tmp"), b"half").unwrap();
    drop(stray);
    let empty = Instance::create(&root).unwrap();
    let empty_dir = empty.dir().to_path_buf();
    drop(empty);
    // A stray temp file next to real autosave files.
    std::fs::write(newer.join("x.sav.1-2.tmp"), b"half").unwrap();

    let found = autosave::find_orphans(&root);
    let dirs: Vec<_> = found.iter().map(|o| o.dir().to_path_buf()).collect();
    assert_eq!(dirs, vec![newer.clone(), older.clone()]);
    assert_eq!(found[0].files().len(), 2);
    assert!(!stray_dir.exists());
    assert!(!empty_dir.exists());
    assert!(!newer.join("x.sav.1-2.tmp").exists());
    // While offered, the folders are locked: a second Statik does not offer them too.
    assert!(orphan_dirs(&root).is_empty());
    drop(found);
    assert_eq!(orphan_dirs(&root).len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

fn app_offering(root: &Path, auto_dir: &Path) -> StatikApp {
    let orphan = autosave::find_orphans(root).into_iter().next().unwrap();
    let mut app = StatikApp::new(AppState::new())
        .with_autosave(Autosave::new(Some(auto_dir.to_path_buf()), Instant::now()));
    app.offer_recovery(orphan);
    app
}

#[test]
fn recover_opens_the_newest_file_as_untitled_dirty_data() {
    let _render = common::render_lock();
    let root = tmp_dir("recover");
    let mine = tmp_dir("recover-mine");
    let mut s = dirty_skripsi();
    let first = s.doc.ds().clone();
    set_first_cell(&mut s, 77.0);
    let dead = crashed_instance(&root, &[&first, s.doc.ds()]);

    let mut h = harness(app_offering(&root, &mine));
    h.get_by_label("Statik closed unexpectedly. Recover data?");
    h.get_by_label("Discard");
    h.get_by_label("Recover").click();
    h.step();
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
    // The recovered data is backed up in this instance's folder right away; then the dead
    // instance's folder is removed.
    h.state_mut().autosave.wait();
    h.run();
    assert_eq!(files(&mine).len(), 1);
    assert!(!dead.exists());
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&mine);
}

#[test]
fn recover_falls_back_to_an_older_file_and_keeps_unreadable_folders() {
    let _render = common::render_lock();
    let root = tmp_dir("fallback");
    let mine = tmp_dir("fallback-mine");
    let mut s = dirty_skripsi();
    set_first_cell(&mut s, 5.0);
    let dead = crashed_instance(&root, &[s.doc.ds(), s.doc.ds()]);
    // The newest file is damaged: the older one is used.
    let f = files(&dead);
    std::fs::write(&f[0], b"not a sav file").unwrap();
    let mut h = harness(app_offering(&root, &mine));
    h.get_by_label("Recover").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    assert!(h.state().state.messages.is_empty());
    assert_eq!(first_cell(h.state().state.doc.ds()), 5.0);
    let _ = std::fs::remove_dir_all(&root);

    // Nothing readable: an error, the data stays empty, and the folder is kept (also at exit).
    let root = tmp_dir("fallback2");
    let dead = crashed_instance(&root, &[s.doc.ds()]);
    std::fs::write(&files(&dead)[0], b"broken").unwrap();
    let mut h = harness(app_offering(&root, &mine));
    h.get_by_label("Recover").click();
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let msg = h.state().state.message().expect("an error").text.clone();
    assert!(msg.contains("recovery files are kept"), "{msg}");
    assert!(!h.state().state.doc.dirty());
    h.state_mut().exit_cleanup();
    assert_eq!(files(&dead).len(), 1);
    assert_eq!(orphan_dirs(&root), vec![dead]);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&mine);
}

#[test]
fn discard_deletes_the_dead_folder() {
    let _render = common::render_lock();
    let root = tmp_dir("discard");
    let mine = tmp_dir("discard-mine");
    let dead = crashed_instance(&root, &[skripsi().doc.ds()]);
    let mut h = harness(app_offering(&root, &mine));
    h.get_by_label("Discard").click();
    h.run();
    assert!(!dead.exists());
    assert!(h.query_by_label("Recover").is_none());
    assert!(!h.state().state.doc.dirty());
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&mine);
}

/// Closing the window while "Recover data?" is shown (the data is clean, so it closes) keeps
/// the recovery files for the next start.
#[test]
fn closing_at_the_recovery_prompt_keeps_the_files() {
    let _render = common::render_lock();
    let root = tmp_dir("close-prompt");
    let mine = tmp_dir("close-prompt-mine");
    let dead = crashed_instance(&root, &[skripsi().doc.ds()]);
    let mut h = harness(app_offering(&root, &mine));
    h.input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    h.step();
    assert!(h.state().closing());
    h.state_mut().exit_cleanup();
    assert_eq!(files(&dead).len(), 1);
    assert_eq!(orphan_dirs(&root), vec![dead]);
    assert!(!mine.exists(), "the own folder is removed at a normal exit");
    let _ = std::fs::remove_dir_all(&root);
}

/// Closing while the Recover job runs keeps them too.
#[test]
fn exit_while_recovering_keeps_the_files() {
    let _render = common::render_lock();
    let root = tmp_dir("close-recovering");
    let mine = tmp_dir("close-recovering-mine");
    let dead = crashed_instance(&root, &[skripsi().doc.ds()]);
    let mut h = harness(app_offering(&root, &mine));
    h.get_by_label("Recover").click();
    h.step();
    assert!(h.state().busy().is_some());
    h.state_mut().exit_cleanup();
    assert_eq!(files(&dead).len(), 1);
    h.state_mut().wait_for_job();
    let _ = std::fs::remove_dir_all(&root);
}
