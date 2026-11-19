//! Data Editor operations without UI: undoable edits ([`EditOp`]), undo/redo, typing into
//! cells (creating `VAR00001` / new cases like SPSS), tab-separated copy/paste, and variable
//! property changes.
//!
//! Every change to the data goes through an [`EditOp`] applied with [`AppState::edit`] (or, for
//! a type change that is easier to perform first, recorded with `record`). Each op stores
//! what it needs to be reverted exactly: the old cell values, the deleted columns and
//! variables, or the whole column before and after a type change. The undo stack holds at most
//! [`UNDO_LIMIT`] ops; the oldest are dropped.
//!
//! The `AppState` methods here never panic on bad input: invalid text or names queue an SPSS-like
//! message and leave the data unchanged (they return `false`).
use crate::editor::{SelKind, View};
use crate::state::{AppState, Message};
use dataset::{
    Column, Dataset, DatasetError, MissingSpec, VarId, VarType, Variable, truncate_bytes,
    validate_name,
};
use std::ops::Range;

/// Most undo steps kept.
pub const UNDO_LIMIT: usize = 100;

/// One cell's value.
#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    Num(f64),
    Text(String),
}

impl CellValue {
    fn get(ds: &Dataset, var: usize, row: usize) -> CellValue {
        match ds.column(VarId(var)) {
            Column::Numeric(v) => CellValue::Num(v[row]),
            Column::Text(v) => CellValue::Text(v[row].clone()),
        }
    }
    fn set(&self, ds: &mut Dataset, var: usize, row: usize) -> Result<(), DatasetError> {
        match self {
            CellValue::Num(x) => ds.set_numeric(VarId(var), row, *x),
            CellValue::Text(s) => ds.set_text(VarId(var), row, s),
        }
    }
    /// The empty value of a variable type (system-missing / "").
    fn blank(t: &VarType) -> CellValue {
        match t {
            VarType::Numeric => CellValue::Num(f64::NAN),
            VarType::String { .. } => CellValue::Text(String::new()),
        }
    }
    /// Same value (NaN equals NaN, -0 equals 0).
    fn same(&self, o: &CellValue) -> bool {
        match (self, o) {
            (CellValue::Num(a), CellValue::Num(b)) => a == b || (a.is_nan() && b.is_nan()),
            (CellValue::Text(a), CellValue::Text(b)) => a == b,
            _ => false,
        }
    }
}

/// An undoable change to the dataset. `apply` performs it, `revert` restores the state before
/// it. Positions are 0-based variable / case indices at the time the op is applied.
#[derive(Clone, Debug)]
pub enum EditOp {
    SetCell {
        var: usize,
        row: usize,
        old: CellValue,
        new: CellValue,
    },
    /// `count` blank cases inserted before case `at`.
    InsertCase { at: usize, count: usize },
    /// Cases `at..at + count` deleted; `values` holds their values, one column per variable.
    DeleteCase {
        at: usize,
        count: usize,
        values: Vec<Column>,
    },
    InsertVar {
        at: usize,
        var: Box<Variable>,
        col: Column,
    },
    /// Variable `at` deleted, with its column; `weight` = it was the weight variable.
    DeleteVar {
        at: usize,
        var: Box<Variable>,
        col: Column,
        weight: bool,
    },
    /// Properties of variable `at` changed. For a type change `cols` holds the column before
    /// and after (the variable is replaced as a whole); `weight` = it was the weight variable
    /// before (a type change clears the weight).
    ChangeVarProp {
        at: usize,
        old: Box<Variable>,
        new: Box<Variable>,
        cols: Option<Box<(Column, Column)>>,
        weight: bool,
    },
    /// Several ops undone and redone as one step (paste, typing into a new column, ...).
    Group(Vec<EditOp>),
}

