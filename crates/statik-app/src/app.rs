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
use crate::data_view::{commit_edit, data_view};
use crate::dialogs::message::message_box;
use crate::editor::View;
use crate::jobs::{Job, JobResult, RunningJob};
use crate::menu::{MenuAction, menu_bar};
use crate::output::{OutputAction, output_window};
use crate::state::{AppState, Command, Message, Prepared};
use crate::variable_view::{commit_var_edit, variable_view};
use egui::{Context, Key, KeyboardShortcut, Modifiers, Ui, ViewportCommand, ViewportId};
use std::path::PathBuf;

pub struct StatikApp {
    pub state: AppState,
    job: Option<RunningJob>,
    /// Last title sent to the window, to send it only when it changes.
    title_sent: String,
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

impl StatikApp {
    pub fn new(state: AppState) -> Self {
        StatikApp {
            state,
            job: None,
            title_sent: String::new(),
        }
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
        if let Err(e) = self.state.finish(r) {
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

    /// Act on a menu choice (file dialogs are shown here).
    pub fn handle(&mut self, action: MenuAction, ctx: &Context) {
        match action {
            MenuAction::Cmd(c) => self.dispatch(c, ctx),
            MenuAction::OpenData => {
                let mut d = rfd::FileDialog::new().set_title("Open Data");
                for (name, ext) in OPEN_FILTERS {
                    d = d.add_filter(name, ext);
                }
                if let Some(p) = d.pick_file() {
                    self.dispatch(Command::Open(p), ctx);
                }
            }
            MenuAction::ImportCsv => {
                let d = rfd::FileDialog::new()
                    .set_title("Open CSV Data")
                    .add_filter("CSV (*.csv;*.txt)", &["csv", "txt"]);
                if let Some(p) = d.pick_file() {
                    // The import dialog with delimiter/decimal options comes later; detect them for now.
                    self.dispatch(Command::ImportCsv(p, Default::default()), ctx);
                }
            }
            MenuAction::ImportExcel => {
                let d = rfd::FileDialog::new()
                    .set_title("Open Excel File")
                    .add_filter("Excel (*.xlsx;*.xls)", &["xlsx", "xls"]);
                if let Some(p) = d.pick_file() {
                    self.dispatch(Command::ImportXlsx(p, None), ctx);
                }
            }
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

    fn pick_save_path(&self) -> Option<PathBuf> {
        let stem = match &self.state.doc.path {
            Some(p) => p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => format!("Untitled{}", self.state.untitled_no),
        };
        let mut d = rfd::FileDialog::new()
            .set_title("Save Data As")
            .add_filter("SPSS Statistics (*.sav)", &["sav"])
            .set_file_name(format!("{stem}.sav"));
        if let Some(dir) = self.state.doc.path.as_ref().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        d.save_file()
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
        let busy = self.busy().is_some();

        let title = self.state.title();
        if title != self.title_sent {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title_sent = title;
        }

        let mut action = None;
        egui::Panel::top("menu").show(ui, |ui| {
            action = menu_bar(ui, &self.state, !busy);
        });

        if !busy && self.state.message().is_none() {
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
        });

        egui::CentralPanel::default_margins().show(ui, |ui| {
            // Disabled while busy, so views cannot edit the data a job is working on.
            ui.add_enabled_ui(!busy, |ui| {
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

        if let Some(msg) = self.state.message()
            && message_box(&ctx, msg)
        {
            self.state.dismiss_message();
        }

        if let Some(a) = action {
            self.handle(a, &ctx);
        }
        if let Some(a) = output_action {
            self.handle_output(a, &ctx);
        }
        if self.state.exit_requested {
            // One close request per Exit; a cancelled close (Task 7) leaves the app running.
            ctx.send_viewport_cmd(ViewportCommand::Close);
            self.state.exit_requested = false;
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
}
