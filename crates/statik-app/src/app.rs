//! The Data Editor window: menu bar, main area, status bar, message box, and the job runner.
//!
//! [`StatikApp::show`] draws one frame into a root `Ui`; `eframe::App::ui` calls it, and so do the
//! `egui_kittest` tests (`Harness::builder().build_ui_state(|ui, app| app.show(ui), app)`).
//! User actions go through [`StatikApp::dispatch`]: quick commands change [`AppState`] at once;
//! long ones (open, import, save, procedures) become a [`RunningJob`] on a worker thread, and
//! while it runs every command is blocked (menus greyed out, shortcuts ignored, busy spinner in
//! the status bar).
use crate::dialogs::message::message_box;
use crate::jobs::{Job, RunningJob};
use crate::menu::{MenuAction, menu_bar};
use crate::state::{AppState, Command, Message, Prepared};
use egui::{Context, Key, KeyboardShortcut, Modifiers, Ui, ViewportCommand};
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

    /// Label of the running job, if any.
    pub fn busy(&self) -> Option<&str> {
        self.job.as_ref().map(|j| j.label.as_str())
    }

    /// Block until the running job (if any) has finished and apply its result (tests).
    pub fn wait_for_job(&mut self) {
        if let Some(j) = self.job.take() {
            let r = j.wait();
            self.finish(r);
        }
    }

    fn finish(&mut self, r: crate::jobs::JobResult) {
        if let Err(e) = self.state.finish(r) {
            self.state.message = Some(Message::error(e));
        }
    }

    /// Run a command: apply it now or start its job. Ignored while a job runs.
    pub fn dispatch(&mut self, cmd: Command, ctx: &Context) {
        if self.job.is_some() {
            return;
        }
        match self.state.prepare(cmd) {
            Prepared::Done(Ok(())) => {}
            Prepared::Done(Err(e)) => self.state.message = Some(Message::error(e)),
            Prepared::Job(job) => self.start(job, ctx),
        }
    }

    /// Start a job on a worker thread (also used for procedures:
    /// `app.start(app.state.procedure_job(...), ctx)`).
    pub fn start(&mut self, job: Job, ctx: &Context) {
        if self.job.is_some() {
            return;
        }
        let waker = ctx.clone();
        match RunningJob::spawn(job, move || waker.request_repaint()) {
            Ok(j) => self.job = Some(j),
            Err(e) => self.state.message = Some(Message::error(e)),
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
            MenuAction::ShowDataEditor => ctx.send_viewport_cmd(ViewportCommand::Focus),
            MenuAction::About => {
                self.state.message = Some(Message::info(
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
        let busy = self.job.is_some();

        let title = self.state.title();
        if title != self.title_sent {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title_sent = title;
        }

        let mut action = None;
        egui::Panel::top("menu").show(ui, |ui| {
            action = menu_bar(ui, &self.state, !busy);
        });

        if !busy && self.state.message.is_none() {
            ui.input_mut(|i| {
                if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O)) {
                    action = Some(MenuAction::OpenData);
                } else if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)) {
                    action = Some(MenuAction::Save);
                }
            });
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
            // Placeholder until the Data View / Variable View (Task 3).
            let ds = &self.state.doc.ds;
            ui.label(format!("{} variables, {} cases", ds.n_vars(), ds.n_cases()));
        });

        if self.state.show_output {
            self.output_placeholder(&ctx);
        }

        if let Some(msg) = &self.state.message
            && message_box(&ctx, msg)
        {
            self.state.message = None;
        }

        if let Some(a) = action {
            self.handle(a, &ctx);
        }
        if self.state.exit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// Minimal output list until the Output viewer (Task 4).
    fn output_placeholder(&mut self, ctx: &Context) {
        let mut open = true;
        egui::Window::new("Output")
            .open(&mut open)
            .default_size([480.0, 360.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for e in &self.state.outputs {
                        ui.strong(&e.title);
                        if !e.log.is_empty() {
                            ui.monospace(&e.log);
                        }
                        for it in &e.items {
                            if let output_model::OutputItem::Warning(w) = it {
                                ui.label(format!("Warning: {w}"));
                            }
                        }
                        ui.separator();
                    }
                });
            });
        if !open {
            self.state.show_output = false;
        }
    }
}

impl eframe::App for StatikApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