/// Replace variable `at` by `v` (and its column by `col`, if given).
fn put_var(
    ds: &mut Dataset,
    at: usize,
    v: &Variable,
    col: Option<&Column>,
    weight: bool,
) -> Result<(), DatasetError> {
    match col {
        Some(col) => {
            ds.delete_variable(VarId(at))?;
            ds.insert_variable(at, v.clone(), col.clone())?;
        }
        None => {
            if ds.try_var(VarId(at)).is_none() {
                return Err(DatasetError::OutOfRange(at));
            }
            if ds.var(VarId(at)).name != v.name {
                ds.rename_variable(VarId(at), &v.name)?;
            }
            debug_assert_eq!(ds.var(VarId(at)).var_type, v.var_type);
            *ds.var_mut(VarId(at)) = v.clone();
        }
    }
    if weight {
        ds.set_weight(Some(VarId(at)))?;
    }
    Ok(())
}

impl EditOp {
    pub fn apply(&self, ds: &mut Dataset) -> Result<(), DatasetError> {
        match self {
            EditOp::SetCell { var, row, new, .. } => new.set(ds, *var, *row),
            EditOp::InsertCase { at, count } => {
                if *at > ds.n_cases() || ds.n_vars() == 0 {
                    return Err(DatasetError::OutOfRange(*at));
                }
                ds.insert_cases(*at, *count);
                Ok(())
            }
            EditOp::DeleteCase { at, count, .. } => ds.delete_cases(*at..*at + *count),
            EditOp::InsertVar { at, var, col } => ds
                .insert_variable(*at, (**var).clone(), col.clone())
                .map(|_| ()),
            EditOp::DeleteVar { at, .. } => ds.delete_variable(VarId(*at)),
            EditOp::ChangeVarProp { at, new, cols, .. } => {
                put_var(ds, *at, new, cols.as_ref().map(|c| &c.1), false)
            }
            EditOp::Group(ops) => {
                for (i, op) in ops.iter().enumerate() {
                    if let Err(e) = op.apply(ds) {
                        // Roll back what was applied, so the group is all or nothing.
                        for done in ops[..i].iter().rev() {
                            let _ = done.revert(ds);
                        }
                        return Err(e);
                    }
                }
                Ok(())
            }
        }
    }

    pub fn revert(&self, ds: &mut Dataset) -> Result<(), DatasetError> {
        match self {
            EditOp::SetCell { var, row, old, .. } => old.set(ds, *var, *row),
            EditOp::InsertCase { at, count } => {
                if *count == 0 {
                    return Ok(());
                }
                ds.delete_cases(*at..*at + *count)
            }
            EditOp::DeleteCase { at, count, values } => {
                if *at > ds.n_cases() {
                    return Err(DatasetError::OutOfRange(*at));
                }
                ds.insert_cases(*at, *count);
                for (j, col) in values.iter().enumerate() {
                    match col {
                        Column::Numeric(v) => {
                            for (i, x) in v.iter().enumerate() {
                                ds.set_numeric(VarId(j), at + i, *x)?;
                            }
                        }
                        Column::Text(v) => {
                            for (i, s) in v.iter().enumerate() {
                                ds.set_text(VarId(j), at + i, s)?;
                            }
                        }
                    }
                }
                Ok(())
            }
            EditOp::InsertVar { at, .. } => ds.delete_variable(VarId(*at)),
            EditOp::DeleteVar {
                at,
                var,
                col,
                weight,
            } => {
                ds.insert_variable(*at, (**var).clone(), col.clone())?;
                if *weight {
                    ds.set_weight(Some(VarId(*at)))?;
                }
                Ok(())
            }
            EditOp::ChangeVarProp {
                at,
                old,
                cols,
                weight,
                ..
            } => put_var(ds, *at, old, cols.as_ref().map(|c| &c.0), *weight),
            EditOp::Group(ops) => {
                for (i, op) in ops.iter().enumerate().rev() {
                    if let Err(e) = op.revert(ds) {
                        for done in &ops[i + 1..] {
                            let _ = done.apply(ds);
                        }
                        return Err(e);
                    }
                }
                Ok(())
            }
        }
    }

