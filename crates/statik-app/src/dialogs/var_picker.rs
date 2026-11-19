//! The variable picker of the procedure dialogs: the source list on the left, target boxes
//! with arrow buttons, as in SPSS.
//!
//! - Items show a measure glyph (ruler = scale, bars = ordinal, circles = nominal; "a" for a
//!   string variable) and "Label [name]" when the variable has a label, else the name.
//! - Click selects, Ctrl+click toggles, Shift+click extends; double-click moves an item to the
//!   first target that accepts it (or back to the source list from a target).
//! - Each target has an arrow button: ▶ (accessible name "Move to <label>") when the selection
//!   is in the source list, ◀ ("Remove from <label>") when the selection is in that target.
//! - Only eligible variables move ([`Eligible`]); a target can be limited to one variable.
//!
//! The picker stores variable **names**; dialogs map them to `VarId`s when OK is pressed
//! (see [`super::resolve`]).
use crate::variable_view::paint_measure_icon;
use dataset::{Dataset, VarType, Variable};
use egui::{
    Color32, Modifiers, Rect, Response, Sense, Stroke, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};

/// Height of one list row.
pub const ROW_H: f32 = 18.0;

/// Which variables a target accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eligible {
    Numeric,
    Any,
}

impl Eligible {
    pub fn accepts(self, v: &Variable) -> bool {
        match self {
            Eligible::Numeric => matches!(v.var_type, VarType::Numeric),
            Eligible::Any => true,
        }
    }
}

/// One target box.
#[derive(Clone, Debug)]
pub struct Target {
    /// Caption above the box, e.g. "Dependent:".
    pub label: &'static str,
    pub vars: Vec<String>,
    /// At most this many variables (`Some(1)` for a single-variable box).
    pub max: Option<usize>,
    pub eligible: Eligible,
    /// A disabled box is drawn greyed out and accepts nothing (features Statik lacks).
    pub enabled: bool,
    /// Visible rows.
    pub rows: usize,
    /// Text shown after a single variable, e.g. "(1 2)" for a grouping variable.
    pub suffix: Option<String>,
}

impl Target {
    /// A list box (any number of numeric variables).
    pub fn list(label: &'static str, rows: usize) -> Self {
        Target {
            label,
            vars: Vec::new(),
            max: None,
            eligible: Eligible::Numeric,
            enabled: true,
            rows,
            suffix: None,
        }
    }
    /// A single-variable box.
    pub fn single(label: &'static str) -> Self {
        Target {
            max: Some(1),
            rows: 1,
            ..Target::list(label, 1)
        }
    }
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
    /// Accessible name of the label without the trailing colon ("Dependent").
    pub fn name(&self) -> &'static str {
        self.label.trim_end_matches(':')
    }
}

/// Where the current selection is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Source,
    Target(usize),
}

#[derive(Clone, Debug, Default)]
pub struct VarPicker {
    pub targets: Vec<Target>,
    /// Variables stay in the source list when moved (Paired-Samples T Test).
    pub keep_source: bool,
    pub side: Side,
    /// Selected names (in the list given by `side`).
    pub selected: Vec<String>,
    anchor: Option<String>,
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// "Label [name]" when the variable has a label, else the name.
pub fn display_name(v: &Variable) -> String {
    match v.label.as_deref().map(str::trim) {
        Some(l) if !l.is_empty() => format!("{l} [{}]", v.name),
        _ => v.name.clone(),
    }
}

/// Paint a variable's glyph into a 14×14 rect.
pub fn paint_glyph(p: &egui::Painter, r: Rect, v: Option<&Variable>) {
    match v {
        Some(v) if matches!(v.var_type, VarType::String { .. }) => {
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "a",
                egui::FontId::proportional(13.0),
                Color32::from_rgb(190, 60, 60),
            );
        }
        Some(v) => paint_measure_icon(p, r, v.measure),
        None => {}
    }
}

