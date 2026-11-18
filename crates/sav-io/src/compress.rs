//! Bytecode compression of case data (`compression = 1`).
//!
//! Data is a sequence of 8-byte command blocks. Each command byte describes one 8-byte
//! slot: 0 = padding (ignored), 1–251 = the number `code - bias`, 252 = end of data,
//! 253 = the slot is stored raw in the 8 bytes following the command block,
//! 254 = eight spaces, 255 = system-missing.
use crate::records::Cursor;
use crate::SavError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Slot {
    Number(f64),
    Raw([u8; 8]),
    Spaces,
    Sysmis,
}

pub(crate) struct Decompressor<'a, 'b> {
    cur: &'b mut Cursor<'a>,
    bias: f64,
    block: [u8; 8],
    next: usize,
    done: bool,
}

impl<'a, 'b> Decompressor<'a, 'b> {
    pub fn new(cur: &'b mut Cursor<'a>, bias: f64) -> Self {
        Decompressor { cur, bias, block: [0; 8], next: 8, done: false }
    }

    /// The next slot, or `None` at the end of the data (code 252 or end of file).
    /// A command block cut short by the end of the file is a format error.
    pub fn next_slot(&mut self) -> Result<Option<Slot>, SavError> {
        loop {
            if self.done {
                return Ok(None);
            }
            if self.next == 8 {
                if self.cur.remaining() == 0 {
                    self.done = true;
                    return Ok(None);
                }
                self.block = self.cur.take8()?;
                self.next = 0;
            }
            let code = self.block[self.next];
            self.next += 1;
            match code {
                0 => continue,
                252 => {
                    self.done = true;
                    return Ok(None);
                }
                253 => return Ok(Some(Slot::Raw(self.cur.take8()?))),
                254 => return Ok(Some(Slot::Spaces)),
                255 => return Ok(Some(Slot::Sysmis)),
                c => return Ok(Some(Slot::Number(f64::from(c) - self.bias))),
            }
        }
    }
}

pub(crate) const BIAS: f64 = 100.0;

#[derive(Default)]
pub(crate) struct Compressor {
    pub out: Vec<u8>,
    block: [u8; 8],
    n: usize,
    data: Vec<u8>,
}

impl Compressor {
    fn push(&mut self, code: u8, raw: Option<[u8; 8]>) {
        self.block[self.n] = code;
        self.n += 1;
        if let Some(r) = raw {
            self.data.extend_from_slice(&r);
        }
        if self.n == 8 {
            self.flush();
        }
    }

    fn flush(&mut self) {
        self.out.extend_from_slice(&self.block);
        self.out.append(&mut self.data);
        self.block = [0; 8];
        self.n = 0;
    }

    pub fn number(&mut self, v: f64) {
        if v.is_nan() {
            self.push(255, None);
        } else if v.fract() == 0.0 && (1.0 - BIAS..=251.0 - BIAS).contains(&v) {
            self.push((v + BIAS) as u8, None);
        } else {
            self.push(253, Some(v.to_le_bytes()));
        }
    }

    pub fn chunk(&mut self, b: [u8; 8]) {
        if b == [b' '; 8] { self.push(254, None) } else { self.push(253, Some(b)) }
    }

    /// Pads the last command block with zeros (padding codes).
    pub fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.flush();
        }
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::Endian;

    #[test]
    fn round_trip_slots() {
        let mut c = Compressor::default();
        let nums = [0.0, -99.0, 151.0, 152.0, -100.0, 0.5, f64::NAN, 1e300, 7.0];
        for &v in &nums {
            c.number(v);
        }
        c.chunk(*b"        ");
        c.chunk(*b"abc     ");
        let bytes = c.finish();
        let mut cur = Cursor::new(&bytes, 0, Endian::Little);
        let mut d = Decompressor::new(&mut cur, BIAS);
        let mut got = Vec::new();
        while let Some(s) = d.next_slot().unwrap() {
            got.push(s);
        }
        assert_eq!(got.len(), nums.len() + 2);
        assert_eq!(got[0], Slot::Number(0.0));
        assert_eq!(got[1], Slot::Number(-99.0));
        assert_eq!(got[2], Slot::Number(151.0));
        assert_eq!(got[3], Slot::Raw(152.0f64.to_le_bytes()));
        assert_eq!(got[4], Slot::Raw((-100.0f64).to_le_bytes()));
        assert_eq!(got[6], Slot::Sysmis);
        assert_eq!(got[9], Slot::Spaces);
        assert_eq!(got[10], Slot::Raw(*b"abc     "));
    }

    #[test]
    fn truncated_raw_value_is_an_error() {
        let bytes = [253u8, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3];
        let mut cur = Cursor::new(&bytes, 0, Endian::Little);
        let mut d = Decompressor::new(&mut cur, BIAS);
        assert!(matches!(d.next_slot(), Err(SavError::Format(_))));
    }
}