    /// Variables renamed by this op, as `(old name, new name)` when applied.
    fn renames(&self, out: &mut Vec<(String, String)>) {
        match self {
            EditOp::ChangeVarProp { old, new, .. } if old.name != new.name => {
                out.push((old.name.clone(), new.name.clone()));
            }
            EditOp::Group(ops) => ops.iter().for_each(|o| o.renames(out)),
            _ => {}
        }
    }

    /// One op, or a group of several (`None` when there is nothing to do).
    fn group(mut ops: Vec<EditOp>) -> Option<EditOp> {
        match ops.len() {
            0 => None,
            1 => ops.pop(),
            _ => Some(EditOp::Group(ops)),
        }
    }
}

/// The first unused name `VAR00001`, `VAR00002`, ...
pub fn next_var_name(ds: &Dataset) -> String {
    next_var_names(ds, 1).pop().expect("one name")
}

/// `n` unused names `VARnnnnn`, in order.
fn next_var_names(ds: &Dataset, n: usize) -> Vec<String> {
    (1..)
        .map(|i| format!("VAR{i:05}"))
        .filter(|c| ds.find(c).is_none())
        .take(n)
        .collect()
}

/// Parse a number typed or pasted by the user: `.` for the decimal point, or a single `,` as in
/// Indonesian/European spreadsheets. Infinite and NaN values are rejected.
pub fn parse_number(t: &str) -> Option<f64> {
    let t = t.trim();
    let ok = |x: f64| x.is_finite().then_some(x + 0.0);
    match t.parse::<f64>() {
        Ok(x) => ok(x),
        Err(_) if !t.contains('.') && t.matches(',').count() == 1 => {
            t.replace(',', ".").parse::<f64>().ok().and_then(ok)
        }
        Err(_) => None,
    }
}

/// Convert text typed into a cell of `var`. Numeric: blank or `.` is system-missing; with
/// `labels` (View > Value Labels) a value label stands for its value. String: cut to the
/// storage width. `Err` holds the SPSS-like message for invalid numeric text.
pub fn parse_cell(var: &Variable, text: &str, labels: bool) -> Result<CellValue, String> {
    match var.var_type {
        VarType::Numeric => {
            let t = text.trim();
            if t.is_empty() || t == "." {
                return Ok(CellValue::Num(f64::NAN));
            }
            if let Some(x) = parse_number(t) {
                return Ok(CellValue::Num(x));
            }
            if labels
                && let Some((k, _)) = var
                    .value_labels
                    .iter()
                    .find(|(_, l)| l.trim().eq_ignore_ascii_case(t))
            {
                return Ok(CellValue::Num(k.0));
            }
            Err(format!(
                "\"{t}\" is not a valid value for the numeric variable {}. Only numbers \
                 can be entered; the old value was kept.",
                var.name
            ))
        }
        VarType::String { width } => {
            if labels
                && let Some((k, _)) = var
                    .string_value_labels
                    .iter()
                    .find(|(_, l)| l.trim() == text.trim())
            {
                return Ok(CellValue::Text(k.clone()));
            }
            Ok(CellValue::Text(truncate_bytes(text, width).to_string()))
        }
    }
}

/// Format a number with `decimals` decimals ("." for system-missing).
pub fn format_number(x: f64, decimals: usize) -> String {
    if x.is_nan() {
        ".".into()
    } else {
        format!("{:.*}", decimals.min(16), x + 0.0)
    }
}

/// Text of a cell as the Data View shows it: numbers with the variable's decimals,
/// system-missing as `.`, and with `labels` the value label when there is one.
pub fn display_value(ds: &Dataset, var: usize, row: usize, labels: bool) -> String {
    let v = ds.var(VarId(var));
    match ds.column(VarId(var)) {
        Column::Numeric(c) => {
            let x = c[row];
            if labels
                && !x.is_nan()
                && let Some(l) = v.value_labels.get(&dataset::OrderedValue::new(x))
            {
                return l.clone();
            }
            format_number(x, v.decimals)
        }
        Column::Text(c) => {
            let s = &c[row];
            if labels && let Some(l) = v.string_value_labels.get(s) {
                return l.clone();
            }
            s.clone()
        }
    }
}

