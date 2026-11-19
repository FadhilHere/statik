//! The Output viewer (kittest): regression output, outline, charts, context menu, copy,
//! save/open output.
mod common;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use output_model::OutputItem;
use procedures::regression::{RegressionOptions, SaveFlags, run_regression};
use statik_app::output::chart_view::histogram_legend;
use statik_app::output::{ItemRef, OUTPUT_TITLE, output_ui};
use statik_app::{AppState, Command, ProcOutput, StatikApp, run_procedure};
use std::path::Path;
use std::sync::Mutex;

/// Tests that write the real Windows clipboard hold this lock.
static CLIP: Mutex<()> = Mutex::new(());

fn skripsi() -> AppState {
    let mut s = AppState::new();
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data/skripsi60.csv");
    s.apply(Command::Open(p)).unwrap();
    s
}

/// skripsi60: y on x1 x2 x3 with all three residual plots, run like the dialog will.
fn regression_state() -> AppState {
    let mut s = skripsi();
    let ds = s.doc.ds();
    let opt = RegressionOptions {
        dependent: ds.find("y").unwrap(),
        independents: ["x1", "x2", "x3"]
            .iter()
            .map(|n| ds.find(n).unwrap())
            .collect(),
        collinearity: true,
        durbin_watson: true,
        plot_hist: true,
        plot_pp: true,
        plot_zpred_sresid: true,
        save: SaveFlags::default(),
    };
    run_procedure(
        &mut s,
        "Regression",
        move |ds| ProcOutput::output(run_regression(ds, &opt)),
        "REGRESSION\n  /DEPENDENT y\n  /METHOD=ENTER x1 x2 x3.".into(),
    );
    assert!(s.message().is_none(), "{:?}", s.message());
    s
}

fn viewer(state: AppState, size: egui::Vec2) -> Harness<'static, AppState> {
    Harness::builder()
        .with_size(size)
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, s: &mut AppState| {
                output_ui(ui, s, false);
            },
            state,
        )
}

fn app(state: AppState) -> Harness<'static, StatikApp> {
    Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .with_theme(egui::Theme::Light)
        .build_ui_state(
            |ui, app: &mut StatikApp| app.show(ui),
            StatikApp::new(state),
        )
}

fn count<S>(h: &Harness<'_, S>, label: &str) -> usize {
    h.query_all_by_label(label).count()
}

#[test]
fn regression_output_page_has_tables_charts_outline_and_snapshot() {
    let _render = common::render_lock();
    let mut s = regression_state();
    // New output is selected (frame + arrow); the snapshot shows the page unselected.
    assert!(s.viewer.selected.is_some());
    s.viewer.selected = None;
    let mut h = viewer(s, egui::vec2(1100.0, 2600.0));
    h.run();
    // Outline + document both show every table title.
    for t in [
        "Variables Entered/Removed",
        "Model Summary",
        "ANOVA",
        "Coefficients",
        "Collinearity Diagnostics",
        "Residuals Statistics",
    ] {
        assert!(count(&h, t) >= 2, "{t}: outline and table");
    }
    // Procedure heading in the document, group in the outline.
    assert!(count(&h, "Regression") >= 2);
    assert!(count(&h, "Output") >= 1);
    // Table contents: nested header, row labels, numbers through format_cell.
    assert!(count(&h, "Unstandardized Coefficients") >= 1);
    assert!(count(&h, "(Constant)") >= 1);
    assert!(count(&h, "R Square") >= 1);
    // The log syntax above the procedure output.
    assert!(
        h.query_all_by_label_contains("REGRESSION").count() >= 1,
        "log syntax shown"
    );
    // Three charts, as image nodes named by their title.
    for c in [
        "Histogram",
        "Normal P-P Plot of Regression Standardized Residual",
        "Scatterplot",
    ] {
        assert!(
            h.query_all_by_role_and_label(Role::Image, c).count() == 1,
            "chart {c}"
        );
    }
    // Histogram legend as SPSS shows it at the side.
    assert!(h.query_all_by_label_contains("Std. Dev. = ").count() >= 1);
    assert!(count(&h, "N = 60") >= 1);
    h.snapshot("output_regression");
}

