//! Copying output to the Windows clipboard, the way Word wants it.
//!
//! A table goes on the clipboard in three formats at once: "HTML Format" (CF_HTML, inline
//! styles so Word draws SPSS-like rules), "Rich Text Format" (a simple RTF table) and
//! CF_UNICODETEXT (tab-separated, for Excel and plain editors). "Copy as Image" puts a CF_DIB
//! bitmap. [`cf_html_wrap`], [`table_tsv`], [`table_rtf`], [`dib_from_rgba`] and
//! [`crop_image`] are pure and tested without a clipboard.
use super::table_view::{CellKind, GridCell, table_grid};
use egui::{ColorImage, Rect};
use output_model::{Table, join_markers, render_table_html};

const HTML_PRE: &str = "<html>\r\n<head>\r\n<meta http-equiv=\"Content-Type\" content=\"text/html; charset=utf-8\">\r\n</head>\r\n<body>\r\n<!--StartFragment-->";
const HTML_POST: &str = "<!--EndFragment-->\r\n</body>\r\n</html>";

/// Wrap an HTML fragment in a CF_HTML document: the `Version:0.9` header with the byte offsets
/// (into the UTF-8 text) of the document (`StartHTML`/`EndHTML`) and of the fragment
/// (`StartFragment`/`EndFragment`), each as 10 digits.
pub fn cf_html_wrap(fragment: &str) -> String {
    let header = |sh: usize, eh: usize, sf: usize, ef: usize| {
        format!(
            "Version:0.9\r\nStartHTML:{sh:010}\r\nEndHTML:{eh:010}\r\nStartFragment:{sf:010}\r\nEndFragment:{ef:010}\r\n"
        )
    };
    let hlen = header(0, 0, 0, 0).len();
    let sf = hlen + HTML_PRE.len();
    let ef = sf + fragment.len();
    let eh = ef + HTML_POST.len();
    let mut s = header(hlen, eh, sf, ef);
    s.push_str(HTML_PRE);
    s.push_str(fragment);
    s.push_str(HTML_POST);
    debug_assert_eq!(s.len(), eh);
    s
}

/// The HTML fragment for one table (inline CSS only, `<sup>` markers).
pub fn table_html(t: &Table) -> String {
    render_table_html(t)
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ")
}

/// Tab-separated text: the title, the header rows (a leaf label sits on the bottom header row,
/// above its data), the data rows with their labels, then the footnotes. CRLF line ends.
pub fn table_tsv(t: &Table) -> String {
    let g = table_grid(t);
    let mut m = vec![vec![String::new(); g.cols]; g.rows];
    for c in &g.cells {
        let row = if c.kind == CellKind::ColHeader && c.rowspan > 1 {
            c.row + c.rowspan - 1
        } else {
            c.row
        };
        m[row][c.col] = clean(&format!("{}{}", c.text, c.marks));
    }
    let mut o = format!(
        "{}{}\r\n",
        clean(&t.title),
        join_markers(&t.title_footnotes)
    );
    for row in m {
        o.push_str(&row.join("\t"));
        o.push_str("\r\n");
    }
    for (mk, s) in &t.footnotes {
        o.push_str(&format!("{mk}. {}\r\n", clean(s)));
    }
    o
}

/// RTF-escape text: `\ { }` escaped, line breaks as `\line`, non-ASCII as `\uN?`.
fn rtf_esc(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => o.push_str("\\\\"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '\n' => o.push_str("\\line "),
            '\r' => {}
            '\t' => o.push_str("\\tab "),
            // Other control characters are not valid RTF text.
            c if (c as u32) < 0x20 || c == '\u{7f}' => o.push(' '),
            c if c.is_ascii() => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    o.push_str(&format!("\\u{}?", *u as i16));
                }
            }
        }
    }
    o
}

fn rtf_text(text: &str, marks: &str) -> String {
    let mut o = rtf_esc(text);
    if !marks.is_empty() {
        o.push_str(&format!("{{\\super {}}}", rtf_esc(marks)));
    }
    o
}

