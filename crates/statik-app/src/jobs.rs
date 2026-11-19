//! Long-running work (open, import, save, procedures, later compute/recode) off the UI thread.
//!
//! A [`Job`] is a label plus a `Send` closure producing a [`JobResult`]. The UI thread spawns it
//! with [`RunningJob::spawn`] on a worker thread with a 16 MB stack (the expression evaluator and
//! some readers recurse), polls it every frame with [`RunningJob::try_take`] and hands the result to
//! [`crate::state::AppState::finish`], which applies it to the state. Tests and other synchronous
//! callers run the same closure inline with [`Job::run_inline`].
//!
//! **Data ownership: clone on start.** A job never borrows the [`Dataset`] of the open document.
//! A job that needs the data gets a clone taken when it is created (see
//! [`crate::state::AppState::procedure_job`]); a job that changes the data (a procedure saving
//! variables, compute, recode) returns the changed dataset in [`JobResult::Ran`] and `finish`
//! swaps it in. Every result carries the document revision the job started from, and
//! `finish` refuses to overwrite newer edits (see "Revision guard" in [`crate::state`]). On top
//! of that, while a job runs the UI blocks other commands and greys out the menus and the main
//! area ([`crate::app::StatikApp`]). If the job fails or panics the document is left as it was.
//!
//! A panic inside the closure is caught (`catch_unwind`; the release profile uses
//! `panic = "unwind"`) and reported as an error message. Stack overflow and out-of-memory still
//! abort the process.
use crate::state::OutputEntry;
use dataset::Dataset;
use std::path::PathBuf;
use std::sync::mpsc;

/// Stack size of worker threads.
pub const WORKER_STACK: usize = 16 << 20;

/// Name of worker threads (the crash hook in `main.rs` skips panics on them: they are caught).
pub const WORKER_NAME: &str = "statik-job";

/// Who asked for a save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    /// File > Save / Save As...: sets the document's path and clears dirty (if unchanged since).
    User,
    /// Background autosave (Task 7): never touches the document's path, dirty flag or output.
    Autosave,
}

/// What a finished job hands back to the UI thread. `rev` is the document revision
/// ([`crate::state::Doc::rev`]) the job started from; `finish` uses it to detect edits made
/// while the job ran.
#[derive(Debug)]
pub enum JobResult {
    /// A data file was opened or imported. `notes` are the reader's warnings (`ReadReport`),
    /// `log` the equivalent syntax.
    Loaded {
        ds: Dataset,
        path: PathBuf,
        notes: Vec<String>,
        log: String,
        rev: u64,
    },
    /// The dataset (as of `rev`) was written to `path`.
    Saved {
        path: PathBuf,
        log: String,
        rev: u64,
        kind: SaveKind,
    },
    /// A procedure (or transformation) ran on a clone of the dataset. `ds` is that clone after the
    /// run; `changed` says whether it must replace the document's dataset (and mark it dirty).
    Ran {
        ds: Dataset,
        changed: bool,
        rev: u64,
        entry: OutputEntry,
    },
    /// The job failed; the message is shown to the user.
    Failed(String),
}

type Work = Box<dyn FnOnce() -> JobResult + Send + 'static>;

/// A unit of long-running work, not yet started.
pub struct Job {
    /// Shown next to the busy indicator, e.g. "Reading data file...".
    pub label: String,
    work: Work,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl Job {
    pub fn new(
        label: impl Into<String>,
        work: impl FnOnce() -> JobResult + Send + 'static,
    ) -> Self {
        Job {
            label: label.into(),
            work: Box::new(work),
        }
    }

    /// Run on the current thread (tests, synchronous `AppState::apply`). Panics are caught like
    /// on a worker thread.
    pub fn run_inline(self) -> JobResult {
        guarded(self.work)
    }
}

fn guarded(work: Work) -> JobResult {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(r) => r,
        Err(p) => {
            let what = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown error".into());
            JobResult::Failed(format!("Internal error: {what}"))
        }
    }
}

/// A job running on a worker thread.
pub struct RunningJob {
    pub label: String,
    rx: mpsc::Receiver<JobResult>,
}

impl RunningJob {
    /// Start `job` on a new worker thread. When it finishes, `wake` is called (the UI passes a
    /// closure that requests a repaint) so the result is picked up promptly.
    pub fn spawn(job: Job, wake: impl FnOnce() + Send + 'static) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let work = job.work;
        std::thread::Builder::new()
            .name(WORKER_NAME.into())
            .stack_size(WORKER_STACK)
            .spawn(move || {
                let r = guarded(work);
                let _ = tx.send(r);
                wake();
            })
            .map_err(|e| format!("Cannot start worker thread: {e}"))?;
        Ok(RunningJob {
            label: job.label,
            rx,
        })
    }

    /// The result, once the job has finished; `None` while it is still running.
    pub fn try_take(&self) -> Option<JobResult> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(JobResult::Failed(
                "Internal error: the worker thread stopped unexpectedly.".into(),
            )),
        }
    }

    /// Block until the job has finished (tests).
    pub fn wait(self) -> JobResult {
        self.rx.recv().unwrap_or_else(|_| {
            JobResult::Failed("Internal error: the worker thread stopped unexpectedly.".into())
        })
    }
}
