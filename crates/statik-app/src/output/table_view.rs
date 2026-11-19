//! Pivot tables in the SPSS look: title centred above, nested column headers spanning their
//! children, row group labels spanning their rows, dark rules above the header and below the
//! last row, light rules between rows, numbers right-aligned, footnotes in small text below.
//!
//! [`table_grid`] places every label and value on one grid; the view, the tab-separated text
//! and the RTF copy all read that grid. All numbers come from `output_model::format_value`.
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, FontId, Galley, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use output_model::{
    CellValue, Table, format_markers, format_value, header_cells, header_depth, join_markers,
    leaf_count, row_header_cells,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// The empty box above the row labels and left of the column labels.
    Corner,
    ColHeader,
    RowHeader,
    Data {
        numeric: bool,
    },
}

/// One cell on the grid (an anchor; the cells it spans are left out).
#[derive(Clone, Debug, PartialEq)]
pub struct GridCell {
    pub row: usize,
    pub col: usize,
    pub rowspan: usize,
    pub colspan: usize,
    pub text: String,
    /// Footnote markers, comma-joined ("" when none).
    pub marks: String,
    pub kind: CellKind,
}

/// A table on a grid: `header_rows` column-header rows, then one row per data row;
/// `row_header_cols` row-label columns, then one column per data column.
#[derive(Clone, Debug, PartialEq)]
pub struct TableGrid {
    pub rows: usize,
    pub cols: usize,
    pub header_rows: usize,
    pub row_header_cols: usize,
    pub cells: Vec<GridCell>,
}

/// Place a table's labels and values on a grid.
pub fn table_grid(t: &Table) -> TableGrid {
    let (rdepth, rcells) = row_header_cells(&t.row_headers);
    let ncols = leaf_count(&t.col_headers).max(t.cells.iter().map(|r| r.len()).max().unwrap_or(0));
    let hrows = header_depth(&t.col_headers);
    let mut cells = Vec::new();
    if hrows > 0 {
        cells.push(GridCell {
            row: 0,
            col: 0,
            rowspan: hrows,
            colspan: rdepth,
            text: String::new(),
            marks: String::new(),
            kind: CellKind::Corner,
        });
    }
    for h in header_cells(&t.col_headers) {
        cells.push(GridCell {
            row: h.level,
            col: rdepth + h.col,
            rowspan: h.rowspan,
            colspan: h.colspan,
            text: h.label,
            marks: join_markers(&h.footnotes),
            kind: CellKind::ColHeader,
        });
    }
    for h in rcells {
        cells.push(GridCell {
            row: hrows + h.row,
            col: h.depth,
            rowspan: h.rowspan,
            colspan: h.colspan,
            text: h.label,
            marks: join_markers(&h.footnotes),
            kind: CellKind::RowHeader,
        });
    }
    for (ri, row) in t.cells.iter().enumerate() {
        for (ci, c) in row.iter().enumerate() {
            cells.push(GridCell {
                row: hrows + ri,
                col: rdepth + ci,
                rowspan: 1,
                colspan: 1,
                text: format_value(c),
                marks: format_markers(c),
                kind: CellKind::Data {
                    numeric: matches!(c.value, CellValue::Num(_)),
                },
            });
        }
    }
    TableGrid {
        rows: hrows + t.cells.len(),
        cols: rdepth + ncols,
        header_rows: hrows,
        row_header_cols: rdepth,
        cells,
    }
}

// ---- look -------------------------------------------------------------------------------

pub const LABEL_COLOR: Color32 = Color32::from_rgb(0x26, 0x4A, 0x60);
pub const DATA_COLOR: Color32 = Color32::from_rgb(0x01, 0x02, 0x05);
pub const RULE: Color32 = Color32::from_rgb(0x15, 0x29, 0x35);
pub const LIGHT: Color32 = Color32::from_rgb(0xAE, 0xAE, 0xAE);
pub const FAINT: Color32 = Color32::from_rgb(0xE0, 0xE0, 0xE0);
pub const TEXT_SIZE: f32 = 12.0;
pub const MARK_SIZE: f32 = 8.0;
pub const TITLE_SIZE: f32 = 13.0;
pub const NOTE_SIZE: f32 = 10.5;
const PAD_X: f32 = 6.0;
const PAD_Y: f32 = 3.0;
/// Narrowest wrap width for a column label (longer labels wrap onto more lines, as in SPSS).
const MIN_HEADER_WRAP: f32 = 64.0;
const GAP: f32 = 6.0;