/// One list row: glyph + text, selectable. `text` is also its accessible name.
pub fn var_row(
    ui: &mut Ui,
    var: Option<&Variable>,
    text: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let w = ui.available_width().max(40.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::click());
    if ui.is_rect_visible(rect) {
        let vis = ui.visuals();
        let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if selected {
            p.rect_filled(rect, 0.0, vis.selection.bg_fill);
        } else if resp.hovered() && enabled {
            p.rect_filled(rect, 0.0, vis.widgets.hovered.weak_bg_fill);
        }
        let icon = Rect::from_min_size(
            pos2(rect.left() + 3.0, rect.center().y - 7.0),
            vec2(14.0, 14.0),
        );
        paint_glyph(&p, icon, var);
        let color = if !enabled {
            vis.weak_text_color()
        } else if selected {
            vis.selection.stroke.color
        } else {
            vis.text_color()
        };
        p.text(
            pos2(rect.left() + 21.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
    }
    let t = text.to_string();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, &t));
    resp
}

/// A small button with a painted triangle (▶ when `right`), named `name` for accessibility.
pub fn arrow_button(ui: &mut Ui, right: bool, enabled: bool, name: &str) -> bool {
    let resp = ui.add_enabled(enabled, egui::Button::new("").min_size(vec2(26.0, 22.0)));
    let r = resp.rect.shrink2(vec2(9.0, 6.0));
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let pts = if right {
        vec![r.left_top(), r.left_bottom(), pos2(r.right(), r.center().y)]
    } else {
        vec![
            r.right_top(),
            r.right_bottom(),
            pos2(r.left(), r.center().y),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
    let n = name.to_string();
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &n));
    resp.clicked()
}

/// A bordered list box of `rows` rows filling the available width.
pub fn list_frame<R>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    height: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let enabled = ui.is_enabled();
    egui::Frame::new()
        .fill(if enabled {
            ui.visuals().extreme_bg_color
        } else {
            ui.visuals().faint_bg_color
        })
        .stroke(ui.visuals().widgets.inactive.bg_stroke)
        .inner_margin(2.0)
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.set_max_height(height);
            egui::ScrollArea::vertical()
                .id_salt(id)
                .auto_shrink([false, false])
                .min_scrolled_height(height)
                .max_height(height)
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        add(ui)
                    })
                    .inner
                })
                .inner
        })
        .inner
}

impl VarPicker {
    pub fn new(targets: Vec<Target>) -> Self {
        VarPicker {
            targets,
            ..Default::default()
        }
    }

    /// Variables stay in the source list when moved.
    pub fn keep_source(mut self) -> Self {
        self.keep_source = true;
        self
    }

    /// Empty every target and the selection (Reset).
    pub fn clear(&mut self) {
        for t in &mut self.targets {
            t.vars.clear();
        }
        self.selected.clear();
        self.anchor = None;
        self.side = Side::Source;
    }

    pub fn vars(&self, i: usize) -> &[String] {
        &self.targets[i].vars
    }

    fn in_targets(&self, name: &str) -> bool {
        self.targets
            .iter()
            .any(|t| t.vars.iter().any(|v| same(v, name)))
    }

    /// Names in the source list, in dataset order.
    pub fn source(&self, ds: &Dataset) -> Vec<String> {
        (0..ds.n_vars())
            .map(|i| &ds.var(dataset::VarId(i)).name)
            .filter(|n| self.keep_source || !self.in_targets(n))
            .cloned()
            .collect()
    }

    /// The selected names when the selection is in the source list (in dataset order).
    pub fn source_selection(&self, ds: &Dataset) -> Vec<String> {
        if self.side != Side::Source {
            return Vec::new();
        }
        self.source(ds)
            .into_iter()
            .filter(|n| self.selected.iter().any(|s| same(s, n)))
            .collect()
    }

