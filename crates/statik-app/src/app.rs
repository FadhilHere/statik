//! The Data Editor window: menu bar, main area, status bar, message box, and the job runner.
//!
//! [`StatikApp::show`] draws one frame into a root `Ui`; `eframe::App::ui` calls it, and so do the
//! `egui_kittest` tests (`Harness::builder().build_ui_state(|ui, app| app.show(ui), app)`).
//! User actions go through [`StatikApp::dispatch`]: quick commands change [`AppState`] at once;
//! long ones (open, import, save, procedures) become a [`RunningJob`] on a worker thread. Code
//! that only has `&mut AppState` queues a job with [`AppState::queue_job`]; `show` starts it on
//! the next frame. While a job runs (or waits to start) the app is busy: commands are ignored,
//! the menus, main area and Output window are disabled, and the status bar shows a spinner.
//!
//! Tests: after queueing or dispatching a job, call [`StatikApp::wait_for_job`] before asserting
//! on `state.outputs` or the document.
//!
//! **Unsaved changes.** Every action that would drop the document (closing the window, File ▸
//! Exit, New, Open, Import CSV / Excel; see [`Guarded`]) goes through [`StatikApp::guard`]: when
//! the data is dirty it asks "Save contents of data editor to <name>?" (Yes / No / Cancel). Yes
//! saves (Save As... for untitled or imported data) and continues the action only once the save
//! has succeeded; No continues; Cancel drops it (a window close is cancelled with
//! `ViewportCommand::CancelClose`). File dialogs go through a replaceable picker
//! ([`StatikApp::with_picker`]) so tests can script them.
use crate::autosave::Autosave;
use crate::data_view::{commit_edit, data_view};
use crate::dialogs::import::show_import;
use crate::dialogs::message::{Answer, message_box, recovery_box, save_changes_box};
use crate::dialogs::show_dialogs;
use crate::editor::View;
use crate::jobs::{Job, JobResult, RunningJob, SaveKind};
use crate::menu::{MenuAction, menu_bar};
use crate::output::{OutputAction, output_window};
use crate::state::{AppState, Command, Message, Prepared};
use crate::syntax::syntax_window;
use crate::variable_view::{commit_var_edit, variable_view};
use egui::{Context, Key, KeyboardShortcut, Modifiers, Ui, ViewportCommand, ViewportId};
use std::path::PathBuf;

/// An action that replaces or closes the document, so it asks about unsaved changes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guarded {
    /// Closing the window, or File ▸ Exit.
    Exit,
    /// File ▸ New ▸ Data.
    New,
    /// File ▸ Open ▸ Data...
    Open,
    /// File ▸ Import Data ▸ CSV Data...
    ImportCsv,
    /// File ▸ Import Data ▸ Excel...
    ImportExcel,
}

/// A file dialog the app needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickRequest {
    /// File ▸ Open ▸ Data...: a data file to open.
    OpenData,
    /// File ▸ Import Data ▸ CSV Data...
    ImportCsv,
    /// File ▸ Import Data ▸ Excel...
    ImportExcel,
    /// Save Data As: a `.sav` file name, suggesting `file_name` in `dir`.
    SaveData {
        file_name: String,
        dir: Option<PathBuf>,
    },
}

/// Shows a file dialog and returns the chosen path (`None` = cancelled).
pub type Picker = Box<dyn FnMut(&PickRequest) -> Option<PathBuf>>;

pub struct StatikApp {
    pub state: AppState,
    job: Option<RunningJob>,
    /// Last title sent to the window, to send it only when it changes.
    title_sent: String,
    picker: Picker,
    /// The "Save contents of data editor" question being asked, and for which action.
    save_prompt: Option<Guarded>,
    /// The action to continue when the save started by answering Yes succeeds.
    after_save: Option<Guarded>,
    /// The action to continue on the next frame (its save succeeded).
    resume: Option<Guarded>,
    /// The window may close: the user has dealt with unsaved changes.
    close_ok: bool,
    /// Autosave of unsaved changes ([`crate::autosave`]); off unless set with
    /// [`Self::with_autosave`].
    pub autosave: Autosave,
    /// A recovery file to offer ("Statik closed unexpectedly. Recover data?").
    recovery: Option<PathBuf>,
}

