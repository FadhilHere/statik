//! Clipboard formats for "Copy" in the Output viewer (paste into Word / Excel).
//!
//! The tests that touch the real Windows clipboard hold `CLIP` so they never overlap (cargo runs
//! test binaries one after another, so a process-wide lock is enough).
use output_model::{Cell, CellFormat, CellValue, HeaderNode, Table};
use statik_app::output::clipboard::{
    cf_html_wrap, crop_image, dib_from_rgba, table_html, table_rtf, table_tsv,
};
use std::sync::Mutex;

static CLIP: Mutex<()> = Mutex::new(());

fn hn(s: &str, f: &[char], children: Vec<HeaderNode>) -> HeaderNode {
    HeaderNode {
        label: s.into(),
        footnotes: f.to_vec(),
        children,
    }
}

fn num(v: f64, f: &[char]) -> Cell {
    Cell {
        value: CellValue::Num(v),
        format: CellFormat::Fixed(3),
        footnotes: f.to_vec(),
    }
}

/// A small Coefficients-like table: nested column headers, a row group, markers, non-ASCII.
fn table() -> Table {
    Table {
        title: "Coefficients".into(),
        title_footnotes: vec!['a'],
        col_headers: vec![
            hn(
                "Unstandardized Coefficients",
                &[],
                vec![hn("B", &[], vec![]), hn("Std. Error", &[], vec![])],
            ),
            hn("Sig.", &['b'], vec![]),
        ],
        row_headers: vec![hn(
            "1",
            &[],
            vec![hn("(Constant)", &[], vec![]), hn("x1 {é}", &[], vec![])],
        )],
        cells: vec![
            vec![num(1.5, &[]), num(0.25, &[]), num(0.0004, &['c'])],
            vec![num(-0.5, &[]), num(2.0, &[]), num(0.123, &[])],
        ],
        footnotes: vec![
            ('a', "Dependent Variable: y".into()),
            ('b', "Two-sided \\ test".into()),
        ],
    }
}

fn header_num(h: &str, key: &str) -> usize {
    let line = h
        .lines()
        .find(|l| l.starts_with(&format!("{key}:")))
        .unwrap_or_else(|| panic!("no {key} in {h}"));
    let v = &line[key.len() + 1..];
    assert_eq!(v.len(), 10, "{key} is not 10 digits: {line}");
    v.parse().unwrap()
}

#[test]
fn cf_html_offsets_point_exactly_at_the_fragment() {
    let frag = "<table><tr><td>Sig. ≥ é ü 日本</td></tr></table>";
    let s = cf_html_wrap(frag);
    let b = s.as_bytes();
    assert!(s.starts_with("Version:0.9\r\nStartHTML:"), "{s}");
    let (sh, eh) = (header_num(&s, "StartHTML"), header_num(&s, "EndHTML"));
    let (sf, ef) = (
        header_num(&s, "StartFragment"),
        header_num(&s, "EndFragment"),
    );
    assert_eq!(&b[sf..ef], frag.as_bytes());
    assert!(b[sh..].starts_with(b"<html>"), "{}", &s[sh..]);
    assert_eq!(eh, b.len());
    assert!(s[..sf].ends_with("<!--StartFragment-->"));
    assert!(s[ef..].starts_with("<!--EndFragment-->"));
    assert!(s[sh..eh].contains("charset=utf-8"), "{s}");
}

#[test]
fn tsv_has_title_headers_rows_and_footnotes() {
    let t = table_tsv(&table());
    let expected = "Coefficientsa\r\n\
                    \t\tUnstandardized Coefficients\t\t\r\n\
                    \t\tB\tStd. Error\tSig.b\r\n\
                    1\t(Constant)\t1.500\t.250\t.000c\r\n\
                    \tx1 {é}\t-.500\t2.000\t.123\r\n\
                    a. Dependent Variable: y\r\n\
                    b. Two-sided \\ test\r\n";
    assert_eq!(t, expected);
}

#[test]
fn html_fragment_is_the_inline_styled_table() {
    let h = table_html(&table());
    assert!(h.starts_with("<table"), "{h}");
    assert!(h.contains("Coefficients<sup>a</sup>"), "{h}");
    assert!(h.contains(">.000<sup>c</sup></td>"), "{h}");
    assert!(h.contains("colspan=\"2\""), "{h}");
    assert!(h.contains("rowspan=\"2\""), "{h}");
}

#[test]
fn rtf_is_a_table_with_escaped_text_and_superscript_markers() {
    let r = table_rtf(&table());
    assert!(r.starts_with("{\\rtf1"), "{r}");
    assert!(r.ends_with('}'), "{r}");
    assert!(r.contains("\\trowd"), "{r}");
    assert!(r.contains("\\cellx"), "{r}");
    assert!(r.contains("\\cell"), "{r}");
    assert!(r.contains("\\row"), "{r}");
    assert!(r.contains("Coefficients{\\super a}"), "{r}");
    assert!(r.contains(".000{\\super c}"), "{r}");
    // Braces, backslashes and non-ASCII are escaped.
    assert!(r.contains("x1 \\{\\u233?\\}"), "{r}");
    assert!(r.contains("Two-sided \\\\ test"), "{r}");
    assert!(r.is_ascii());
    // Horizontal merge for the spanning header.
    assert!(r.contains("\\clmgf"), "{r}");
    assert!(r.contains("\\clmrg"), "{r}");
    // Balanced braces.
    let mut depth = 0i32;
    let mut prev = ' ';
    for c in r.chars() {
        if prev != '\\' {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
            }
        }
        prev = if prev == '\\' && c == '\\' { ' ' } else { c };
        assert!(depth >= 0);
    }
    assert_eq!(depth, 0, "{r}");
}

