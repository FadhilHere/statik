//! Shared constants, byte-order handling and variable layout of the SPSS system file
//! format (as documented in the GNU PSPP manual, "System File Format").
use crate::SavError;

/// System-missing value as written by SPSS (`-DBL_MAX`).
pub(crate) const SYSMIS: f64 = -f64::MAX;
/// `HIGHEST` in missing-value ranges (`DBL_MAX`).
pub(crate) const HIGHEST: f64 = f64::MAX;
/// `LOWEST` in missing-value ranges: the next double above `-DBL_MAX`.
pub(crate) const LOWEST_BITS: u64 = 0xffef_ffff_ffff_fffe;
pub(crate) const HEADER_LEN: usize = 176;

pub(crate) const REC_VARIABLE: i32 = 2;
pub(crate) const REC_VALUE_LABELS: i32 = 3;
pub(crate) const REC_VALUE_LABEL_VARS: i32 = 4;
pub(crate) const REC_DOCUMENT: i32 = 6;
pub(crate) const REC_EXTENSION: i32 = 7;
pub(crate) const REC_END: i32 = 999;

pub(crate) const SUB_INTEGER_INFO: i32 = 3;
pub(crate) const SUB_FLOAT_INFO: i32 = 4;
pub(crate) const SUB_DISPLAY: i32 = 11;
pub(crate) const SUB_LONG_NAMES: i32 = 13;
pub(crate) const SUB_VERY_LONG_STRINGS: i32 = 14;
pub(crate) const SUB_ENCODING: i32 = 20;

/// SPSS format type codes used for print/write formats.
pub(crate) const FMT_A: u32 = 1;
pub(crate) const FMT_F: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Endian {
    Little,
    Big,
}

pub(crate) fn format_error(msg: impl Into<String>) -> SavError {
    SavError::Format(msg.into())
}

/// Bounds-checked reader over the whole file.
pub(crate) struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    pub endian: Endian,
}

impl<'a> Cursor<'a> {
    pub fn new(buf: &'a [u8], pos: usize, endian: Endian) -> Self {
        Cursor { buf, pos, endian }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], SavError> {
        if n > self.remaining() {
            return Err(format_error(format!("unexpected end of file at byte {}", self.pos)));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn take8(&mut self) -> Result<[u8; 8], SavError> {
        Ok(self.take(8)?.try_into().expect("8 bytes"))
    }

    pub fn i32(&mut self) -> Result<i32, SavError> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("4 bytes");
        Ok(match self.endian {
            Endian::Little => i32::from_le_bytes(b),
            Endian::Big => i32::from_be_bytes(b),
        })
    }

    pub fn f64(&mut self) -> Result<f64, SavError> {
        let b = self.take8()?;
        Ok(f64_from(b, self.endian))
    }

    /// A non-negative count whose `elem`-byte items must fit in the rest of the file.
    pub fn count(&mut self, elem: usize, what: &str) -> Result<usize, SavError> {
        let n = self.i32()?;
        let n = usize::try_from(n).map_err(|_| format_error(format!("negative {what} count")))?;
        match n.checked_mul(elem) {
            Some(bytes) if bytes <= self.remaining() => Ok(n),
            _ => Err(format_error(format!("{what} count {n} exceeds file size"))),
        }
    }
}

pub(crate) fn f64_from(b: [u8; 8], endian: Endian) -> f64 {
    match endian {
        Endian::Little => f64::from_le_bytes(b),
        Endian::Big => f64::from_be_bytes(b),
    }
}

/// Number of physical variables ("segments") a string of `width` bytes occupies.
/// Strings wider than 255 bytes are "very long strings": the segment count is
/// `ceil(width / 252)`, every segment but the last is declared 255 bytes wide.
pub(crate) fn segment_count(width: usize) -> usize {
    if width < 256 { 1 } else { width.div_ceil(252) }
}

/// Declared width of segment `i` (255 for all but the last segment of a very long string).
pub(crate) fn segment_width(width: usize, i: usize) -> usize {
    if width < 256 {
        width
    } else if i + 1 < segment_count(width) {
        255
    } else {
        width - i * 252
    }
}

/// Byte range of the value stored in segment `i`. The value is split into consecutive
/// 255-byte pieces (as SPSS and ReadStat write it), so the last segment, declared
/// `width - 252 * i` wide, ends with a few bytes of padding.
pub(crate) fn segment_range(width: usize, i: usize) -> std::ops::Range<usize> {
    if width < 256 {
        0..width
    } else {
        let start = (i * 255).min(width);
        start..(start + 255).min(width)
    }
}

/// 8-byte slots used by a physical variable of the given type code (0 = numeric).
pub(crate) fn slots(type_width: usize) -> usize {
    if type_width == 0 { 1 } else { type_width.div_ceil(8) }
}

pub(crate) fn make_format(kind: u32, width: usize, decimals: usize) -> i32 {
    ((kind << 16) | ((width as u32 & 0xff) << 8) | (decimals as u32 & 0xff)) as i32
}

/// (type, width, decimals) of a packed print/write format.
pub(crate) fn split_format(f: i32) -> (u32, usize, usize) {
    let f = f as u32;
    ((f >> 16) & 0xff, ((f >> 8) & 0xff) as usize, (f & 0xff) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn very_long_string_layout() {
        assert_eq!(segment_count(255), 1);
        assert_eq!(segment_count(256), 2);
        assert_eq!(segment_count(300), 2);
        assert_eq!(segment_count(504), 2);
        assert_eq!(segment_count(505), 3);
        assert_eq!((segment_width(300, 0), segment_width(300, 1)), (255, 48));
        assert_eq!((segment_range(300, 0), segment_range(300, 1)), (0..255, 255..300));
        assert_eq!(segment_range(600, 2), 510..600);
        assert_eq!(segment_width(600, 2), 96);
        assert_eq!(slots(255), 32);
        assert_eq!(slots(0), 1);
    }

    #[test]
    fn format_packing() {
        let f = make_format(FMT_F, 8, 2);
        assert_eq!(f, 0x0005_0802);
        assert_eq!(split_format(f), (FMT_F, 8, 2));
    }
}