impl Default for StatikApp {
    fn default() -> Self {
        Self::new(AppState::new())
    }
}

const OPEN_FILTERS: [(&str, &[&str]); 4] = [
    ("All supported files", &["sav", "csv", "txt", "xlsx", "xls"]),
    ("SPSS Statistics (*.sav)", &["sav"]),
    ("CSV (*.csv;*.txt)", &["csv", "txt"]),
    ("Excel (*.xlsx;*.xls)", &["xlsx", "xls"]),
];

/// The native file dialogs.
fn rfd_pick(req: &PickRequest) -> Option<PathBuf> {
    match req {
        PickRequest::OpenData => {
            let mut d = rfd::FileDialog::new().set_title("Open Data");
            for (name, ext) in OPEN_FILTERS {
                d = d.add_filter(name, ext);
            }
            d.pick_file()
        }
        PickRequest::ImportCsv => rfd::FileDialog::new()
            .set_title("Open CSV Data")
            .add_filter("CSV (*.csv;*.txt)", &["csv", "txt"])
            .pick_file(),
        PickRequest::ImportExcel => rfd::FileDialog::new()
            .set_title("Open Excel File")
            .add_filter("Excel (*.xlsx;*.xls)", &["xlsx", "xls"])
            .pick_file(),
        PickRequest::SaveData { file_name, dir } => {
            let mut d = rfd::FileDialog::new()
                .set_title("Save Data As")
                .add_filter("SPSS Statistics (*.sav)", &["sav"])
                .set_file_name(file_name);
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            d.save_file()
        }
    }
}

impl StatikApp {
    pub fn new(state: AppState) -> Self {
        StatikApp {
            state,
            job: None,
            title_sent: String::new(),
            picker: Box::new(rfd_pick),
            save_prompt: None,
            after_save: None,
            resume: None,
            close_ok: false,
            autosave: Autosave::default(),
            recovery: None,
        }
    }

    /// Autosave unsaved changes with `autosave`.
    pub fn with_autosave(mut self, autosave: Autosave) -> Self {
        self.autosave = autosave;
        self
    }

    /// Ask "Statik closed unexpectedly. Recover data?" for the recovery file `path` (found at
    /// startup with [`crate::autosave::pending_recovery`]).
    pub fn offer_recovery(&mut self, path: PathBuf) {
        self.recovery = Some(path);
    }

    /// Use `picker` instead of the native file dialogs (tests).
    pub fn with_picker(mut self, picker: Picker) -> Self {
        self.picker = picker;
        self
    }

    /// The action waiting at the "Save contents of data editor" question, if it is asked.
    pub fn save_prompt(&self) -> Option<Guarded> {
        self.save_prompt
    }

    /// The window is closing (or may close): unsaved changes were dealt with.
    pub fn closing(&self) -> bool {
        self.close_ok
    }

    /// Label of the running (or queued) job, if any.
    pub fn busy(&self) -> Option<&str> {
        self.job
            .as_ref()
            .map(|j| j.label.as_str())
            .or_else(|| self.state.pending_job.as_ref().map(|j| j.label.as_str()))
    }

    /// Block until the running job and any queued job have finished, applying their results
    /// (tests). A queued job that was not started yet runs inline.
    pub fn wait_for_job(&mut self) {
        loop {
            if let Some(j) = self.job.take() {
                let r = j.wait();
                self.finish(r);
            } else if let Some(job) = self.state.pending_job.take() {
                let r = job.run_inline();
                self.finish(r);
            } else {
                break;
            }
        }
    }