#[test]
fn procedure_opens_the_output_window_with_its_title() {
    let _render = common::render_lock();
    let mut h = app(skripsi());
    h.run();
    assert!(h.query_by_label(OUTPUT_TITLE).is_none());
    let s = regression_state();
    h.state_mut().state.outputs = s.outputs.clone();
    h.state_mut().state.show_output = true;
    h.run();
    assert!(
        h.query_all_by_label(OUTPUT_TITLE).count() >= 1,
        "window title"
    );
    assert!(count(&h, "Model Summary") >= 2);
    // The data editor menus and the viewer's own menus are both there.
    assert_eq!(count(&h, "Analyze"), 1, "Analyze only in the Data Editor");
    assert_eq!(count(&h, "File"), 2, "File in both windows");
}

#[test]
fn outline_click_selects_and_scrolls_to_the_item() {
    let _render = common::render_lock();
    let mut h = viewer(regression_state(), egui::vec2(1000.0, 600.0));
    h.run();
    let e = h.state().outputs.len() - 1;
    // The scatterplot is far below the fold: not drawn yet.
    assert_eq!(
        h.query_all_by_role_and_label(Role::Image, "Scatterplot")
            .count(),
        0
    );
    // Outline nodes are the first match (left panel comes first).
    h.get_all_by_label("Scatterplot").next().unwrap().click();
    h.run();
    let items = &h.state().outputs[e].items;
    let idx = items
        .iter()
        .position(|i| matches!(i, OutputItem::Chart(c) if c.title() == "Scatterplot"))
        .unwrap();
    assert_eq!(h.state().viewer.selected, Some(ItemRef::item(e, idx)));
    assert_eq!(
        h.query_all_by_role_and_label(Role::Image, "Scatterplot")
            .count(),
        1,
        "scrolled into view"
    );
}

#[test]
fn context_menu_offers_copy_copy_as_image_and_disabled_export() {
    let _render = common::render_lock();
    let mut h = viewer(regression_state(), egui::vec2(1100.0, 1400.0));
    h.run();
    // Right-click the Model Summary table (its title in the document is the 2nd match).
    h.get_all_by_label("Model Summary")
        .nth(1)
        .unwrap()
        .click_secondary();
    h.run();
    assert!(!h.get_by_label("Copy").accesskit_node().is_disabled());
    assert!(
        !h.get_by_label("Copy as Image")
            .accesskit_node()
            .is_disabled()
    );
    assert!(h.get_by_label("Export...").accesskit_node().is_disabled());
}

#[cfg(windows)]
#[test]
fn copy_from_the_context_menu_puts_the_table_on_the_clipboard() {
    let _render = common::render_lock();
    use clipboard_win::{Clipboard, raw, register_format};
    let _g = CLIP.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = viewer(regression_state(), egui::vec2(1100.0, 1400.0));
    h.run();
    h.get_all_by_label("ANOVA")
        .nth(1)
        .unwrap()
        .click_secondary();
    h.run();
    h.get_by_label("Copy").click();
    h.run();
    assert!(h.state().message().is_none(), "{:?}", h.state().message());
    let _c = Clipboard::new_attempts(20).unwrap();
    let html = register_format("HTML Format").unwrap().get();
    let mut out = Vec::new();
    raw::get_vec(html, &mut out).unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("ANOVA<sup>a</sup>"), "{s}");
    let mut txt = Vec::new();
    raw::get_string(&mut txt).unwrap();
    let txt = String::from_utf8(txt).unwrap();
    assert!(txt.starts_with("ANOVAa\r\n"), "{txt}");
    assert!(txt.contains("Regression\t"), "{txt}");
}