    fn is_selected(&self, side: Side, name: &str) -> bool {
        self.side == side && self.selected.iter().any(|s| same(s, name))
    }

    /// Whether the source selection can move to target `i`.
    pub fn can_move_to(&self, ds: &Dataset, i: usize) -> bool {
        let t = &self.targets[i];
        let sel = self.source_selection(ds);
        if !t.enabled || sel.is_empty() {
            return false;
        }
        let ok = sel.iter().all(|n| {
            ds.find(n).is_some_and(|id| t.eligible.accepts(ds.var(id)))
                && !t.vars.iter().any(|v| same(v, n))
        });
        let room = t.max.is_none_or(|m| t.vars.len() + sel.len() <= m);
        ok && room
    }

    /// Move the source selection to target `i` (if allowed). The moved names stay selected.
    pub fn move_to(&mut self, ds: &Dataset, i: usize) -> bool {
        if !self.can_move_to(ds, i) {
            return false;
        }
        let sel = self.source_selection(ds);
        self.targets[i].vars.extend(sel.iter().cloned());
        if self.keep_source {
            return true;
        }
        self.side = Side::Target(i);
        self.selected = sel;
        true
    }

    pub fn can_remove(&self, i: usize) -> bool {
        self.side == Side::Target(i) && !self.selected.is_empty() && self.targets[i].enabled
    }

    /// Move the selection of target `i` back to the source list (it stays selected there).
    pub fn remove(&mut self, i: usize) {
        if !self.can_remove(i) {
            return;
        }
        let sel = self.selected.clone();
        self.targets[i]
            .vars
            .retain(|v| !sel.iter().any(|s| same(s, v)));
        self.side = Side::Source;
    }

    /// Handle a click on `name` in the list `side` whose items are `order`.
    fn click(&mut self, side: Side, name: &str, order: &[String], mods: Modifiers) {
        if self.side != side {
            self.selected.clear();
            self.anchor = None;
            self.side = side;
        }
        if mods.shift
            && let Some(a) = self.anchor.as_deref()
            && let (Some(i), Some(j)) = (
                order.iter().position(|n| same(n, a)),
                order.iter().position(|n| same(n, name)),
            )
        {
            let (lo, hi) = (i.min(j), i.max(j));
            self.selected = order[lo..=hi].to_vec();
            return;
        }
        if mods.command || mods.ctrl {
            if let Some(p) = self.selected.iter().position(|s| same(s, name)) {
                self.selected.remove(p);
            } else {
                self.selected.push(name.to_string());
            }
        } else {
            self.selected = vec![name.to_string()];
        }
        self.anchor = Some(name.to_string());
    }

