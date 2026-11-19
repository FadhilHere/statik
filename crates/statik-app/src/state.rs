//! Application state, independent of any UI code so it can be tested without a window.
//!
//! Every user action is a [`Command`]. [`AppState::prepare`] either applies a command at once or,
//! for long-running work (open, import, save), returns a [`Job`] to run on a worker thread; the
//! job's [`JobResult`] is applied with [`AppState::finish`]. [`AppState::apply`] does both
//! synchronously (tests, scripted use). Code that only has `&mut AppState` (views, dialogs)
//! starts work with [`AppState::queue_job`].
//!
//! **Revision guard.** Every change to the document's data goes through [`Doc::ds_mut`] /
//! [`AppState::touch`], which bump [`Doc::rev`] and set dirty; replacing the document (New,
//! Open) also moves `rev` on. A job records the revision it started from, and
//! [`AppState::finish`] only swaps data in, replaces the document or clears `dirty` if the
//! revision is unchanged. Otherwise the newer in-memory state is kept and a conflict message is
//! queued.
use crate::dialogs::DialogHost;
use crate::dialogs::import::ImportDialog;
use crate::edit::EditOp;
use crate::editor::EditorState;
use crate::jobs::{Job, JobResult};
use crate::output::{ItemRef, OutputView};
use crate::syntax::SyntaxWindow;
use dataset::Dataset;
use output_model::OutputItem;
use sav_io::CsvOptions;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// The open data document. The data is only reachable mutably through [`Doc::ds_mut`], which
/// bumps the revision and marks the document dirty.
#[derive(Debug, Default)]
pub struct Doc {
    ds: Dataset,
    /// The file the data came from (a `.sav`, or an imported CSV/XLSX file); `None` for a new
    /// document. Only a `.sav` path can be saved to directly.
    pub path: Option<PathBuf>,
    dirty: bool,
    rev: u64,
}

impl Doc {
    pub fn new(ds: Dataset, path: Option<PathBuf>) -> Self {
        Doc {
            ds,
            path,
            dirty: false,
            rev: 0,
        }
    }
    pub fn ds(&self) -> &Dataset {
        &self.ds
    }
    /// Mutable access to the data: bumps [`Self::rev`] and sets dirty. Use it for every edit.
    pub fn ds_mut(&mut self) -> &mut Dataset {
        self.touch();
        &mut self.ds
    }
    /// Record a change (bump the revision, mark dirty).
    pub fn touch(&mut self) {
        self.rev += 1;
        self.dirty = true;
    }
    /// Changed since it was opened or last saved.
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    /// Revision counter; changes with every mutation and every document replacement.
    pub fn rev(&self) -> u64 {
        self.rev
    }
}

/// Largest `.stko` file File ▸ Open Output reads (bytes).
pub const MAX_OUTPUT_FILE: u64 = 200_000_000;

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

/// Dialogs opened from the menus. The Analyze dialogs are in [`crate::dialogs`]; Compute and
/// Recode (Task 6) still show a "not available yet" message.
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
    /// Edit > Undo / Redo (see [`crate::edit`]).
    Undo,
    Redo,
    /// Edit > Insert Variable: a new numeric variable before the selected one.
    InsertVariable,
    /// Edit > Insert Cases: a blank case before the selected one.
    InsertCases,
    /// Edit > Clear: set the selected cells to missing, or delete the selected variables/cases.
    Clear,
    /// View > Value Labels.
    ToggleValueLabels,
    /// Window > Output (checked / unchecked).
    ShowOutput,
    HideOutput,
    /// File > Exit.
    Exit,
    /// Recover data after a crash: read the first readable of these recovery files (newest
    /// first; see [`crate::autosave`]) as a new untitled document with unsaved changes.
    Recover(Vec<PathBuf>),
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

/// What a procedure closure returns: its output, and whether it changed the data.
#[derive(Debug, Default)]
pub struct ProcOutput {
    pub items: Vec<OutputItem>,
    /// True when the procedure changed the dataset (saved variables); compute/recode say true.
    pub changed: bool,
    /// How to undo the change (Compute, Recode). `None` with `changed` = not undoable: the undo
    /// history is cleared when the result is applied (procedures saving variables, as in SPSS).
    pub undo: Option<EditOp>,
    /// The run failed: this message is shown, and neither data nor output is applied.
    pub error: Option<String>,
}

