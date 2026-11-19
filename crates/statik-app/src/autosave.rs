//! Autosave and crash recovery.
//!
//! **Per-instance folders.** Every running Statik owns one folder
//! `%LOCALAPPDATA%\Statik\recovery\<pid>-<start millis>\` ([`Instance`]) holding a `lock` file that
//! it keeps open without sharing for as long as it runs. It writes, prunes and deletes only in its
//! own folder, so several Statik windows never touch each other's files.
//!
//! **Autosave.** While the document has unsaved changes, [`Autosave::tick`] (called every frame
//! by [`crate::StatikApp`]) writes a copy of the data every [`INTERVAL`] as
//! `autosave-<unix millis>.sav`, keeping the [`KEEP`] newest files. A file is only written when
//! the revision changed since the last autosave. When the document becomes clean again (saved)
//! its files are deleted.
//!
//! **Own lane.** The dataset is cloned on the UI thread (a snapshot of the current revision) and
//! written on a separate `statik-autosave` thread, independent of the single job slot
//! ([`crate::jobs`]): a running procedure does not delay an autosave and an autosave never makes
//! the UI busy. It never touches the document's path, dirty flag or output. A failure becomes one
//! status-bar note ([`Autosave::note`]), not a message box; it is retried at the next interval.
//!
//! **Recovery.** A normal exit ([`Autosave::clean_exit`], from `eframe::App::on_exit`) deletes
//! the instance's folder. A folder whose lock can be taken belongs to an instance that is gone;
//! if it still holds autosave files, that instance ended unexpectedly. [`find_orphans`] returns
//! those folders ([`Orphan`]) at startup (deleting dead folders without autosave files and stray
//! `*.tmp` files of a write cut short), and the app asks "Statik closed unexpectedly. Recover
//! data?". The orphan folder is removed after Recover (once the recovered data is backed up in
//! the new instance's folder or saved) or Discard; otherwise it stays for the next start.
use crate::state::AppState;
use dataset::Dataset;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Time between autosaves.
pub const INTERVAL: Duration = Duration::from_secs(180);
/// Recovery files kept per instance (the newest).
pub const KEEP: usize = 3;
/// Name of the autosave thread (the crash hook in `main.rs` skips its caught panics).
pub const THREAD_NAME: &str = "statik-autosave";

const PREFIX: &str = "autosave-";
const LOCK: &str = "lock";

/// `%LOCALAPPDATA%\Statik\recovery`, or `None` when `LOCALAPPDATA` is not set (autosave off).
pub fn default_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Statik").join("recovery"))
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Open (creating) `path` for exclusive use: fails while another handle has it open. Off Windows
/// there is no such check (Statik targets Windows).
fn open_locked(path: &Path) -> std::io::Result<File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(0);
    }
    o.open(path)
}

/// This instance's recovery folder, locked while the instance runs. Dropping it releases the
/// lock but keeps the folder (as a crash would); [`Instance::remove`] deletes it.
#[derive(Debug)]
pub struct Instance {
    dir: PathBuf,
    lock: Option<File>,
}