    fn finish(&mut self, r: JobResult) {
        let user_save = matches!(
            r,
            JobResult::Saved {
                kind: SaveKind::User,
                ..
            }
        );
        let recovered = matches!(r, JobResult::Recovered { .. });
        let result = self.state.finish(r);
        if recovered && result.is_ok() && self.state.doc.dirty() {
            // Write the recovered data to a fresh recovery file at once.
            self.autosave.recovered();
        }
        // The save started by "Yes" at the save-changes question: continue only if it worked.
        if let Some(a) = self.after_save.take() {
            if user_save && result.is_ok() && !self.state.doc.dirty() {
                self.resume = Some(a);
            } else {
                self.state.cancel_exit();
            }
        }
        if let Err(e) = result {
            self.state.push_message(Message::error(e));
        }
    }

    /// Run a command: apply it now or start its job. Ignored while busy.
    pub fn dispatch(&mut self, cmd: Command, ctx: &Context) {
        if self.busy().is_some() {
            return;
        }
        match self.state.prepare(cmd) {
            Prepared::Done(Ok(())) => {}
            Prepared::Done(Err(e)) => self.state.push_message(Message::error(e)),
            Prepared::Job(job) => self.start(job, ctx),
        }
    }

    /// Start a job on a worker thread. Ignored while another job runs.
    pub fn start(&mut self, job: Job, ctx: &Context) {
        if self.job.is_some() {
            return;
        }
        let waker = ctx.clone();
        match RunningJob::spawn(job, move || waker.request_repaint()) {
            Ok(j) => self.job = Some(j),
            Err(e) => self.state.push_message(Message::error(e)),
        }
    }

    /// Run `action`, first asking about unsaved changes if the data is dirty. Ignored while busy
    /// (the menus are disabled then; a window close is handled in [`Self::show`]).
    pub fn guard(&mut self, action: Guarded, ctx: &Context) {
        if self.busy().is_some() {
            return;
        }
        self.state.commit_pending_edits();
        if self.state.doc.dirty() {
            self.save_prompt = Some(action);
        } else {
            self.proceed(action, ctx);
        }
    }

    /// Carry out a guarded action (unsaved changes were dealt with).
    fn proceed(&mut self, action: Guarded, ctx: &Context) {
        match action {
            Guarded::Exit => {
                self.close_ok = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Guarded::New => self.dispatch(Command::New, ctx),
            Guarded::Open => {
                if let Some(p) = (self.picker)(&PickRequest::OpenData) {
                    self.dispatch(Command::Open(p), ctx);
                }
            }
            Guarded::ImportCsv => {
                if let Some(p) = (self.picker)(&PickRequest::ImportCsv) {
                    self.state.open_import_csv(p);
                }
            }
            Guarded::ImportExcel => {
                if let Some(p) = (self.picker)(&PickRequest::ImportExcel) {
                    self.state.open_import_excel(p);
                }
            }
        }
    }

    /// The answer to "Save contents of data editor to <name>?" for `action`.
    fn answer_save_prompt(&mut self, action: Guarded, answer: Answer, ctx: &Context) {
        match answer {
            Answer::Yes => {
                let cmd = if self.state.needs_save_as() {
                    match self.pick_save_path() {
                        Some(p) => Command::SaveAs(p),
                        None => {
                            self.state.cancel_exit();
                            return;
                        }
                    }
                } else {
                    Command::Save
                };
                self.dispatch(cmd, ctx);
                if self.job.is_some() {
                    self.after_save = Some(action);
                } else {
                    self.state.cancel_exit();
                }
            }
            Answer::No => self.proceed(action, ctx),
            Answer::Cancel => self.state.cancel_exit(),
        }
    }

    /// Act on a menu choice (file dialogs are shown here).
    pub fn handle(&mut self, action: MenuAction, ctx: &Context) {
        match action {
            MenuAction::Cmd(Command::New) => self.guard(Guarded::New, ctx),
            MenuAction::Cmd(Command::Exit) => self.guard(Guarded::Exit, ctx),
            MenuAction::Cmd(c) => self.dispatch(c, ctx),
            MenuAction::OpenData => self.guard(Guarded::Open, ctx),
            MenuAction::ImportCsv => self.guard(Guarded::ImportCsv, ctx),
            MenuAction::ImportExcel => self.guard(Guarded::ImportExcel, ctx),
            MenuAction::Save if !self.state.needs_save_as() => self.dispatch(Command::Save, ctx),
            MenuAction::Save | MenuAction::SaveAs => {
                if let Some(p) = self.pick_save_path() {
                    self.dispatch(Command::SaveAs(p), ctx);
                }
            }
            MenuAction::Copy | MenuAction::Cut => {
                if self.state.editor.view == View::Data {
                    commit_edit(&mut self.state);
                    ctx.copy_text(self.state.copy_selection());
                    if matches!(action, MenuAction::Cut) {
                        self.dispatch(Command::Clear, ctx);
                    }
                }
            }
            // The text arrives as an `Event::Paste`, which the Data View pastes at the cursor.
            MenuAction::Paste => ctx.send_viewport_cmd(ViewportCommand::RequestPaste),
            MenuAction::ShowDataEditor => ctx.send_viewport_cmd(ViewportCommand::Focus),
            MenuAction::About => {
                self.state.push_message(Message::info(
                    "About Statik",
                    format!(
                        "Statik {}\n\nA lightweight statistics program for students.\nOpen .sav, CSV and Excel data, run analyses and copy the output to Word.",
                        env!("CARGO_PKG_VERSION")
                    ),
                ));
            }
        }
    }

    fn pick_save_path(&mut self) -> Option<PathBuf> {
        let stem = match &self.state.doc.path {
            Some(p) => p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => format!("Untitled{}", self.state.untitled_no),
        };
        let dir = self
            .state
            .doc
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|d| d.to_path_buf());
        (self.picker)(&PickRequest::SaveData {
            file_name: format!("{stem}.sav"),
            dir,
        })
    }