impl ProcOutput {
    /// Output only; the data is unchanged.
    pub fn output(items: Vec<OutputItem>) -> Self {
        ProcOutput {
            items,
            ..Default::default()
        }
    }
    /// Output, and the data was changed.
    pub fn changed(items: Vec<OutputItem>) -> Self {
        ProcOutput {
            items,
            changed: true,
            ..Default::default()
        }
    }
    /// The data was changed, and `undo` reverts it (transformations).
    pub fn undoable(items: Vec<OutputItem>, undo: EditOp) -> Self {
        ProcOutput {
            items,
            changed: true,
            undo: Some(undo),
            error: None,
        }
    }
    /// The run failed with `message`; nothing is applied.
    pub fn failed(message: impl Into<String>) -> Self {
        ProcOutput {
            error: Some(message.into()),
            ..Default::default()
        }
    }
}

/// Outline title of the output entry of a transformation (Compute, Recode), as in SPSS.
pub const LOG_TITLE: &str = "Log";

#[derive(Debug)]
pub struct AppState {
    pub doc: Doc,
    pub outputs: Vec<OutputEntry>,
    /// The Output window is shown.
    pub show_output: bool,
    /// View > Value Labels is on.
    pub value_labels: bool,
    /// Modal messages waiting to be shown by the UI, one at a time, oldest first
    /// ([`Self::push_message`], [`Self::message`], [`Self::dismiss_message`]).
    pub messages: VecDeque<Message>,
    /// File > Exit was chosen; the UI sends a close request to the window and then resets this.
    /// Call [`Self::cancel_exit`] if a close is cancelled (e.g. at "Save changes?", Task 7).
    pub exit_requested: bool,
    /// A job queued by code that only has `&mut AppState` (views, dialogs); see
    /// [`Self::queue_job`]. `StatikApp` starts it on the next frame; until then the app is busy.
    pub pending_job: Option<Job>,
    /// "DataSetN" number of the current document (SPSS starts at 0 for the empty startup
    /// document and counts up for every document opened or created).
    pub dataset_no: usize,
    /// "UntitledN" number of the current document if it has no path.
    pub untitled_no: usize,
    next_dataset_no: usize,
    next_untitled_no: usize,
    /// UI-side state of the Data Editor (view tab, selection, cell being edited, Align/Role/
    /// Columns that the data model does not hold). Plain data, no UI types.
    pub editor: EditorState,
    /// Applied edits, oldest first (at most [`crate::edit::UNDO_LIMIT`]); see [`crate::edit`].
    pub(crate) undo_stack: Vec<EditOp>,
    /// Undone edits, most recently undone last.
    pub(crate) redo_stack: Vec<EditOp>,
    /// UI state of the Output viewer (selection, scroll target, pending image copy).
    pub viewer: OutputView,
    /// The open procedure dialog and the remembered settings of every dialog.
    pub dialogs: DialogHost,
    /// The Syntax window (text pasted by dialogs).
    pub syntax: SyntaxWindow,
    /// The open Read CSV File / Read Excel File dialog (File ▸ Import Data).
    pub import: Option<ImportDialog>,
    /// Output was added since the output was last saved ([`Self::save_output`]); File ▸ Exit
    /// asks "Save contents of output viewer to Output1?" then. Opening a `.stko` file does
    /// not change it (the opened entries are on disk, the ones already shown may not be).
    pub output_dirty: bool,
    /// A procedure added output: the app brings the Output window to the front (and clears
    /// this).
    pub raise_output: bool,
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
            messages: VecDeque::new(),
            exit_requested: false,
            pending_job: None,
            dataset_no: 0,
            untitled_no: 1,
            next_dataset_no: 1,
            next_untitled_no: 2,
            editor: EditorState::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            viewer: OutputView::default(),
            dialogs: DialogHost::default(),
            syntax: SyntaxWindow::default(),
            import: None,
            output_dirty: false,
            raise_output: false,
        }
    }

    /// Append an output entry (marks the output as unsaved).
    fn push_output(&mut self, entry: OutputEntry) {
        self.outputs.push(entry);
        self.output_dirty = true;
    }

    /// File ▸ Save Output As in the viewer: all output entries as JSON (`.stko`, added when
    /// the path has another or no extension). Written to a temporary file, then renamed.
    /// Returns the path written. Clears [`Self::output_dirty`].
    pub fn save_output(&mut self, path: &Path) -> Result<PathBuf, String> {
        let path = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("stko"))
        {
            path.to_path_buf()
        } else {
            let mut p = path.as_os_str().to_owned();
            p.push(".stko");
            PathBuf::from(p)
        };
        let json = serde_json::to_vec(&self.outputs)
            .map_err(|e| format!("Cannot save the output: {e}"))?;
        let tmp = path.with_extension("stko.tmp");
        std::fs::write(&tmp, json)
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("Cannot write {}: {e}", path.display())
            })?;
        self.output_dirty = false;
        Ok(path)
    }

    /// File ▸ Open Output in the viewer: read a `.stko` file and append its entries (the
    /// output already shown is kept), show the viewer and scroll to the first opened entry.
    pub fn open_output(&mut self, path: &Path) -> Result<(), String> {
        let len = std::fs::metadata(path)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?
            .len();
        if len > MAX_OUTPUT_FILE {
            return Err(format!(
                "{} is too large to open as output ({} MB; the limit is {} MB).",
                path.display(),
                len / 1_000_000,
                MAX_OUTPUT_FILE / 1_000_000
            ));
        }
        let bytes =
            std::fs::read(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        let entries: Vec<OutputEntry> = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{} is not a Statik output file: {e}", path.display()))?;
        let first = self.outputs.len();
        self.outputs.extend(entries);
        self.show_output = true;
        self.viewer.scroll_to = Some(ItemRef::entry(first));
        self.viewer.selected = None;
        Ok(())
    }

    /// Window title: `"basic.sav [DataSet1] - Statik Data Editor"`, `"Untitled1 [DataSet0] - ..."`,
    /// with a leading `*` when there are unsaved changes.
    pub fn title(&self) -> String {
        let name = self.doc_name();
        let star = if self.doc.dirty { "*" } else { "" };
        format!(
            "{star}{name} [DataSet{}] - Statik Data Editor",
            self.dataset_no
        )
    }

    /// The document's name: its file name (`basic.sav`, `data.csv`) or `UntitledN`.
    pub fn doc_name(&self) -> String {
        match &self.doc.path {
            Some(p) => p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string()),
            None => format!("Untitled{}", self.untitled_no),
        }
    }

    /// Save must go through Save As... (new document, or data imported from CSV/XLSX).
    pub fn needs_save_as(&self) -> bool {
        !self.doc.path.as_deref().is_some_and(is_sav)
    }

    /// Queue a modal message (shown after the ones already queued).
    pub fn push_message(&mut self, m: Message) {
        self.messages.push_back(m);
    }
    /// The message currently shown, if any.
    pub fn message(&self) -> Option<&Message> {
        self.messages.front()
    }
    /// Close the current message; the next queued one is shown.
    pub fn dismiss_message(&mut self) {
        self.messages.pop_front();
    }

    /// Record a change to the document's data (same as `doc.touch()`).
    pub fn touch(&mut self) {
        self.doc.touch();
    }

    /// Replace the document; the revision moves on, so jobs started on the old one conflict.
    /// The undo history and the editor state (selection, Align/Role/Columns) belong to the old
    /// document and are dropped.
    fn replace_doc(&mut self, mut doc: Doc) {
        doc.rev = self.doc.rev + 1;
        self.doc = doc;
        self.clear_undo();
        self.editor = EditorState::default();
    }

    /// Queue a job for `StatikApp` to start on the next frame (for code that only has
    /// `&mut AppState`). Returns false, and queues an error message, if one is already queued.
    /// Tests must call `StatikApp::wait_for_job` before asserting on its results.
    pub fn queue_job(&mut self, job: Job) -> bool {
        if self.pending_job.is_some() {
            self.push_message(Message::error(
                "Another operation is already waiting to run.",
            ));
            return false;
        }
        self.pending_job = Some(job);
        true
    }

    /// The window close was cancelled (e.g. at a "Save changes?" prompt).
    pub fn cancel_exit(&mut self) {
        self.exit_requested = false;
    }

    /// Edit > Undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    /// Edit > Redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
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
        // Text typed into a cell (Data View or Variable View) is committed first, for every
        // command: it lands on the cell it was typed into and not on wherever an edit moves the
        // data, and a save or procedure sees it.
        self.commit_pending_edits();
        let rev = self.doc.rev;
        match cmd {
            Command::New => {
                self.replace_doc(Doc::default());
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
                                rev,
                            }
                        }
                        Err(e) => {
                            JobResult::Failed(format!("Cannot open {}:\n{e}", path.display()))
                        }
                    },
                ))
            }
            Command::ImportCsv(path, opt) => {
                let log = format!("GET DATA\n  /TYPE=TXT\n  /FILE={}.", quoted(&path));
                Prepared::Job(csv_job(path, opt, log, rev))
            }
            Command::ImportXlsx(path, sheet) => Prepared::Job(xlsx_job(path, sheet, true, rev)),
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
            Command::Undo => {
                self.undo();
                Done(Ok(()))
            }
            Command::Redo => {
                self.redo();
                Done(Ok(()))
            }
            Command::InsertVariable => {
                self.insert_variable_here();
                Done(Ok(()))
            }
            Command::InsertCases => {
                self.insert_cases_here();
                Done(Ok(()))
            }
            Command::Clear => {
                self.clear_selection();
                Done(Ok(()))
            }
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
            Command::Recover(files) => Prepared::Job(Job::new(
                "Recovering data...",
                move || {
                    // Newest first; a file that cannot be read falls back to the next one.
                    let mut errors = Vec::new();
                    for path in &files {
                        match sav_io::read_sav(path) {
                            Ok((ds, _)) => return JobResult::Recovered { ds, rev },
                            Err(e) => errors.push(format!("{}: {e}", path.display())),
                        }
                    }
                    if errors.is_empty() {
                        errors.push("no recovery file was found".into());
                    }
                    JobResult::Failed(format!(
                        "Cannot recover the data. The recovery files are kept.\n{}",
                        errors.join("\n")
                    ))
                },
            )),
            Command::OpenDialog(kind) => {
                if self.dialogs.open(kind) {
                    return Done(Ok(()));
                }
                self.push_message(Message::info(
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

    /// File ▸ Save / Save As: write a clone of the current dataset to `path` (atomic, via
    /// `sav_io::write_sav`); the result sets the document's path and clears dirty.
    pub fn save_job(&self, path: PathBuf) -> Job {
        let ds = self.doc.ds.clone();
        let rev = self.doc.rev;
        Job::new("Saving data file...", move || match sav_io::write_sav(&ds, &path) {
            Ok(()) => {
                let log = format!("SAVE OUTFILE={}\n  /COMPRESSED.", quoted(&path));
                JobResult::Saved { path, log, rev }
            }
            Err(e) => JobResult::Failed(format!("Cannot save {}:\n{e}", path.display())),
        })
    }

    /// A job running procedure `f` on a clone of the dataset (see [`crate::jobs`]). `f` returns
    /// its output and whether it changed the data ([`ProcOutput::changed`], e.g. saved
    /// residuals). A changed clone replaces the document's dataset when the job finishes, unless
    /// the data was edited meanwhile (then the edit is kept and a conflict message is queued).
    pub fn procedure_job(
        &self,
        title: &str,
        f: impl FnOnce(&mut Dataset) -> ProcOutput + Send + 'static,
        syntax: String,
    ) -> Job {
        let mut ds = self.doc.ds.clone();
        let rev = self.doc.rev;
        let title = title.to_string();
        let label = if title == LOG_TITLE {
            "Running transformations...".to_string()
        } else {
            format!("Running {title}...")
        };
        Job::new(label, move || {
            let ProcOutput {
                items,
                changed,
                undo,
                error,
            } = f(&mut ds);
            if let Some(e) = error {
                return JobResult::Failed(e);
            }
            JobResult::Ran {
                ds,
                changed,
                rev,
                entry: OutputEntry {
                    title,
                    items,
                    log: syntax,
                },
                undo,
            }
        })
    }

    /// Apply the result of a finished job. Errors are returned (the UI shows them as a message).
    /// A conflict with edits made while the job ran keeps the newer in-memory state and queues a
    /// "Data changed" warning.
    pub fn finish(&mut self, r: JobResult) -> Result<(), String> {
        match r {
            JobResult::Loaded {
                ds,
                path,
                notes,
                log,
                rev,
            } => {
                if rev != self.doc.rev {
                    self.push_message(Message::warning(
                        "Data changed",
                        format!(
                            "The data was changed while {} was being read, so it was not \
                             opened. Save your changes and open the file again.",
                            path.display()
                        ),
                    ));
                    return Ok(());
                }
                self.replace_doc(Doc::new(ds, Some(path)));
                self.dataset_no = self.take_dataset_no();
                let log = format!(
                    "{log}\nDATASET NAME DataSet{} WINDOW=FRONT.",
                    self.dataset_no
                );
                let items = notes.iter().cloned().map(OutputItem::Warning).collect();
                self.push_output(OutputEntry {
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
                    self.push_message(Message::warning("Warning", text));
                }
                Ok(())
            }
            JobResult::Saved { path, log, rev } => {
                if rev == self.doc.rev {
                    self.doc.dirty = false;
                } else {
                    self.push_message(Message::warning(
                        "Data changed",
                        format!(
                            "The data was changed while it was being saved. {} holds the data \
                             as it was when saving started; save again to keep the newer changes.",
                            path.display()
                        ),
                    ));
                }
                self.doc.path = Some(path);
                self.push_output(OutputEntry {
                    title: "Log".into(),
                    items: Vec::new(),
                    log,
                });
                Ok(())
            }
            JobResult::Ran {
                ds,
                changed,
                rev,
                entry,
                undo,
            } => {
                if changed {
                    if rev == self.doc.rev {
                        self.doc.ds = ds;
                        self.doc.touch();
                        match undo {
                            // Compute/Recode: the op reverts exactly what changed, and the
                            // older edits still apply to the data before it.
                            Some(op) => self.record(op),
                            // The edits on the undo stack refer to the data before the
                            // procedure (positions, old values), so they cannot be undone
                            // across it.
                            None => self.clear_undo(),
                        }
                    } else if undo.is_some() {
                        // A transformation only changes data: nothing ran, so no log entry.
                        self.push_message(Message::warning(
                            "Data changed",
                            "The data was edited while the transformation was running, so it \
                             was not applied. Run it again.",
                        ));
                        return Ok(());
                    } else {
                        self.push_message(Message::warning(
                            "Data changed",
                            format!(
                                "The data was edited while {} was running. Its output is shown, \
                                 but its changes to the data were not applied; run it again.",
                                entry.title
                            ),
                        ));
                    }
                }
                // Bring the new output into view, selected, as SPSS does.
                let e = self.outputs.len();
                let at = if entry.log.is_empty() && !entry.items.is_empty() {
                    ItemRef::item(e, 0)
                } else {
                    ItemRef::entry(e)
                };
                self.push_output(entry);
                self.viewer.scroll_to = Some(at);
                self.viewer.selected = Some(at);
                self.show_output = true;
                self.raise_output = true;
                Ok(())
            }
            JobResult::Recovered { ds, rev } => {
                if rev != self.doc.rev {
                    self.push_message(Message::warning(
                        "Data changed",
                        "The data was changed while the recovered data was being read, so it \
                         was not opened.",
                    ));
                    return Ok(());
                }
                self.replace_doc(Doc::new(ds, None));
                self.doc.touch();
                self.dataset_no = self.take_dataset_no();
                self.untitled_no = self.next_untitled_no;
                self.next_untitled_no += 1;
                Ok(())
            }
            JobResult::Failed(e) => Err(e),
        }
    }
}

/// Quote a path for syntax (`'C:\data\x.csv'`).
pub(crate) fn quoted_path(p: &Path) -> String {
    quoted(p)
}

/// Read a CSV file with `opt`; `log` is the syntax shown in the output (see `finish`).
pub(crate) fn csv_job(path: PathBuf, opt: CsvOptions, log: String, rev: u64) -> Job {
    Job::new("Reading text data...", move || {
        match sav_io::import_csv(&path, &opt) {
            Ok(ds) => JobResult::Loaded {
                ds,
                path,
                notes: Vec::new(),
                log,
                rev,
            },
            Err(e) => JobResult::Failed(format!("Cannot import {}:\n{e}", path.display())),
        }
    })
}

/// GET DATA syntax for reading an Excel sheet.
pub(crate) fn xlsx_syntax(path: &Path, sheet: Option<&str>, header: bool) -> String {
    let mut log = format!("GET DATA\n  /TYPE=XLSX\n  /FILE={}", quoted(path));
    if let Some(s) = sheet {
        log.push_str(&format!("\n  /SHEET=name '{}'", s.replace('\'', "''")));
    }
    log.push_str(&format!(
        "\n  /CELLRANGE=FULL\n  /READNAMES={}.",
        if header { "ON" } else { "OFF" }
    ));
    log
}

/// Read one sheet (`None` = the first) of an Excel file; `header` = the first row holds names.
pub(crate) fn xlsx_job(path: PathBuf, sheet: Option<String>, header: bool, rev: u64) -> Job {
    Job::new("Reading Excel file...", move || {
        match sav_io::import_xlsx(&path, sheet.as_deref(), header) {
            Ok(ds) => {
                let log = xlsx_syntax(&path, sheet.as_deref(), header);
                JobResult::Loaded {
                    ds,
                    path,
                    notes: Vec::new(),
                    log,
                    rev,
                }
            }
            Err(e) => JobResult::Failed(format!("Cannot import {}:\n{e}", path.display())),
        }
    })
}

/// Run a procedure synchronously on the document's dataset and append its output (tests,
/// scripted use; the UI runs [`AppState::procedure_job`] on a worker thread instead).
pub fn run_procedure(
    state: &mut AppState,
    title: &str,
    f: impl FnOnce(&mut Dataset) -> ProcOutput + Send + 'static,
    syntax: String,
) {
    let r = state.procedure_job(title, f, syntax).run_inline();
    if let Err(e) = state.finish(r) {
        state.push_message(Message::error(e));
    }
}
