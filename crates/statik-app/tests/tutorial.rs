//! The student tutorial end to end (egui_kittest): open skripsi60.csv, Linear Regression with
//! collinearity, Durbin-Watson and saved residuals, 1-Sample K-S on the residuals, Compute
//! ABS_RES = ABS(RES_1), the Glejser regression of ABS_RES, Save As .sav, exit.
mod common;
use egui::accesskit::Role;
use egui::{ViewportCommand, ViewportEvent, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use statik_app::app::PickRequest;
use statik_app::menu::MenuAction;
use statik_app::state::MessageKind;
use statik_app::{AppState, StatikApp};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel)
}

fn node<'a>(h: &'a Harness<'_, StatikApp>, label: &'a str) -> egui_kittest::Node<'a> {
    let all: Vec<_> = h.query_all_by_label(label).collect();
    assert!(!all.is_empty(), "no node labelled {label:?}");
    let widgets: Vec<_> = all
        .iter()
        .filter(|n| n.accesskit_node().role() != Role::Label)
        .cloned()
        .collect();
    widgets
        .last()
        .cloned()
        .unwrap_or_else(|| *all.last().unwrap())
}

fn click(h: &mut Harness<'_, StatikApp>, label: &str) {
    node(h, label).click();
    h.run();
}

fn open_menu(h: &mut Harness<'_, StatikApp>, top: &str, path: &[&str]) {
    h.get_by_label(top).click();
    h.run();
    for (i, p) in path.iter().enumerate() {
        if i + 1 < path.len() {
            h.get_by_label(p).hover();
        } else {
            h.get_by_label(p).click();
        }
        h.run();
    }
}

fn move_var(h: &mut Harness<'_, StatikApp>, var: &str, target: &str) {
    click(h, var);
    click(h, &format!("Move to {target}"));
}

/// The text field labelled `field` (the last one: dialogs are drawn last).
fn text_field<'a>(h: &'a Harness<'_, StatikApp>, field: &'a str) -> egui_kittest::Node<'a> {
    h.query_all_by_label(field)
        .rfind(|n| {
            matches!(
                n.accesskit_node().role(),
                Role::TextInput | Role::MultilineTextInput
            )
        })
        .unwrap_or_else(|| panic!("no text field labelled {field:?}"))
}

fn type_into(h: &mut Harness<'_, StatikApp>, field: &str, text: &str) {
    text_field(h, field).click();
    h.run();
    text_field(h, field).type_text(text);
    h.run();
}

/// Click OK, run the job, close the Output window so it does not cover the next dialog.
fn ok_and_wait(h: &mut Harness<'_, StatikApp>) {
    let before = h.state().state.outputs.len();
    click(h, "OK");
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    let st = &h.state().state;
    assert!(
        st.dialogs.open.is_none(),
        "dialog still open: {:?}",
        st.messages
    );
    assert_eq!(st.outputs.len(), before + 1, "{:?}", st.messages);
    assert!(st.messages.is_empty(), "{:?}", st.messages);
    h.state_mut().state.show_output = false;
    h.run();
}

