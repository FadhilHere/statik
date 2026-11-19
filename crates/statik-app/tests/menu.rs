mod common;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use statik_app::StatikApp;
use statik_app::menu::TOP_MENUS;
use std::path::Path;

fn harness() -> Harness<'static, StatikApp> {
    Harness::builder()
        .with_size(egui::vec2(1024.0, 700.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(|ui, app: &mut StatikApp| app.show(ui), StatikApp::default())
}

/// Labels of all buttons on screen, in tree order.
fn button_labels(h: &Harness<'_, StatikApp>) -> Vec<String> {
    h.root()
        .children_recursive()
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .collect()
}

#[test]
fn rendered_menu_bar_is_exactly_the_spss_menus() {
    let _render = common::render_lock();
    // With no menu open, the only buttons on screen are the top-level menus (exactly these 11)
    // and the Data View / Variable View tabs at the bottom.
    let h = harness();
    assert_eq!(
        button_labels(&h),
        [
            "File",
            "Edit",
            "View",
            "Data",
            "Transform",
            "Analyze",
            "Graphs",
            "Utilities",
            "Extensions",
            "Window",
            "Help",
            "Data View",
            "Variable View"
        ]
    );
}

#[test]
fn queued_job_is_started_by_the_app_and_gates_the_ui() {
    let _render = common::render_lock();
    let mut h = harness();
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/skripsi60.csv");
    h.state_mut()
        .state
        .apply(statik_app::Command::Open(p))
        .unwrap();
    // A view with only `&mut AppState` queues a job; a barrier keeps it running while we look.
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let job = h.state().state.procedure_job(
        "Slow",
        move |_| {
            let _ = rx.recv();
            statik_app::ProcOutput::output(vec![])
        },
        "SLOW.".into(),
    );
    assert!(h.state_mut().state.queue_job(job));
    assert_eq!(h.state().busy(), Some("Running Slow..."));
    h.step();
    assert!(
        h.state().state.pending_job.is_none(),
        "app did not start it"
    );
    assert_eq!(h.state().busy(), Some("Running Slow..."));
    h.step();
    // Busy: menus and the main area are disabled, the status bar shows the job.
    assert!(h.get_by_label("File").accesskit_node().is_disabled());
    h.get_by_label("Running Slow...");
    tx.send(()).unwrap();
    h.state_mut().wait_for_job();
    h.run();
    assert!(h.state().busy().is_none());
    assert_eq!(h.state().state.outputs.last().unwrap().title, "Slow");
    // The output window (with its own File menu) is open now; the Data Editor's comes first.
    let file = h.get_all_by_label("File").next().unwrap();
    assert!(!file.accesskit_node().is_disabled());
}

#[test]
fn exit_sends_close_once_and_resets() {
    let _render = common::render_lock();
    let mut h = harness();
    h.state_mut().state.exit_requested = true;
    h.step();
    assert!(!h.state().state.exit_requested);
}

#[test]
fn top_level_menus_in_spss_order() {
    let _render = common::render_lock();
    let h = harness();
    let mut xs = Vec::new();
    for label in TOP_MENUS {
        let node = h.get_by_label(label);
        assert!(!node.accesskit_node().is_disabled(), "{label} disabled");
        xs.push(node.rect().left());
    }
    assert_eq!(
        TOP_MENUS,
        [
            "File",
            "Edit",
            "View",
            "Data",
            "Transform",
            "Analyze",
            "Graphs",
            "Utilities",
            "Extensions",
            "Window",
            "Help"
        ]
    );
    assert!(
        xs.windows(2).all(|w| w[0] < w[1]),
        "menus not left-to-right: {xs:?}"
    );
    // The main area shows the empty Data View, the status bar the idle processor.
    h.get_by_label("Data View");
    assert!(h.query_all_by_label("var").next().is_some());
    h.get_by_label("Statik Processor is ready");
}

#[test]
fn analyze_regression_linear_enabled_glm_disabled() {
    let _render = common::render_lock();
    let mut h = harness();
    h.get_by_label("Analyze").click();
    h.run();
    assert!(
        h.get_by_label("General Linear Model")
            .accesskit_node()
            .is_disabled()
    );
    assert!(h.get_by_label("Reports").accesskit_node().is_disabled());
    h.get_by_label("Regression").hover();
    h.run();
    let linear = h.get_by_label("Linear...");
    assert!(!linear.accesskit_node().is_disabled());
    assert!(
        h.get_by_label("Curve Estimation...")
            .accesskit_node()
            .is_disabled()
    );
    linear.click();
    h.run();
    // Opens the Linear Regression dialog.
    assert_eq!(
        h.state().state.dialogs.open,
        Some(statik_app::DialogKind::LinearRegression)
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().state.dialogs.open.is_none());
}

#[test]
fn file_menu_items() {
    let _render = common::render_lock();
    let mut h = harness();
    h.get_by_label("File").click();
    h.run();
    for l in ["Save", "Save As...", "Exit"] {
        assert!(!h.get_by_label(l).accesskit_node().is_disabled(), "{l}");
    }
    assert!(h.get_by_label("Close").accesskit_node().is_disabled());
    h.get_by_label("Import Data").hover();
    h.run();
    assert!(!h.get_by_label("CSV Data...").accesskit_node().is_disabled());
    assert!(!h.get_by_label("Excel...").accesskit_node().is_disabled());
    assert!(h.get_by_label("Stata...").accesskit_node().is_disabled());
}

#[test]
fn edit_undo_disabled_view_value_labels_toggles() {
    let _render = common::render_lock();
    let mut h = harness();
    h.get_by_label("Edit").click();
    h.run();
    assert!(h.get_by_label("Undo").accesskit_node().is_disabled());
    assert!(h.get_by_label("Redo").accesskit_node().is_disabled());
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Value Labels").click();
    h.run();
    assert!(h.state().state.value_labels);
}

#[test]
fn help_about_and_window_output() {
    let _render = common::render_lock();
    let mut h = harness();
    h.get_by_label("Help").click();
    h.run();
    h.get_by_label("About Statik").click();
    h.run();
    assert_eq!(h.state().state.message().unwrap().title, "About Statik");
    h.get_by_label("OK").click();
    h.run();
    h.get_by_label("Window").click();
    h.run();
    h.get_by_label("Output").click();
    h.run();
    assert!(h.state().state.show_output);
}

#[test]
fn open_via_job_updates_main_area_and_blocks_while_busy() {
    let _render = common::render_lock();
    let mut h = harness();
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/skripsi60.csv");
    let ctx = h.ctx.clone();
    h.state_mut().dispatch(statik_app::Command::Open(p), &ctx);
    assert!(h.state().busy().is_some());
    // A second command while busy is ignored.
    h.state_mut()
        .dispatch(statik_app::Command::ToggleValueLabels, &ctx);
    assert!(!h.state().state.value_labels);
    h.state_mut().wait_for_job();
    h.run();
    assert!(h.state().busy().is_none());
    // The Data View shows the opened data.
    h.get_by_label("grp");
    assert_eq!(h.state().state.doc.ds().n_cases(), 60);
    assert!(
        h.state()
            .state
            .title()
            .starts_with("skripsi60.csv [DataSet1]")
    );
}

#[test]
fn error_shows_message_box() {
    let _render = common::render_lock();
    let mut h = harness();
    let ctx = h.ctx.clone();
    h.state_mut()
        .dispatch(statik_app::Command::Open("does/not/exist.sav".into()), &ctx);
    h.state_mut().wait_for_job();
    h.run();
    h.get_by_label("Error");
    h.get_by_label("OK").click();
    h.run();
    assert!(h.state().state.message().is_none());
}

#[test]
fn icon_is_white_s_on_blue() {
    let _render = common::render_lock();
    use statik_app::icon::{ICON_SIZE, icon_rgba};
    let px = icon_rgba(ICON_SIZE);
    assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    let at = |x: u32, y: u32| &px[((y * ICON_SIZE + x) * 4) as usize..][..4];
    assert_eq!(at(0, 0)[3], 0, "corner transparent");
    assert_eq!(at(20, 128), &[0x1f, 0x3a, 0x6b, 255], "blue background");
    assert_eq!(
        at(128, 40 + 14),
        &[255, 255, 255, 255],
        "top stroke of the S is white"
    );
    assert_eq!(
        at(128, 128),
        &[255, 255, 255, 255],
        "middle stroke of the S is white"
    );
    // Write/compare the checked-in asset.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.png");
    let img = image::RgbaImage::from_raw(ICON_SIZE, ICON_SIZE, px).unwrap();
    if path.exists() {
        let saved = image::open(&path).unwrap().to_rgba8();
        assert!(
            saved == img,
            "assets/icon.png is stale; delete it and rerun this test"
        );
    } else {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        img.save(&path).unwrap();
    }
}

/// Snapshot of the empty main window (rendered with wgpu; needs a GPU or software adapter).
#[test]
fn snapshot_empty_window() {
    let _render = common::render_lock();
    let mut h = harness();
    h.run();
    h.snapshot("empty_window");
}