/// A simple RTF table (`\trowd`/`\cellx`/`\cell`/`\row`) in the SPSS look: bold centred title,
/// merged spanning labels (`\clmgf`/`\clmrg`, `\clvmgf`/`\clvmrg`), dark rules above the header
/// and below the last row, light rules elsewhere; footnote markers as `{\super a}`.
#[allow(clippy::needless_range_loop)] // walks the grid by (row, column) over two arrays
pub fn table_rtf(t: &Table) -> String {
    let g = table_grid(t);
    // Column widths in twips from the longest text in each column (Arial 9 pt ≈ 100 twips/char).
    let tw = |c: &GridCell| {
        ((c.text.chars().count() + c.marks.chars().count()) as i32 * 100 + 240).max(700)
    };
    let mut w = vec![700i32; g.cols];
    for c in g
        .cells
        .iter()
        .filter(|c| c.colspan == 1 && c.kind != CellKind::ColHeader)
    {
        w[c.col] = w[c.col].max(tw(c));
    }
    for c in g
        .cells
        .iter()
        .filter(|c| c.colspan == 1 && c.kind == CellKind::ColHeader)
    {
        // Column labels may wrap onto two lines.
        w[c.col] = w[c.col].max((tw(c) / 2).max(700));
    }
    let mut cover: Vec<Vec<Option<usize>>> = vec![vec![None; g.cols]; g.rows];
    for (i, c) in g.cells.iter().enumerate() {
        for row in cover.iter_mut().skip(c.row).take(c.rowspan) {
            for slot in row.iter_mut().skip(c.col).take(c.colspan) {
                *slot = Some(i);
            }
        }
    }
    let mut o = String::from(
        "{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1{\\fonttbl{\\f0\\fswiss\\fcharset0 Arial;}}\
         {\\colortbl;\\red21\\green41\\blue53;\\red174\\green174\\blue174;\\red38\\green74\\blue96;\\red1\\green2\\blue5;}\r\n",
    );
    o.push_str(&format!(
        "\\pard\\plain\\f0\\fs20\\qc\\b {}\\b0\\par\r\n",
        rtf_text(&t.title, &join_markers(&t.title_footnotes))
    ));
    let rule = "\\brdrs\\brdrw15\\brdrcf1";
    let light = "\\brdrs\\brdrw10\\brdrcf2";
    for r in 0..g.rows {
        o.push_str("\\trowd\\trgaph70\\trleft0");
        let mut x = 0;
        for c in 0..g.cols {
            let anchor = cover[r][c].map(|i| &g.cells[i]);
            if r == 0 {
                o.push_str(&format!("\\clbrdrt{rule}"));
            }
            let last = anchor
                .map(|a| a.row + a.rowspan == g.rows)
                .unwrap_or(r + 1 == g.rows);
            let header = r < g.header_rows;
            o.push_str(&format!(
                "\\clbrdrb{}",
                if last && !header { rule } else { light }
            ));
            if c == g.row_header_cols && !header {
                o.push_str(&format!("\\clbrdrl{light}"));
            }
            if let Some(a) = anchor {
                if a.rowspan > 1 {
                    o.push_str(if a.row == r { "\\clvmgf" } else { "\\clvmrg" });
                }
                if a.colspan > 1 {
                    o.push_str(if a.col == c { "\\clmgf" } else { "\\clmrg" });
                }
                if a.kind == CellKind::ColHeader {
                    o.push_str("\\clvertalb");
                }
            }
            x += w[c];
            o.push_str(&format!("\\cellx{x}"));
        }
        o.push_str("\r\n");
        for c in 0..g.cols {
            let text = match cover[r][c].map(|i| &g.cells[i]) {
                Some(a) if a.row == r && a.col == c => {
                    let (align, color) = match a.kind {
                        CellKind::ColHeader => ("\\qc", 3),
                        CellKind::RowHeader | CellKind::Corner => ("\\ql", 3),
                        CellKind::Data { numeric: true } => ("\\qr", 4),
                        CellKind::Data { numeric: false } => ("\\ql", 4),
                    };
                    format!("{align}\\cf{color} {}", rtf_text(&a.text, &a.marks))
                }
                _ => String::new(),
            };
            o.push_str(&format!("\\pard\\intbl\\plain\\f0\\fs18{text}\\cell\r\n"));
        }
        o.push_str("\\row\r\n");
    }
    for (mk, s) in &t.footnotes {
        o.push_str(&format!(
            "\\pard\\plain\\f0\\fs16 {}. {}\\par\r\n",
            mk,
            rtf_esc(s)
        ));
    }
    o.push('}');
    o
}

