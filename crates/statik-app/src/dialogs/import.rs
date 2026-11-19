//! File ▸ Import Data: the Read CSV File and Read Excel File dialogs.
//!
//! The menu first picks the file (rfd), then [`AppState::open_import_csv`] /
//! [`AppState::open_import_excel`] open the dialog; [`show_import`] draws it.
//! - **Read CSV File:** a preview of the first 10 rows (variable names as they will be
//!   created, cells as in the file), "First line contains variable names", Delimiter between
//!   values (Auto / Comma / Semicolon / Tab) and Decimal Symbol (Auto / Period / Comma). The
//!   preview is read again whenever an option changes ([`sav_io::sniff_csv`]) and shows what
//!   Auto detected.
//! - **Read Excel File:** the worksheet and "Read variable names from the first row of data".
//!
//! OK reads the file on the worker thread (the document is replaced, the undo history
//! dropped); Paste writes the GET DATA syntax to the Syntax window. The data keeps the CSV/XLSX
//! path, so Save works as Save As.
use super::DialogAction;
use super::frame::bottom_buttons;
use crate::state::{AppState, Message, csv_job, quoted_path, xlsx_job, xlsx_syntax};
use egui::{Context, Ui, vec2};
use sav_io::{CsvOptions, CsvPreview};
use std::path::PathBuf;

/// Rows shown in the CSV preview.
pub const PREVIEW_ROWS: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Delimiter {
    #[default]
    Auto,
    Comma,
    Semicolon,
    Tab,
}