#[cfg(windows)]
#[test]
fn copy_as_image_puts_the_cropped_item_on_the_clipboard() {
    let _render = common::render_lock();
    use clipboard_win::{Clipboard, formats, raw};
    let _g = CLIP.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = viewer(regression_state(), egui::vec2(1100.0, 1400.0));
    h.run();
    h.get_all_by_label("Model Summary")
        .nth(1)
        .unwrap()
        .click_secondary();
    h.run();
    h.get_by_label("Copy as Image").click();
    // A few frames: the menu closes, the screenshot is taken (kittest renders it), the event
    // comes back and the crop goes to the clipboard.
    for _ in 0..8 {
        h.step();
    }
    h.run();
    assert!(h.state().message().is_none(), "{:?}", h.state().message());
    assert!(h.state().viewer.shot.is_none(), "screenshot handled");
    let e = h.state().outputs.len() - 1;
    let idx = h.state().outputs[e]
        .items
        .iter()
        .position(|i| matches!(i, OutputItem::Table(t) if t.title == "Model Summary"))
        .unwrap();
    let rect = h.state().viewer.item_rects[&ItemRef::item(e, idx)];
    let _c = Clipboard::new_attempts(20).unwrap();
    let mut dib = Vec::new();
    raw::get_vec(formats::CF_DIB, &mut dib).unwrap();
    let w = i32::from_le_bytes(dib[4..8].try_into().unwrap());
    let ht = i32::from_le_bytes(dib[8..12].try_into().unwrap());
    assert!(
        (w as f32 - rect.width()).abs() <= 2.0,
        "width {w} vs {}",
        rect.width()
    );
    assert!(
        (ht as f32 - rect.height()).abs() <= 2.0,
        "height {ht} vs {}",
        rect.height()
    );
    // Not blank: the table text and rules are in it.
    // (GlobalSize may round the buffer up: only read the pixels.)
    let px = &dib[40..40 + (w * ht * 4) as usize];
    assert!(
        px.chunks(4).any(|p| p[0] < 128 && p[1] < 128 && p[2] < 128),
        "dark pixels"
    );
}