impl Instance {
    /// Create and lock a new folder `<root>\<pid>-<millis>`.
    pub fn create(root: &Path) -> Result<Instance, String> {
        let pid = std::process::id();
        let mut t = now_millis();
        let dir = loop {
            let d = root.join(format!("{pid}-{t}"));
            if !d.exists() {
                break d;
            }
            t += 1;
        };
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let lock = open_locked(&dir.join(LOCK))
            .map_err(|e| format!("cannot lock {}: {e}", dir.display()))?;
        Ok(Instance {
            dir,
            lock: Some(lock),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Release the lock and delete the folder (normal exit).
    pub fn remove(mut self) {
        self.lock = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The folder of an instance that ended unexpectedly, still holding autosave files. Its lock is
/// held while this value lives, so no other Statik offers it at the same time. Dropping it keeps
/// the folder for the next start; [`Orphan::remove`] deletes it.
#[derive(Debug)]
pub struct Orphan {
    dir: PathBuf,
    lock: Option<File>,
    files: Vec<PathBuf>,
}

impl Orphan {
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    /// The autosave files, newest first (never empty).
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
    /// Release the lock and delete the folder (after Recover or Discard).
    pub fn remove(mut self) {
        self.lock = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Folders under `root` left by instances that are gone and still holding autosave files,
/// newest first. Dead folders without autosave files are deleted, and so are `*.tmp` files left
/// in dead folders by a write that was cut short. Folders of running instances are skipped.
pub fn find_orphans(root: &Path) -> Vec<Orphan> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dir in rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()) {
        let Ok(lock) = open_locked(&dir.join(LOCK)) else {
            continue; // its instance is still running
        };
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for p in rd.filter_map(|e| e.ok()).map(|e| e.path()) {
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("tmp")) {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        let files = recovery_files(&dir);
        if files.is_empty() {
            drop(lock);
            let _ = std::fs::remove_dir_all(&dir);
        } else {
            out.push(Orphan {
                dir,
                lock: Some(lock),
                files,
            });
        }
    }
    out.sort_by_key(|o| std::cmp::Reverse(o.files.first().and_then(|f| stamp(f))));
    out
}

/// The time stamp of a recovery file name (`autosave-<millis>.sav`).
fn stamp(path: &Path) -> Option<u128> {
    let name = path.file_name()?.to_str()?;
    let digits = name.strip_prefix(PREFIX)?.strip_suffix(".sav")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn stamped_files(dir: &Path) -> Vec<(u128, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(u128, PathBuf)> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| stamp(&p).map(|t| (t, p)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v
}

/// The recovery files in `dir`, newest first.
pub fn recovery_files(dir: &Path) -> Vec<PathBuf> {
    stamped_files(dir).into_iter().map(|(_, p)| p).collect()
}

/// Write `ds` as a new recovery file in `dir` (created if needed) and delete all but the
/// [`KEEP`] newest. Returns the file written.
pub fn write_recovery(dir: &Path, ds: &Dataset) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    // Never older than (or equal to) a file already there, so "newest" is the last written.
    let newest = stamped_files(dir).first().map_or(0, |f| f.0);
    let mut t = now_millis().max(newest + 1);
    let path = loop {
        let p = dir.join(format!("{PREFIX}{t:016}.sav"));
        if !p.exists() {
            break p;
        }
        t += 1;
    };
    sav_io::write_sav(ds, &path).map_err(|e| e.to_string())?;
    prune(dir, KEEP);
    Ok(path)
}

/// Delete all but the `keep` newest recovery files.
pub fn prune(dir: &Path, keep: usize) {
    for p in recovery_files(dir).into_iter().skip(keep) {
        let _ = std::fs::remove_file(p);
    }
}

/// Delete every recovery file in `dir`.
pub fn remove_all(dir: &Path) {
    prune(dir, 0);
}

struct Running {
    rev: u64,
    rx: mpsc::Receiver<Result<PathBuf, String>>,
    handle: JoinHandle<()>,
}

/// The autosave timer and its background writer. See the module docs.
pub struct Autosave {
    /// The folder written to (this instance's).
    dir: Option<PathBuf>,
    /// Holds the folder's lock while the app runs.
    instance: Option<Instance>,
    /// Time between autosaves ([`INTERVAL`]; tests shorten it).
    pub interval: Duration,
    last_try: Instant,
    /// Revision written by the last successful autosave.
    saved_rev: Option<u64>,
    running: Option<Running>,
    note: Option<String>,
    /// Recovery files of the current document may exist in the folder.
    have_files: bool,
    /// Autosave at the next tick, whatever the time.
    due_now: bool,
}

impl std::fmt::Debug for Autosave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Autosave")
            .field("dir", &self.dir)
            .field("saved_rev", &self.saved_rev)
            .field("running", &self.running.is_some())
            .field("note", &self.note)
            .finish_non_exhaustive()
    }
}

impl Default for Autosave {
    /// Autosave off (no folder).
    fn default() -> Self {
        Self::new(None, Instant::now())
    }
}

impl Autosave {
    /// Autosave into the plain folder `dir` (`None` = off; no lock, tests); the first autosave
    /// is due [`INTERVAL`] after `now`.
    pub fn new(dir: Option<PathBuf>, now: Instant) -> Self {
        Autosave {
            dir,
            instance: None,
            interval: INTERVAL,
            last_try: now,
            saved_rev: None,
            running: None,
            note: None,
            have_files: false,
            due_now: false,
        }
    }

    /// Autosave into this instance's locked folder.
    pub fn for_instance(instance: Instance, now: Instant) -> Self {
        let mut a = Self::new(Some(instance.dir().to_path_buf()), now);
        a.instance = Some(instance);
        a
    }

    /// Autosave is off because its folder could not be set up; `why` is shown in the status bar.
    pub fn unavailable(why: impl Into<String>) -> Self {
        let mut a = Self::default();
        a.failed(why.into());
        a
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Why the last autosave failed (shown in the status bar); cleared by a successful one.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// An autosave is being written.
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// The current document has been autosaved (at some revision) since [`Self::recovered`].
    pub fn has_saved(&self) -> bool {
        self.saved_rev.is_some()
    }

    /// Data was recovered from another instance's folder: autosave it at the next tick.
    pub fn recovered(&mut self) {
        self.saved_rev = None;
        self.due_now = true;
    }

    /// Call every frame. Starts an autosave when one is due; returns how long until the next
    /// one could be due (to schedule a repaint), or `None` when nothing is waiting on time.
    /// `wake` is called by the writer thread when it has finished.
    pub fn tick(
        &mut self,
        state: &AppState,
        now: Instant,
        wake: impl FnOnce() + Send + 'static,
    ) -> Option<Duration> {
        self.poll(false);
        let dir = self.dir.clone()?;
        if self.running.is_some() {
            return None;
        }
        if !state.doc.dirty() {
            if self.have_files {
                remove_all(&dir);
                self.have_files = false;
            }
            return None;
        }
        if self.saved_rev == Some(state.doc.rev()) {
            return None;
        }
        let since = now.saturating_duration_since(self.last_try);
        if since < self.interval && !self.due_now {
            return Some(self.interval - since);
        }
        self.last_try = now;
        self.due_now = false;
        self.start(dir, state.doc.ds().clone(), state.doc.rev(), wake);
        None
    }

    fn start(
        &mut self,
        dir: PathBuf,
        ds: Dataset,
        rev: u64,
        wake: impl FnOnce() + Send + 'static,
    ) {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    write_recovery(&dir, &ds)
                }))
                .unwrap_or_else(|_| Err("internal error".into()));
                let _ = tx.send(r);
                wake();
            });
        match spawned {
            Ok(handle) => self.running = Some(Running { rev, rx, handle }),
            Err(e) => self.failed(format!("cannot start the autosave thread: {e}")),
        }
    }

    fn failed(&mut self, why: String) {
        if self.note.is_none() {
            self.note = Some(format!("Autosave failed: {why}"));
        }
    }

    /// Pick up a finished autosave (`block` = wait for it).
    fn poll(&mut self, block: bool) {
        let Some(r) = &self.running else {
            return;
        };
        let result = if block {
            r.rx.recv().map_err(|_| ())
        } else {
            match r.rx.try_recv() {
                Ok(v) => Ok(v),
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Err(()),
            }
        };
        let Running { rev, handle, .. } = self.running.take().expect("checked above");
        let _ = handle.join();
        match result {
            Ok(Ok(_)) => {
                self.saved_rev = Some(rev);
                self.have_files = true;
                self.note = None;
            }
            Ok(Err(e)) => self.failed(e),
            Err(()) => self.failed("the autosave thread stopped".into()),
        }
    }

    /// Wait for a running autosave to finish (tests, exit).
    pub fn wait(&mut self) {
        self.poll(true);
    }

    /// A normal exit: wait for a running autosave, then delete this instance's folder.
    pub fn clean_exit(&mut self) {
        self.wait();
        match self.instance.take() {
            Some(i) => i.remove(),
            None => {
                if let Some(d) = &self.dir {
                    let _ = std::fs::remove_dir_all(d);
                }
            }
        }
        self.dir = None;
        self.have_files = false;
    }
}
