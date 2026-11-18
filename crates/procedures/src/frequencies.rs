//! Frequencies (SPSS FREQUENCIES with optional histogram).
use crate::cells::{empty, group, label, leaf, num, parent, warning};
use dataset::{Dataset, OrderedValue, VarId, VarType};
use output_model::{Cell, CellFormat, Chart, HeaderNode, OutputItem, Table};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct FrequenciesOptions {
    pub vars: Vec<VarId>,
    pub histogram: bool,
}

/// Run the procedure: a Statistics table, then one frequency table (and optional histogram) per variable.
pub fn run_frequencies(ds: &Dataset, o: &FrequenciesOptions) -> Vec<OutputItem> {
    let mut out = vec![OutputItem::Title("Frequencies".into())];
    let mut stat_h = vec![];
    let mut stat_cols = vec![];
    let mut body = vec![];
    for &id in &o.vars {
        let var = ds.var(id);
        if !matches!(var.var_type, VarType::Numeric) {
            out.extend(warning(format!("Frequencies: variable {} is not numeric. It has been omitted.", var.name)));
            continue;
        }
        let total = ds.n_cases();
        let mut valid: BTreeMap<OrderedValue, usize> = BTreeMap::new();
        let mut user: BTreeMap<OrderedValue, usize> = BTreeMap::new();
        let mut system = 0usize;
        let mut values = vec![];
        for r in 0..total {
            let v = ds.numeric(id)[r];
            if v.is_nan() {
                system += 1;
            } else if ds.is_missing(id, r) {
                *user.entry(OrderedValue::new(v)).or_default() += 1;
            } else {
                *valid.entry(OrderedValue::new(v)).or_default() += 1;
                values.push(v);
            }
        }
        let n_valid: usize = valid.values().sum();
        let n_missing = total - n_valid;
        let name = label(ds, id);
        stat_h.push(leaf(&name));
        stat_cols.push((n_valid, n_missing));

        let vlabel =
            |v: f64| var.value_labels.get(&OrderedValue(v)).cloned().unwrap_or_else(|| format!("{:.*}", var.decimals, v + 0.0));
        let pct = |c: usize, d: usize| 100.0 * c as f64 / d as f64;
        let f1 = CellFormat::Fixed(1);
        let freq = |c: usize| num(c as f64, CellFormat::Count);
        let mut rows: Vec<Vec<Cell>> = vec![];
        let mut valid_rows: Vec<HeaderNode> = vec![];
        let mut cum = 0usize;
        for (k, &c) in &valid {
            cum += c;
            valid_rows.push(leaf(&vlabel(k.0)));
            rows.push(vec![freq(c), num(pct(c, total), f1), num(pct(c, n_valid), f1), num(pct(cum, n_valid), f1)]);
        }
        valid_rows.push(leaf("Total"));
        rows.push(vec![freq(n_valid), num(pct(n_valid, total), f1), num(pct(n_valid, n_valid), f1), empty()]);
        let mut row_headers = vec![parent("Valid", valid_rows)];
        if n_missing > 0 {
            let mut missing_rows = vec![];
            for (k, &c) in &user {
                missing_rows.push(leaf(&vlabel(k.0)));
                rows.push(vec![freq(c), num(pct(c, total), f1), empty(), empty()]);
            }
            if system > 0 {
                missing_rows.push(leaf("System"));
                rows.push(vec![freq(system), num(pct(system, total), f1), empty(), empty()]);
            }
            missing_rows.push(leaf("Total"));
            rows.push(vec![freq(n_missing), num(pct(n_missing, total), f1), empty(), empty()]);
            row_headers.push(parent("Missing", missing_rows));
            row_headers.push(leaf("Total"));
            rows.push(vec![freq(total), num(100.0, f1), empty(), empty()]);
        }
        body.push(OutputItem::Table(Table {
            title: name.clone(),
            col_headers: ["Frequency", "Percent", "Valid Percent", "Cumulative Percent"].map(leaf).to_vec(),
            row_headers,
            cells: rows,
            footnotes: vec![],
        }));
        if o.histogram && !values.is_empty() {
            body.push(OutputItem::Chart(Chart::Histogram {
                title: format!("Histogram\n{name}"),
                values,
                normal_curve: false,
            }));
        }
    }
    if stat_cols.is_empty() {
        return out;
    }
    let count = |v: usize| num(v as f64, CellFormat::Count);
    out.push(OutputItem::Table(Table {
        title: "Statistics".into(),
        col_headers: stat_h,
        row_headers: vec![group("N", &["Valid", "Missing"])],
        cells: vec![stat_cols.iter().map(|c| count(c.0)).collect(), stat_cols.iter().map(|c| count(c.1)).collect()],
        footnotes: vec![],
    }));
    out.extend(body);
    out
}
