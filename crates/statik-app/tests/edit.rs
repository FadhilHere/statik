//! Data editor logic without a window: edit operations, undo/redo, typing into empty cells,
//! clipboard paste/copy, variable properties.
use dataset::{Column, MissingSpec, OrderedValue, VarId, VarType, Variable};
use statik_app::edit::{CellValue, EditOp, UNDO_LIMIT, display_value, next_var_name};
use statik_app::editor::{SelKind, View};
use statik_app::{AppState, Command};
use std::path::Path;

fn fixture(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn skripsi() -> AppState {
    let mut s = AppState::new();
    s.apply(Command::Open(fixture("data/skripsi60.csv")))
        .unwrap();
    s
}

fn small() -> AppState {
    let mut s = AppState::new();
    let ds = s.doc.ds_mut();
    ds.add_variable(Variable::numeric("a"), Column::Numeric(vec![1.0, 2.0]))
        .unwrap();
    ds.add_variable(
        Variable::string("s", 4),
        Column::Text(vec!["x".into(), "y".into()]),
    )
    .unwrap();
    s
}

fn num(s: &AppState, var: usize) -> Vec<f64> {
    s.doc.ds().numeric(VarId(var)).to_vec()
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x == y || (x.is_nan() && y.is_nan()))
}

#[test]
fn set_cell_undo_redo() {
    let mut s = skripsi();
    assert!(!s.can_undo() && !s.can_redo());
    let old = num(&s, 0)[0];
    assert!(s.set_cell_text(0, 0, "99.5"));
    assert_eq!(num(&s, 0)[0], 99.5);
    assert!(s.can_undo() && !s.can_redo());
    let rev = s.doc.rev();
    s.apply(Command::Undo).unwrap();
    assert_eq!(num(&s, 0)[0], old);
    assert!(s.doc.rev() > rev, "undo is a change");
    assert!(s.can_redo() && !s.can_undo());
    s.apply(Command::Redo).unwrap();
    assert_eq!(num(&s, 0)[0], 99.5);
    // A new edit clears the redo stack.
    s.apply(Command::Undo).unwrap();
    assert!(s.set_cell_text(1, 0, "1"));
    assert!(!s.can_redo());
}

#[test]
fn system_missing_and_invalid_numbers() {
    let mut s = skripsi();
    let old = num(&s, 0)[2];
    assert!(s.set_cell_text(2, 0, ""));
    assert!(num(&s, 0)[2].is_nan());
    assert_eq!(display_value(s.doc.ds(), 0, 2, false), ".");
    assert!(s.set_cell_text(2, 0, "3,5"), "comma decimal accepted");
    assert_eq!(num(&s, 0)[2], 3.5);
    let n_undo = s.undo_len();
    assert!(!s.set_cell_text(2, 0, "abc"));
    assert_eq!(num(&s, 0)[2], 3.5, "old value kept");
    assert_eq!(s.undo_len(), n_undo);
    assert!(s.message().unwrap().text.contains("abc"));
    let _ = old;
}

#[test]
fn display_uses_decimals_and_value_labels() {
    let mut s = small();
    {
        let ds = s.doc.ds_mut();
        ds.var_mut(VarId(0)).decimals = 1;
        ds.var_mut(VarId(0))
            .value_labels
            .insert(OrderedValue::new(2.0), "Two".into());
    }
    assert_eq!(display_value(s.doc.ds(), 0, 0, false), "1.0");
    assert_eq!(display_value(s.doc.ds(), 0, 1, false), "2.0");
    assert_eq!(display_value(s.doc.ds(), 0, 1, true), "Two");
    assert_eq!(display_value(s.doc.ds(), 0, 0, true), "1.0");
    assert_eq!(display_value(s.doc.ds(), 1, 1, true), "y");
    // Typing a label with Value Labels on enters its value.
    s.value_labels = true;
    assert!(s.set_cell_text(0, 0, "two"));
    assert_eq!(num(&s, 0)[0], 2.0);
}