impl Delimiter {
    const ALL: [Delimiter; 4] = [
        Delimiter::Auto,
        Delimiter::Comma,
        Delimiter::Semicolon,
        Delimiter::Tab,
    ];
    fn label(self) -> &'static str {
        match self {
            Delimiter::Auto => "Auto",
            Delimiter::Comma => "Comma",
            Delimiter::Semicolon => "Semicolon",
            Delimiter::Tab => "Tab",
        }
    }
    fn byte(self) -> Option<u8> {
        match self {
            Delimiter::Auto => None,
            Delimiter::Comma => Some(b','),
            Delimiter::Semicolon => Some(b';'),
            Delimiter::Tab => Some(b'\t'),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Decimal {
    #[default]
    Auto,
    Period,
    Comma,
}

impl Decimal {
    const ALL: [Decimal; 3] = [Decimal::Auto, Decimal::Period, Decimal::Comma];
    fn label(self) -> &'static str {
        match self {
            Decimal::Auto => "Auto",
            Decimal::Period => "Period (.)",
            Decimal::Comma => "Comma (,)",
        }
    }
    fn comma(self) -> Option<bool> {
        match self {
            Decimal::Auto => None,
            Decimal::Period => Some(false),
            Decimal::Comma => Some(true),
        }
    }
}

/// The Read CSV File dialog.
#[derive(Clone, Debug)]
pub struct CsvDialog {
    pub path: PathBuf,
    pub header: bool,
    pub delimiter: Delimiter,
    pub decimal: Decimal,
    /// The preview for the current options (`Err` = why the file cannot be read like this).
    pub preview: Option<Result<CsvPreview, String>>,
    preview_for: Option<(bool, Delimiter, Decimal)>,
}

impl CsvDialog {
    pub fn new(path: PathBuf) -> Self {
        let mut d = CsvDialog {
            path,
            header: true,
            delimiter: Delimiter::Auto,
            decimal: Decimal::Auto,
            preview: None,
            preview_for: None,
        };
        d.refresh();
        d
    }

    pub fn options(&self) -> CsvOptions {
        CsvOptions {
            delimiter: self.delimiter.byte(),
            decimal_comma: self.decimal.comma(),
            header: self.header,
        }
    }

    /// Read the preview again if the options changed since it was read.
    pub fn refresh(&mut self) {
        let key = (self.header, self.delimiter, self.decimal);
        if self.preview_for == Some(key) {
            return;
        }
        self.preview_for = Some(key);
        self.preview = Some(
            sav_io::sniff_csv(&self.path, &self.options(), PREVIEW_ROWS).map_err(|e| e.to_string()),
        );
    }

    /// GET DATA syntax (SPSS style) for the current options, with the delimiter and decimal
    /// symbol Auto resolved as in the preview.
    pub fn syntax(&self) -> String {
        let p = self.preview.as_ref().and_then(|p| p.as_ref().ok());
        let delim = self
            .delimiter
            .byte()
            .or(p.map(|p| p.delimiter))
            .unwrap_or(b',');
        let comma = self
            .decimal
            .comma()
            .or(p.map(|p| p.decimal_comma))
            .unwrap_or(false);
        let delim = match delim {
            b'\t' => "\\t".to_string(),
            0 => String::new(),
            d => (d as char).to_string(),
        };
        let mut lines = Vec::new();
        if comma {
            lines.push("PRESERVE.".to_string());
            lines.push("SET DECIMAL COMMA.".to_string());
        }
        lines.push("GET DATA".into());
        lines.push("  /TYPE=TXT".into());
        lines.push(format!("  /FILE={}", quoted_path(&self.path)));
        lines.push(format!("  /DELIMITERS=\"{delim}\""));
        lines.push("  /QUALIFIER='\"'".into());
        lines.push("  /ARRANGEMENT=DELIMITED".into());
        lines.push(format!("  /FIRSTCASE={}", if self.header { 2 } else { 1 }));
        match p {
            Some(p) if !p.header.is_empty() => {
                lines.push("  /VARIABLES=".into());
                for n in &p.header {
                    lines.push(format!("  {n} AUTO"));
                }
            }
            _ => {}
        }
        if let Some(last) = lines.last_mut() {
            last.push('.');
        }
        if comma {
            lines.push("RESTORE.".into());
        }
        lines.push("CACHE.".into());
        lines.push("EXECUTE.".into());
        lines.join("\n")
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.label(format!("File: {}", self.path.display()));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Data preview").strong());
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(ui.visuals().widgets.inactive.bg_stroke)
            .inner_margin(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("csv_preview")
                    .auto_shrink([false, true])
                    .max_height(230.0)
                    .show(ui, |ui| match &self.preview {
                        Some(Ok(p)) => {
                            egui::Grid::new("csv_preview_grid")
                                .striped(true)
                                .min_col_width(60.0)
                                .show(ui, |ui| {
                                    for n in &p.header {
                                        ui.label(egui::RichText::new(n).strong());
                                    }
                                    ui.end_row();
                                    for r in &p.rows {
                                        for c in r {
                                            ui.label(c);
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                        Some(Err(e)) => {
                            ui.colored_label(ui.visuals().error_fg_color, e);
                        }
                        None => {}
                    });
            });
        if let Some(Ok(p)) = &self.preview {
            let d = match p.delimiter {
                b',' => "comma",
                b';' => "semicolon",
                b'\t' => "tab",
                0 => "none (one column)",
                _ => "other",
            };
            let dec = if p.decimal_comma { "comma" } else { "period" };
            ui.label(
                egui::RichText::new(format!("Delimiter: {d}; decimal symbol: {dec}.")).small(),
            );
        }
        ui.add_space(6.0);
        ui.checkbox(&mut self.header, "First line contains variable names");
        egui::Grid::new("csv_options").show(ui, |ui| {
            ui.label("Delimiter between values:");
            egui::ComboBox::from_id_salt("csv_delimiter")
                .selected_text(self.delimiter.label())
                .show_ui(ui, |ui| {
                    for d in Delimiter::ALL {
                        ui.selectable_value(&mut self.delimiter, d, d.label());
                    }
                });
            ui.end_row();
            ui.label("Decimal Symbol:");
            egui::ComboBox::from_id_salt("csv_decimal")
                .selected_text(self.decimal.label())
                .show_ui(ui, |ui| {
                    for d in Decimal::ALL {
                        ui.selectable_value(&mut self.decimal, d, d.label());
                    }
                });
            ui.end_row();
            ui.label("Text Qualifier:");
            ui.add_enabled_ui(false, |ui| {
                egui::ComboBox::from_id_salt("csv_qualifier")
                    .selected_text("Double Quote")
                    .show_ui(ui, |_| {});
            });
            ui.end_row();
        });
        self.refresh();
    }
}

/// The Read Excel File dialog.
#[derive(Clone, Debug)]
pub struct ExcelDialog {
    pub path: PathBuf,
    pub sheets: Vec<String>,
    /// Index into `sheets`.
    pub sheet: usize,
    /// "Read variable names from the first row of data".
    pub header: bool,
}

impl ExcelDialog {
    fn sheet_name(&self) -> Option<String> {
        self.sheets.get(self.sheet).cloned()
    }

    pub fn syntax(&self) -> String {
        xlsx_syntax(&self.path, self.sheet_name().as_deref(), self.header)
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.label(format!("File: {}", self.path.display()));
        ui.add_space(6.0);
        egui::Grid::new("xlsx_options").show(ui, |ui| {
            ui.label("Worksheet:");
            let current = self.sheet_name().unwrap_or_default();
            egui::ComboBox::from_id_salt("xlsx_sheet")
                .selected_text(current)
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (i, s) in self.sheets.iter().enumerate() {
                        ui.selectable_value(&mut self.sheet, i, s);
                    }
                });
            ui.end_row();
            ui.label("Range:");
            ui.add_enabled_ui(false, |ui| {
                let mut r = String::new();
                ui.add(egui::TextEdit::singleline(&mut r).desired_width(120.0));
            });
            ui.end_row();
        });
        ui.checkbox(
            &mut self.header,
            "Read variable names from the first row of data",
        );
    }
}

/// The open import dialog.
#[derive(Clone, Debug)]
pub enum ImportDialog {
    Csv(CsvDialog),
    Excel(ExcelDialog),
}

impl ImportDialog {
    pub fn title(&self) -> &'static str {
        match self {
            ImportDialog::Csv(_) => "Read CSV File",
            ImportDialog::Excel(_) => "Read Excel File",
        }
    }
    fn help(&self) -> &'static str {
        match self {
            ImportDialog::Csv(_) => {
                "Read CSV File reads a text file with one case per line. Auto detects the \
                 delimiter (comma, semicolon or tab) and the decimal symbol: files saved by an \
                 Indonesian Excel usually use semicolons and a decimal comma. Change an option \
                 if the preview does not look right."
            }
            ImportDialog::Excel(_) => {
                "Read Excel File reads one worksheet. Each column becomes a variable; with \
                 \"Read variable names from the first row of data\" the first row holds the \
                 names."
            }
        }
    }
    fn syntax(&self) -> String {
        match self {
            ImportDialog::Csv(d) => d.syntax(),
            ImportDialog::Excel(d) => d.syntax(),
        }
    }
    fn ready(&self) -> bool {
        match self {
            ImportDialog::Csv(d) => matches!(d.preview, Some(Ok(_))),
            ImportDialog::Excel(d) => !d.sheets.is_empty(),
        }
    }
}

impl AppState {
    /// File ▸ Import Data ▸ CSV Data... after the file was picked: open Read CSV File.
    pub fn open_import_csv(&mut self, path: PathBuf) {
        self.commit_pending_edits();
        self.import = Some(ImportDialog::Csv(CsvDialog::new(path)));
    }

    /// File ▸ Import Data ▸ Excel... after the file was picked: open Read Excel File (an
    /// unreadable workbook gives an error message instead).
    pub fn open_import_excel(&mut self, path: PathBuf) {
        self.commit_pending_edits();
        match sav_io::xlsx_sheet_names(&path) {
            Ok(sheets) if !sheets.is_empty() => {
                self.import = Some(ImportDialog::Excel(ExcelDialog {
                    path,
                    sheets,
                    sheet: 0,
                    header: true,
                }));
            }
            Ok(_) => self.push_message(Message::error(format!(
                "{} has no worksheets.",
                path.display()
            ))),
            Err(e) => self.push_message(Message::error(format!(
                "Cannot read {}:\n{e}",
                path.display()
            ))),
        }
    }

    /// OK in the import dialog: queue the read and close the dialog.
    pub fn import_ok(&mut self) -> bool {
        let Some(d) = &self.import else {
            return false;
        };
        if !d.ready() {
            return false;
        }
        let rev = self.doc.rev();
        let job = match d {
            ImportDialog::Csv(c) => csv_job(c.path.clone(), c.options(), c.syntax(), rev),
            ImportDialog::Excel(x) => xlsx_job(x.path.clone(), x.sheet_name(), x.header, rev),
        };
        if self.queue_job(job) {
            self.import = None;
            return true;
        }
        false
    }
}

/// Draw the open import dialog and act on its buttons.
pub fn show_import(ctx: &Context, state: &mut AppState, busy: bool) {
    let Some(d) = &mut state.import else {
        return;
    };
    let message = state.messages.front().is_some();
    let title = d.title();
    let screen = ctx.content_rect();
    let size = vec2(620.0, 500.0).min(screen.size() - vec2(16.0, 40.0));
    let mut action = DialogAction::None;
    egui::Window::new(title)
        .id(egui::Id::new("statik-import"))
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
            ui.add_enabled_ui(!busy, |ui| {
                let body_h = (ui.available_height() - 34.0).max(60.0);
                egui::ScrollArea::both()
                    .id_salt("import_body")
                    .auto_shrink([false, false])
                    .max_height(body_h)
                    .show(ui, |ui| match d {
                        ImportDialog::Csv(c) => c.ui(ui),
                        ImportDialog::Excel(x) => x.ui(ui),
                    });
                ui.separator();
                action = bottom_buttons(ui, d.ready());
            });
        });
    if !message && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = DialogAction::Cancel;
    }
    match action {
        DialogAction::Ok => {
            state.import_ok();
        }
        DialogAction::Paste => {
            if let Some(d) = state.import.take() {
                state.syntax.append(&d.syntax());
            }
        }
        DialogAction::Reset => match &mut state.import {
            Some(ImportDialog::Csv(c)) => {
                c.header = true;
                c.delimiter = Delimiter::Auto;
                c.decimal = Decimal::Auto;
                c.refresh();
            }
            Some(ImportDialog::Excel(x)) => {
                x.sheet = 0;
                x.header = true;
            }
            None => {}
        },
        DialogAction::Cancel => state.import = None,
        DialogAction::Help => {
            let (t, h) = (title, state.import.as_ref().map(|d| d.help()).unwrap_or(""));
            state.push_message(Message::info(t, h));
        }
        DialogAction::None | DialogAction::Sub(_) => {}
    }
}
