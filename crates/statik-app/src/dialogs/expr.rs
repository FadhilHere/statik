//! Parts shared by the Compute Variable dialog and its If sub-dialog: an expression text box
//! that remembers its selection (so list double-clicks and keypad buttons insert at the
//! cursor), the calculator keypad, and a plain variable list.
use super::var_picker::{display_name, list_frame_rows, var_row};
use dataset::{Dataset, VarId};
use egui::text::{CCursor, CCursorRange};
use egui::{Button, Ui, vec2};

/// A multi-line expression box. Positions are character indices (not bytes).
#[derive(Clone, Debug, Default)]
pub struct ExprBox {
    pub text: String,
    /// The selection the last time the box had focus (start, end), start <= end.
    sel: (usize, usize),
    /// Put this selection into the editor (and focus it) on the next frame.
    pending: Option<(usize, usize)>,
}

/// Byte offset of character `i` in `s` (the end for `i` past it).
fn byte_at(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map_or(s.len(), |(b, _)| b)
}

/// Character index of byte offset `b` in `s` (a parse error position).
pub fn char_index(s: &str, b: usize) -> usize {
    let mut b = b.min(s.len());
    while !s.is_char_boundary(b) {
        b -= 1;
    }
    s[..b].chars().count()
}

impl ExprBox {
    pub fn new(text: &str) -> Self {
        ExprBox {
            text: text.into(),
            ..Default::default()
        }
    }

    /// The current selection (start, end) in characters, clamped to the text.
    pub fn selection(&self) -> (usize, usize) {
        let n = self.text.chars().count();
        let (a, b) = self.pending.unwrap_or(self.sel);
        (a.min(n), b.min(n))
    }

    /// Select `start..end` (characters) and focus the box on the next frame.
    pub fn select(&mut self, start: usize, end: usize) {
        self.pending = Some((start.min(end), start.max(end)));
    }

    /// Replace the selection with `s`. `select` = the part of `s` to select afterwards
    /// (character offsets in `s`); otherwise the cursor goes after it.
    pub fn insert(&mut self, s: &str, select: Option<(usize, usize)>) {
        let (a, b) = self.selection();
        let (ba, bb) = (byte_at(&self.text, a), byte_at(&self.text, b));
        self.text.replace_range(ba..bb, s);
        let end = a + s.chars().count();
        let (x, y) = select.map_or((end, end), |(x, y)| (a + x, a + y));
        self.select(x, y);
    }

    /// Insert a function template such as `ABS(?)`, selecting its first `?`.
    pub fn insert_template(&mut self, t: &str) {
        let q = t.chars().position(|c| c == '?');
        self.insert(t, q.map(|q| (q, q + 1)));
    }

    /// Keypad Delete: remove the selection, or the character before the cursor.
    pub fn delete(&mut self) {
        let (a, b) = self.selection();
        let (a, b) = if a == b {
            (a.saturating_sub(1), b)
        } else {
            (a, b)
        };
        let (ba, bb) = (byte_at(&self.text, a), byte_at(&self.text, b));
        self.text.replace_range(ba..bb, "");
        self.select(a, a);
    }

    /// Draw the box. `label_id` names it for accessibility.
    pub fn show(&mut self, ui: &mut Ui, salt: &str, rows: usize, width: f32, label_id: egui::Id) {
        let id = ui.make_persistent_id(salt);
        let mut out = egui::TextEdit::multiline(&mut self.text)
            .id(id)
            .desired_rows(rows)
            .desired_width(width)
            .show(ui);
        out.response.response = out.response.response.labelled_by(label_id);
        if let Some((a, b)) = self.pending.take() {
            out.state
                .cursor
                .set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
            out.state.store(ui.ctx(), id);
            out.response.response.request_focus();
            self.sel = (a, b);
        } else if let Some(r) = out.cursor_range {
            let (p, q): (usize, usize) = (r.primary.index.into(), r.secondary.index.into());
            self.sel = (p.min(q), p.max(q));
        }
    }
}

/// The calculator keypad, as in SPSS: each button inserts its text into `target`.
pub fn keypad(ui: &mut Ui, target: &mut ExprBox) {
    const ROWS: [&[&str]; 5] = [
        &["+", "<", ">", "7", "8", "9"],
        &["-", "<=", ">=", "4", "5", "6"],
        &["*", "=", "~=", "1", "2", "3"],
        &["/", "&", "|", "0", "."],
        &["**", "~", "( )", "Delete"],
    ];
    const W: f32 = 30.0;
    const GAP: f32 = 2.0;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        for row in ROWS {
            ui.horizontal(|ui| {
                for &k in row {
                    // "0" and "Delete" are two keys wide, as in SPSS.
                    let w = if k == "0" || k == "Delete" {
                        2.0 * W + GAP
                    } else {
                        W
                    };
                    if ui.add(Button::new(k).min_size(vec2(w, 20.0))).clicked() {
                        press(target, k);
                    }
                }
            });
        }
    });
}

fn press(target: &mut ExprBox, k: &str) {
    match k {
        "Delete" => target.delete(),
        "( )" => {
            // Wrap the selection in parentheses (or insert "()" with the cursor inside).
            let (a, b) = target.selection();
            let inner: String = target.text.chars().skip(a).take(b - a).collect();
            let n = inner.chars().count();
            target.insert(&format!("({inner})"), Some((n + 1, n + 1)));
        }
        d if d.len() == 1 && (d.as_bytes()[0].is_ascii_digit() || d == ".") => {
            target.insert(d, None)
        }
        op => target.insert(&format!(" {op} "), None),
    }
}

/// A list of all variables (glyph + "Label [name]"). Click selects (`selected`); returns the
/// name of a double-clicked variable.
pub fn var_list(
    ui: &mut Ui,
    ds: &Dataset,
    salt: &str,
    size: egui::Vec2,
    selected: &mut Option<String>,
) -> Option<String> {
    let mut double = None;
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(size.x);
        list_frame_rows(ui, salt, size.y - 4.0, ds.n_vars(), |ui, range| {
            for i in range {
                let v = ds.var(VarId(i));
                let sel = selected
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&v.name));
                let r = var_row(ui, Some(v), &display_name(v), sel, true);
                if r.double_clicked() {
                    double = Some(v.name.clone());
                    *selected = Some(v.name.clone());
                } else if r.clicked() {
                    *selected = Some(v.name.clone());
                }
            }
        });
    });
    double
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_replace_and_delete_by_characters() {
        let mut e = ExprBox::new("é + ");
        e.select(4, 4);
        e.insert_template("ABS(?)");
        assert_eq!(e.text, "é + ABS(?)");
        assert_eq!(e.selection(), (8, 9));
        e.insert("RES_1", None);
        assert_eq!(e.text, "é + ABS(RES_1)");
        assert_eq!(e.selection(), (13, 13));
        e.select(1, 1);
        e.delete();
        assert_eq!(e.text, " + ABS(RES_1)");
        assert_eq!(char_index("'éé' + * 2", 9), 7);
        assert_eq!(char_index("ab", 10), 2);
    }

    #[test]
    fn keypad_keys() {
        let mut e = ExprBox::new("x");
        e.select(1, 1);
        press(&mut e, "*");
        press(&mut e, "2");
        assert_eq!(e.text, "x * 2");
        e.select(0, 5);
        press(&mut e, "( )");
        assert_eq!(e.text, "(x * 2)");
        assert_eq!(e.selection(), (6, 6));
        press(&mut e, "Delete");
        assert_eq!(e.text, "(x * )");
    }
}
