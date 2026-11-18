//! Application state, independent of any UI code so it can be tested without a window.
//!
//! Every user action is a [`Command`]. [`AppState::prepare`] either applies a command at once or,
//! for long-running work (open, import, save), returns a [`Job`] to run on a worker thread; the
//! job's [`JobResult`] is applied with [`AppState::finish`]. [`AppState::apply`] does both
//! synchronously (tests, scripted use).
use crate::jobs::{Job, JobResult};
use dataset::Dataset;
use output_model::OutputItem;
use sav_io::CsvOptions;
use std::path::{Path, PathBuf};

/// The open data document.
#[derive(Debug, Default)]
pub struct Doc {
    pub ds: Dataset,
    /// The file the data came from (a `.sav`, or an imported CSV/XLSX file); `None` for a new
    /// document. Only a `.sav` path can be saved to directly.
    pub path: Option<PathBuf>,
    /// Changed since it was opened or last saved.
    pub dirty: bool,
}

/// One run in the output document: a procedure, or a log of an open/save.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutputEntry {
    /// Outline title, e.g. "Regression" or "Log".
    pub title: String,
    pub items: Vec<OutputItem>,
    /// Equivalent syntax (shown as the Log item, as SPSS does).
    pub log: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Warning,
    Error,
}

/// A modal message box (see `dialogs::message`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    pub title: String,
    pub text: String,
}

impl Message {
    pub fn info(title: impl Into<String>, text: impl Into<String>) -> Self {
        Message {
            kind: MessageKind::Info,
            title: title.into(),
            text: text.into(),
        }
    }
    pub fn warning(title: impl Into<String>, text: impl Into<String>) -> Self {
        Message {
            kind: MessageKind::Warning,
            title: title.into(),
            text: text.into(),
        }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Message {
            kind: MessageKind::Error,
            title: "Error".into(),
            text: text.into(),
        }
    }
}

/// Dialogs opened from the menus. Later tasks implement them; until then opening one shows a
/// "not available yet" message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DialogKind {
    Frequencies,
    Descriptives,
    Explore,
    OneSampleT,
    IndependentT,
    PairedT,
    OneWayAnova,
    LinearRegression,
    NparOneSampleKs,
    NparRuns,
    Compute,
    RecodeDifferent,
}

impl DialogKind {
    /// Dialog window title, as in SPSS.
    pub fn title(self) -> &'static str {
        match self {
            DialogKind::Frequencies => "Frequencies",
            DialogKind::Descriptives => "Descriptives",
            DialogKind::Explore => "Explore",
            DialogKind::OneSampleT => "One-Sample T Test",
            DialogKind::IndependentT => "Independent-Samples T Test",
            DialogKind::PairedT => "Paired-Samples T Test",
            DialogKind::OneWayAnova => "One-Way ANOVA",
            DialogKind::LinearRegression => "Linear Regression",
            DialogKind::NparOneSampleKs => "One-Sample Kolmogorov-Smirnov Test",
            DialogKind::NparRuns => "Runs Test",
            DialogKind::Compute => "Compute Variable",
            DialogKind::RecodeDifferent => "Recode into Different Variables",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    /// File > New > Data.
    New,
    /// File > Open > Data... (`.sav`, or CSV/XLSX by extension via `sav_io::open_any`).
    Open(PathBuf),
    /// File > Save. Only valid when [`AppState::needs_save_as`] is false; the UI turns it into
    /// Save As... otherwise.
    Save,
    /// File > Save As... (the path gets a `.sav` extension if it has none).
    SaveAs(PathBuf),
    ImportCsv(PathBuf, CsvOptions),
    /// Excel file and sheet (`None` = first sheet); the first row holds the names.
    ImportXlsx(PathBuf, Option<String>),
    /// Edit > Undo / Redo (the undo stack comes with the data editor; no-ops for now).
    Undo,
    Redo,
    /// View > Value Labels.
    ToggleValueLabels,
    /// Window > Output (checked / unchecked).
    ShowOutput,
    HideOutput,
    /// File > Exit.
    Exit,
    OpenDialog(DialogKind),
}

/// What [`AppState::prepare`] made of a command.
#[derive(Debug)]
pub enum Prepared {
    /// Applied at once, with this outcome.
    Done(Result<(), String>),
    /// Run this job, then pass its result to [`AppState::finish`].
    Job(Job),
}

#[derive(Debug)]
pub struct AppState {
    pub doc: Doc,
    pub outputs: Vec<OutputEntry>,
    /// The Output window is shown.
    pub show_output: bool,
    /// View > Value Labels is on.
    pub value_labels: bool,
    /// Modal message waiting to be shown (and dismissed) by the UI.
    pub message: Option<Message>,
    /// File > Exit was chosen; the UI closes the window.
    pub exit_requested: bool,
    /// "DataSetN" number of the current document (SPSS starts at 0 for the empty startup
    /// document and counts up for every document opened or created).
    pub dataset_no: usize,
    /// "UntitledN" number of the current document if it has no path.
    pub untitled_no: usize,
    next_dataset_no: usize,
    next_untitled_no: usize,
    // Undo/redo stacks are added with the data editor (Task 3).
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Quote a path for syntax: single quotes, embedded quotes doubled.
fn quoted(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "''"))
}

