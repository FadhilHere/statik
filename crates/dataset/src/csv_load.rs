use crate::dataset::{Column, Dataset, DatasetError};
use crate::variable::*;
use std::path::Path;

fn decimals_of(cell: &str) -> usize {
    let mant = cell.split(['e', 'E']).next().unwrap_or(cell);
    mant.split_once('.').map_or(0, |(_, f)| f.len())
}

/// Row 1 = variable names. A column is numeric if every non-empty cell parses
/// as a finite f64 (empty cells become NaN); otherwise it is text.
pub fn load_csv(path: &Path) -> Result<Dataset, DatasetError> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|e| DatasetError::Csv(e.to_string()))?;
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| DatasetError::Csv(e.to_string()))?
        .iter()
        .map(|h| h.trim_start_matches('\u{feff}').trim().to_string())
        .collect();
    let mut raw: Vec<Vec<String>> = vec![Vec::new(); headers.len()];
    for rec in rdr.records() {
        let rec = rec.map_err(|e| DatasetError::Csv(e.to_string()))?;
        for (i, col) in raw.iter_mut().enumerate() {
            col.push(rec.get(i).unwrap_or("").trim().to_string());
        }
    }
    let mut ds = Dataset::new();
    for (name, cells) in headers.iter().zip(raw) {
        let numeric = cells.iter().all(|c| c.is_empty() || c.parse::<f64>().is_ok_and(f64::is_finite));
        if numeric {
            let vals = cells
                .iter()
                .map(|c| if c.is_empty() { f64::NAN } else { c.parse().unwrap() })
                .collect();
            let mut v = Variable::numeric(name);
            v.decimals = cells.iter().map(|c| decimals_of(c)).max().unwrap_or(0).min(16);
            ds.add_variable(v, Column::Numeric(vals))?;
        } else {
            let width = cells.iter().map(|c| c.chars().count()).max().unwrap_or(0).max(1);
            ds.add_variable(Variable::string(name, width), Column::Text(cells))?;
        }
    }
    Ok(ds)
}
