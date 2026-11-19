//! Dialog windows.
//!
//! **Procedure dialogs** (Analyze menu) implement [`ProcDialog`]; their state lives in
//! [`Dialogs`] inside [`DialogHost`] (`AppState::dialogs`), so the last settings are kept
//! between openings, as in SPSS. `Command::OpenDialog(kind)` calls [`DialogHost::open`];
//! `StatikApp::show` draws the open dialog with [`show_dialogs`]:
//! - a resizable window titled like SPSS, its body in a scroll area, and the bottom row
//!   OK / Paste / Reset / Cancel / Help (OK and Paste disabled until [`ProcDialog::ready`]);
//! - while a dialog is open the menus and the Data Editor are disabled (modal);
//! - sub-dialogs (Statistics..., Plots..., ...) are small windows on top with Continue /
//!   Cancel / Help; Cancel restores the settings from before the sub-dialog opened.
//!
//! OK maps the stored variable names to current `VarId`s ([`ProcDialog::options`]; a missing
//! name gives "Variable x no longer exists." and nothing runs), queues the procedure as a job
//! with the dialog's syntax as the log entry, and closes the dialog. Paste appends the syntax to
//! the Syntax window (`AppState::syntax`) and closes the dialog.
pub mod descriptives;
pub mod explore;
pub mod frame;
pub mod frequencies;
pub mod message;
pub mod npar_ks;
pub mod npar_runs;
pub mod oneway;
pub mod regression;
pub mod ttest_ind;
pub mod ttest_one;
pub mod ttest_paired;
pub mod var_picker;

use crate::state::{AppState, DialogKind, Message, ProcOutput};
use dataset::{Dataset, VarId};
use egui::{Context, Ui, Vec2, vec2};
use var_picker::Eligible;

/// What the user did in a dialog this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogAction {
    None,
    Ok,
    Paste,
    Reset,
    Cancel,
    Help,
    /// Open the sub-dialog with this button label (e.g. `"Statistics..."`).
    Sub(&'static str),
}

impl DialogAction {
    /// `self`, unless it is `None`, then `other`.
    pub fn or(self, other: DialogAction) -> DialogAction {
        if self == DialogAction::None {
            other
        } else {
            self
        }
    }
}

/// Buttons of a sub-dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubAction {
    None,
    Continue,
    Cancel,
    Help,
}

/// A procedure ready to run: the output title and the work (run on a worker thread).
pub struct RunSpec {
    /// Outline title of the output entry ("Regression", "T-Test", ...).
    pub title: &'static str,
    pub run: Box<dyn FnOnce(&mut Dataset) -> ProcOutput + Send>,
}

impl RunSpec {
    /// A procedure that reads the data only.
    pub fn read(
        title: &'static str,
        f: impl FnOnce(&Dataset) -> Vec<output_model::OutputItem> + Send + 'static,
    ) -> Self {
        RunSpec {
            title,
            run: Box::new(move |ds: &mut Dataset| ProcOutput::output(f(ds))),
        }
    }
    /// A procedure that may add variables (it changed the data when the count grew).
    pub fn write(
        title: &'static str,
        f: impl FnOnce(&mut Dataset) -> Vec<output_model::OutputItem> + Send + 'static,
    ) -> Self {
        RunSpec {
            title,
            run: Box::new(move |ds: &mut Dataset| {
                let before = ds.n_vars();
                let items = f(ds);
                ProcOutput {
                    items,
                    changed: ds.n_vars() != before,
                }
            }),
        }
    }
}