fn is_sav(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("sav"))
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            doc: Doc::default(),
            outputs: Vec::new(),
            show_output: false,
            value_labels: false,
            message: None,
            exit_requested: false,
            dataset_no: 0,
            untitled_no: 1,
            next_dataset_no: 1,
            next_untitled_no: 2,
        }
    }

    /// Window title: `"basic.sav [DataSet1] - Statik Data Editor"`, `"Untitled1 [DataSet0] - ..."`,
    /// with a leading `*` when there are unsaved changes.
    pub fn title(&self) -> String {
        let name = match &self.doc.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            None => format!("Untitled{}", self.untitled_no),
        };
        let star = if self.doc.dirty { "*" } else { "" };
        format!(
            "{star}{name} [DataSet{}] - Statik Data Editor",
            self.dataset_no
        )
    }

    /// Save must go through Save As... (new document, or data imported from CSV/XLSX).
    pub fn needs_save_as(&self) -> bool {
        !self.doc.path.as_deref().is_some_and(is_sav)
    }

    pub fn can_undo(&self) -> bool {
        false
    }
    pub fn can_redo(&self) -> bool {
        false
    }

    /// Apply a command synchronously (long-running ones run inline on this thread).
    pub fn apply(&mut self, cmd: Command) -> Result<(), String> {
        match self.prepare(cmd) {
            Prepared::Done(r) => r,
            Prepared::Job(job) => self.finish(job.run_inline()),
        }
    }

    /// Apply a quick command now, or package a long-running one as a [`Job`].
    pub fn prepare(&mut self, cmd: Command) -> Prepared {
        use Prepared::Done;
        match cmd {
            Command::New => {
                self.doc = Doc::default();
                self.dataset_no = self.take_dataset_no();
                self.untitled_no = self.next_untitled_no;
                self.next_untitled_no += 1;
                Done(Ok(()))
            }
            Command::Open(path) => {
                Prepared::Job(Job::new(
                    "Reading data file...",
                    move || match sav_io::open_any(&path) {
                        Ok((ds, report)) => {
                            let log = if is_sav(&path) {
                                format!("GET\n  FILE={}.", quoted(&path))
                            } else {
                                format!("GET DATA\n  /FILE={}.", quoted(&path))
                            };
                            JobResult::Loaded {
                                ds,
                                path,
                                notes: report.notes,
                                log,
                            }
                        }
                        Err(e) => {
                            JobResult::Failed(format!("Cannot open {}:\n{e}", path.display()))
                        }
                    },
                ))
            }
            Command::ImportCsv(path, opt) => Prepared::Job(Job::new(
                "Reading text data...",
                move || match sav_io::import_csv(&path, &opt) {
                    Ok(ds) => {
                        let log = format!("GET DATA\n  /TYPE=TXT\n  /FILE={}.", quoted(&path));
                        JobResult::Loaded {
                            ds,
                            path,
                            notes: Vec::new(),
                            log,
                        }
                    }
                    Err(e) => JobResult::Failed(format!("Cannot import {}:\n{e}", path.display())),
                },
            )),
            Command::ImportXlsx(path, sheet) => Prepared::Job(Job::new(
                "Reading Excel file...",
                move || match sav_io::import_xlsx(&path, sheet.as_deref(), true) {
                    Ok(ds) => {
                        let mut log = format!("GET DATA\n  /TYPE=XLSX\n  /FILE={}", quoted(&path));
                        if let Some(s) = &sheet {
                            log.push_str(&format!("\n  /SHEET=name '{}'", s.replace('\'', "''")));
                        }
                        log.push_str("\n  /READNAMES=ON.");
                        JobResult::Loaded {
                            ds,
                            path,
                            notes: Vec::new(),
                            log,
                        }
                    }
                    Err(e) => JobResult::Failed(format!("Cannot import {}:\n{e}", path.display())),
                },
            )),
            Command::Save => match &self.doc.path {
                Some(p) if is_sav(p) => Prepared::Job(self.save_job(p.clone())),
                _ => Done(Err(
                    "This data has not been saved as a .sav file yet. Use Save As...".into(),
                )),
            },
            Command::SaveAs(mut path) => {
                if !is_sav(&path) {
                    let mut s = path.into_os_string();
                    s.push(".sav");
                    path = PathBuf::from(s);
                }
                Prepared::Job(self.save_job(path))
            }
            Command::Undo | Command::Redo => Done(Ok(())),
            Command::ToggleValueLabels => {
                self.value_labels = !self.value_labels;
                Done(Ok(()))
            }
            Command::ShowOutput => {
                self.show_output = true;
                Done(Ok(()))
            }
            Command::HideOutput => {
                self.show_output = false;
                Done(Ok(()))
            }
            Command::Exit => {
                self.exit_requested = true;
                Done(Ok(()))
            }
            Command::OpenDialog(kind) => {
                self.message = Some(Message::info(
                    kind.title(),
                    format!("{} is not available yet.", kind.title()),
                ));
                Done(Ok(()))
            }
        }
    }

    fn take_dataset_no(&mut self) -> usize {
        let n = self.next_dataset_no;
        self.next_dataset_no += 1;
        n
    }

    /// Write a clone of the current dataset to `path` (atomic, via `sav_io::write_sav`).
    fn save_job(&self, path: PathBuf) -> Job {
        let ds = self.doc.ds.clone();
        Job::new("Saving data file...", move || {
            match sav_io::write_sav(&ds, &path) {
                Ok(()) => {
                    let log = format!("SAVE OUTFILE={}\n  /COMPRESSED.", quoted(&path));
                    JobResult::Saved { path, log }
                }
                Err(e) => JobResult::Failed(format!("Cannot save {}:\n{e}", path.display())),
            }
        })
    }

    /// A job running procedure `f` on a clone of the dataset (see [`crate::jobs`]). The result is
    /// one output entry; if `f` added or removed variables or cases the clone replaces the
    /// document's dataset. (Procedures only ever add variables, e.g. saved residuals, so the
    /// shape check is enough to detect a change.)
    pub fn procedure_job(
        &self,
        title: &str,
        f: impl FnOnce(&mut Dataset) -> Vec<OutputItem> + Send + 'static,
        syntax: String,
    ) -> Job {
        let mut ds = self.doc.ds.clone();
        let title = title.to_string();
        Job::new(format!("Running {title}..."), move || {
            let shape = (ds.n_vars(), ds.n_cases());
            let items = f(&mut ds);
            let changed = (ds.n_vars(), ds.n_cases()) != shape;
            JobResult::Ran {
                ds,
                changed,
                entry: OutputEntry {
                    title,
                    items,
                    log: syntax,
                },
            }
        })
    }

    /// Apply the result of a finished job. Errors are returned (the UI shows them as a message).
    pub fn finish(&mut self, r: JobResult) -> Result<(), String> {
        match r {
            JobResult::Loaded {
                ds,
                path,
                notes,
                log,
            } => {
                self.doc = Doc {
                    ds,
                    path: Some(path),
                    dirty: false,
                };
                self.dataset_no = self.take_dataset_no();
                let log = format!(
                    "{log}\nDATASET NAME DataSet{} WINDOW=FRONT.",
                    self.dataset_no
                );
                let items = notes.iter().cloned().map(OutputItem::Warning).collect();
                self.outputs.push(OutputEntry {
                    title: "Log".into(),
                    items,
                    log,
                });
                if !notes.is_empty() {
                    let text = notes
                        .iter()
                        .map(|n| format!("- {n}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.message = Some(Message::warning("Warning", text));
                }
                Ok(())
            }
            JobResult::Saved { path, log } => {
                self.doc.path = Some(path);
                self.doc.dirty = false;
                self.outputs.push(OutputEntry {
                    title: "Log".into(),
                    items: Vec::new(),
                    log,
                });
                Ok(())
            }
            JobResult::Ran { ds, changed, entry } => {
                if changed {
                    self.doc.ds = ds;
                    self.doc.dirty = true;
                }
                self.outputs.push(entry);
                self.show_output = true;
                Ok(())
            }
            JobResult::Failed(e) => Err(e),
        }
    }
}

/// Run a procedure synchronously on the document's dataset and append its output (tests,
/// scripted use; the UI runs [`AppState::procedure_job`] on a worker thread instead).
pub fn run_procedure(
    state: &mut AppState,
    title: &str,
    f: impl FnOnce(&mut Dataset) -> Vec<OutputItem> + Send + 'static,
    syntax: String,
) {
    let r = state.procedure_job(title, f, syntax).run_inline();
    if let Err(e) = state.finish(r) {
        state.message = Some(Message::error(e));
    }
}
