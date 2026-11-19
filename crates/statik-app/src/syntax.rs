//! Command syntax: the text a dialog's Paste button writes to the Syntax window, and the log
//! shown above a procedure's output (as SPSS does).
//!
//! [`Syntax`] builds one command (`REGRESSION` + `/SUBCOMMAND` lines + the final period).
//! [`SyntaxWindow`] is the read-only "Syntax" window in the Data Editor; dialogs append to it
//! with [`SyntaxWindow::append`] and it is drawn by [`syntax_window`].
use crate::state::AppState;

/// Title of the Syntax window.
pub const SYNTAX_TITLE: &str = "Syntax1 - Statik Syntax Editor";

/// One command: a head line, indented `/SUBCOMMAND` lines, and a closing period.
#[derive(Clone, Debug)]
pub struct Syntax {
    lines: Vec<String>,
}

impl Syntax {
    /// `head` is the command and its main specification, e.g. `"T-TEST GROUPS=grp(1 2)"`.
    pub fn new(head: impl Into<String>) -> Self {
        Syntax {
            lines: vec![head.into()],
        }
    }
    /// Add `  /text`.
    pub fn subcommand(mut self, text: impl AsRef<str>) -> Self {
        self.lines.push(format!("  /{}", text.as_ref()));
        self
    }
    /// Add `  /text` when `on`.
    pub fn subcommand_if(self, on: bool, text: impl AsRef<str>) -> Self {
        if on { self.subcommand(text) } else { self }
    }
    /// The command text, ending in a period.
    pub fn finish(self) -> String {
        let mut s = self.lines.join("\n");
        s.push('.');
        s
    }
}

/// Variable names separated by spaces; long lists continue on indented lines (about 72
/// characters per line), as pasted syntax does.
pub fn var_list(names: &[String]) -> String {
    let mut out = String::new();
    let mut line = 0usize;
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            if line + 1 + n.len() > 72 {
                out.push_str("\n    ");
                line = 4;
            } else {
                out.push(' ');
                line += 1;
            }
        }
        out.push_str(n);
        line += n.len();
    }
    out
}

/// A confidence percentage as a syntax fraction with at least `min_dec` decimals and no leading
/// zero: `fraction(95.0, 2)` = `".95"`, `fraction(95.0, 4)` = `".9500"`, `fraction(99.5, 2)` =
/// `".995"`.
pub fn fraction(percent: f64, min_dec: usize) -> String {
    let f = percent / 100.0;
    let mut s = format!("{f:.10}");
    while s.ends_with('0') && s.len() - s.find('.').unwrap_or(0) - 1 > min_dec {
        s.pop();
    }
    s.trim_start_matches('0').to_string()
}

/// A value as written in syntax (`1`, `2.5`, `-3`).
pub fn value(v: f64) -> String {
    format!("{v}")
}

/// The read-only Syntax window (text pasted by dialogs).
#[derive(Clone, Debug, Default)]
pub struct SyntaxWindow {
    pub text: String,
    pub show: bool,
}

impl SyntaxWindow {
    /// Append one command (a blank line separates commands) and show the window.
    pub fn append(&mut self, cmd: &str) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(cmd);
        self.text.push('\n');
        self.show = true;
    }
}

/// Draw the Syntax window when it is shown: the pasted commands (read-only, selectable) and
/// a Copy button that puts all of them on the clipboard.
pub fn syntax_window(ctx: &egui::Context, state: &mut AppState) {
    if !state.syntax.show {
        return;
    }
    let mut open = true;
    egui::Window::new(SYNTAX_TITLE)
        .id(egui::Id::new("statik-syntax"))
        .open(&mut open)
        .default_size([560.0, 360.0])
        .min_size([300.0, 160.0])
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(state.syntax.text.clone());
                }
                if ui.button("Clear").clicked() {
                    state.syntax.text.clear();
                }
            });
            ui.separator();
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut view = state.syntax.text.as_str();
                    ui.add(
                        egui::TextEdit::multiline(&mut view)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
        });
    if !open {
        state.syntax.show = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_commands() {
        let s = Syntax::new("ONEWAY y BY g")
            .subcommand_if(false, "X")
            .subcommand("MISSING ANALYSIS")
            .finish();
        assert_eq!(s, "ONEWAY y BY g\n  /MISSING ANALYSIS.");
        assert_eq!(fraction(95.0, 2), ".95");
        assert_eq!(fraction(90.0, 2), ".90");
        assert_eq!(fraction(99.5, 2), ".995");
        assert_eq!(fraction(95.0, 4), ".9500");
        assert_eq!(value(1.0), "1");
        assert_eq!(value(2.5), "2.5");
        let many: Vec<String> = (0..30).map(|i| format!("var{i:05}")).collect();
        let l = var_list(&many);
        assert!(l.lines().all(|x| x.len() <= 76), "{l}");
        assert_eq!(l.split_whitespace().count(), 30);
    }
}