/// Split tab-separated clipboard text into rows of cells (CRLF or LF; a final line break does
/// not make an empty row).
pub fn parse_clipboard(text: &str) -> Vec<Vec<String>> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines
        .last()
        .is_some_and(|l| l.trim_end_matches('\r').is_empty())
    {
        lines.pop();
    }
    lines
        .iter()
        .map(|l| {
            l.trim_end_matches('\r')
                .split('\t')
                .map(str::to_string)
                .collect()
        })
        .collect()
}

fn invalid(text: impl Into<String>) -> Message {
    Message::warning("Invalid Value", text)
}

impl AppState {
    // ----- undo / redo -----

    /// Apply `op` to the document and push it on the undo stack (the redo stack is cleared).
    pub fn edit(&mut self, op: EditOp) -> Result<(), String> {
        op.apply(self.doc.ds_mut()).map_err(|e| e.to_string())?;
        self.rename_keys(&op, true);
        self.record(op);
        Ok(())
    }

    /// Push an op that was already applied.
    pub(crate) fn record(&mut self, op: EditOp) {
        self.redo_stack.clear();
        self.undo_stack.push(op);
        if self.undo_stack.len() > UNDO_LIMIT {
            let extra = self.undo_stack.len() - UNDO_LIMIT;
            self.undo_stack.drain(..extra);
        }
    }

