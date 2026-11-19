//! UI-side state of the Data Editor: which view is shown, the selection and the cell being
//! edited, the open Variable View dialog, and the variable properties the data model does not
//! hold (Align, Role, and Columns of numeric variables). Plain data without egui types, so it
//! lives in [`crate::AppState::editor`] and the logic tests can drive it.
//!
//! Align/Role/Columns are keyed by the lower-case variable name (variable names are unique
//! case-insensitively); [`crate::edit`] moves the keys when a variable is renamed. They are not
//! saved to `.sav` files (see `KNOWN_GAPS.md`).
use crate::variable_view::VarDialog;
use dataset::{VarType, Variable};
use std::collections::HashMap;
use std::ops::Range;

/// The tab at the bottom of the Data Editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Data,
    Variable,
}

/// What the Data View selection covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelKind {
    /// A block of cells between `anchor` and `cursor`.
    #[default]
    Cells,
    /// Whole variables (columns `anchor.1..=cursor.1`), picked on the column headers.
    Vars,
    /// Whole cases (rows `anchor.0..=cursor.0`), picked on the row numbers.
    Cases,
}

/// The Data View cell being typed into.
#[derive(Clone, Debug, PartialEq)]
pub struct CellEdit {
    pub row: usize,
    pub col: usize,
    pub text: String,
    /// Started by typing (arrow keys then commit and move, as in SPSS) rather than by F2 or a
    /// double click (arrow keys move the text cursor).
    pub from_typing: bool,
    /// Keyboard focus was given to the text field.
    pub focused: bool,
}

/// Column alignment in the Data View (Variable View "Align").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
}

impl Align {
    pub const ALL: [Align; 3] = [Align::Left, Align::Right, Align::Center];
    pub fn label(self) -> &'static str {
        match self {
            Align::Left => "Left",
            Align::Right => "Right",
            Align::Center => "Center",
        }
    }
}

/// Variable View "Role".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Input,
    Target,
    Both,
    None,
    Partition,
    Split,
}

impl Role {
    pub const ALL: [Role; 6] = [
        Role::Input,
        Role::Target,
        Role::Both,
        Role::None,
        Role::Partition,
        Role::Split,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Role::Input => "Input",
            Role::Target => "Target",
            Role::Both => "Both",
            Role::None => "None",
            Role::Partition => "Partition",
            Role::Split => "Split",
        }
    }
}

/// A Variable View text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarField {
    Name,
    Width,
    Decimals,
    Label,
    Columns,
}

/// The Variable View cell being typed into.
#[derive(Clone, Debug, PartialEq)]
pub struct VarEdit {
    pub row: usize,
    pub field: VarField,
    pub text: String,
    pub focused: bool,
}

/// The UI-side properties of one variable (`None` = default).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiProps {
    pub align: Option<Align>,
    pub role: Option<Role>,
    pub columns: Option<usize>,
}

#[derive(Debug, Default)]
pub struct EditorState {
    pub view: View,
    /// Current cell `(row, col)`, 0-based. May lie in the empty area after the data.
    pub cursor: (usize, usize),
    /// Other corner of the selection.
    pub anchor: (usize, usize),
    pub sel_kind: SelKind,
    pub editing: Option<CellEdit>,
    /// Scroll the Data View so that the cursor is visible (set by keyboard moves and jumps).
    pub scroll_to_cursor: bool,
    /// Selected row of the Variable View.
    pub var_row: usize,
    /// Scroll the Variable View to `var_row`.
    pub scroll_to_var_row: bool,
    pub var_edit: Option<VarEdit>,
    /// The open Variable View dialog (Variable Type, Value Labels, Missing Values).
    pub var_dialog: Option<VarDialog>,
    /// A modal question or dialog is up (message, save prompt, recovery, procedure dialog,
    /// Variable View dialog) or a job runs: the views ignore the keyboard. Set by the app
    /// every frame before the views are drawn.
    pub blocked: bool,
    align: HashMap<String, Align>,
    role: HashMap<String, Role>,
    columns: HashMap<String, usize>,
}

fn key(name: &str) -> String {
    name.to_lowercase()
}

impl EditorState {
    /// Select the cells between `anchor` and `cursor` (`(row, col)`, inclusive).
    pub fn select_cells(&mut self, anchor: (usize, usize), cursor: (usize, usize)) {
        self.anchor = anchor;
        self.cursor = cursor;
        self.sel_kind = SelKind::Cells;
    }
    /// Select whole variables `c0..=c1` (the cursor goes to the first case of `c1`).
    pub fn select_vars(&mut self, c0: usize, c1: usize) {
        self.anchor = (0, c0);
        self.cursor = (0, c1);
        self.sel_kind = SelKind::Vars;
    }
    /// Select whole cases `r0..=r1`.
    pub fn select_cases(&mut self, r0: usize, r1: usize) {
        self.anchor = (r0, 0);
        self.cursor = (r1, 0);
        self.sel_kind = SelKind::Cases;
    }
    /// Move the cursor to a cell, select only it and scroll to it.
    pub fn jump_to(&mut self, row: usize, col: usize) {
        self.select_cells((row, col), (row, col));
        self.scroll_to_cursor = true;
    }

