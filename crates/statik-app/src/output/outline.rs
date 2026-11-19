//! The outline pane: "Output" ▸ one group per procedure (Log, Title, tables, charts, ...).
//! Clicking a node selects that item and scrolls the document to it.
use super::{ItemRef, item_name};
use crate::state::AppState;
use egui::{CollapsingHeader, Ui};

pub fn outline_ui(ui: &mut Ui, state: &mut AppState) {
    let mut clicked = None;
    let selected = state.viewer.selected;
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            CollapsingHeader::new("Output")
                .id_salt("outline_root")
                .default_open(true)
                .show(ui, |ui| {
                    for (e, entry) in state.outputs.iter().enumerate() {
                        CollapsingHeader::new(&entry.title)
                            .id_salt(("outline", e))
                            .default_open(true)
                            .show(ui, |ui| {
                                if !entry.log.is_empty() {
                                    let r = ItemRef::entry(e);
                                    if ui.selectable_label(selected == Some(r), "Log").clicked() {
                                        clicked = Some(r);
                                    }
                                }
                                for (i, it) in entry.items.iter().enumerate() {
                                    let r = ItemRef::item(e, i);
                                    if ui
                                        .selectable_label(selected == Some(r), item_name(it))
                                        .clicked()
                                    {
                                        clicked = Some(r);
                                    }
                                }
                            });
                    }
                });
        });
    if let Some(r) = clicked {
        state.viewer.selected = Some(r);
        state.viewer.scroll_to = Some(r);
    }
}
