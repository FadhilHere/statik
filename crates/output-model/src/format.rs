use crate::table::{Cell, CellFormat, CellValue};

/// Round half away from zero to `n` decimals, SPSS style: no leading zero for |v| < 1,
/// no minus sign when the value rounds to zero. Non-finite -> ".".
fn fixed(v: f64, n: u8) -> String {
    if !v.is_finite() {
        return ".".to_string();
    }
    let p = 10f64.powi(n as i32);
    // Beyond 2^52 every f64 is an integer and v·10^n would lose digits: format directly.
    if v.abs() * p >= 4_503_599_627_370_496.0 {
        return format!("{:.*}", n as usize, v);
    }
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

/// Cell text including footnote markers (comma-joined, SPSS style: `.200c,d`).
pub fn format_cell(c: &Cell) -> String {
    format_value(c) + &format_markers(c)
}

/// Footnote markers of a cell joined with ",", or "" when there are none.
pub fn format_markers(c: &Cell) -> String {
    join_markers(&c.footnotes)
}

/// Footnote markers joined with ",", or "" when there are none (titles, headers, cells).
pub fn join_markers(marks: &[char]) -> String {
    marks.iter().map(char::to_string).collect::<Vec<_>>().join(",")
}

/// Cell text without footnote markers.
pub fn format_value(c: &Cell) -> String {
    match &c.value {
        CellValue::Empty => String::new(),
        CellValue::Text(t) => t.clone(),
        CellValue::Num(v) => match c.format {
            CellFormat::Fixed(n) => fixed(*v, n),
            CellFormat::Sig => fixed(*v, 3),
            CellFormat::Count => {
                if v.is_finite() {
                    let r = v.round();
                    if r == 0.0 { "0".to_string() } else { format!("{:.0}", r) }
                } else {
                    ".".to_string()
                }
            }
            CellFormat::Text => {
                if v.is_finite() {
                    v.to_string()
                } else {
                    ".".to_string()
                }
            }
        },
    }
}
