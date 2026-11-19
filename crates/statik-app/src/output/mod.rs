//! The Output viewer: a second window "Output1 [Document1] - Statik Viewer" with its own menu
//! bar, the outline on the left and the output document on the right.
//!
//! [`output_window`] shows it as a separate native window (an immediate egui viewport); where
//! the backend cannot open one (tests), egui embeds it as an inner window. [`output_ui`] draws
//! the contents and is what the tests render directly.
//!
//! Each item can be selected (click, or click its outline node) and has a context menu:
//! Copy (tables: HTML + RTF + tab-separated text; charts: as an image), Copy as Image (the
//! viewport is captured with `ViewportCommand::Screenshot`, cropped to the item, put on the
//! clipboard as a bitmap) and Export... (not available yet). UI state lives in
//! [`AppState::viewer`]. Everything is disabled while a job runs (`busy`).
pub mod chart_view;
pub mod clipboard;
pub mod outline;
pub mod table_view;

use crate::state::{AppState, Message};
use chart_view::{chart_size, paint_chart};
use egui::{
    Align, Color32, Context, Event, FontId, Id, Rect, Sense, Stroke, StrokeKind, Ui, UserData,
    Vec2, ViewportBuilder, ViewportClass, ViewportCommand, ViewportId, pos2, vec2,
};
use output_model::OutputItem;
use std::collections::HashMap;
use std::sync::Arc;
use table_view::{
    DATA_COLOR, TableMetrics, marked_job, measure, paint_bold, paint_table, text_node,
};

/// Hashed into the Output window's `ViewportId` (`ViewportId::from_hash_of`).
pub const OUTPUT_VIEWPORT: &str = "statik_output";

/// Title of the Output window (SPSS: "Output1 [Document1] - IBM SPSS Statistics Viewer").
pub const OUTPUT_TITLE: &str = "Output1 [Document1] - Statik Viewer";

/// One thing in the output document: the log block of an entry (`item: None`) or one item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemRef {
    pub entry: usize,
    pub item: Option<usize>,
}

impl ItemRef {
    /// The start of an entry (its log block).
    pub fn entry(entry: usize) -> Self {
        ItemRef { entry, item: None }
    }
    pub fn item(entry: usize, item: usize) -> Self {
        ItemRef {
            entry,
            item: Some(item),
        }
    }
}

/// A "Copy as Image" in progress: wait a frame or two (so the context menu is gone), request a
/// screenshot, then crop it to the item when it arrives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub item: ItemRef,
    /// Frames to wait before capturing (the context menu must be gone).
    pub wait: u8,
    /// Scrolls done to bring the item fully into view.
    pub scrolls: u8,
    pub sent: bool,
    /// `InputState::time` when the request was sent; after [`SHOT_TIMEOUT`] s without a
    /// picture the copy fails with a message.
    pub sent_at: f64,
}

/// Seconds to wait for the screenshot of a "Copy as Image".
pub const SHOT_TIMEOUT: f64 = 2.0;

impl Shot {
    pub fn new(item: ItemRef) -> Self {
        Shot {
            item,
            wait: 2,
            scrolls: 0,
            sent: false,
            sent_at: 0.0,
        }
    }
}

/// Tag carried by our screenshot request.
#[derive(Debug)]
struct ShotTag;

/// UI state of the Output viewer (plain data, kept in [`AppState::viewer`]).
#[derive(Debug, Default)]
pub struct OutputView {
    pub selected: Option<ItemRef>,
    /// Scroll the document to this item on the next frame.
    pub scroll_to: Option<ItemRef>,
    /// Jump there without the scroll animation (before a screenshot).
    scroll_instant: bool,
    /// Where each item was drawn in the last frame (screen points).
    pub item_rects: HashMap<ItemRef, Rect>,
    pub shot: Option<Shot>,
    /// View ▸ Outline unchecked.
    pub hide_outline: bool,
    /// Visible part of the document in the last frame.
    doc_clip: Option<Rect>,
    /// Table measurements and item sizes, so big tables are only painted where visible.
    metrics: HashMap<ItemRef, Arc<TableMetrics>>,
    sizes: HashMap<ItemRef, Vec2>,
    /// pixels_per_point and number of entries the caches were made for.
    cache_key: (u32, usize),
    /// The viewer is a window of its own and has the keyboard focus: queued messages are
    /// shown there instead of in the Data Editor (set every frame).
    pub hosts_messages: bool,
}

