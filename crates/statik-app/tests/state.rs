use statik_app::jobs::{JobResult, RunningJob};
use statik_app::state::{AppState, Command, MessageKind, Prepared, run_procedure};
use std::path::{Path, PathBuf};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("statik-app-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn startup_title_is_untitled() {
    let s = AppState::new();
    assert_eq!(s.title(), "Untitled1 [DataSet0] - Statik Data Editor");
    assert!(s.needs_save_as());
}

#[test]
fn open_sav_fills_dataset_and_title() {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("sav/basic.sav"))).unwrap();
    assert!(s.doc.ds.n_vars() > 0);
    assert!(s.doc.ds.n_cases() > 0);
    assert!(!s.doc.dirty);
    assert_eq!(s.title(), "basic.sav [DataSet1] - Statik Data Editor");
    assert!(!s.needs_save_as());
    // The open is logged with its syntax.
    let log = &s.outputs.last().unwrap().log;
    assert!(log.starts_with("GET\n  FILE="), "{log}");
    assert!(
        log.ends_with("DATASET NAME DataSet1 WINDOW=FRONT."),
        "{log}"
    );
}

#[test]
fn edit_marks_title_dirty() {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("sav/basic.sav"))).unwrap();
    let id = dataset::VarId(0);
    if s.doc.ds.var(id).var_type == dataset::VarType::Numeric {
        s.doc.ds.set_numeric(id, 0, 42.0).unwrap();
    } else {
        s.doc.ds.set_text(id, 0, "x").unwrap();
    }
    s.doc.dirty = true;
    assert!(s.title().starts_with("*basic.sav"), "{}", s.title());
}

#[test]
fn save_as_then_open_round_trips() {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("sav/basic.sav"))).unwrap();
    let before = format!("{:?}", s.doc.ds);
    s.doc.dirty = true;
    let out = tmp_dir("roundtrip").join("copy"); // no extension: ".sav" is added
    s.apply(Command::SaveAs(out.clone())).unwrap();
    let saved = out.with_extension("sav");
    assert_eq!(s.doc.path.as_deref(), Some(saved.as_path()));
    assert!(!s.doc.dirty);
    assert!(
        s.title().starts_with("copy.sav [DataSet1]"),
        "{}",
        s.title()
    );

    let mut t = AppState::new();
    t.apply(Command::Open(saved.clone())).unwrap();
    assert_eq!(format!("{:?}", t.doc.ds), before);
    // Plain Save writes to the same file again.
    t.doc.dirty = true;
    t.apply(Command::Save).unwrap();
    assert!(!t.doc.dirty);
    let _ = std::fs::remove_file(saved);
}

#[test]
fn open_corrupt_file_is_an_error_not_a_panic() {
    let p = tmp_dir("corrupt").join("broken.sav");
    std::fs::write(&p, b"$FL2 this is not a real system file").unwrap();
    let mut s = AppState::new();
    let err = s.apply(Command::Open(p.clone())).unwrap_err();
    assert!(err.contains("broken.sav"), "{err}");
    assert_eq!(s.title(), "Untitled1 [DataSet0] - Statik Data Editor");
    let err = s
        .apply(Command::Open(fixture("nope/missing.sav")))
        .unwrap_err();
    assert!(!err.is_empty());
    let _ = std::fs::remove_file(p);
}

#[test]
fn imported_csv_saves_via_save_as() {
    let mut s = AppState::new();
    s.apply(Command::ImportCsv(
        fixture("data/skripsi60.csv"),
        Default::default(),
    ))
    .unwrap();
    assert_eq!(s.doc.ds.n_vars(), 5);
    assert_eq!(s.doc.ds.n_cases(), 60);
    assert!(s.title().starts_with("skripsi60.csv [DataSet1]"));
    assert!(s.needs_save_as());
    assert!(s.apply(Command::Save).is_err());
}

