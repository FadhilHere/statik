#[derive(Debug, Clone, PartialEq)]
pub enum Chart {
    Histogram { title: String, values: Vec<f64>, normal_curve: bool },
    PPPlot { title: String, observed_cum: Vec<f64>, expected_cum: Vec<f64> },
    QQPlot { title: String, observed: Vec<f64>, expected: Vec<f64> },
    Scatter { title: String, x_label: String, y_label: String, points: Vec<(f64, f64)> },
}

impl Chart {
    pub fn title(&self) -> &str {
        match self {
            Chart::Histogram { title, .. }
            | Chart::PPPlot { title, .. }
            | Chart::QQPlot { title, .. }
            | Chart::Scatter { title, .. } => title,
        }
    }
}