/// Text followed by small raised footnote markers.
pub fn marked_job(
    text: &str,
    marks: &str,
    size: f32,
    color: Color32,
    wrap: f32,
    halign: Align,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    job.halign = halign;
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    if !marks.is_empty() {
        job.append(
            marks,
            0.0,
            TextFormat {
                font_id: FontId::proportional(size * MARK_SIZE / TEXT_SIZE),
                color,
                valign: Align::TOP,
                ..Default::default()
            },
        );
    }
    job
}

/// Paint a galley so that its bounding box sits at `at` (works for any `halign`).
pub fn paint_galley(ui: &Ui, at: Pos2, g: &Arc<Galley>, color: Color32) {
    ui.painter()
        .galley(at - g.rect.min.to_vec2(), g.clone(), color);
}

/// Paint text in a faux-bold weight (the bundled fonts have no bold face): drawn twice,
/// the second time shifted by a fraction of a pixel.
pub fn paint_bold(ui: &Ui, at: Pos2, g: &Arc<Galley>, color: Color32) {
    paint_galley(ui, at, g, color);
    let dx = 1.0 / ui.ctx().pixels_per_point();
    paint_galley(ui, at + vec2(dx, 0.0), g, color);
}

/// Add an accessibility node (a label) for painted text, so tests and screen readers see it.
pub fn text_node(ui: &mut Ui, rect: Rect, id: egui::Id, text: &str) {
    if text.is_empty() {
        return;
    }
    let r = ui.interact(rect, id, Sense::hover());
    let text = text.to_owned();
    r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
}

/// Column widths, row heights and wrap widths of a table, measured once and cached by the
/// viewer (a big table is then only painted where it is visible).
#[derive(Clone, Debug)]
pub struct TableMetrics {
    pub grid: TableGrid,
    pub col_w: Vec<f32>,
    pub row_h: Vec<f32>,
    /// Wrap width per grid cell (headers wrap; everything else does not).
    pub wrap: Vec<f32>,
    pub title_h: f32,
    pub notes_h: f32,
    /// Width of the table body (sum of the columns).
    pub table_w: f32,
    /// Left offset of the table body (a title wider than the table pushes it right).
    pub table_x: f32,
    /// Whole item: title, table, footnotes.
    pub size: Vec2,
}

fn cell_color(kind: CellKind) -> Color32 {
    match kind {
        CellKind::Data { .. } => DATA_COLOR,
        _ => LABEL_COLOR,
    }
}

fn cell_job(c: &GridCell, wrap: f32) -> LayoutJob {
    let halign = if c.kind == CellKind::ColHeader {
        Align::Center
    } else {
        Align::LEFT
    };
    marked_job(
        &c.text,
        &c.marks,
        TEXT_SIZE,
        cell_color(c.kind),
        wrap,
        halign,
    )
}

fn title_job(t: &Table, wrap: f32) -> LayoutJob {
    marked_job(
        &t.title,
        &join_markers(&t.title_footnotes),
        TITLE_SIZE,
        DATA_COLOR,
        wrap,
        Align::Center,
    )
}

fn note_job(m: char, text: &str, wrap: f32) -> LayoutJob {
    marked_job(
        &format!("{m}. {text}"),
        "",
        NOTE_SIZE,
        DATA_COLOR,
        wrap,
        Align::LEFT,
    )
}

/// Width of the widest word of a label (with its markers on the last word): labels wrap
/// between words only, never inside one ("Standardized", not "Standardize/d").
fn longest_word(p: &egui::Painter, c: &GridCell) -> f32 {
    let words: Vec<&str> = c.text.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .map(|(k, w)| {
            let marks = if k + 1 == words.len() {
                c.marks.as_str()
            } else {
                ""
            };
            p.layout_job(marked_job(
                w,
                marks,
                TEXT_SIZE,
                LABEL_COLOR,
                f32::INFINITY,
                Align::LEFT,
            ))
            .size()
            .x + 1.0
        })
        .fold(0.0, f32::max)
}