/// Menu choices the app handles (file dialogs, other windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputAction {
    SaveOutput,
    OpenOutput,
    ShowDataEditor,
    About,
}

/// Outline name of an item (SPSS shows table and chart titles; "Title", "Notes", "Warnings").
pub fn item_name(it: &OutputItem) -> &str {
    match it {
        OutputItem::Title(_) => "Title",
        OutputItem::Notes(_) => "Notes",
        OutputItem::Table(t) => &t.title,
        OutputItem::Chart(c) => c.title(),
        OutputItem::Warning(_) => "Warnings",
    }
}

/// Show the Output window (call every frame while `state.show_output`). Closing it hides it.
pub fn output_window(ctx: &Context, state: &mut AppState, busy: bool) -> Option<OutputAction> {
    let builder = ViewportBuilder::default()
        .with_title(OUTPUT_TITLE)
        .with_inner_size([960.0, 680.0])
        .with_min_inner_size([360.0, 240.0]);
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of(OUTPUT_VIEWPORT),
        builder,
        |ui, class| {
            if class == ViewportClass::Immediate && ui.input(|i| i.viewport().close_requested()) {
                state.show_output = false;
            }
            let action = output_ui(ui, state, busy);
            // In its own window with the focus, the viewer shows the messages (e.g. a failed
            // copy), so they are not hidden behind it in the Data Editor; otherwise the Data
            // Editor shows them. Never both at once.
            state.viewer.hosts_messages =
                messages_in_output(class, ui.input(|i| i.viewport().focused));
            if state.viewer.hosts_messages
                && let Some(msg) = state.message()
                && crate::dialogs::message::message_box(ui.ctx(), msg)
            {
                state.dismiss_message();
            }
            action
        },
    )
}

/// Whether queued messages are shown in the Output viewer (its own window, focused) rather
/// than in the Data Editor.
pub fn messages_in_output(class: ViewportClass, focused: Option<bool>) -> bool {
    class == ViewportClass::Immediate && focused == Some(true)
}

/// The viewer contents: menu bar, outline, document. Returns a menu choice for the app.
pub fn output_ui(ui: &mut Ui, state: &mut AppState, busy: bool) -> Option<OutputAction> {
    take_screenshot(ui, state);
    let mut action = None;
    ui.add_enabled_ui(!busy, |ui| {
        egui::Panel::top("output_menu").show(ui, |ui| {
            action = menu_bar(ui, state);
        });
        if !state.viewer.hide_outline {
            egui::Panel::left("output_outline")
                .resizable(true)
                .default_size(230.0)
                .show(ui, |ui| outline::outline_ui(ui, state));
        }
        let frame = egui::Frame::central_panel(ui.style()).fill(Color32::WHITE);
        egui::CentralPanel::default_margins()
            .frame(frame)
            .show(ui, |ui| document(ui, state));
    });
    if !busy {
        let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Copy)));
        if copy
            && !ui.ctx().text_edit_focused()
            && let Some(r) = state.viewer.selected
        {
            copy_item(ui.ctx(), state, r, false);
        }
    }
    pump_screenshot(ui.ctx(), state);
    action
}

fn menu_bar(ui: &mut Ui, state: &mut AppState) -> Option<OutputAction> {
    let mut out = None;
    let sel = state.viewer.selected;
    let mut copy = None;
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open Output...").clicked() {
                out = Some(OutputAction::OpenOutput);
            }
            if ui.button("Save Output As...").clicked() {
                out = Some(OutputAction::SaveOutput);
            }
            ui.add_enabled(false, egui::Button::new("Export..."));
            ui.separator();
            if ui.button("Close").clicked() {
                state.show_output = false;
            }
        });
        ui.menu_button("Edit", |ui| {
            if ui
                .add_enabled(sel.is_some(), egui::Button::new("Copy"))
                .clicked()
            {
                copy = Some(false);
            }
            if ui
                .add_enabled(sel.is_some(), egui::Button::new("Copy as Image"))
                .clicked()
            {
                copy = Some(true);
            }
        });
        ui.menu_button("View", |ui| {
            let mut show = !state.viewer.hide_outline;
            if ui.checkbox(&mut show, "Outline").clicked() {
                state.viewer.hide_outline = !show;
            }
        });
        ui.menu_button("Window", |ui| {
            if ui.button("Data Editor").clicked() {
                out = Some(OutputAction::ShowDataEditor);
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("About Statik").clicked() {
                out = Some(OutputAction::About);
            }
        });
    });
    if let (Some(image), Some(r)) = (copy, sel) {
        copy_item(ui.ctx(), state, r, image);
    }
    out
}

