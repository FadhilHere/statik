use crate::table::num_vec;
use serde::{Deserialize, Serialize};

/// Charts. `title` is a single line; extra context (e.g. "Dependent Variable: y") goes in `subtitle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Numbers are serialized NaN-safe (NaN and the infinities as strings), like table cells.
pub enum Chart {
    Histogram {
        title: String,
        subtitle: Option<String>,
        /// Caption of the x axis ("Regression Standardized Residual", the variable label).
        #[serde(default)]
        x_label: String,
        #[serde(with = "num_vec")]
        values: Vec<f64>,
        normal_curve: bool,
    },
    PPPlot {
        title: String,
        subtitle: Option<String>,
        #[serde(with = "num_vec")]
        observed_cum: Vec<f64>,
        #[serde(with = "num_vec")]
        expected_cum: Vec<f64>,
    },
    QQPlot {
        title: String,
        subtitle: Option<String>,
        #[serde(with = "num_vec")]
        observed: Vec<f64>,
        #[serde(with = "num_vec")]
        expected: Vec<f64>,
    },
    Scatter {
        title: String,
        subtitle: Option<String>,
        x_label: String,
        y_label: String,
        #[serde(with = "num_vec::pairs")]
        points: Vec<(f64, f64)>,
    },
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
