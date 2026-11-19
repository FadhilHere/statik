//! Shared parts of the procedure dialogs: the bottom button row, the column of sub-dialog
//! buttons, sub-dialog buttons, group boxes, labelled text fields, disabled options.
use super::{DialogAction, SubAction};
use egui::{Button, Checkbox, RadioButton, Response, Ui, vec2};

const BUTTON_W: f32 = 74.0;

/// OK / Paste / Reset / Cancel / Help, centred. OK and Paste are enabled when `ready`.
pub fn bottom_buttons(ui: &mut Ui, ready: bool) -> DialogAction {
    let mut a = DialogAction::None;
    let total = 5.0 * BUTTON_W + 4.0 * ui.spacing().item_spacing.x;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
        for (label, act, on) in [
            ("OK", DialogAction::Ok, ready),
            ("Paste", DialogAction::Paste, ready),
            ("Reset", DialogAction::Reset, true),
            ("Cancel", DialogAction::Cancel, true),
            ("Help", DialogAction::Help, true),
        ] {
            if ui
                .add_enabled(on, Button::new(label).min_size(vec2(BUTTON_W, 0.0)))
                .clicked()
            {
                a = act;
            }
        }
    });
    a
}

/// Continue / Cancel / Help of a sub-dialog, centred.
pub fn sub_buttons(ui: &mut Ui) -> SubAction {
    let mut a = SubAction::None;
    let total = 3.0 * BUTTON_W + 2.0 * ui.spacing().item_spacing.x;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
        for (label, act) in [
            ("Continue", SubAction::Continue),
            ("Cancel", SubAction::Cancel),
            ("Help", SubAction::Help),
        ] {
            if ui
                .add(Button::new(label).min_size(vec2(BUTTON_W, 0.0)))
                .clicked()
            {
                a = act;
            }
        }
    });
    a
}

/// The column of sub-dialog buttons at the right of a dialog (`(label, enabled)`, e.g.
/// `("Statistics...", true)`). A click returns `DialogAction::Sub(label)`.
pub fn side_buttons(ui: &mut Ui, items: &[(&'static str, bool)]) -> DialogAction {
    let mut a = DialogAction::None;
    ui.vertical(|ui| {
        ui.add_space(16.0);
        for &(label, on) in items {
            if ui
                .add_enabled(on, Button::new(label).min_size(vec2(96.0, 0.0)))
                .clicked()
            {
                a = DialogAction::Sub(label);
            }
        }
    });
    a
}

/// A titled group box (SPSS frame with a caption).
pub fn group<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title).strong());
                add(ui)
            })
            .inner
        })
        .inner
}

/// "Label: [text]" on one line; the field is named by the label for accessibility.
pub fn labeled_text(ui: &mut Ui, label: &str, text: &mut String, width: f32) -> Response {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        ui.add(egui::TextEdit::singleline(text).desired_width(width))
            .labelled_by(l.id)
    })
    .inner
}

/// Options Statik does not support yet: visible, greyed out.
pub fn checks_off(ui: &mut Ui, items: &[(&str, bool)]) {
    for &(label, checked) in items {
        let mut c = checked;
        ui.add_enabled(false, Checkbox::new(&mut c, label));
    }
}

pub fn radios_off(ui: &mut Ui, items: &[(&str, bool)]) {
    for &(label, on) in items {
        ui.add_enabled(false, RadioButton::new(on, label));
    }
}

/// "Missing Values" group with only SPSS's default choice available.
pub fn missing_group(ui: &mut Ui, default: &str, other: &str) {
    group(ui, "Missing Values", |ui| {
        radios_off(ui, &[(default, true), (other, false)])
    });
}

/// The Options sub-dialog of the T tests: confidence percentage and missing values.
pub fn ci_options(ui: &mut Ui, conf: &mut String) {
    ui.horizontal(|ui| {
        let l = ui.label("Confidence Interval Percentage:");
        ui.add(egui::TextEdit::singleline(conf).desired_width(48.0))
            .labelled_by(l.id);
        ui.label("%");
    });
    missing_group(
        ui,
        "Exclude cases analysis by analysis",
        "Exclude cases listwise",
    );
}

/// Parse a confidence percentage (strictly between 0 and 100).
pub fn parse_ci(s: &str) -> Result<f64, String> {
    match crate::edit::parse_number(s.trim()) {
        Some(c) if (1.0..=99.99).contains(&c) => Ok(c),
        _ => Err("The confidence interval percentage must be a number between 1 and 99.99.".into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ci_range_matches_the_message() {
        for ok in ["1", "95", "99.99", "99,5"] {
            assert!(super::parse_ci(ok).is_ok(), "{ok}");
        }
        for bad in ["0.5", "0", "99.995", "100", "x", ""] {
            assert!(super::parse_ci(bad).is_err(), "{bad}");
        }
    }
}