/// A packed DIB (CF_DIB): a 40-byte BITMAPINFOHEADER, then 32-bit BGRA rows bottom-up.
/// `rgba` is top-down RGBA, `w * h * 4` bytes.
pub fn dib_from_rgba(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    let size = w * h * 4;
    let mut d = Vec::with_capacity(40 + size);
    d.extend_from_slice(&40u32.to_le_bytes());
    d.extend_from_slice(&(w as i32).to_le_bytes());
    d.extend_from_slice(&(h as i32).to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&32u16.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    d.extend_from_slice(&(size as u32).to_le_bytes());
    d.extend_from_slice(&3780i32.to_le_bytes()); // 96 dpi
    d.extend_from_slice(&3780i32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    d.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..h).rev() {
        for px in rgba[y * w * 4..(y + 1) * w * 4].chunks_exact(4) {
            d.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    d
}

/// Crop a screenshot (physical pixels) to a rect in points; `None` when nothing is left.
pub fn crop_image(img: &ColorImage, rect: Rect, pixels_per_point: f32) -> Option<ColorImage> {
    let [w, h] = img.size;
    let px = |v: f32, max: usize| ((v * pixels_per_point).round().max(0.0) as usize).min(max);
    let (x0, x1) = (px(rect.min.x, w), px(rect.max.x, w));
    let (y0, y1) = (px(rect.min.y, h), px(rect.max.y, h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut pixels = Vec::with_capacity((x1 - x0) * (y1 - y0));
    for y in y0..y1 {
        pixels.extend_from_slice(&img.pixels[y * w + x0..y * w + x1]);
    }
    Some(ColorImage::new([x1 - x0, y1 - y0], pixels))
}

/// Unpremultiplied RGBA bytes of an image, flattened onto white (Word ignores alpha in DIBs).
pub fn image_rgba(img: &ColorImage) -> Vec<u8> {
    let mut o = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        let a = p.a() as u32;
        // Color32 is premultiplied: over white = c + (255 - a).
        let over = |c: u8| (c as u32 + (255 - a)).min(255) as u8;
        o.extend_from_slice(&[over(p.r()), over(p.g()), over(p.b()), 255]);
    }
    o
}

#[cfg(windows)]
mod win {
    use clipboard_win::{Clipboard, formats, options::NoClear, raw, register_format};

    pub enum Data<'a> {
        Bytes(u32, Vec<u8>),
        Text(&'a str),
    }

    pub fn format(name: &str) -> Result<u32, String> {
        register_format(name)
            .map(|f| f.get())
            .ok_or_else(|| format!("Cannot register the clipboard format {name}."))
    }

    /// Empty the clipboard and put all `items` on it in one go.
    pub fn put(items: &[Data]) -> Result<(), String> {
        let err = |e: clipboard_win::ErrorCode| format!("Cannot write to the clipboard: {e}");
        let _c = Clipboard::new_attempts(10).map_err(err)?;
        raw::empty().map_err(err)?;
        for it in items {
            match it {
                Data::Bytes(f, data) => raw::set_without_clear(*f, data).map_err(err)?,
                Data::Text(t) => raw::set_string_with(t, NoClear).map_err(err)?,
            }
        }
        Ok(())
    }

    pub const DIB: u32 = formats::CF_DIB;
}

fn nul(s: String) -> Vec<u8> {
    let mut b = s.into_bytes();
    b.push(0);
    b
}

/// Put HTML (a fragment, wrapped as CF_HTML), optional RTF and plain text on the clipboard.
pub fn copy_rich(html_fragment: &str, rtf: Option<String>, text: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use win::Data;
        let mut items = vec![Data::Bytes(
            win::format("HTML Format")?,
            nul(cf_html_wrap(html_fragment)),
        )];
        if let Some(r) = rtf {
            items.push(Data::Bytes(win::format("Rich Text Format")?, nul(r)));
        }
        items.push(Data::Text(text));
        win::put(&items)
    }
    #[cfg(not(windows))]
    {
        let _ = (html_fragment, rtf, text, nul);
        Err("Copying to the clipboard is only available on Windows.".into())
    }
}

/// Copy a table: "HTML Format" + "Rich Text Format" + tab-separated text.
pub fn copy_table(t: &Table) -> Result<(), String> {
    copy_rich(&table_html(t), Some(table_rtf(t)), &table_tsv(t))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Copy plain text (titles, notes, warnings, log): HTML paragraph(s) + text.
pub fn copy_text(text: &str, monospace: bool) -> Result<(), String> {
    let font = if monospace {
        "Courier New,monospace"
    } else {
        "Arial,sans-serif"
    };
    let body = esc(text).replace('\n', "<br>");
    let html = format!("<p style=\"font-family:{font};font-size:9pt\">{body}</p>");
    copy_rich(&html, None, &text.replace('\n', "\r\n"))
}

/// Copy an image (top-down RGBA, `w * h * 4` bytes) as a CF_DIB bitmap.
pub fn copy_image(rgba: &[u8], w: usize, h: usize) -> Result<(), String> {
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return Err("Nothing to copy: the image is empty.".into());
    }
    #[cfg(windows)]
    {
        win::put(&[win::Data::Bytes(win::DIB, dib_from_rgba(rgba, w, h))])
    }
    #[cfg(not(windows))]
    {
        Err("Copying to the clipboard is only available on Windows.".into())
    }
}
