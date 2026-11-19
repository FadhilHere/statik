//! Modal message box (errors, warnings such as `.sav` read notes, About).
use crate::state::{Message, MessageKind};

/// Show `msg` as a modal box with an OK button. Returns true once it is dismissed (OK, Enter,
/// Escape or a click outside); the caller then clears it.
pub fn message_box(ctx: &egui::Context, msg: &Message) -> bool {
    let mut ok = false;
    let resp = egui::Modal::new(egui::Id::new("statik-message")).show(ctx, |ui| {
        ui.set_min_width(320.0);
        ui.set_max_width(520.0);
        ui.horizontal(|ui| {
            let (icon, color) = match msg.kind {
                MessageKind::Info => ("i", egui::Color32::from_rgb(0x2b, 0x5c, 0xa8)),
                MessageKind::Warning => ("!", egui::Color32::from_rgb(0xd8, 0x9b, 0x00)),
                MessageKind::Error => ("x", egui::Color32::from_rgb(0xc0, 0x30, 0x30)),
            };
            ui.label(egui::RichText::new(icon).strong().size(22.0).color(color));
            ui.heading(&msg.title);
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                ui.label(&msg.text);
            });
        ui.separator();
        ui.vertical_centered(|ui| {
            if ui
                .add(egui::Button::new("OK").min_size(egui::vec2(80.0, 0.0)))
                .clicked()
            {
                ok = true;
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            ok = true;
        }
    });
    ok || resp.should_close()
}

/// Answer to a three-way question box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
    Cancel,
}

/// The modal "Save contents of data editor to <name>?" box with Yes / No / Cancel. Escape (or
/// a click outside) is Cancel. `None` while unanswered.
pub fn save_changes_box(ctx: &egui::Context, name: &str) -> Option<Answer> {
    let mut answer = None;
    let resp = egui::Modal::new(egui::Id::new("statik-save-changes")).show(ctx, |ui| {
        ui.set_min_width(320.0);
        ui.set_max_width(520.0);
        ui.heading("Statik");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("!")
                    .strong()
                    .size(22.0)
                    .color(egui::Color32::from_rgb(0xd8, 0x9b, 0x00)),
            );
            ui.label(format!("Save contents of data editor to {name}?"));
        });
        ui.separator();
        ui.horizontal(|ui| {
            let bw = 80.0;
            let w = (ui.available_width() - 3.0 * bw - 2.0 * ui.spacing().item_spacing.x) / 2.0;
            ui.add_space(w.max(0.0));
            for (label, a) in [("Yes", Answer::Yes), ("No", Answer::No), ("Cancel", Answer::Cancel)]
            {
                if ui
                    .add(egui::Button::new(label).min_size(egui::vec2(bw, 0.0)))
                    .clicked()
                {
                    answer = Some(a);
                }
            }
        });
    });
    if answer.is_none() && resp.should_close() {
        answer = Some(Answer::Cancel);
    }
    answer
}

/// The modal "Statik closed unexpectedly. Recover data?" box with Recover / Discard.
/// `Some(true)` = Recover. Escape does not dismiss it (the question must be answered).
pub fn recovery_box(ctx: &egui::Context) -> Option<bool> {
    let mut answer = None;
    egui::Modal::new(egui::Id::new("statik-recovery")).show(ctx, |ui| {
        ui.set_min_width(320.0);
        ui.heading("Statik");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("!")
                    .strong()
                    .size(22.0)
                    .color(egui::Color32::from_rgb(0xd8, 0x9b, 0x00)),
            );
            ui.label("Statik closed unexpectedly. Recover data?");
        });
        ui.separator();
        ui.horizontal(|ui| {
            let bw = 80.0;
            let w = (ui.available_width() - 2.0 * bw - ui.spacing().item_spacing.x) / 2.0;
            ui.add_space(w.max(0.0));
            if ui
                .add(egui::Button::new("Recover").min_size(egui::vec2(bw, 0.0)))
                .clicked()
            {
                answer = Some(true);
            }
            if ui
                .add(egui::Button::new("Discard").min_size(egui::vec2(bw, 0.0)))
                .clicked()
            {
                answer = Some(false);
            }
        });
    });
    answer
}
