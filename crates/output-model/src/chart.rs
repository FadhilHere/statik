use serde::{Deserialize, Serialize};

/// Charts. `title` is a single line; extra context (e.g. "Dependent Variable: y") goes in `subtitle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Chart {
    Histogram { title: String, subtitle: Option<String>, values: Vec<f64>, normal_curve: bool },
    PPPlot { title: String, subtitle: Option<String>, observed_cum: Vec<f64>, expected_cum: Vec<f64> },
    QQPlot { title: String, subtitle: Option<String>, observed: Vec<f64>, expected: Vec<f64> },
    Scatter { title: String, subtitle: Option<String>, x_label: String, y_label: String, points: Vec<(f64, f64)> },
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

    pub fn subtitle(&self) -> Option<&str> {
        match self {
            Chart::Histogram { subtitle, .. }
            | Chart::PPPlot { subtitle, .. }
            | Chart::QQPlot { subtitle, .. }
            | Chart::Scatter { subtitle, .. } => subtitle.as_deref(),
        }
    }
}
