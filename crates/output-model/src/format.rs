use crate::table::{Cell, CellFormat, CellValue};

/// Round half away from zero to `n` decimals, SPSS style: no leading zero for |v| < 1,
/// no minus sign when the value rounds to zero. Non-finite -> ".".
fn fixed(v: f64, n: u8) -> String {
    if !v.is_finite() {
        return ".".to_string();
    }
    let p = 10f64.powi(n as i32);
    let scaled = (v.abs() * p).round();
    let r = scaled / p;
    let mut s = format!("{:.*}", n as usize, r);
    if let Some(rest) = s.strip_prefix("0.") {
        s = format!(".{rest}");
    }
    if v < 0.0 && scaled != 0.0 {
        s.insert(0, '-');
    }
    s
}

pub fn format_cell(c: &Cell) -> String {
    let mut s = match &c.value {
        CellValue::Empty => String::new(),
        CellValue::Text(t) => t.clone(),
        CellValue::Num(v) => match c.format {
            CellFormat::Fixed(n) => fixed(*v, n),
            CellFormat::Sig => fixed(*v, 3),
            CellFormat::Count => {
                if v.is_finite() {
                    format!("{:.0}", v.round())
                } else {
                    ".".to_string()
                }
            }
            CellFormat::Text => {
                if v.is_nan() {
                    ".".to_string()
                } else {
                    v.to_string()
                }
            }
        },
    };
    s.extend(c.footnotes.iter());
    s
}