    /// Draw the source list (`size` in points).
    pub fn source_ui(&mut self, ui: &mut Ui, ds: &Dataset, size: Vec2) {
        let names = self.source(ds);
        let mut clicked: Option<(String, Modifiers, bool)> = None;
        ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(size.x);
            list_frame(ui, "source", size.y - 4.0, |ui| {
                for n in &names {
                    let var = ds.find(n).map(|id| ds.var(id));
                    let text = var.map(display_name).unwrap_or_else(|| n.clone());
                    let r = var_row(ui, var, &text, self.is_selected(Side::Source, n), true);
                    if r.double_clicked() {
                        clicked = Some((n.clone(), Modifiers::NONE, true));
                    } else if r.clicked() {
                        clicked = Some((n.clone(), ui.input(|i| i.modifiers), false));
                    }
                }
            });
        });
        if let Some((n, mods, double)) = clicked {
            self.click(Side::Source, &n, &names, mods);
            if double {
                self.selected = vec![n];
                if let Some(i) = (0..self.targets.len()).find(|&i| self.can_move_to(ds, i)) {
                    self.move_to(ds, i);
                }
            }
        }
    }

    /// Draw target `i`: its arrow button, caption and box, `width` points wide.
    pub fn target_ui(&mut self, ui: &mut Ui, ds: &Dataset, i: usize, width: f32) {
        let enabled = self.targets[i].enabled;
        ui.horizontal_top(|ui| {
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.add_space(16.0);
                self.arrow_ui(ui, ds, i);
            });
            ui.add_enabled_ui(enabled, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.label(self.targets[i].label);
                    self.target_box(ui, ds, i);
                });
            });
        });
    }

    /// The arrow button of target `i`.
    pub fn arrow_ui(&mut self, ui: &mut Ui, ds: &Dataset, i: usize) {
        let t = &self.targets[i];
        if self.side == Side::Target(i) {
            let name = format!("Remove from {}", t.name());
            if arrow_button(ui, false, self.can_remove(i), &name) {
                self.remove(i);
            }
        } else {
            let name = format!("Move to {}", t.name());
            let ok = self.can_move_to(ds, i);
            if arrow_button(ui, true, ok, &name) {
                self.move_to(ds, i);
            }
        }
    }

    fn target_box(&mut self, ui: &mut Ui, ds: &Dataset, i: usize) {
        let t = &self.targets[i];
        let names = t.vars.clone();
        let suffix = t.suffix.clone().unwrap_or_default();
        let h = t.rows as f32 * ROW_H + 2.0;
        let mut clicked: Option<(String, Modifiers, bool)> = None;
        list_frame(ui, ("target", i), h, |ui| {
            for n in &names {
                let var = ds.find(n).map(|id| ds.var(id));
                let mut text = var.map(display_name).unwrap_or_else(|| n.clone());
                text.push_str(&suffix);
                let r = var_row(ui, var, &text, self.is_selected(Side::Target(i), n), true);
                if r.double_clicked() {
                    clicked = Some((n.clone(), Modifiers::NONE, true));
                } else if r.clicked() {
                    clicked = Some((n.clone(), ui.input(|i| i.modifiers), false));
                }
            }
        });
        if let Some((n, mods, double)) = clicked {
            self.click(Side::Target(i), &n, &names, mods);
            if double {
                self.selected = vec![n];
                self.remove(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dataset::{Column, Variable};

    fn ds() -> Dataset {
        let mut d = Dataset::new();
        for n in ["a", "b", "c"] {
            d.add_variable(Variable::numeric(n), Column::Numeric(vec![1.0]))
                .unwrap();
        }
        d.add_variable(Variable::string("s", 4), Column::Text(vec!["x".into()]))
            .unwrap();
        d
    }

    #[test]
    fn moves_respect_type_and_capacity() {
        let d = ds();
        let mut p = VarPicker::new(vec![
            Target::single("Dependent:"),
            Target::list("Independent(s):", 4),
        ]);
        p.click(Side::Source, "a", &p.source(&d), Modifiers::NONE);
        assert!(p.move_to(&d, 0));
        assert_eq!(p.source(&d), ["b", "c", "s"]);
        assert_eq!(p.side, Side::Target(0));
        // Shift-select b..s: s is a string, so nothing moves.
        let src = p.source(&d);
        p.click(Side::Source, "b", &src, Modifiers::NONE);
        p.click(Side::Source, "s", &src, Modifiers::SHIFT);
        assert_eq!(p.selected, ["b", "c", "s"]);
        assert!(!p.can_move_to(&d, 1));
        p.click(Side::Source, "s", &src, Modifiers::COMMAND);
        assert!(!p.can_move_to(&d, 0), "single box is full");
        assert!(p.move_to(&d, 1));
        assert_eq!(p.vars(1), ["b", "c"]);
        p.click(
            Side::Target(1),
            "b",
            &["b".into(), "c".into()],
            Modifiers::NONE,
        );
        assert!(p.can_remove(1));
        p.remove(1);
        assert_eq!(p.vars(1), ["c"]);
        assert_eq!(p.side, Side::Source);
        p.clear();
        assert!(p.vars(0).is_empty() && p.vars(1).is_empty());
    }
}