#[test]
fn typing_in_empty_var_column_creates_var00001() {
    let mut s = small();
    assert_eq!(next_var_name(s.doc.ds()), "VAR00001");
    // Column 2 is the first grey "var" column.
    assert!(s.set_cell_text(0, 2, "7"));
    let ds = s.doc.ds();
    assert_eq!(ds.n_vars(), 3);
    let v = ds.var(VarId(2));
    assert_eq!(v.name, "VAR00001");
    assert_eq!(v.var_type, VarType::Numeric);
    assert_eq!((v.width, v.decimals), (8, 2));
    assert_eq!(num(&s, 2)[0], 7.0);
    assert!(num(&s, 2)[1].is_nan());
    // Two columns further right: the skipped column is created too, as SPSS does.
    assert!(s.set_cell_text(1, 4, "1"));
    assert_eq!(s.doc.ds().n_vars(), 5);
    assert_eq!(s.doc.ds().var(VarId(3)).name, "VAR00002");
    assert_eq!(s.doc.ds().var(VarId(4)).name, "VAR00003");
    // One undo step removes both.
    assert!(s.undo());
    assert_eq!(s.doc.ds().n_vars(), 3);
    assert!(s.undo());
    assert_eq!(s.doc.ds().n_vars(), 2);
    // Invalid text into a new variable creates nothing.
    assert!(!s.set_cell_text(0, 2, "abc"));
    assert_eq!(s.doc.ds().n_vars(), 2);
}

#[test]
fn typing_below_last_case_appends_cases() {
    let mut s = small();
    assert!(s.set_cell_text(4, 1, "zz"));
    let ds = s.doc.ds();
    assert_eq!(ds.n_cases(), 5);
    assert!(ds.numeric(VarId(0))[2..].iter().all(|v| v.is_nan()));
    let Column::Text(t) = ds.column(VarId(1)) else {
        panic!()
    };
    assert_eq!(t, &["x", "y", "", "", "zz"]);
    assert_eq!(display_value(ds, 0, 3, false), ".");
    s.undo();
    assert_eq!(s.doc.ds().n_cases(), 2);
    // Strings are cut to the storage width.
    assert!(s.set_cell_text(0, 1, "abcdefg"));
    assert_eq!(display_value(s.doc.ds(), 1, 0, false), "abcd");
}

#[test]
fn typing_into_empty_dataset() {
    let mut s = AppState::new();
    assert!(s.set_cell_text(2, 1, "5"));
    let ds = s.doc.ds();
    assert_eq!((ds.n_vars(), ds.n_cases()), (2, 3));
    assert_eq!(ds.var(VarId(0)).name, "VAR00001");
    assert_eq!(num(&s, 1)[2], 5.0);
    s.undo();
    assert_eq!((s.doc.ds().n_vars(), s.doc.ds().n_cases()), (0, 0));
    s.redo();
    assert_eq!(num(&s, 1)[2], 5.0);
}

#[test]
fn paste_block_fills_cells_and_adds_cases() {
    let mut s = small();
    // 3 rows x 2 columns from Excel (CRLF, trailing newline), starting at row 1, column 0.
    assert!(s.paste_text(1, 0, "10\tp\r\n11\tq\r\n12\tr\r\n"));
    let ds = s.doc.ds();
    assert_eq!(ds.n_cases(), 4);
    assert_eq!(num(&s, 0), vec![1.0, 10.0, 11.0, 12.0]);
    let Column::Text(t) = ds.column(VarId(1)) else {
        panic!()
    };
    assert_eq!(t, &["x", "p", "q", "r"]);
    // One undo step for the whole paste.
    assert!(s.undo());
    assert_eq!(s.doc.ds().n_cases(), 2);
    assert_eq!(num(&s, 0), vec![1.0, 2.0]);
    assert!(!s.can_undo());
}