    /// Draw one frame.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();

        if let Some(r) = self.job.as_ref().and_then(|j| j.try_take()) {
            self.job = None;
            self.finish(r);
        }
        if self.job.is_none()
            && let Some(job) = self.state.pending_job.take()
        {
            self.start(job, &ctx);
        }
        if let Some(a) = self.resume.take() {
            self.proceed(a, &ctx);
        }
        // Autosave runs on its own thread, also while a job runs.
        let waker = ctx.clone();
        if let Some(wait) = self
            .autosave
            .tick(&self.state, std::time::Instant::now(), move || {
                waker.request_repaint()
            })
        {
            ctx.request_repaint_after(wait);
        }
        // Closing the window (title-bar X, Alt+F4): ask about unsaved changes first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_ok {
            self.state.commit_pending_edits();
            if self.state.doc.dirty() {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.state.cancel_exit();
                self.save_prompt = Some(Guarded::Exit);
            } else {
                self.close_ok = true;
            }
        }
        let busy = self.busy().is_some();
        // A procedure dialog is modal: the menus and the Data Editor wait until it closes.
        let modal = self.state.dialogs.is_open() || self.state.import.is_some();

        let title = self.state.title();
        if title != self.title_sent {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title_sent = title;
        }

        let mut action = None;
        egui::Panel::top("menu").show(ui, |ui| {
            action = menu_bar(ui, &self.state, !busy && !modal);
        });

        if !busy && !modal && self.state.message().is_none() && self.save_prompt.is_none() {
            ui.input_mut(|i| {
                if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O)) {
                    action = Some(MenuAction::OpenData);
                } else if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)) {
                    action = Some(MenuAction::Save);
                }
            });
            // Undo/redo of data edits, unless a text field (cell editor, dialog) has the keys.
            if !ctx.text_edit_focused() && self.state.editor.var_dialog.is_none() {
                ui.input_mut(|i| {
                    let redo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
                    let redo2 =
                        KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
                    if i.consume_shortcut(&redo) || i.consume_shortcut(&redo2) {
                        action = Some(MenuAction::Cmd(Command::Redo));
                    } else if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z))
                    {
                        action = Some(MenuAction::Cmd(Command::Undo));
                    }
                });
            }
        }

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| match self.busy() {
                Some(label) => {
                    ui.spinner();
                    ui.label(label);
                }
                None => {
                    ui.label("Statik Processor is ready");
                }
            });
            if let Some(note) = self.autosave.note() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(note).color(ui.visuals().warn_fg_color))
                        .on_hover_text("Unsaved changes are not being backed up. Save your data.");
                });
            }
        });

        egui::CentralPanel::default_margins().show(ui, |ui| {
            // Disabled while busy, so views cannot edit the data a job is working on.
            ui.add_enabled_ui(!busy && !modal, |ui| {
                let state = &mut self.state;
                egui::Panel::bottom("view_tabs").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (label, view) in
                            [("Data View", View::Data), ("Variable View", View::Variable)]
                        {
                            if ui
                                .selectable_label(state.editor.view == view, label)
                                .clicked()
                                && state.editor.view != view
                            {
                                commit_edit(state);
                                commit_var_edit(state);
                                state.editor.view = view;
                            }
                        }
                    });
                });
                match state.editor.view {
                    View::Data => data_view(ui, state),
                    View::Variable => variable_view(ui, state),
                }
            });
        });

        // The Output viewer (its contents are disabled while busy).
        let mut output_action = None;
        if self.state.show_output {
            output_action = output_window(&ctx, &mut self.state, busy);
        }

        show_dialogs(&ctx, &mut self.state, busy);
        show_import(&ctx, &mut self.state, busy);
        syntax_window(&ctx, &mut self.state);

        if let Some(msg) = self.state.message()
            && message_box(&ctx, msg)
        {
            self.state.dismiss_message();
        }

        // Startup after a crash: offer the newest recovery file.
        if self.recovery.is_some()
            && self.busy().is_none()
            && self.state.message().is_none()
            && let Some(recover) = recovery_box(&ctx)
        {
            let path = self.recovery.take().expect("checked above");
            if recover {
                self.dispatch(Command::Recover(path), &ctx);
            } else {
                self.autosave.discard();
            }
        }

        // The save-changes question waits for a running job (e.g. a save) and for messages.
        if let Some(a) = self.save_prompt
            && self.recovery.is_none()
            && self.busy().is_none()
            && self.state.message().is_none()
        {
            if !self.state.doc.dirty() {
                // Saved meanwhile: nothing to ask.
                self.save_prompt = None;
                self.proceed(a, &ctx);
            } else if let Some(answer) = save_changes_box(&ctx, &self.state.doc_name()) {
                self.save_prompt = None;
                self.answer_save_prompt(a, answer, &ctx);
            }
        }

        if let Some(a) = action {
            self.handle(a, &ctx);
        }
        if let Some(a) = output_action {
            self.handle_output(a, &ctx);
        }
        if self.state.exit_requested {
            // File ▸ Exit from code that only has the state: ask about unsaved changes, then
            // close.
            self.state.exit_requested = false;
            self.guard(Guarded::Exit, &ctx);
        }
    }

    /// Act on a choice from the Output window's menus.
    pub fn handle_output(&mut self, action: OutputAction, ctx: &Context) {
        match action {
            OutputAction::SaveOutput => {
                let d = rfd::FileDialog::new()
                    .set_title("Save Output As")
                    .add_filter("Statik Output (*.stko)", &["stko"])
                    .set_file_name("Output1.stko");
                if let Some(p) = d.save_file()
                    && let Err(e) = self.state.save_output(&p)
                {
                    self.state.push_message(Message::error(e));
                }
            }
            OutputAction::OpenOutput => {
                let d = rfd::FileDialog::new()
                    .set_title("Open Output")
                    .add_filter("Statik Output (*.stko)", &["stko"]);
                if let Some(p) = d.pick_file()
                    && let Err(e) = self.state.open_output(&p)
                {
                    self.state.push_message(Message::error(e));
                }
            }
            OutputAction::ShowDataEditor => {
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus)
            }
            OutputAction::About => self.handle(MenuAction::About, ctx),
        }
    }
}

impl eframe::App for StatikApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }

    /// A normal exit: the recovery files are deleted and the clean-exit marker is written.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.autosave.clean_exit();
    }
}