#[test]
fn dib_is_bottom_up_bgra_with_infoheader() {
    // 2x2: red, green / blue, white (top row first).
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let d = dib_from_rgba(&rgba, 2, 2);
    let u32_at = |i: usize| u32::from_le_bytes(d[i..i + 4].try_into().unwrap());
    let i32_at = |i: usize| i32::from_le_bytes(d[i..i + 4].try_into().unwrap());
    let u16_at = |i: usize| u16::from_le_bytes(d[i..i + 2].try_into().unwrap());
    assert_eq!(u32_at(0), 40); // biSize
    assert_eq!(i32_at(4), 2); // width
    assert_eq!(i32_at(8), 2); // height > 0: bottom-up
    assert_eq!(u16_at(12), 1); // planes
    assert_eq!(u16_at(14), 32); // bpp
    assert_eq!(u32_at(16), 0); // BI_RGB
    assert_eq!(u32_at(20), 16); // image size
    assert_eq!(d.len(), 40 + 16);
    // First stored row is the bottom one: blue, white (as BGRA).
    assert_eq!(&d[40..48], &[255, 0, 0, 255, 255, 255, 255, 255]);
    // Then the top row: red, green.
    assert_eq!(&d[48..56], &[0, 0, 255, 255, 0, 255, 0, 255]);
}

#[test]
fn crop_respects_pixels_per_point() {
    // 8x6 physical pixels, each pixel encodes its (x, y).
    let (w, h) = (8usize, 6usize);
    let pixels = (0..h)
        .flat_map(|y| (0..w).map(move |x| egui::Color32::from_rgb(x as u8, y as u8, 0)))
        .collect();
    let img = egui::ColorImage::new([w, h], pixels);
    // Logical rect (1,1)-(3,2) at 2 px per point = physical (2,2)-(6,4).
    let r = egui::Rect::from_min_max(egui::pos2(1.0, 1.0), egui::pos2(3.0, 2.0));
    let c = crop_image(&img, r, 2.0).unwrap();
    assert_eq!(c.size, [4, 2]);
    assert_eq!(c.pixels[0], egui::Color32::from_rgb(2, 2, 0));
    assert_eq!(c.pixels[7], egui::Color32::from_rgb(5, 3, 0));
    // Clamped to the image; empty when outside.
    let big = egui::Rect::from_min_max(egui::pos2(3.0, 2.0), egui::pos2(100.0, 100.0));
    assert_eq!(crop_image(&img, big, 2.0).unwrap().size, [2, 2]);
    let out = egui::Rect::from_min_max(egui::pos2(50.0, 50.0), egui::pos2(60.0, 60.0));
    assert!(crop_image(&img, out, 2.0).is_none());
}

#[cfg(windows)]
mod windows_clipboard {
    use super::*;
    use clipboard_win::{Clipboard, formats, raw, register_format};
    use statik_app::output::clipboard::{copy_image, copy_table};

    fn read(format: u32) -> Option<Vec<u8>> {
        let _c = Clipboard::new_attempts(20).unwrap();
        if !raw::is_format_avail(format) {
            return None;
        }
        let mut out = Vec::new();
        raw::get_vec(format, &mut out).ok()?;
        Some(out)
    }

    fn read_text() -> String {
        let _c = Clipboard::new_attempts(20).unwrap();
        let mut out = Vec::new();
        raw::get_string(&mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn copy_table_puts_html_text_and_rtf_on_the_clipboard() {
        let _g = CLIP.lock().unwrap_or_else(|e| e.into_inner());
        let t = table();
        copy_table(&t).unwrap();

        let html_fmt = register_format("HTML Format").unwrap().get();
        let html = read(html_fmt).expect("HTML Format on the clipboard");
        let html = String::from_utf8(html).unwrap();
        // GlobalSize may round up: the data ends at the first NUL.
        let html = html.split('\0').next().unwrap();
        let sf = header_num(html, "StartFragment");
        let ef = header_num(html, "EndFragment");
        assert_eq!(&html.as_bytes()[sf..ef], table_html(&t).as_bytes());
        assert_eq!(header_num(html, "EndHTML"), html.len());

        assert_eq!(read_text(), table_tsv(&t));

        let rtf_fmt = register_format("Rich Text Format").unwrap().get();
        let rtf = read(rtf_fmt).expect("RTF on the clipboard");
        let rtf = String::from_utf8(rtf).unwrap();
        assert_eq!(rtf.split('\0').next().unwrap(), table_rtf(&t));
    }

    #[test]
    fn copy_image_puts_a_dib_on_the_clipboard() {
        let _g = CLIP.lock().unwrap_or_else(|e| e.into_inner());
        let rgba: Vec<u8> = (0..3 * 2)
            .flat_map(|i| [i as u8 * 40, 10, 20, 255])
            .collect();
        copy_image(&rgba, 3, 2).unwrap();
        let dib = read(formats::CF_DIB).expect("CF_DIB on the clipboard");
        assert_eq!(&dib[..40 + 24], &dib_from_rgba(&rgba, 3, 2)[..]);
    }
}
