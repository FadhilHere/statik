//! Charts with `egui_plot`, drawn like SPSS's: bold title, subtitle, a static plot (no zoom or
//! drag). Histogram: bars, optional normal curve and "Mean / Std. Dev. / N" at the side.
//! P-P: points and the diagonal. Q-Q: points and the y = x line. Scatterplot: points.
use super::table_view::{
    DATA_COLOR, LABEL_COLOR, TITLE_SIZE, marked_job, paint_bold, paint_galley,
};
use egui::{
    Align, Color32, Layout, Rect, Response, Sense, Stroke, Ui, UiBuilder, Vec2, pos2, vec2,
};
use egui_plot::{Bar, BarChart, Line, MarkerShape, Plot, PlotPoints, Points};
use output_model::{Cell, CellFormat, CellValue, Chart, format_value};

pub const PLOT_W: f32 = 460.0;
pub const PLOT_H: f32 = 380.0;
pub const LEGEND_W: f32 = 130.0;
const SUB_SIZE: f32 = 12.0;
const GAP: f32 = 4.0;
const BAR_FILL: Color32 = Color32::from_rgb(0x5B, 0x8D, 0xC9);
const POINT: Color32 = Color32::from_rgb(0x1F, 0x4E, 0x8C);

/// SPSS chart numbers have a leading zero ("0.983"); table numbers do not (".983").
fn chart_number(v: f64, format: CellFormat) -> String {
    let s = format_value(&Cell {
        value: CellValue::Num(v),
        format,
        footnotes: vec![],
    });
    if let Some(r) = s.strip_prefix("-.") {
        format!("-0.{r}")
    } else if let Some(r) = s.strip_prefix('.') {
        if r.is_empty() { s } else { format!("0.{r}") }
    } else {
        s
    }
}

fn mean_sd(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    let m = values.iter().sum::<f64>() / n;
    let ss: f64 = values.iter().map(|v| (v - m) * (v - m)).sum();
    (
        m,
        if n > 1.0 {
            (ss / (n - 1.0)).sqrt()
        } else {
            f64::NAN
        },
    )
}

/// The legend SPSS prints beside a histogram: mean (2 decimals), standard deviation
/// (3 decimals), N. Computed from the chart's values.
pub fn histogram_legend(values: &[f64]) -> Vec<String> {
    let v: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let (m, sd) = mean_sd(&v);
    vec![
        format!("Mean = {}", chart_number(m, CellFormat::Fixed(2))),
        format!("Std. Dev. = {}", chart_number(sd, CellFormat::Fixed(3))),
        format!("N = {}", chart_number(v.len() as f64, CellFormat::Count)),
    ]
}

/// "Nice" bins: about √n of them (5..=30), width rounded to 1, 2, 2.5 or 5 × 10^k.
/// Returns (start, width, counts).
pub fn histogram_bins(values: &[f64]) -> (f64, f64, Vec<usize>) {
    let v: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if v.is_empty() {
        return (0.0, 1.0, vec![]);
    }
    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let k = ((v.len() as f64).sqrt().ceil()).clamp(5.0, 30.0);
    let raw = if hi > lo { (hi - lo) / k } else { 1.0 };
    let p = 10f64.powf(raw.log10().floor());
    let width = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * p)
        .find(|w| *w >= raw)
        .unwrap_or(10.0 * p);
    let start = (lo / width).floor() * width;
    let n = (((hi - start) / width).floor() as usize + 1).max(1);
    let mut counts = vec![0usize; n];
    for x in v {
        let i = (((x - start) / width).floor() as usize).min(n - 1);
        counts[i] += 1;
    }
    (start, width, counts)
}

fn heading_jobs(
    ui: &Ui,
    c: &Chart,
    width: f32,
) -> (
    std::sync::Arc<egui::Galley>,
    Option<std::sync::Arc<egui::Galley>>,
) {
    let p = ui.painter();
    let title = p.layout_job(marked_job(
        c.title(),
        "",
        TITLE_SIZE,
        DATA_COLOR,
        width,
        Align::Center,
    ));
    let sub = c.subtitle().map(|s| {
        p.layout_job(marked_job(
            s,
            "",
            SUB_SIZE,
            DATA_COLOR,
            width,
            Align::Center,
        ))
    });
    (title, sub)
}

fn chart_width(c: &Chart) -> f32 {
    match c {
        Chart::Histogram { .. } => PLOT_W + LEGEND_W,
        _ => PLOT_W,
    }
}

/// Size of a chart item (title, subtitle, plot).
pub fn chart_size(ui: &Ui, c: &Chart) -> Vec2 {
    let w = chart_width(c);
    let (t, s) = heading_jobs(ui, c, w);
    let h = t.size().y + s.map(|s| s.size().y + GAP).unwrap_or(0.0) + GAP * 2.0 + PLOT_H;
    vec2(w, h)
}

/// Draw a chart at the cursor; its id comes from the chart title and the parent id.
pub fn chart_view(ui: &mut Ui, c: &Chart) -> Response {
    let id = ui.id().with(("chart", c.title()));
    chart_view_id(ui, c, id)
}

/// Draw a chart at the cursor with an explicit id (needed when titles repeat).
pub fn chart_view_id(ui: &mut Ui, c: &Chart, id: egui::Id) -> Response {
    let size = chart_size(ui, c);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_chart(ui, c, rect, id);
    }
    let title = c.title().to_owned();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Image, true, &title));
    resp
}