enum DocAction {
    Select(ItemRef),
    Copy(ItemRef, bool),
}

const INDENT: f32 = 24.0;
const ITEM_GAP: f32 = 18.0;

fn document(ui: &mut Ui, state: &mut AppState) {
    let ppp = (ui.ctx().pixels_per_point() * 100.0).round() as u32;
    let v = &mut state.viewer;
    if v.cache_key.0 != ppp || v.cache_key.1 > state.outputs.len() {
        v.metrics.clear();
        v.sizes.clear();
    }
    v.cache_key = (ppp, state.outputs.len());
    let mut actions = Vec::new();
    let scroll_to = v.scroll_to.take();
    let shooting = v.shot.is_some();
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .id_salt("output_doc")
        .show(ui, |ui| {
            v.doc_clip = Some(ui.clip_rect());
            v.item_rects.clear();
            ui.spacing_mut().item_spacing.y = 0.0;
            for (e, entry) in state.outputs.iter().enumerate() {
                let r = ItemRef::entry(e);
                if entry.log.is_empty() {
                    let at = ui.cursor().min;
                    v.item_rects.insert(r, Rect::from_min_size(at, Vec2::ZERO));
                    if scroll_to == Some(r) {
                        ui.scroll_to_rect(
                            Rect::from_min_size(at, vec2(1.0, 1.0)),
                            Some(Align::TOP),
                        );
                    }
                } else {
                    let resp = log_block(ui, &entry.log, Id::new(("log", e)));
                    item_frame(ui, v, r, &resp, scroll_to, shooting, &mut actions, false);
                    ui.add_space(ITEM_GAP);
                }
                for (i, it) in entry.items.iter().enumerate() {
                    let r = ItemRef::item(e, i);
                    let resp = draw_item(ui, v, r, it);
                    let image = matches!(it, OutputItem::Chart(_));
                    item_frame(ui, v, r, &resp, scroll_to, shooting, &mut actions, image);
                    ui.add_space(ITEM_GAP);
                }
                ui.add_space(ITEM_GAP);
            }
        });
    state.viewer.scroll_instant = false;
    for a in actions {
        match a {
            DocAction::Select(r) => state.viewer.selected = Some(r),
            DocAction::Copy(r, image) => {
                state.viewer.selected = Some(r);
                copy_item(ui.ctx(), state, r, image);
            }
        }
    }
}

/// Selection, click, context menu and scrolling for one drawn item.
#[allow(clippy::too_many_arguments)]
fn item_frame(
    ui: &mut Ui,
    v: &mut OutputView,
    r: ItemRef,
    resp: &egui::Response,
    scroll_to: Option<ItemRef>,
    shooting: bool,
    actions: &mut Vec<DocAction>,
    copy_is_image: bool,
) {
    let rect = resp.rect;
    v.item_rects.insert(r, rect);
    if scroll_to == Some(r) {
        let anim = if v.scroll_instant {
            egui::style::ScrollAnimation::none()
        } else {
            ui.style().scroll_animation
        };
        ui.scroll_to_rect_animation(rect.expand(6.0), Some(Align::TOP), anim);
    }
    let click = ui.interact(
        rect,
        Id::new(("output_item", r.entry, r.item)),
        Sense::click(),
    );
    if click.clicked() || click.secondary_clicked() {
        actions.push(DocAction::Select(r));
    }
    click.context_menu(|ui| {
        if ui.button("Copy").clicked() {
            actions.push(DocAction::Copy(r, copy_is_image));
        }
        if ui.button("Copy as Image").clicked() {
            actions.push(DocAction::Copy(r, true));
        }
        ui.add_enabled(false, egui::Button::new("Export..."));
    });
    if v.selected == Some(r) && !shooting && ui.is_rect_visible(rect) {
        // SPSS: a thin frame around the selected item and a red arrow at its left.
        let p = ui.painter();
        p.rect_stroke(
            rect.expand(3.0),
            0.0,
            Stroke::new(1.0, Color32::BLACK),
            StrokeKind::Outside,
        );
        let y = rect.top() + 8.0;
        let x = rect.left() - 8.0;
        p.add(egui::Shape::convex_polygon(
            vec![pos2(x - 8.0, y - 5.0), pos2(x, y), pos2(x - 8.0, y + 5.0)],
            Color32::from_rgb(0xC0, 0x10, 0x10),
            Stroke::NONE,
        ));
    }
}