    /// Selected rows and columns (half-open), with whole variables / cases spanning
    /// `n_cases` / `n_vars`.
    pub fn block(&self, n_cases: usize, n_vars: usize) -> (Range<usize>, Range<usize>) {
        let (r0, r1) = minmax(self.anchor.0, self.cursor.0);
        let (c0, c1) = minmax(self.anchor.1, self.cursor.1);
        match self.sel_kind {
            SelKind::Cells => (r0..r1 + 1, c0..c1 + 1),
            SelKind::Vars => (0..n_cases, c0..c1 + 1),
            SelKind::Cases => (r0..r1 + 1, 0..n_vars),
        }
    }

    /// Whether cell `(row, col)` is selected.
    pub fn is_selected(&self, row: usize, col: usize) -> bool {
        let (r0, r1) = minmax(self.anchor.0, self.cursor.0);
        let (c0, c1) = minmax(self.anchor.1, self.cursor.1);
        match self.sel_kind {
            SelKind::Cells => (r0..=r1).contains(&row) && (c0..=c1).contains(&col),
            SelKind::Vars => (c0..=c1).contains(&col),
            SelKind::Cases => (r0..=r1).contains(&row),
        }
    }

    /// Alignment of a variable's cells: set in the Variable View, else right for numbers and
    /// left for strings (SPSS defaults).
    pub fn align(&self, v: &Variable) -> Align {
        self.align
            .get(&key(&v.name))
            .copied()
            .unwrap_or(match v.var_type {
                VarType::Numeric => Align::Right,
                VarType::String { .. } => Align::Left,
            })
    }
    pub fn set_align(&mut self, name: &str, a: Align) {
        self.align.insert(key(name), a);
    }
    /// Role of a variable (default Input).
    pub fn role(&self, v: &Variable) -> Role {
        self.role.get(&key(&v.name)).copied().unwrap_or(Role::Input)
    }
    pub fn set_role(&mut self, name: &str, r: Role) {
        self.role.insert(key(name), r);
    }
    /// Displayed width in characters ("Columns"). For strings it is the variable's display
    /// width (saved); for numbers a UI-side value defaulting to the format width.
    pub fn columns(&self, v: &Variable) -> usize {
        match v.var_type {
            VarType::String { .. } => v.width,
            VarType::Numeric => self.columns.get(&key(&v.name)).copied().unwrap_or(v.width),
        }
    }
    /// Set the UI-side Columns of a numeric variable.
    pub fn set_numeric_columns(&mut self, name: &str, n: usize) {
        self.columns.insert(key(name), n);
    }

    /// The UI-side properties set for variable `name`.
    pub fn props(&self, name: &str) -> UiProps {
        let k = key(name);
        UiProps {
            align: self.align.get(&k).copied(),
            role: self.role.get(&k).copied(),
            columns: self.columns.get(&k).copied(),
        }
    }
    /// Remove (and return) the UI-side properties of a deleted variable.
    pub(crate) fn take_props(&mut self, name: &str) -> UiProps {
        let k = key(name);
        UiProps {
            align: self.align.remove(&k),
            role: self.role.remove(&k),
            columns: self.columns.remove(&k),
        }
    }
    /// Put back the UI-side properties of a variable (undo of a delete).
    pub(crate) fn restore_props(&mut self, name: &str, p: UiProps) {
        let k = key(name);
        if let Some(a) = p.align {
            self.align.insert(k.clone(), a);
        }
        if let Some(r) = p.role {
            self.role.insert(k.clone(), r);
        }
        if let Some(c) = p.columns {
            self.columns.insert(k, c);
        }
    }

    /// A variable was renamed: keep its UI-side properties.
    pub(crate) fn rename_key(&mut self, old: &str, new: &str) {
        let (old, new) = (key(old), key(new));
        if old == new {
            return;
        }
        if let Some(a) = self.align.remove(&old) {
            self.align.insert(new.clone(), a);
        }
        if let Some(r) = self.role.remove(&old) {
            self.role.insert(new.clone(), r);
        }
        if let Some(c) = self.columns.remove(&old) {
            self.columns.insert(new, c);
        }
    }
}

fn minmax(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}