/// A procedure dialog.
pub trait ProcDialog {
    /// Window title, as in SPSS ("Linear Regression").
    fn title(&self) -> &str;
    /// Draw the body (variable lists, options, sub-dialog buttons). The host draws the bottom
    /// button row.
    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction;
    /// Required boxes are filled (OK and Paste enabled).
    fn ready(&self) -> bool;
    /// Map names to the current variables and build the run; `Err` explains what is wrong.
    fn options(&self, ds: &Dataset) -> Result<RunSpec, String>;
    /// Equivalent command syntax for the current settings.
    fn syntax(&self, ds: &Dataset) -> String;
    /// Back to the defaults (Reset).
    fn reset(&mut self);
    /// One paragraph shown by Help.
    fn help(&self) -> &'static str;
    /// Default window size.
    fn size(&self) -> Vec2 {
        vec2(600.0, 400.0)
    }
    /// Draw sub-dialog `name` (a side-button label such as `"Statistics..."`).
    fn sub_ui(&mut self, _ui: &mut Ui, _name: &str) {}
    /// Continue in sub-dialog `name`: `Err` keeps it open and shows the message.
    fn sub_continue(&mut self, _name: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The state of every procedure dialog (kept for the whole session).
#[derive(Clone, Debug, Default)]
pub struct Dialogs {
    pub frequencies: frequencies::FrequenciesDialog,
    pub descriptives: descriptives::DescriptivesDialog,
    pub explore: explore::ExploreDialog,
    pub one_sample: ttest_one::OneSampleDialog,
    pub independent: ttest_ind::IndependentDialog,
    pub paired: ttest_paired::PairedDialog,
    pub oneway: oneway::OneWayDialog,
    pub regression: regression::RegressionDialog,
    pub npar_ks: npar_ks::KsDialog,
    pub runs: npar_runs::RunsDialog,
}

impl Dialogs {
    pub fn get(&self, kind: DialogKind) -> Option<&dyn ProcDialog> {
        Some(match kind {
            DialogKind::Frequencies => &self.frequencies,
            DialogKind::Descriptives => &self.descriptives,
            DialogKind::Explore => &self.explore,
            DialogKind::OneSampleT => &self.one_sample,
            DialogKind::IndependentT => &self.independent,
            DialogKind::PairedT => &self.paired,
            DialogKind::OneWayAnova => &self.oneway,
            DialogKind::LinearRegression => &self.regression,
            DialogKind::NparOneSampleKs => &self.npar_ks,
            DialogKind::NparRuns => &self.runs,
            DialogKind::Compute | DialogKind::RecodeDifferent => return None,
        })
    }
    pub fn get_mut(&mut self, kind: DialogKind) -> Option<&mut dyn ProcDialog> {
        Some(match kind {
            DialogKind::Frequencies => &mut self.frequencies,
            DialogKind::Descriptives => &mut self.descriptives,
            DialogKind::Explore => &mut self.explore,
            DialogKind::OneSampleT => &mut self.one_sample,
            DialogKind::IndependentT => &mut self.independent,
            DialogKind::PairedT => &mut self.paired,
            DialogKind::OneWayAnova => &mut self.oneway,
            DialogKind::LinearRegression => &mut self.regression,
            DialogKind::NparOneSampleKs => &mut self.npar_ks,
            DialogKind::NparRuns => &mut self.runs,
            DialogKind::Compute | DialogKind::RecodeDifferent => return None,
        })
    }
}

/// Which dialog is open, its sub-dialog, and all dialog settings.
#[derive(Clone, Debug, Default)]
pub struct DialogHost {
    pub open: Option<DialogKind>,
    /// Open sub-dialog (its button label).
    pub sub: Option<&'static str>,
    /// Settings before the sub-dialog opened (restored by its Cancel).
    backup: Option<Box<Dialogs>>,
    pub dialogs: Dialogs,
    /// Bring the dialog window to the front on the next frame.
    raise: bool,
}

impl DialogHost {
    /// Open the dialog for `kind`; false if it is not a procedure dialog.
    pub fn open(&mut self, kind: DialogKind) -> bool {
        if self.dialogs.get(kind).is_none() {
            return false;
        }
        self.open = Some(kind);
        self.sub = None;
        self.backup = None;
        self.raise = true;
        true
    }
    pub fn close(&mut self) {
        self.open = None;
        self.sub = None;
        self.backup = None;
    }
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
    fn open_sub(&mut self, name: &'static str) {
        self.backup = Some(Box::new(self.dialogs.clone()));
        self.sub = Some(name);
        self.raise = true;
    }
    fn cancel_sub(&mut self) {
        if let Some(b) = self.backup.take() {
            self.dialogs = *b;
        }
        self.sub = None;
    }
}

/// Map a stored name to the current variable, checking its type.
pub fn resolve_one(ds: &Dataset, name: &str, eligible: Eligible) -> Result<VarId, String> {
    let id = ds
        .find(name)
        .ok_or_else(|| format!("Variable {name} no longer exists."))?;
    if !eligible.accepts(ds.var(id)) {
        return Err(format!(
            "Variable {} is a string variable. Only numeric variables can be used here.",
            ds.var(id).name
        ));
    }
    Ok(id)
}

/// Map stored names to the current variables (all must exist and be eligible).
pub fn resolve(ds: &Dataset, names: &[String], eligible: Eligible) -> Result<Vec<VarId>, String> {
    names.iter().map(|n| resolve_one(ds, n, eligible)).collect()
}

/// Current spelling of stored names (a variable renamed only in case keeps working).
pub fn current_names(ds: &Dataset, names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|n| {
            ds.find(n)
                .map(|id| ds.var(id).name.clone())
                .unwrap_or_else(|| n.clone())
        })
        .collect()
}

/// Press OK in the open dialog: run it, or show why it cannot run. Returns true if a job was
/// queued (the dialog is then closed).
pub fn ok(state: &mut AppState) -> bool {
    let Some(kind) = state.dialogs.open else {
        return false;
    };
    let Some(d) = state.dialogs.dialogs.get(kind) else {
        return false;
    };
    let ds = state.doc.ds();
    match d.options(ds) {
        Ok(spec) => {
            let syntax = d.syntax(ds);
            let job = state.procedure_job(spec.title, spec.run, syntax);
            if state.queue_job(job) {
                state.dialogs.close();
                return true;
            }
            false
        }
        Err(e) => {
            state.push_message(Message::warning(kind.title(), e));
            false
        }
    }
}

/// Press Paste in the open dialog: append its syntax to the Syntax window and close it.
pub fn paste(state: &mut AppState) -> bool {
    let Some(kind) = state.dialogs.open else {
        return false;
    };
    let Some(d) = state.dialogs.dialogs.get(kind) else {
        return false;
    };
    let ds = state.doc.ds();
    match d.options(ds) {
        Ok(_) => {
            let s = d.syntax(ds);
            state.syntax.append(&s);
            state.dialogs.close();
            true
        }
        Err(e) => {
            state.push_message(Message::warning(kind.title(), e));
            false
        }
    }
}

/// Draw the open procedure dialog and its sub-dialog, and act on their buttons.
pub fn show_dialogs(ctx: &Context, state: &mut AppState, busy: bool) {
    let Some(kind) = state.dialogs.open else {
        return;
    };
    let message = state.message().is_some();
    let sub = state.dialogs.sub;
    let raise = std::mem::take(&mut state.dialogs.raise);
    let screen = ctx.content_rect();
    let mut action = DialogAction::None;
    let ds = state.doc.ds();
    let Some(d) = state.dialogs.dialogs.get_mut(kind) else {
        state.dialogs.close();
        return;
    };
    let title = d.title().to_string();
    let size = d.size().min(screen.size() - vec2(16.0, 40.0));
    let win = egui::Window::new(title.as_str())
        .id(egui::Id::new(("statik-dialog", kind)))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(true)
        .default_size(size)
        .min_size(vec2(360.0, 220.0).min(size))
        .max_size(screen.size() - vec2(8.0, 8.0))
        .default_pos(screen.center())
        .pivot(egui::Align2::CENTER_CENTER)
        .constrain(true)
        .show(ctx, |ui| {
            ui.add_enabled_ui(sub.is_none() && !busy, |ui| {
                let body_h = (ui.available_height() - 34.0).max(60.0);
                egui::ScrollArea::both()
                    .id_salt("body")
                    .auto_shrink([false, false])
                    .max_height(body_h)
                    .show(ui, |ui| {
                        action = d.ui(ui, ds);
                    });
                ui.separator();
                action = action.or(frame::bottom_buttons(ui, d.ready()));
            });
        });
    if raise
        && sub.is_none()
        && let Some(r) = &win
    {
        ctx.move_to_top(r.response.layer_id);
    }

    let mut sub_action = SubAction::None;
    if let Some(name) = sub {
        let sub_title = format!("{title}: {}", name.trim_end_matches("..."));
        let r = egui::Window::new(sub_title)
            .id(egui::Id::new(("statik-subdialog", kind, name)))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .default_pos(screen.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .constrain(true)
            .show(ctx, |ui| {
                ui.set_min_width(300.0);
                ui.add_enabled_ui(!busy, |ui| {
                    d.sub_ui(ui, name);
                    ui.separator();
                    sub_action = frame::sub_buttons(ui);
                });
            });
        if raise && let Some(r) = &r {
            ctx.move_to_top(r.response.layer_id);
        }
        if !message && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            sub_action = SubAction::Cancel;
        }
    } else if !message && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = DialogAction::Cancel;
    }

    let help = d.help();
    match sub_action {
        SubAction::None => {}
        SubAction::Continue => {
            let name = sub.unwrap_or_default();
            match d.sub_continue(name) {
                Ok(()) => {
                    state.dialogs.sub = None;
                    state.dialogs.backup = None;
                }
                Err(e) => state.push_message(Message::warning(title.clone(), e)),
            }
        }
        SubAction::Cancel => state.dialogs.cancel_sub(),
        SubAction::Help => state.push_message(Message::info(title.clone(), help)),
    }

    match action {
        DialogAction::None => {}
        DialogAction::Ok => {
            ok(state);
        }
        DialogAction::Paste => {
            paste(state);
        }
        DialogAction::Reset => {
            if let Some(d) = state.dialogs.dialogs.get_mut(kind) {
                d.reset();
            }
        }
        DialogAction::Cancel => state.dialogs.close(),
        DialogAction::Help => state.push_message(Message::info(title, help)),
        DialogAction::Sub(name) => state.dialogs.open_sub(name),
    }
}