fn log_block(ui: &mut Ui, log: &str, id: Id) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(INDENT);
        let g = ui
            .painter()
            .layout_no_wrap(log.to_owned(), FontId::monospace(11.0), DATA_COLOR);
        let (rect, resp) = ui.allocate_exact_size(g.size() + vec2(8.0, 6.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter()
                .galley(rect.min + vec2(4.0, 3.0), g, DATA_COLOR);
            text_node(ui, rect, id.with("text"), log);
        }
        resp
    })
    .inner
}

fn draw_item(ui: &mut Ui, v: &mut OutputView, r: ItemRef, it: &OutputItem) -> egui::Response {
    let id = Id::new(("output_item_body", r.entry, r.item));
    match it {
        OutputItem::Title(t) => {
            let g = ui.painter().layout_job(marked_job(
                t,
                "",
                16.0,
                DATA_COLOR,
                f32::INFINITY,
                Align::LEFT,
            ));
            let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
            if ui.is_rect_visible(rect) {
                paint_bold(ui, rect.min, &g, DATA_COLOR);
                text_node(ui, rect, id, t);
            }
            resp
        }
        OutputItem::Notes(t) => indented(ui, |ui| {
            let g =
                ui.painter()
                    .layout_job(marked_job(t, "", 12.0, DATA_COLOR, 600.0, Align::LEFT));
            let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
            if ui.is_rect_visible(rect) {
                table_view::paint_galley(ui, rect.min, &g, DATA_COLOR);
                text_node(ui, rect, id, t);
            }
            resp
        }),
        OutputItem::Warning(t) => indented(ui, |ui| warning_box(ui, t, id)),
        OutputItem::Table(t) => indented(ui, |ui| {
            let m = match v.metrics.get(&r) {
                Some(m) => m.clone(),
                None => {
                    let m = Arc::new(measure(ui, t));
                    v.metrics.insert(r, m.clone());
                    m
                }
            };
            let (rect, resp) = ui.allocate_exact_size(m.size, Sense::hover());
            if ui.is_rect_visible(rect) {
                paint_table(ui, t, &m, rect.min, id);
            }
            resp
        }),
        OutputItem::Chart(c) => indented(ui, |ui| {
            let size = *v.sizes.entry(r).or_insert_with(|| chart_size(ui, c));
            let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
            if ui.is_rect_visible(rect) {
                paint_chart(ui, c, rect, id);
                // The chart as one image node named by its title (only while it is drawn).
                let n = ui.interact(rect, id.with("image"), Sense::hover());
                let title = c.title().to_owned();
                n.widget_info(move || {
                    egui::WidgetInfo::labeled(egui::WidgetType::Image, true, &title)
                });
            }
            resp
        }),
    }
}

fn indented(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> egui::Response) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(INDENT);
        add(ui)
    })
    .inner
}

/// SPSS shows warnings as a one-cell table titled "Warnings".
fn warning_box(ui: &mut Ui, text: &str, id: Id) -> egui::Response {
    let p = ui.painter().clone();
    let title = p.layout_job(marked_job(
        "Warnings",
        "",
        table_view::TITLE_SIZE,
        DATA_COLOR,
        f32::INFINITY,
        Align::LEFT,
    ));
    let body = p.layout_job(marked_job(
        text,
        "",
        table_view::TEXT_SIZE,
        DATA_COLOR,
        460.0,
        Align::LEFT,
    ));
    let w = body.size().x.max(title.size().x) + 16.0;
    let size = vec2(w, title.size().y + 6.0 + body.size().y + 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        let tr = Rect::from_min_size(
            pos2(rect.center().x - title.size().x / 2.0, rect.top()),
            title.size(),
        );
        paint_bold(ui, tr.min, &title, DATA_COLOR);
        text_node(ui, tr, id.with("title"), "Warnings");
        let bx = Rect::from_min_max(pos2(rect.left(), tr.bottom() + 6.0), rect.max);
        p.rect_stroke(
            bx,
            0.0,
            Stroke::new(1.0, table_view::RULE),
            StrokeKind::Inside,
        );
        let at = bx.min + vec2(8.0, 6.0);
        table_view::paint_galley(ui, at, &body, DATA_COLOR);
        text_node(
            ui,
            Rect::from_min_size(at, body.size()),
            id.with("text"),
            text,
        );
    }
    resp
}