/// Measure a table (lays out every cell once).
pub fn measure(ui: &Ui, t: &Table) -> TableMetrics {
    let grid = table_grid(t);
    let p = ui.painter();
    let mut col_w = vec![0.0f32; grid.cols];
    let mut row_h = vec![0.0f32; grid.rows];
    let mut wrap = vec![f32::INFINITY; grid.cells.len()];
    let mut size = vec![Vec2::ZERO; grid.cells.len()];
    let pad = vec2(2.0 * PAD_X, 2.0 * PAD_Y);
    // 1. Everything except column labels, one column wide: natural width.
    for (i, c) in grid.cells.iter().enumerate() {
        if c.kind != CellKind::ColHeader {
            size[i] = p.layout_job(cell_job(c, f32::INFINITY)).size() + pad;
            if c.colspan == 1 {
                col_w[c.col] = col_w[c.col].max(size[i].x);
            }
        }
    }
    // 2. Leaf column labels wrap at the width of their data (at least MIN_HEADER_WRAP).
    for (i, c) in grid.cells.iter().enumerate() {
        if c.kind == CellKind::ColHeader && c.colspan == 1 {
            wrap[i] = (col_w[c.col] - 2.0 * PAD_X)
                .max(MIN_HEADER_WRAP)
                .max(longest_word(p, c));
            size[i] = p.layout_job(cell_job(c, wrap[i])).size() + pad;
            col_w[c.col] = col_w[c.col].max(size[i].x);
        }
    }
    // 3. Spanning cells: wrap labels over their columns, widen the last column if needed.
    for (i, c) in grid.cells.iter().enumerate() {
        if c.colspan > 1 {
            let span: f32 = col_w[c.col..c.col + c.colspan].iter().sum();
            if c.kind == CellKind::ColHeader {
                wrap[i] = (span - 2.0 * PAD_X)
                    .max(MIN_HEADER_WRAP)
                    .max(longest_word(p, c));
                size[i] = p.layout_job(cell_job(c, wrap[i])).size() + pad;
            }
            if size[i].x > span {
                col_w[c.col + c.colspan - 1] += size[i].x - span;
            }
        }
    }
    // 4. Row heights, then rows spanned by tall cells.
    let line =
        p.layout_job(marked_job(
            "Ag",
            "",
            TEXT_SIZE,
            DATA_COLOR,
            f32::INFINITY,
            Align::LEFT,
        ))
        .size()
        .y + pad.y;
    row_h.iter_mut().for_each(|h| *h = line);
    for (i, c) in grid.cells.iter().enumerate() {
        if c.rowspan == 1 {
            row_h[c.row] = row_h[c.row].max(size[i].y);
        }
    }
    for (i, c) in grid.cells.iter().enumerate() {
        if c.rowspan > 1 {
            let span: f32 = row_h[c.row..c.row + c.rowspan].iter().sum();
            if size[i].y > span {
                row_h[c.row + c.rowspan - 1] += size[i].y - span;
            }
        }
    }
    let table_w: f32 = col_w.iter().sum::<f32>().max(1.0);
    let table_h: f32 = row_h.iter().sum();
    let title = p.layout_job(title_job(t, table_w.max(240.0)));
    let note_wrap = table_w.max(320.0);
    let notes: Vec<Vec2> = t
        .footnotes
        .iter()
        .map(|(m, s)| p.layout_job(note_job(*m, s, note_wrap)).size())
        .collect();
    let notes_h: f32 = notes.iter().map(|s| s.y).sum();
    let notes_w = notes.iter().map(|s| s.x).fold(0.0, f32::max);
    let table_x = ((title.size().x - table_w) / 2.0).max(0.0);
    let width = title.size().x.max(table_x + table_w).max(table_x + notes_w);
    let notes_gap = if t.footnotes.is_empty() { 0.0 } else { GAP };
    TableMetrics {
        grid,
        col_w,
        row_h,
        wrap,
        title_h: title.size().y,
        notes_h,
        table_w,
        table_x,
        size: vec2(width, title.size().y + GAP + table_h + notes_gap + notes_h),
    }
}

