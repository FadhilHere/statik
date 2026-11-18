//! Descriptives (SPSS DESCRIPTIVES, optionally /SAVE standardized values).
use crate::cells::{empty, fixed_plus, group, label, leaf, num, num_or_empty, warning};
use dataset::{Column, Dataset, DatasetError, VarId, VarType, Variable};
use output_model::{CellFormat, OutputItem, Table};
use stat_core::describe;

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptivesOptions {
    pub vars: Vec<VarId>,
    pub skew_kurt: bool,
    pub save_z: bool,
}

/// Run the procedure. With `save_z`, a variable `Z<name>` is added per analysed variable (`ZSC001`, ... when the
/// name is taken). Each variable uses its own valid cases; the table's "Valid N (listwise)" row uses complete cases.
pub fn run_descriptives(ds: &mut Dataset, o: &DescriptivesOptions) -> Vec<OutputItem> {
    let mut out = vec![OutputItem::Title("Descriptives".into())];
    let mut names = vec![];
    let mut cells = vec![];
    let mut warns = vec![];
    let mut zs: Vec<(VarId, Vec<f64>)> = vec![];
    let mut used = vec![];
    for &id in &o.vars {
        let var = ds.var(id);
        if !matches!(var.var_type, VarType::Numeric) {
            warns.extend(warning(format!("Descriptives: variable {} is not numeric. It has been omitted.", var.name)));
            continue;
        }
        let x: Vec<f64> = (0..ds.n_cases()).filter(|&r| !ds.is_missing(id, r)).map(|r| ds.numeric(id)[r]).collect();
        let Some(d) = describe(&x) else {
            warns.extend(warning(format!("Descriptives: variable {} has no valid cases. It has been omitted.", var.name)));
            continue;
        };
        let dec = var.decimals;
        let mut row = vec![
            num(d.n as f64, CellFormat::Count),
            num(d.min, fixed_plus(dec, 0)),
            num(d.max, fixed_plus(dec, 0)),
            num(d.mean, fixed_plus(dec, 2)),
            num_or_empty(d.sd, fixed_plus(dec, 3)),
        ];
        if o.skew_kurt {
            let f3 = CellFormat::Fixed(3);
            let opt = |v: Option<f64>| v.map_or_else(empty, |v| num(v, f3));
            row.extend([opt(d.skewness), opt(d.se_skewness), opt(d.kurtosis), opt(d.se_kurtosis)]);
        }
        names.push(label(ds, id));
        cells.push(row);
        used.push(id);
        if o.save_z {
            // Constant data: describe forces sd to exactly 0, so test min == max, never sd <= 0.
            if d.n < 2 || d.min == d.max || !d.sd.is_finite() {
                warns.extend(warning(format!(
                    "Descriptives: variable {} is constant or has fewer than 2 valid cases, so its Z scores cannot be \
                     computed. They are set to system-missing.",
                    var.name
                )));
                zs.push((id, vec![f64::NAN; ds.n_cases()]));
            } else {
                let z = (0..ds.n_cases())
                    .map(|r| if ds.is_missing(id, r) { f64::NAN } else { (ds.numeric(id)[r] - d.mean) / d.sd })
                    .collect();
                zs.push((id, z));
            }
        }
    }
    if cells.is_empty() {
        out.extend(warns);
        return out;
    }
    let listwise = ds.listwise_rows(&used).len();
    let width = cells[0].len();
    let mut last = vec![num(listwise as f64, CellFormat::Count)];
    last.resize(width, empty());
    cells.push(last);
    names.push("Valid N (listwise)".into());
    let mut col_headers = vec![leaf("N"), leaf("Minimum"), leaf("Maximum"), leaf("Mean"), leaf("Std. Deviation")];
    if o.skew_kurt {
        col_headers.push(group("Skewness", &["Statistic", "Std. Error"]));
        col_headers.push(group("Kurtosis", &["Statistic", "Std. Error"]));
    }
    out.push(OutputItem::Table(Table {
        title: "Descriptive Statistics".into(),
        col_headers,
        row_headers: names.iter().map(|s| leaf(s)).collect(),
        cells,
        footnotes: vec![],
    }));
    out.extend(warns);
    let mut zsc = 0;
    for (id, z) in zs {
        let (src_name, src_label) = (ds.var(id).name.clone(), ds.var(id).label.clone().filter(|l| !l.is_empty()));
        let make = |name: &str| {
            let mut v = Variable::numeric(name);
            v.decimals = 5;
            v.width = 11;
            v.label = Some(match &src_label {
                Some(l) => format!("Zscore:  {l}"),
                None => format!("Zscore({src_name})"),
            });
            v
        };
        let mut zname = format!("Z{src_name}");
        loop {
            if ds.find(&zname).is_none() {
                match ds.add_variable(make(&zname), Column::Numeric(z.clone())) {
                    Ok(_) => break,
                    // A name that is invalid (e.g. too long) falls through to ZSC###; other errors cannot be fixed by renaming.
                    Err(DatasetError::InvalidName(_)) | Err(DatasetError::DuplicateName(_)) => {}
                    Err(_) => {
                        zsc = 1000;
                    }
                }
            }
            loop {
                zsc += 1;
                if zsc > 999 {
                    break;
                }
                zname = format!("ZSC{zsc:03}");
                if ds.find(&zname).is_none() {
                    break;
                }
            }
            if zsc > 999 {
                out.extend(warning(format!("Descriptives: no name could be created for the Z scores of {src_name}.")));
                break;
            }
        }
    }
    out
}
