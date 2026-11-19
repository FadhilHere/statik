//! Output model: tables, charts and SPSS-style number formatting.
mod chart;
mod format;
mod render_html;
mod render_text;
mod table;

pub use chart::Chart;
pub use format::{format_cell, format_markers, format_value, join_markers};
pub use render_html::{render_html, render_table_html};
pub use render_text::render_text;
pub use table::{
    Cell, CellFormat, CellValue, HeaderCell, HeaderNode, Label, OutputItem, RowHeaderCell, Table, header_cells, header_depth,
    leaf_count, leaf_paths, row_header_cells, row_label,
};