#[test]
fn paste_into_new_columns_and_rejects_bad_numbers() {
    let mut s = small();
    assert!(s.paste_text(0, 1, "a\t1,5\nb\t."));
    assert_eq!(s.doc.ds().n_vars(), 3);
    assert_eq!(s.doc.ds().var(VarId(2)).name, "VAR00001");
    assert!(same(&num(&s, 2), &[1.5, f64::NAN]));
    let before = format!("{:?}", s.doc.ds());
    let undo = s.undo_len();
    // Text into a numeric column rejects the whole paste.
    assert!(!s.paste_text(0, 0, "5\nnope\n7\n8"));
    assert_eq!(format!("{:?}", s.doc.ds()), before);
    assert_eq!(s.undo_len(), undo);
    assert!(s.message().unwrap().text.contains("nope"));
}

#[test]
fn copy_block_is_tab_separated() {
    let mut s = small();
    s.set_cell_text(1, 0, "");
    assert_eq!(s.copy_text(0..2, 0..2), "1.00\tx\r\n\ty\r\n");
    // What is copied pastes back unchanged.
    let copied = s.copy_text(0..2, 0..2);
    assert!(s.paste_text(2, 0, &copied));
    assert!(same(&num(&s, 0), &[1.0, f64::NAN, 1.0, f64::NAN]));
}

#[test]
fn rename_invalid_keeps_state_and_shows_error() {
    let mut s = small();
    let rev = s.doc.rev();
    for bad in ["1abc", "a b", "ALL", "", "S"] {
        assert!(!s.rename_variable(0, bad), "{bad}");
        assert_eq!(s.doc.ds().var(VarId(0)).name, "a");
        assert!(s.message().is_some(), "{bad}: no message");
        s.dismiss_message();
    }
    assert_eq!(s.doc.rev(), rev);
    assert!(!s.can_undo());
    assert!(s.rename_variable(0, "income"));
    assert_eq!(s.doc.ds().var(VarId(0)).name, "income");
    s.undo();
    assert_eq!(s.doc.ds().var(VarId(0)).name, "a");
}

#[test]
fn type_change_reports_lost_values_and_undoes_exactly() {
    let mut s = small();
    s.set_cell_text(0, 1, "3.25");
    let before = format!("{:?}", s.doc.ds());
    // String -> numeric: "y" cannot be converted.
    assert!(s.set_variable_type(1, VarType::Numeric, 8, 2));
    assert_eq!(s.doc.ds().var(VarId(1)).var_type, VarType::Numeric);
    assert!(same(&num(&s, 1), &[3.25, f64::NAN]));
    let m = s.message().unwrap();
    assert!(m.text.contains('1'), "{m:?}");
    s.dismiss_message();
    s.undo();
    assert_eq!(format!("{:?}", s.doc.ds()), before);
    s.redo();
    assert_eq!(s.doc.ds().var(VarId(1)).var_type, VarType::Numeric);
    // Numeric width/decimals only change the format.
    assert!(s.set_variable_type(0, VarType::Numeric, 10, 3));
    let v = s.doc.ds().var(VarId(0));
    assert_eq!((v.width, v.decimals), (10, 3));
}

#[test]
fn variable_properties_undo() {
    let mut s = small();
    assert!(s.change_variable(0, |v| {
        v.label = Some("Age".into());
        v.missing = MissingSpec::Discrete(vec![9.0]);
    }));
    assert_eq!(s.doc.ds().var(VarId(0)).label.as_deref(), Some("Age"));
    s.undo();
    assert_eq!(s.doc.ds().var(VarId(0)).label, None);
    assert_eq!(s.doc.ds().var(VarId(0)).missing, MissingSpec::None);
    // No change: nothing recorded.
    assert!(!s.change_variable(0, |_| {}));
    assert!(!s.can_undo());
}