#[test]
fn busy_disables_the_viewer() {
    let _render = common::render_lock();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(
            |ui, s: &mut AppState| {
                output_ui(ui, s, true);
            },
            regression_state(),
        );
    h.run();
    assert!(
        h.get_all_by_label("File")
            .next()
            .unwrap()
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn histogram_legend_is_mean_sd_n() {
    let _render = common::render_lock();
    let l = histogram_legend(&[1.0, 2.0, 3.0, 4.0]);
    assert_eq!(l, ["Mean = 2.50", "Std. Dev. = 1.291", "N = 4"]);
    // Residual means are ~0: plain decimals, no "-0.00".
    let l = histogram_legend(&[-1e-17, 1.0, -1.0]);
    assert_eq!(l, ["Mean = 0.00", "Std. Dev. = 1.000", "N = 3"]);
}

#[test]
fn save_and_open_output_round_trip() {
    let _render = common::render_lock();
    let mut s = regression_state();
    assert!(s.output_dirty);
    let dir = std::env::temp_dir().join(format!("statik-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The extension is added.
    let saved = s.save_output(&dir.join("hasil")).unwrap();
    assert_eq!(saved, dir.join("hasil.stko"));
    assert!(!s.output_dirty, "saved output is clean");
    let mut t = AppState::new();
    t.open_output(&saved).unwrap();
    // Debug text: NaN cells compare unequal with ==, and every f64 must come back exactly.
    assert_eq!(format!("{:?}", t.outputs), format!("{:?}", s.outputs));
    assert!(t.show_output);
    // Opening again appends (nothing on screen is lost) and scrolls to the opened output.
    t.open_output(&saved).unwrap();
    assert_eq!(t.outputs.len(), 2 * s.outputs.len());
    assert_eq!(t.viewer.scroll_to, Some(ItemRef::entry(s.outputs.len())));
    // A bad file is an error naming it, and changes nothing.
    let bad = dir.join("bad.stko");
    std::fs::write(&bad, "not json").unwrap();
    let err = t.open_output(&bad).unwrap_err();
    assert!(err.contains("bad.stko"), "{err}");
    assert_eq!(t.outputs.len(), 2 * s.outputs.len());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn qq_reference_line_is_the_fitted_normal_line() {
    use statik_app::output::chart_view::qq_line;
    // mean 50, sd 10 (n-1): line from (min, (min-50)/10) to (max, (max-50)/10).
    let obs = [40.0, 50.0, 60.0];
    let l = qq_line(&obs).unwrap();
    assert_eq!(l, [[40.0, -1.0], [60.0, 1.0]]);
    assert!(qq_line(&[3.0, 3.0]).is_none(), "no line for a constant");
    assert!(qq_line(&[]).is_none());
}

#[test]
fn tick_steps_are_nice_and_whole_for_histograms() {
    use statik_app::output::chart_view::{histogram_x_step, nice_step};
    assert_eq!(nice_step(10.0, 8.0), 2.0);
    assert_eq!(nice_step(1.0, 5.0), 0.2);
    assert_eq!(nice_step(50.0, 8.0), 10.0);
    // Standardized residuals -2.5..2.5: whole-number ticks.
    assert_eq!(histogram_x_step(-2.5, 2.5), 1.0);
    assert_eq!(histogram_x_step(0.0, 1.0), 0.2);
}

#[test]
fn explore_qq_plot_snapshot() {
    let _render = common::render_lock();
    use procedures::explore::{ExploreOptions, run_explore};
    let mut s = skripsi();
    let opt = ExploreOptions {
        dependents: vec![s.doc.ds().find("y").unwrap()],
        normality: true,
        qq_plot: true,
    };
    run_procedure(
        &mut s,
        "Explore",
        move |ds| ProcOutput::output(run_explore(ds, &opt)),
        "EXAMINE VARIABLES=y\n  /PLOT NPPLOT.".into(),
    );
    let e = s.outputs.len() - 1;
    let i = s.outputs[e]
        .items
        .iter()
        .position(|it| matches!(it, OutputItem::Chart(_)))
        .unwrap();
    s.viewer.scroll_to = Some(ItemRef::item(e, i));
    s.viewer.hide_outline = true;
    let mut h = viewer(s, egui::vec2(700.0, 560.0));
    h.run();
    assert_eq!(
        h.query_all_by_role_and_label(Role::Image, "Normal Q-Q Plot of y")
            .count(),
        1
    );
    h.snapshot("output_explore_qq");
}

#[test]
fn copy_as_image_without_a_picture_times_out_with_a_message() {
    let _render = common::render_lock();
    use statik_app::output::{SHOT_TIMEOUT, Shot};
    let mut s = regression_state();
    let mut shot = Shot::new(ItemRef::item(1, 2));
    shot.wait = 0;
    shot.sent = true;
    shot.sent_at = 0.0;
    s.viewer.shot = Some(shot);
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .with_step_dt(0.25)
        .build_ui_state(
            |ui, s: &mut AppState| {
                output_ui(ui, s, false);
            },
            s,
        );
    for _ in 0..((SHOT_TIMEOUT / 0.25) as usize + 3) {
        h.step();
    }
    assert!(h.state().viewer.shot.is_none());
    let m = h.state().message().expect("error message");
    assert!(m.text.contains("Copy as Image"), "{}", m.text);
}

#[test]
fn log_entry_is_a_single_outline_node() {
    let _render = common::render_lock();
    let mut h = viewer(skripsi(), egui::vec2(900.0, 500.0));
    h.run();
    assert_eq!(count(&h, "Log"), 1);
}

#[test]
fn open_output_refuses_huge_files() {
    let dir = std::env::temp_dir().join(format!("statik-big-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("big.stko");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(statik_app::state::MAX_OUTPUT_FILE + 1).unwrap();
    drop(f);
    let mut s = AppState::new();
    let err = s.open_output(&p).unwrap_err();
    assert!(err.contains("too large"), "{err}");
    assert!(s.outputs.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// Histogram bins, normal curve and legend are computed once, not on every frame.
#[test]
fn histogram_figures_are_computed_once() {
    let _render = common::render_lock();
    let mut h = viewer(regression_state(), egui::vec2(1100.0, 2600.0));
    h.run();
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(h.state().viewer.histogram_stats_computed, 1);
    assert_eq!(
        h.query_all_by_role_and_label(Role::Image, "Histogram")
            .count(),
        1
    );
    assert!(count(&h, "N = 60") >= 1, "legend still drawn");
}

/// Log / title / notes / warning items are measured once; off-screen ones are not laid out.
#[test]
fn off_screen_text_items_are_not_laid_out() {
    let _render = common::render_lock();
    let mut s = AppState::new();
    for i in 0..300 {
        s.outputs.push(statik_app::OutputEntry {
            title: "Log".into(),
            items: vec![
                OutputItem::Title(format!("Title {i}")),
                OutputItem::Warning(format!("Warning {i}")),
            ],
            log: format!(
                "COMPUTE v{i}=1.
EXECUTE."
            ),
        });
    }
    let mut h = viewer(s, egui::vec2(1000.0, 600.0));
    h.run();
    h.step();
    let n = h.state().viewer.text_layouts;
    assert!(n > 0 && n < 60, "{n} text items laid out in one frame");
    assert!(h.query_by_label("Title 0").is_some());
    assert!(h.query_by_label("Title 299").is_none());
    // Scrolling to the end still finds the last items at the right place.
    h.state_mut().viewer.scroll_to = Some(ItemRef::item(299, 1));
    h.run();
    assert!(h.query_by_label("Title 299").is_some());
}
