//! Analyze ▸ Compare Means ▸ Paired-Samples T Test... with the Options... sub-dialog.
//!
//! The source list keeps every variable (one can be used in several pairs). Selecting one
//! variable and pressing the arrow fills the next free slot of the grid; selecting two makes a
//! pair at once. Selecting a row of the grid flips the arrow to remove that pair.
use super::frame::{ci_options, parse_ci, side_buttons};
use super::var_picker::{
    Eligible, ROW_H, Side, VarPicker, arrow_button, display_name, list_frame, paint_glyph,
};
use super::{DialogAction, ProcDialog, RunSpec, current_names, resolve_one};
use crate::syntax::{Syntax, fraction, var_list};
use dataset::Dataset;
use egui::{Rect, Sense, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2};
use procedures::ttest_paired::{PairedOptions, run_paired};

/// One row of the Paired Variables grid.
pub type Pair = (Option<String>, Option<String>);

#[derive(Clone, Debug)]
pub struct PairedDialog {
    pub picker: VarPicker,
    pub pairs: Vec<Pair>,
    /// Selected row of the grid.
    pub selected_pair: Option<usize>,
    pub conf: String,
}

impl Default for PairedDialog {
    fn default() -> Self {
        PairedDialog {
            picker: VarPicker::new(vec![]).keep_source(),
            pairs: Vec::new(),
            selected_pair: None,
            conf: "95".into(),
        }
    }
}

const GRID: &str = "Paired Variables";

impl PairedDialog {
    fn can_add(&self, ds: &Dataset) -> bool {
        let sel = self.picker.source_selection(ds);
        !sel.is_empty()
            && sel.len() <= 2
            && sel.iter().all(|n| {
                ds.find(n)
                    .is_some_and(|id| Eligible::Numeric.accepts(ds.var(id)))
            })
    }

    /// Add the source selection to the grid.
    pub fn add(&mut self, ds: &Dataset) {
        if !self.can_add(ds) {
            return;
        }
        let sel = self.picker.source_selection(ds);
        if sel.len() == 2 {
            self.pairs
                .push((Some(sel[0].clone()), Some(sel[1].clone())));
        } else if let Some(last) = self.pairs.last_mut().filter(|p| p.1.is_none()) {
            last.1 = Some(sel[0].clone());
        } else {
            self.pairs.push((Some(sel[0].clone()), None));
        }
    }

    fn complete(&self) -> Vec<(String, String)> {
        self.pairs
            .iter()
            .filter_map(|p| Some((p.0.clone()?, p.1.clone()?)))
            .collect()
    }