#[test]
fn import_xlsx_and_open_by_extension() {
    let mut s = AppState::new();
    s.apply(Command::ImportXlsx(fixture("data/sample.xlsx"), None))
        .unwrap();
    assert!(s.doc.ds.n_vars() > 0);
    s.apply(Command::Open(fixture("data/skripsi60.csv")))
        .unwrap();
    assert_eq!(s.doc.ds.n_cases(), 60);
    assert_eq!(s.dataset_no, 2);
}

#[test]
fn new_document_counts_up() {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("sav/basic.sav"))).unwrap();
    s.apply(Command::New).unwrap();
    assert_eq!(s.doc.ds.n_vars(), 0);
    assert_eq!(s.title(), "Untitled2 [DataSet2] - Statik Data Editor");
}

#[test]
fn open_with_read_notes_shows_warning_and_logs_them() {
    // metadata.sav carries features Statik reports (weight / date formats / ...).
    let mut s = AppState::new();
    let mut found = false;
    for f in [
        "sav/metadata.sav",
        "sav/basic.sav",
        "sav/long_string.sav",
        "sav/cp1252.sav",
    ] {
        s.apply(Command::Open(fixture(f))).unwrap();
        let entry = s.outputs.last().unwrap();
        let (_, report) = sav_io::open_any(&fixture(f)).unwrap();
        assert_eq!(entry.items.len(), report.notes.len());
        if !report.notes.is_empty() {
            let m = s.message.take().expect("warning message");
            assert_eq!(m.kind, MessageKind::Warning);
            assert!(m.text.contains(&report.notes[0]));
            found = true;
        } else {
            assert!(s.message.is_none());
        }
    }
    assert!(found, "no fixture produced read notes");
}

#[test]
fn toggles_and_exit() {
    let mut s = AppState::new();
    s.apply(Command::ToggleValueLabels).unwrap();
    assert!(s.value_labels);
    s.apply(Command::ShowOutput).unwrap();
    assert!(s.show_output);
    s.apply(Command::Undo).unwrap();
    s.apply(Command::Exit).unwrap();
    assert!(s.exit_requested);
}

#[test]
fn open_runs_on_worker_thread() {
    let mut s = AppState::new();
    let Prepared::Job(job) = s.prepare(Command::Open(fixture("sav/basic.sav"))) else {
        panic!("open must be a job")
    };
    let running = RunningJob::spawn(job, || {}).unwrap();
    let r = running.wait();
    assert!(matches!(r, JobResult::Loaded { .. }));
    s.finish(r).unwrap();
    assert!(s.title().starts_with("basic.sav"));
}

#[test]
fn procedure_output_and_saved_variables() {
    let mut s = AppState::new();
    s.apply(Command::ImportCsv(
        fixture("data/skripsi60.csv"),
        Default::default(),
    ))
    .unwrap();
    run_procedure(
        &mut s,
        "Descriptives",
        |_ds| vec![output_model::OutputItem::Title("Descriptives".into())],
        "DESCRIPTIVES x1.".into(),
    );
    assert!(!s.doc.dirty);
    assert!(s.show_output);
    assert_eq!(s.outputs.last().unwrap().title, "Descriptives");
    run_procedure(
        &mut s,
        "Regression",
        |ds| {
            let n = ds.n_cases();
            ds.add_variable(
                dataset::Variable::numeric("RES_1"),
                dataset::Column::Numeric(vec![0.0; n]),
            )
            .unwrap();
            vec![]
        },
        "REGRESSION.".into(),
    );
    assert!(s.doc.dirty);
    assert!(s.doc.ds.find("RES_1").is_some());
}

#[test]
fn panicking_job_reports_error() {
    let mut s = AppState::new();
    run_procedure(&mut s, "Boom", |_| panic!("kaboom"), String::new());
    let m = s.message.unwrap();
    assert_eq!(m.kind, MessageKind::Error);
    assert!(m.text.contains("kaboom"));
}