/// Paint a chart into `rect` (as returned by [`chart_size`]).
pub fn paint_chart(ui: &mut Ui, c: &Chart, rect: Rect, id: egui::Id) {
    let w = rect.width();
    let (title, sub) = heading_jobs(ui, c, w);
    let mut y = rect.top();
    paint_bold(
        ui,
        pos2(rect.center().x - title.size().x / 2.0, y),
        &title,
        DATA_COLOR,
    );
    y += title.size().y + GAP;
    if let Some(s) = &sub {
        paint_galley(
            ui,
            pos2(rect.center().x - s.size().x / 2.0, y),
            s,
            DATA_COLOR,
        );
        y += s.size().y + GAP;
    }
    y += GAP;
    let plot_rect = Rect::from_min_size(pos2(rect.left(), y), vec2(PLOT_W, PLOT_H));
    let plot = Plot::new(id.with("plot"))
        .width(PLOT_W)
        .height(PLOT_H)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show_x(false)
        .show_y(false)
        .show_grid(false)
        .sense(Sense::hover());
    let black = Stroke::new(1.2, Color32::BLACK);
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(plot_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    match c {
        Chart::Histogram {
            values,
            normal_curve,
            ..
        } => {
            let (start, width, counts) = histogram_bins(values);
            let bars: Vec<Bar> = counts
                .iter()
                .enumerate()
                .map(|(i, &n)| {
                    Bar::new(start + (i as f64 + 0.5) * width, n as f64)
                        .width(width)
                        .fill(BAR_FILL)
                        .stroke(Stroke::new(1.0, Color32::BLACK))
                })
                .collect();
            let end = start + width * counts.len() as f64;
            let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
            let (m, sd) = mean_sd(&finite);
            let curve = (*normal_curve && sd.is_finite() && sd > 0.0).then(|| {
                let n = finite.len() as f64;
                let (a, b) = ((m - 3.5 * sd).min(start), (m + 3.5 * sd).max(end));
                (0..=200)
                    .map(|i| {
                        let x = a + (b - a) * i as f64 / 200.0;
                        let z = (x - m) / sd;
                        [
                            x,
                            n * width * (-0.5 * z * z).exp()
                                / (sd * (2.0 * std::f64::consts::PI).sqrt()),
                        ]
                    })
                    .collect::<Vec<_>>()
            });
            plot.y_axis_label("Frequency")
                .include_y(0.0)
                .show(&mut child, |p| {
                    p.bar_chart(BarChart::new("bars", bars));
                    if let Some(pts) = curve {
                        p.line(Line::new("normal", PlotPoints::from(pts)).stroke(black));
                    }
                });
            // Legend at the side.
            let lp = ui.painter().clone();
            let mut ly = plot_rect.top() + 12.0;
            for line in histogram_legend(values) {
                let g = lp.layout_job(marked_job(
                    &line,
                    "",
                    SUB_SIZE,
                    LABEL_COLOR,
                    f32::INFINITY,
                    Align::LEFT,
                ));
                let at = pos2(plot_rect.right() + 10.0, ly);
                paint_galley(ui, at, &g, LABEL_COLOR);
                super::table_view::text_node(
                    ui,
                    egui::Rect::from_min_size(at, g.size()),
                    id.with(("legend", ly as i32)),
                    &line,
                );
                ly += g.size().y + 2.0;
            }
        }
        Chart::PPPlot {
            observed_cum,
            expected_cum,
            ..
        } => {
            let pts: Vec<[f64; 2]> = observed_cum
                .iter()
                .zip(expected_cum)
                .map(|(&x, &y)| [x, y])
                .collect();
            plot.x_axis_label("Observed Cum Prob")
                .y_axis_label("Expected Cum Prob")
                .include_x(0.0)
                .include_x(1.0)
                .include_y(0.0)
                .include_y(1.0)
                .show(&mut child, |p| {
                    p.line(
                        Line::new("diagonal", PlotPoints::from(vec![[0.0, 0.0], [1.0, 1.0]]))
                            .stroke(black),
                    );
                    p.points(points(pts));
                });
        }
        Chart::QQPlot {
            observed, expected, ..
        } => {
            let pts: Vec<[f64; 2]> = observed
                .iter()
                .zip(expected)
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .map(|(&x, &y)| [x, y])
                .collect();
            let lo = pts.iter().flat_map(|p| *p).fold(f64::INFINITY, f64::min);
            let hi = pts
                .iter()
                .flat_map(|p| *p)
                .fold(f64::NEG_INFINITY, f64::max);
            plot.x_axis_label("Observed Value")
                .y_axis_label("Expected Normal")
                .show(&mut child, |p| {
                    if lo.is_finite() && hi.is_finite() {
                        p.line(
                            Line::new("reference", PlotPoints::from(vec![[lo, lo], [hi, hi]]))
                                .stroke(black),
                        );
                    }
                    p.points(points(pts));
                });
        }
        Chart::Scatter {
            x_label,
            y_label,
            points: xy,
            ..
        } => {
            let pts: Vec<[f64; 2]> = xy.iter().map(|&(x, y)| [x, y]).collect();
            plot.x_axis_label(x_label.as_str())
                .y_axis_label(y_label.as_str())
                .show(&mut child, |p| {
                    p.points(points(pts));
                });
        }
    }
}

fn points(pts: Vec<[f64; 2]>) -> Points<'static> {
    Points::new("points", PlotPoints::from(pts))
        .shape(MarkerShape::Circle)
        .radius(3.0)
        .filled(false)
        .color(POINT)
}