    fn grid_ui(&mut self, ui: &mut Ui, ds: &Dataset) {
        let w = ui.available_width();
        let cols = [0.0, 52.0, 52.0 + (w - 52.0) / 2.0, w];
        let header = |ui: &mut Ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
            for (i, t) in ["Pair", "Variable1", "Variable2"].iter().enumerate() {
                ui.painter().text(
                    pos2(rect.left() + cols[i] + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    *t,
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().strong_text_color(),
                );
            }
        };
        header(ui);
        let mut clicked = None;
        list_frame(ui, "pairs", 8.0 * ROW_H, |ui| {
            for row in 0..=self.pairs.len() {
                let (rect, resp) = ui.allocate_exact_size(vec2(w - 6.0, ROW_H), Sense::click());
                let sel = self.selected_pair == Some(row) && self.picker.side != Side::Source;
                let vis = ui.visuals().clone();
                let p = ui.painter();
                if sel {
                    p.rect_filled(rect, 0.0, vis.selection.bg_fill);
                }
                let color = if sel {
                    vis.selection.stroke.color
                } else {
                    vis.text_color()
                };
                let font = egui::TextStyle::Body.resolve(ui.style());
                p.text(
                    pos2(rect.left() + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    (row + 1).to_string(),
                    font.clone(),
                    color,
                );
                let pair = self.pairs.get(row).cloned().unwrap_or((None, None));
                let mut names = vec![];
                for (c, v) in [pair.0, pair.1].into_iter().enumerate() {
                    let Some(n) = v else { continue };
                    let var = ds.find(&n).map(|id| ds.var(id));
                    let x = rect.left() + cols[c + 1];
                    paint_glyph(
                        p,
                        Rect::from_min_size(pos2(x + 2.0, rect.center().y - 7.0), vec2(14.0, 14.0)),
                        var,
                    );
                    let text = var.map(display_name).unwrap_or(n);
                    p.with_clip_rect(Rect::from_x_y_ranges(
                        x..=rect.left() + cols[c + 2] - 2.0,
                        rect.y_range(),
                    ))
                    .text(
                        pos2(x + 19.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &text,
                        font.clone(),
                        color,
                    );
                    names.push(text);
                }
                let label = if names.is_empty() {
                    format!("Pair {}", row + 1)
                } else {
                    format!("Pair {}: {}", row + 1, names.join(", "))
                };
                resp.widget_info(|| {
                    WidgetInfo::selected(WidgetType::SelectableLabel, true, sel, &label)
                });
                if resp.clicked() && row < self.pairs.len() {
                    clicked = Some(row);
                }
            }
        });
        if let Some(r) = clicked {
            self.selected_pair = Some(r);
            self.picker.selected.clear();
            self.picker.side = Side::Target(0);
        }
    }
}

impl ProcDialog for PairedDialog {
    fn title(&self) -> &str {
        "Paired-Samples T Test"
    }

    fn size(&self) -> Vec2 {
        vec2(640.0, 380.0)
    }

    fn ui(&mut self, ui: &mut Ui, ds: &Dataset) -> DialogAction {
        let mut a = DialogAction::None;
        if self.picker.side == Side::Source {
            self.selected_pair = None;
        }
        ui.horizontal_top(|ui| {
            self.picker.source_ui(ui, ds, vec2(200.0, 260.0));
            ui.vertical(|ui| {
                ui.add_space(40.0);
                if let Some(r) = self.selected_pair {
                    if arrow_button(ui, false, true, &format!("Remove from {GRID}")) {
                        if r < self.pairs.len() {
                            self.pairs.remove(r);
                        }
                        self.selected_pair = None;
                        self.picker.side = Side::Source;
                    }
                } else if arrow_button(ui, true, self.can_add(ds), &format!("Move to {GRID}")) {
                    self.add(ds);
                }
            });
            ui.vertical(|ui| {
                ui.set_width(270.0);
                ui.label(format!("{GRID}:"));
                self.grid_ui(ui, ds);
            });
            a = side_buttons(ui, &[("Options...", true), ("Bootstrap...", false)]);
        });
        a
    }

    fn ready(&self) -> bool {
        !self.pairs.is_empty() && self.pairs.iter().all(|p| p.0.is_some() && p.1.is_some())
    }

    fn options(&self, ds: &Dataset) -> Result<RunSpec, String> {
        let pairs = self
            .complete()
            .iter()
            .map(|(a, b)| {
                Ok((
                    resolve_one(ds, a, Eligible::Numeric)?,
                    resolve_one(ds, b, Eligible::Numeric)?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if pairs.is_empty() {
            return Err("At least one complete pair is required.".into());
        }
        let o = PairedOptions {
            pairs,
            conf: parse_ci(&self.conf)? / 100.0,
        };
        Ok(RunSpec::read("T-Test", move |ds| run_paired(ds, &o)))
    }

    fn syntax(&self, ds: &Dataset) -> String {
        let (a, b): (Vec<String>, Vec<String>) = self.complete().into_iter().unzip();
        let conf = parse_ci(&self.conf).unwrap_or(95.0);
        Syntax::new(format!(
            "T-TEST PAIRS={} WITH {} (PAIRED)",
            var_list(&current_names(ds, &a)),
            var_list(&current_names(ds, &b))
        ))
        .subcommand(format!("CRITERIA=CI({})", fraction(conf, 4)))
        .subcommand("MISSING=ANALYSIS")
        .finish()
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn help(&self) -> &'static str {
        "Paired-Samples T Test compares the means of two variables measured on the same cases \
         (for example before and after). For each pair it shows the means, the correlation \
         between the two variables, and the t test of the mean difference with its confidence \
         interval."
    }

    fn sub_ui(&mut self, ui: &mut Ui, name: &str) {
        if name == "Options..." {
            ci_options(ui, &mut self.conf);
        }
    }

    fn sub_continue(&mut self, name: &str) -> Result<(), String> {
        if name == "Options..." {
            parse_ci(&self.conf)?;
        }
        Ok(())
    }
}