#[test]
fn student_tutorial_end_to_end() {
    let _render = common::render_lock();
    let dir = std::env::temp_dir().join(format!("statik-tutorial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let saved = dir.join("skripsi_hasil.sav");

    let answers: Arc<Mutex<VecDeque<PathBuf>>> = Arc::new(Mutex::new(
        vec![fixture("data/skripsi60.csv"), saved.clone()].into(),
    ));
    let asked: Arc<Mutex<Vec<PickRequest>>> = Arc::default();
    let (a, q) = (answers.clone(), asked.clone());
    let app = StatikApp::new(AppState::new()).with_picker(Box::new(move |r: &PickRequest| {
        q.lock().unwrap().push(r.clone());
        a.lock().unwrap().pop_front()
    }));
    let mut h = Harness::builder()
        .with_size(egui::vec2(1024.0, 760.0))
        .with_theme(egui::Theme::Light)
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(120)
        .build_ui_state(|ui, app: &mut StatikApp| app.show(ui), app);
    h.run();

    // 1. File ▸ Open ▸ Data: skripsi60.csv.
    let ctx = h.ctx.clone();
    h.state_mut().handle(MenuAction::OpenData, &ctx);
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    assert_eq!(h.state().state.doc.ds().n_cases(), 60);
    assert!(h.state().state.messages.is_empty());

    // 2. Analyze ▸ Regression ▸ Linear: y on x1 x2 x3, Collinearity diagnostics, Durbin-Watson,
    // Save ▸ Unstandardized residuals.
    open_menu(&mut h, "Analyze", &["Regression", "Linear..."]);
    move_var(&mut h, "y", "Dependent");
    for v in ["x1", "x2", "x3"] {
        move_var(&mut h, v, "Independent(s)");
    }
    click(&mut h, "Statistics...");
    click(&mut h, "Collinearity diagnostics");
    click(&mut h, "Durbin-Watson");
    click(&mut h, "Continue");
    click(&mut h, "Save...");
    h.get_all_by_label("Unstandardized").nth(1).unwrap().click();
    h.run();
    click(&mut h, "Continue");
    ok_and_wait(&mut h);
    assert!(h.state().state.doc.ds().find("RES_1").is_some());

    // 3. Analyze ▸ Nonparametric Tests ▸ Legacy Dialogs ▸ 1-Sample K-S on RES_1.
    open_menu(
        &mut h,
        "Analyze",
        &["Nonparametric Tests", "Legacy Dialogs", "1-Sample K-S..."],
    );
    move_var(
        &mut h,
        "Unstandardized Residual [RES_1]",
        "Test Variable List",
    );
    ok_and_wait(&mut h);

    // 4. Transform ▸ Compute Variable: ABS_RES = ABS(RES_1).
    open_menu(&mut h, "Transform", &["Compute Variable..."]);
    type_into(&mut h, "Target Variable:", "ABS_RES");
    type_into(&mut h, "Numeric Expression:", "ABS(RES_1)");
    ok_and_wait(&mut h);
    assert!(h.state().state.doc.ds().find("ABS_RES").is_some());

    // 5. Regression of ABS_RES on x1 x2 x3 (Glejser), nothing saved.
    open_menu(&mut h, "Analyze", &["Regression", "Linear..."]);
    click(&mut h, "Reset");
    move_var(&mut h, "ABS_RES", "Dependent");
    for v in ["x1", "x2", "x3"] {
        move_var(&mut h, v, "Independent(s)");
    }
    let n_vars = h.state().state.doc.ds().n_vars();
    ok_and_wait(&mut h);
    assert_eq!(h.state().state.doc.ds().n_vars(), n_vars, "nothing saved");

    // 6. File ▸ Save As: a .sav file.
    h.state_mut().handle(MenuAction::SaveAs, &ctx);
    h.step();
    h.state_mut().wait_for_job();
    h.run();
    assert!(!h.state().state.doc.dirty());
    assert!(saved.exists());

    let st = &h.state().state;
    let titles: Vec<&str> = st.outputs.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Log",
            "Regression",
            "NPar Tests",
            "Log",
            "Regression",
            "Log"
        ]
    );
    assert!(st.outputs[5].log.starts_with("SAVE OUTFILE="));
    assert!(
        st.messages
            .iter()
            .all(|m| !matches!(m.kind, MessageKind::Error | MessageKind::Warning)),
        "{:?}",
        st.messages
    );
    assert!(st.messages.is_empty(), "{:?}", st.messages);
    assert_eq!(asked.lock().unwrap().len(), 2);

    // The saved file reopens with the new variables.
    let (ds, _) = sav_io::read_sav(&saved).unwrap();
    assert!(ds.find("RES_1").is_some());
    assert!(ds.find("ABS_RES").is_some());
    assert_eq!(ds.n_cases(), 60);

    // 7. Close the window: the data is saved, the output is not: answer No.
    h.input_mut()
        .viewports
        .entry(ViewportId::ROOT)
        .or_default()
        .events
        .push(ViewportEvent::Close);
    h.step();
    let cmds = h
        .output()
        .viewport_output
        .get(&ViewportId::ROOT)
        .map(|v| v.commands.clone())
        .unwrap_or_default();
    assert!(cmds.contains(&ViewportCommand::CancelClose));
    h.run();
    assert!(h.state().save_prompt().is_none(), "the data is saved");
    h.get_by_label("Save contents of output viewer to Output1?");
    click(&mut h, "No");
    assert!(h.state().closing());
    let _ = std::fs::remove_dir_all(&dir);
}