    /// Undo the last edit. False when there is nothing to undo (or it failed: then the history
    /// is dropped and an error message queued).
    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo_stack.pop() else {
            return false;
        };
        match op.revert(self.doc.ds_mut()) {
            Ok(()) => {
                self.rename_keys(&op, false);
                self.redo_stack.push(op);
                true
            }
            Err(e) => {
                self.clear_undo();
                self.push_message(Message::error(format!("Cannot undo: {e}")));
                false
            }
        }
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo_stack.pop() else {
            return false;
        };
        match op.apply(self.doc.ds_mut()) {
            Ok(()) => {
                self.rename_keys(&op, true);
                self.undo_stack.push(op);
                true
            }
            Err(e) => {
                self.clear_undo();
                self.push_message(Message::error(format!("Cannot redo: {e}")));
                false
            }
        }
    }

    /// Number of undo steps available.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Forget the undo/redo history.
    pub fn clear_undo(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn rename_keys(&mut self, op: &EditOp, forward: bool) {
        let mut r = Vec::new();
        op.renames(&mut r);
        for (old, new) in r {
            if forward {
                self.editor.rename_key(&old, &new);
            } else {
                self.editor.rename_key(&new, &old);
            }
        }
    }

    fn apply_or_message(&mut self, op: Option<EditOp>) -> bool {
        let Some(op) = op else { return true };
        match self.edit(op) {
            Ok(()) => true,
            Err(e) => {
                self.push_message(Message::error(e));
                false
            }
        }
    }

    // ----- cells -----

    /// Ops creating the variables and cases needed for cells up to `(last_row, last_col)`:
    /// new numeric variables `VARnnnnn` (F8.2) and blank cases, as SPSS does when typing past
    /// the data.
    fn grow_ops(&self, last_row: usize, last_col: usize) -> Vec<EditOp> {
        let ds = self.doc.ds();
        let (n_vars, n_cases) = (ds.n_vars(), ds.n_cases());
        let mut ops = Vec::new();
        if last_col >= n_vars {
            let names = next_var_names(ds, last_col + 1 - n_vars);
            for (i, name) in names.into_iter().enumerate() {
                ops.push(EditOp::InsertVar {
                    at: n_vars + i,
                    var: Box::new(Variable::numeric(&name)),
                    col: Column::Numeric(vec![f64::NAN; n_cases]),
                });
            }
        }
        if last_row >= n_cases {
            ops.push(EditOp::InsertCase {
                at: n_cases,
                count: last_row + 1 - n_cases,
            });
        }
        ops
    }

    /// The variable a cell in column `col` belongs to (a new numeric one past the data).
    fn target_var(&self, col: usize) -> Variable {
        let ds = self.doc.ds();
        ds.try_var(VarId(col))
            .cloned()
            .unwrap_or_else(|| Variable::numeric(&next_var_name(ds)))
    }

    /// Enter `text` into cell `(row, col)` as typed in the Data View. Past the last variable or
    /// case, the needed variables (`VAR00001`, ...) and cases are created first (one undo step).
    /// Invalid numeric text queues a message and changes nothing (returns false).
    pub fn set_cell_text(&mut self, row: usize, col: usize, text: &str) -> bool {
        let var = self.target_var(col);
        let new = match parse_cell(&var, text, self.value_labels) {
            Ok(v) => v,
            Err(e) => {
                self.push_message(invalid(e));
                return false;
            }
        };
        let ds = self.doc.ds();
        let inside = row < ds.n_cases() && col < ds.n_vars();
        if !inside && text.trim().is_empty() {
            return false; // nothing typed into the empty area: nothing to create
        }
        let old = if inside {
            CellValue::get(ds, col, row)
        } else {
            CellValue::blank(&var.var_type)
        };
        let mut ops = self.grow_ops(row, col);
        if !new.same(&old) {
            ops.push(EditOp::SetCell {
                var: col,
                row,
                old,
                new,
            });
        }
        self.apply_or_message(EditOp::group(ops))
    }

    /// Paste tab-separated text (e.g. copied from Excel) with its top-left cell at
    /// `(row, col)`, creating variables (numeric) and cases as needed. Text that is not a number
    /// in a numeric column rejects the whole paste with a message. One undo step.
    pub fn paste_text(&mut self, row: usize, col: usize, text: &str) -> bool {
        let grid = parse_clipboard(text);
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);
        if grid.is_empty() || width == 0 {
            return false;
        }
        let vars: Vec<Variable> = (col..col + width).map(|c| self.target_var(c)).collect();
        // New variables get consecutive names.
        let new_names = next_var_names(self.doc.ds(), width);
        let n_vars = self.doc.ds().n_vars();
        let mut cells = Vec::new();
        for (i, line) in grid.iter().enumerate() {
            for (j, t) in line.iter().enumerate() {
                let c = col + j;
                let var = if c < n_vars {
                    vars[j].clone()
                } else {
                    Variable::numeric(&new_names[c - n_vars])
                };
                match parse_cell(&var, t, self.value_labels) {
                    Ok(v) => cells.push((row + i, c, v)),
                    Err(_) => {
                        self.push_message(invalid(format!(
                            "The data cannot be pasted: \"{}\" is not a valid value for the \
                             numeric variable {}. Nothing was changed.",
                            t.trim(),
                            var.name
                        )));
                        return false;
                    }
                }
            }
        }
        let ds = self.doc.ds();
        let (n_cases, n_vars) = (ds.n_cases(), ds.n_vars());
        let mut ops = self.grow_ops(row + grid.len() - 1, col + width - 1);
        for (r, c, new) in cells {
            let old = if r < n_cases && c < n_vars {
                CellValue::get(ds, c, r)
            } else {
                CellValue::blank(&VarType::Numeric)
            };
            let old = match (&new, old) {
                // Blank text into a new string cell, etc.: compare against the right type.
                (CellValue::Text(_), CellValue::Num(_)) => CellValue::Text(String::new()),
                (_, o) => o,
            };
            if !new.same(&old) {
                ops.push(EditOp::SetCell {
                    var: c,
                    row: r,
                    old,
                    new,
                });
            }
        }
        self.apply_or_message(EditOp::group(ops))
    }

    /// Tab-separated text of a block (CRLF after every row, as Excel copies). Values are copied
    /// as shown (decimals, value labels when shown); system-missing becomes an empty cell.
    pub fn copy_text(&self, rows: Range<usize>, cols: Range<usize>) -> String {
        let ds = self.doc.ds();
        let rows = rows.start..rows.end.min(ds.n_cases());
        let cols = cols.start..cols.end.min(ds.n_vars());
        let mut out = String::new();
        if cols.is_empty() {
            return out;
        }
        for r in rows {
            let cells: Vec<String> = cols
                .clone()
                .map(|c| {
                    let numeric_missing =
                        matches!(ds.column(VarId(c)), Column::Numeric(v) if v[r].is_nan());
                    if numeric_missing {
                        String::new()
                    } else {
                        display_value(ds, c, r, self.value_labels)
                    }
                })
                .collect();
            out.push_str(&cells.join("\t"));
            out.push_str("\r\n");
        }
        out
    }

    /// Set the cells of a block to system-missing / empty (one undo step).
    pub fn clear_cells(&mut self, rows: Range<usize>, cols: Range<usize>) -> bool {
        let ds = self.doc.ds();
        let rows = rows.start..rows.end.min(ds.n_cases());
        let cols = cols.start..cols.end.min(ds.n_vars());
        let mut ops = Vec::new();
        for c in cols {
            let blank = CellValue::blank(&ds.var(VarId(c)).var_type);
            for r in rows.clone() {
                let old = CellValue::get(ds, c, r);
                if !old.same(&blank) {
                    ops.push(EditOp::SetCell {
                        var: c,
                        row: r,
                        old,
                        new: blank.clone(),
                    });
                }
            }
        }
        self.apply_or_message(EditOp::group(ops))
    }

    // ----- variables and cases -----

    /// Insert a new numeric variable `VARnnnnn` before position `at` (clamped to the end).
    pub fn insert_variable(&mut self, at: usize) -> bool {
        let ds = self.doc.ds();
        let op = EditOp::InsertVar {
            at: at.min(ds.n_vars()),
            var: Box::new(Variable::numeric(&next_var_name(ds))),
            col: Column::Numeric(vec![f64::NAN; ds.n_cases()]),
        };
        self.apply_or_message(Some(op))
    }

    /// Insert `count` blank cases before case `at` (clamped to the end). Needs a variable.
    pub fn insert_cases(&mut self, at: usize, count: usize) -> bool {
        let ds = self.doc.ds();
        if ds.n_vars() == 0 || count == 0 {
            return false;
        }
        let op = EditOp::InsertCase {
            at: at.min(ds.n_cases()),
            count,
        };
        self.apply_or_message(Some(op))
    }

    /// Delete variables `range` (clamped), one undo step.
    pub fn delete_variables(&mut self, range: Range<usize>) -> bool {
        let ds = self.doc.ds();
        let range = range.start..range.end.min(ds.n_vars());
        let weight = ds.weight();
        // Highest first, so the positions of the others stay valid.
        let ops: Vec<EditOp> = range
            .rev()
            .map(|i| EditOp::DeleteVar {
                at: i,
                var: Box::new(ds.var(VarId(i)).clone()),
                col: ds.column(VarId(i)).clone(),
                weight: weight == Some(VarId(i)),
            })
            .collect();
        self.apply_or_message(EditOp::group(ops))
    }

    /// Delete cases `range` (clamped), one undo step.
    pub fn delete_cases(&mut self, range: Range<usize>) -> bool {
        let ds = self.doc.ds();
        let range = range.start..range.end.min(ds.n_cases());
        if range.is_empty() {
            return false;
        }
        let values = (0..ds.n_vars())
            .map(|j| match ds.column(VarId(j)) {
                Column::Numeric(v) => Column::Numeric(v[range.clone()].to_vec()),
                Column::Text(v) => Column::Text(v[range.clone()].to_vec()),
            })
            .collect();
        let op = EditOp::DeleteCase {
            at: range.start,
            count: range.len(),
            values,
        };
        self.apply_or_message(Some(op))
    }

    /// Variable View: a name typed into empty row `row` creates a numeric variable with that
    /// name there (and `VARnnnnn` variables for the empty rows before it), as one undo step.
    pub fn create_variable(&mut self, row: usize, name: &str) -> bool {
        let name = name.trim();
        if let Some(e) = self.name_error(None, name) {
            self.push_message(Message::warning("Invalid Variable Name", e));
            return false;
        }
        let ds = self.doc.ds();
        let (n_vars, n_cases) = (ds.n_vars(), ds.n_cases());
        if row < n_vars {
            return false;
        }
        let mut names = next_var_names(ds, row - n_vars);
        names.push(name.to_string());
        let ops = names
            .into_iter()
            .enumerate()
            .map(|(i, n)| EditOp::InsertVar {
                at: n_vars + i,
                var: Box::new(Variable::numeric(&n)),
                col: Column::Numeric(vec![f64::NAN; n_cases]),
            })
            .collect();
        self.apply_or_message(EditOp::group(ops))
    }

    /// Message for a name that cannot be used for variable `at`, if any.
    pub fn name_error(&self, at: Option<usize>, name: &str) -> Option<String> {
        if name.is_empty() {
            return Some("A variable name cannot be empty.".into());
        }
        if validate_name(name).is_err() {
            return Some(format!(
                "The variable name \"{name}\" is not valid. A name must begin with a letter, \
                 contain only letters, digits and the characters _ . $ # @, be at most 64 bytes \
                 long, and must not be a reserved word (ALL, AND, BY, EQ, GE, GT, LE, LT, NE, \
                 NOT, OR, TO, WITH)."
            ));
        }
        if let Some(other) = self.doc.ds().find(name)
            && Some(other.0) != at
        {
            return Some(format!(
                "The variable name \"{name}\" is already in use. Variable names must be unique."
            ));
        }
        None
    }

    /// Rename variable `at`. An invalid or duplicate name queues a message and changes nothing.
    pub fn rename_variable(&mut self, at: usize, name: &str) -> bool {
        let name = name.trim();
        if let Some(e) = self.name_error(Some(at), name) {
            self.push_message(Message::warning("Invalid Variable Name", e));
            return false;
        }
        self.change_variable(at, |v| v.name = name.to_string())
    }

    /// Change properties of variable `at` (not its type: see [`Self::set_variable_type`]) as one
    /// undo step. False if nothing changed or the new name is invalid.
    pub fn change_variable(&mut self, at: usize, f: impl FnOnce(&mut Variable)) -> bool {
        let Some(old) = self.doc.ds().try_var(VarId(at)).cloned() else {
            return false;
        };
        let mut new = old.clone();
        f(&mut new);
        new.var_type = old.var_type;
        if format!("{new:?}") == format!("{old:?}") {
            return false;
        }
        if new.name != old.name
            && let Some(e) = self.name_error(Some(at), &new.name)
        {
            self.push_message(Message::warning("Invalid Variable Name", e));
            return false;
        }
        self.apply_or_message(Some(EditOp::ChangeVarProp {
            at,
            old: Box::new(old),
            new: Box::new(new),
            cols: None,
            weight: false,
        }))
    }

    /// Variable Type dialog: make variable `at` numeric with `width`/`decimals`, or a string
    /// with `width` characters. Values are converted (`Dataset::set_type`); how many were lost
    /// or cut is reported in a message. One undo step that restores the column exactly.
    pub fn set_variable_type(
        &mut self,
        at: usize,
        t: VarType,
        width: usize,
        decimals: usize,
    ) -> bool {
        let ds = self.doc.ds();
        let Some(old) = ds.try_var(VarId(at)).cloned() else {
            return false;
        };
        if old.var_type == VarType::Numeric && t == VarType::Numeric {
            return self.change_variable(at, |v| {
                v.width = width;
                v.decimals = decimals;
                v.format_type = None;
            });
        }
        if old.var_type == t {
            return false;
        }
        let weight = ds.weight() == Some(VarId(at));
        let old_col = ds.column(VarId(at)).clone();
        let lost = match self.doc.ds_mut().set_type(VarId(at), t) {
            Ok(n) => n,
            Err(e) => {
                self.push_message(Message::error(e.to_string()));
                return false;
            }
        };
        let ds = self.doc.ds_mut();
        if t == VarType::Numeric {
            let v = ds.var_mut(VarId(at));
            v.width = width;
            v.decimals = decimals;
        }
        let new = ds.var(VarId(at)).clone();
        let new_col = ds.column(VarId(at)).clone();
        self.record(EditOp::ChangeVarProp {
            at,
            old: Box::new(old.clone()),
            new: Box::new(new),
            cols: Some(Box::new((old_col, new_col))),
            weight,
        });
        if lost > 0 {
            let text = match (old.var_type, t) {
                (VarType::String { .. }, VarType::Numeric) => format!(
                    "{lost} value(s) of {} could not be converted to numbers and were set to \
                     system-missing.",
                    old.name
                ),
                _ => format!("{lost} value(s) of {} were truncated.", old.name),
            };
            self.push_message(Message::warning("Variable Type", text));
        }
        true
    }

    /// Missing values of a numeric variable / discrete missing strings of a string variable.
    pub fn set_missing(&mut self, at: usize, spec: MissingSpec, strings: Vec<String>) -> bool {
        self.change_variable(at, |v| {
            v.missing = spec;
            v.string_missing = strings;
        })
    }

    // ----- acting on the selection (Edit menu, context menus) -----

    /// Edit > Insert Variable: before the selected variable (Data View column or Variable View
    /// row).
    pub fn insert_variable_here(&mut self) -> bool {
        let at = match self.editor.view {
            View::Variable => self.editor.var_row,
            View::Data => {
                let ds = self.doc.ds();
                let (_, cols) = self.editor.block(ds.n_cases(), ds.n_vars());
                cols.start
            }
        };
        self.insert_variable(at)
    }

    /// Edit > Insert Cases: one blank case before the selected case.
    pub fn insert_cases_here(&mut self) -> bool {
        let ds = self.doc.ds();
        let (rows, _) = self.editor.block(ds.n_cases(), ds.n_vars());
        self.insert_cases(rows.start, 1)
    }

    /// Edit > Clear: selected cells become missing; selected variables or cases are deleted.
    /// In the Variable View the selected variable is deleted.
    pub fn clear_selection(&mut self) -> bool {
        let ds = self.doc.ds();
        if self.editor.view == View::Variable {
            let r = self.editor.var_row;
            return self.delete_variables(r..r + 1);
        }
        let (rows, cols) = self.editor.block(ds.n_cases(), ds.n_vars());
        match self.editor.sel_kind {
            SelKind::Cells => self.clear_cells(rows, cols),
            SelKind::Vars => self.delete_variables(cols),
            SelKind::Cases => self.delete_cases(rows),
        }
    }

    /// Text of the Data View selection for the clipboard.
    pub fn copy_selection(&self) -> String {
        let ds = self.doc.ds();
        let (rows, cols) = self.editor.block(ds.n_cases(), ds.n_vars());
        self.copy_text(rows, cols)
    }

    /// Paste at the top-left cell of the Data View selection.
    pub fn paste_at_cursor(&mut self, text: &str) -> bool {
        let ds = self.doc.ds();
        let (rows, cols) = self.editor.block(ds.n_cases(), ds.n_vars());
        self.paste_text(rows.start, cols.start, text)
    }
}