/// Copy an item: tables as HTML + RTF + text, text items as text, charts (or `image`) as a
/// bitmap via a screenshot.
fn copy_item(ctx: &Context, state: &mut AppState, r: ItemRef, image: bool) {
    let Some(entry) = state.outputs.get(r.entry) else {
        return;
    };
    let item = r.item.and_then(|i| entry.items.get(i));
    if image || matches!(item, Some(OutputItem::Chart(_))) {
        state.viewer.shot = Some(Shot::new(r));
        ctx.request_repaint();
        return;
    }
    let res = match item {
        None => clipboard::copy_text(&entry.log, true),
        Some(OutputItem::Table(t)) => clipboard::copy_table(t),
        Some(OutputItem::Title(t) | OutputItem::Notes(t)) => clipboard::copy_text(t, false),
        Some(OutputItem::Warning(t)) => clipboard::copy_text(&format!("Warnings\n{t}"), false),
        Some(OutputItem::Chart(_)) => Ok(()),
    };
    if let Err(e) = res {
        state.push_message(Message::error(e));
    }
}

/// Count down a pending "Copy as Image" and send the screenshot request.
fn pump_screenshot(ctx: &Context, state: &mut AppState) {
    let now = ctx.input(|i| i.time);
    let v = &mut state.viewer;
    let Some(s) = &mut v.shot else {
        return;
    };
    if s.sent {
        let left = SHOT_TIMEOUT - (now - s.sent_at);
        if left <= 0.0 {
            v.shot = None;
            state.push_message(Message::error(
                "Copy as Image failed: the picture of the window was not received.",
            ));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
        }
        return;
    }
    if s.wait > 0 {
        s.wait -= 1;
        ctx.request_repaint();
        return;
    }
    // Capture only once the item is fully on screen (or as far as it fits): jump there
    // without animation and look again on the next frame.
    let rect = v.item_rects.get(&s.item).copied();
    let clip = v.doc_clip;
    let fully_visible = match (rect, clip) {
        (Some(r), Some(c)) => {
            c.contains_rect(r)
                || (r.height() > c.height() && r.top() >= c.top() - 0.5 && r.top() <= c.top() + 8.0)
        }
        _ => true,
    };
    if !fully_visible && s.scrolls < 4 {
        s.scrolls += 1;
        v.scroll_to = Some(s.item);
        v.scroll_instant = true;
        ctx.request_repaint();
        return;
    }
    ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(ShotTag)));
    s.sent = true;
    s.sent_at = now;
    ctx.request_repaint_after(std::time::Duration::from_secs_f64(SHOT_TIMEOUT));
}

/// A screenshot we asked for arrived: crop it to the item and copy it.
fn take_screenshot(ui: &Ui, state: &mut AppState) {
    let image = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            Event::Screenshot {
                user_data, image, ..
            } if user_data
                .data
                .as_ref()
                .is_some_and(|d| d.downcast_ref::<ShotTag>().is_some()) =>
            {
                Some(image.clone())
            }
            _ => None,
        })
    });
    let Some(image) = image else { return };
    let Some(shot) = state.viewer.shot.take() else {
        return;
    };
    let Some(rect) = state.viewer.item_rects.get(&shot.item).copied() else {
        return;
    };
    let rect = match state.viewer.doc_clip {
        Some(c) => rect.intersect(c),
        None => rect,
    };
    let res = match clipboard::crop_image(&image, rect, ui.ctx().pixels_per_point()) {
        Some(img) => clipboard::copy_image(&clipboard::image_rgba(&img), img.size[0], img.size[1]),
        None => Err("Nothing to copy: the item is not on screen.".into()),
    };
    if let Err(e) = res {
        state.push_message(Message::error(e));
    }
}