#[test]
fn insert_and_delete_variables_and_cases() {
    let mut s = small();
    s.doc.ds_mut().set_weight(Some(VarId(0))).unwrap();
    let before = format!("{:?}", s.doc.ds());
    s.insert_variable(1);
    assert_eq!(s.doc.ds().var(VarId(1)).name, "VAR00001");
    s.insert_cases(1, 2);
    assert_eq!(s.doc.ds().n_cases(), 4);
    s.delete_variables(0..2);
    assert_eq!(s.doc.ds().n_vars(), 1);
    s.delete_cases(0..4);
    assert_eq!(s.doc.ds().n_cases(), 0);
    for _ in 0..4 {
        assert!(s.undo());
    }
    assert_eq!(
        format!("{:?}", s.doc.ds()),
        before,
        "weight and values restored"
    );
    s.clear_cells(0..2, 0..2);
    assert!(num(&s, 0).iter().all(|v| v.is_nan()));
    s.undo();
    assert_eq!(format!("{:?}", s.doc.ds()), before);
}

#[test]
fn edit_op_apply_revert_roundtrip() {
    let mut s = small();
    let ds0 = format!("{:?}", s.doc.ds());
    let op = EditOp::SetCell {
        var: 0,
        row: 1,
        old: CellValue::Num(2.0),
        new: CellValue::Num(5.0),
    };
    let ds = s.doc.ds_mut();
    op.apply(ds).unwrap();
    assert_eq!(ds.numeric(VarId(0))[1], 5.0);
    op.revert(ds).unwrap();
    assert_eq!(format!("{ds:?}"), ds0);
}

#[test]
fn undo_stack_is_capped() {
    let mut s = small();
    for i in 0..UNDO_LIMIT + 20 {
        assert!(s.set_cell_text(0, 0, &i.to_string()));
    }
    assert_eq!(s.undo_len(), UNDO_LIMIT);
    while s.undo() {}
    assert_eq!(num(&s, 0)[0], 19.0, "oldest 20 steps were dropped");
}

#[test]
fn open_and_new_clear_undo() {
    let mut s = small();
    s.set_cell_text(0, 0, "5");
    s.apply(Command::New).unwrap();
    assert!(!s.can_undo());
    s.set_cell_text(0, 0, "5");
    s.apply(Command::Open(fixture("data/skripsi60.csv")))
        .unwrap();
    assert!(!s.can_undo() && !s.can_redo());
}

#[test]
fn selection_commands() {
    let mut s = small();
    // Cells selection: Clear sets missing.
    s.editor.select_cells((0, 0), (1, 0));
    s.apply(Command::Clear).unwrap();
    assert!(num(&s, 0).iter().all(|v| v.is_nan()));
    // Variable selection: Clear deletes variables; Insert Variable inserts before it.
    s.editor.select_vars(1, 1);
    s.apply(Command::InsertVariable).unwrap();
    assert_eq!(s.doc.ds().var(VarId(1)).name, "VAR00001");
    s.editor.select_vars(1, 1);
    s.apply(Command::Clear).unwrap();
    assert_eq!(s.doc.ds().var(VarId(1)).name, "s");
    // Case selection: Insert Cases inserts before it; Clear deletes the cases.
    s.editor.select_cases(1, 1);
    s.apply(Command::InsertCases).unwrap();
    assert_eq!(s.doc.ds().n_cases(), 3);
    s.editor.select_cases(0, 1);
    assert_eq!(s.editor.sel_kind, SelKind::Cases);
    s.apply(Command::Clear).unwrap();
    assert_eq!(s.doc.ds().n_cases(), 1);
    // Variable View: Clear deletes the selected variable row.
    s.editor.view = View::Variable;
    s.editor.var_row = 0;
    s.apply(Command::Clear).unwrap();
    assert_eq!(s.doc.ds().n_vars(), 1);
    // Copy / paste at the cursor.
    s.editor.view = View::Data;
    s.editor.select_cells((0, 0), (0, 0));
    assert_eq!(s.copy_selection(), s.copy_text(0..1, 0..1));
    assert!(s.paste_at_cursor("q\nr"));
    let Column::Text(t) = s.doc.ds().column(VarId(0)) else {
        panic!()
    };
    assert_eq!(t, &["q", "r"]);
}