/// Paint a measured table with its top-left corner at `origin` (only the visible rows).
pub fn paint_table(ui: &mut Ui, t: &Table, m: &TableMetrics, origin: Pos2, id: egui::Id) {
    let clip = ui.clip_rect();
    let p = ui.painter().clone();
    let x0 = origin.x + m.table_x;
    // Title, centred over the table.
    let title = p.layout_job(title_job(t, m.table_w.max(240.0)));
    let title_rect = Rect::from_min_size(
        pos2(x0 + (m.table_w - title.size().x) / 2.0, origin.y),
        title.size(),
    );
    if clip.intersects(title_rect) {
        paint_bold(ui, title_rect.min, &title, DATA_COLOR);
        text_node(ui, title_rect, id.with("title"), &t.title);
    }
    let y0 = origin.y + m.title_h + GAP;
    let mut xs = vec![x0];
    for w in &m.col_w {
        xs.push(xs.last().unwrap() + w);
    }
    let mut ys = vec![y0];
    for h in &m.row_h {
        ys.push(ys.last().unwrap() + h);
    }
    let g = &m.grid;
    let x_end = xs[g.cols];
    let y_end = ys[g.rows];
    let cell_rect = |c: &GridCell| {
        Rect::from_min_max(
            pos2(xs[c.col], ys[c.row]),
            pos2(xs[c.col + c.colspan], ys[c.row + c.rowspan]),
        )
    };
    let hline = |y: f32, a: f32, b: f32, color: Color32| {
        p.hline(a..=b, y, Stroke::new(1.0, color));
    };

    for (i, c) in g.cells.iter().enumerate() {
        let r = cell_rect(c);
        if !clip.intersects(r) {
            continue;
        }
        // Rules: under every column label (spanning labels get a line above their children),
        // under every row (faint; dark under the last row).
        match c.kind {
            CellKind::ColHeader | CellKind::Corner => hline(r.bottom(), r.left(), r.right(), LIGHT),
            _ => {
                let color = if c.row + c.rowspan == g.rows {
                    RULE
                } else {
                    FAINT
                };
                hline(r.bottom(), r.left(), r.right(), color);
            }
        }
        if c.text.is_empty() && c.marks.is_empty() {
            continue;
        }
        let gal = p.layout_job(cell_job(c, m.wrap[i]));
        let inner = r.shrink2(vec2(PAD_X, PAD_Y));
        let s = gal.size();
        let at = match c.kind {
            CellKind::ColHeader => pos2(inner.center().x - s.x / 2.0, inner.bottom() - s.y),
            CellKind::RowHeader | CellKind::Corner => inner.min,
            CellKind::Data { numeric: true } => {
                pos2(inner.right() - s.x, inner.center().y - s.y / 2.0)
            }
            CellKind::Data { numeric: false } => pos2(inner.left(), inner.center().y - s.y / 2.0),
        };
        paint_galley(ui, at, &gal, cell_color(c.kind));
        text_node(ui, Rect::from_min_size(at, s), id.with(i), &c.text);
    }
    // Dark rule above the header (or the first row), light line between labels and data.
    hline(y0, x0, x_end, RULE);
    if g.header_rows > 0 {
        hline(ys[g.header_rows], x0, x_end, LIGHT);
    }
    if g.row_header_cols > 0 && g.rows > g.header_rows {
        let x = xs[g.row_header_cols];
        p.vline(x, ys[g.header_rows]..=y_end, Stroke::new(1.0, LIGHT));
    }
    // Footnotes.
    let mut y = y_end + GAP;
    let note_wrap = m.table_w.max(320.0);
    for (k, (mk, s)) in t.footnotes.iter().enumerate() {
        let gal = p.layout_job(note_job(*mk, s, note_wrap));
        let r = Rect::from_min_size(pos2(x0, y), gal.size());
        if clip.intersects(r) {
            paint_galley(ui, r.min, &gal, DATA_COLOR);
            text_node(ui, r, id.with(("note", k)), &format!("{mk}. {s}"));
        }
        y += gal.size().y;
    }
}

/// Draw a table at the cursor (measures it every call; the viewer caches the measurement).
pub fn table_view(ui: &mut Ui, t: &Table) -> egui::Response {
    let m = measure(ui, t);
    let (rect, resp) = ui.allocate_exact_size(m.size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_table(ui, t, &m, rect.min, resp.id);
    }
    resp
}
