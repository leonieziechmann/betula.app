//! FlateDecode (ISO 32000-1 §7.4.4): DEFLATE (RFC 1951) in the zlib format (RFC 1950), decoded as
//! zlib's `puff` decodes it — canonical Huffman codes read a bit at a time, small rather than fast,
//! which a Notenübersicht's few hundred kilobytes do not notice.

/// The most bytes one stream may inflate to. A page's text is tens of kilobytes; this keeps a file
/// made to inflate without end from taking the browser's memory.
pub const MAX_OUT: usize = 16 << 20;

/// The data of a zlib stream; one without the zlib header is read as bare DEFLATE. A stream that
/// ends early gives what it holds up to there, as readers of PDF do.
pub fn zlib(data: &[u8]) -> Option<Vec<u8>> {
    let header = match (data.first(), data.get(1)) {
        (Some(&cmf), Some(&flg)) => cmf & 0x0f == 8 && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0 && flg & 0x20 == 0,
        _ => false,
    };
    inflate(if header { data.get(2..)? } else { data })
}

/// Bare DEFLATE.
pub fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut bits = Bits { data, pos: 0, buffer: 0, count: 0 };
    let mut out = Vec::new();
    match blocks(&mut bits, &mut out) {
        Some(()) => Some(out),
        None if bits.pos >= data.len() && !out.is_empty() && out.len() <= MAX_OUT => Some(out),
        None => None,
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buffer: u32,
    count: u32,
}

impl Bits<'_> {
    /// `n` bits (at most 16), the first the lowest.
    fn need(&mut self, n: u32) -> Option<u32> {
        while self.count < n {
            let byte = *self.data.get(self.pos)?;
            self.pos += 1;
            self.buffer |= u32::from(byte) << self.count;
            self.count += 8;
        }
        let value = self.buffer & ((1u32 << n) - 1);
        self.buffer >>= n;
        self.count -= n;
        Some(value)
    }

    /// The rest of the byte begun, dropped (a stored block starts at a byte).
    fn align(&mut self) {
        self.buffer = 0;
        self.count = 0;
    }
}

/// A canonical Huffman code: how many codes each length has, and the symbols by code.
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    /// The code of these lengths; `None` for one with more codes of a length than fit.
    fn new(lengths: &[u8]) -> Option<Huffman> {
        let mut counts = [0u16; 16];
        for &length in lengths {
            *counts.get_mut(usize::from(length))? += 1;
        }
        let mut left: i32 = 1;
        for count in counts.iter().skip(1) {
            left = (left << 1) - i32::from(*count);
            if left < 0 {
                return None;
            }
        }
        let mut offsets = [0u16; 16];
        for length in 1..15usize {
            let next = offsets.get(length).copied()? + counts.get(length).copied()?;
            *offsets.get_mut(length + 1)? = next;
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                let offset = offsets.get_mut(usize::from(length))?;
                *symbols.get_mut(usize::from(*offset))? = u16::try_from(symbol).ok()?;
                *offset += 1;
            }
        }
        Some(Huffman { counts, symbols })
    }

    fn decode(&self, bits: &mut Bits) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for count in self.counts.iter().skip(1) {
            code |= i32::try_from(bits.need(1)?).ok()?;
            let count = i32::from(*count);
            if code - count < first {
                return self.symbols.get(usize::try_from(index + (code - first)).ok()?).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DISTANCE_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DISTANCE_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
/// The order the lengths of the code lengths come in.
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn blocks(bits: &mut Bits, out: &mut Vec<u8>) -> Option<()> {
    loop {
        let last = bits.need(1)? == 1;
        match bits.need(2)? {
            0 => stored(bits, out)?,
            1 => fixed(bits, out)?,
            2 => dynamic(bits, out)?,
            _ => return None,
        }
        if last {
            return Some(());
        }
    }
}

fn stored(bits: &mut Bits, out: &mut Vec<u8>) -> Option<()> {
    bits.align();
    let at = bits.pos;
    let word = |at: usize| -> Option<u16> { Some(u16::from_le_bytes([*bits.data.get(at)?, *bits.data.get(at + 1)?])) };
    let (length, complement) = (word(at)?, word(at + 2)?);
    if length != !complement {
        return None;
    }
    let data = bits.data.get(at + 4..at + 4 + usize::from(length))?;
    out.extend_from_slice(data);
    bits.pos = at + 4 + usize::from(length);
    (out.len() <= MAX_OUT).then_some(())
}

fn fixed(bits: &mut Bits, out: &mut Vec<u8>) -> Option<()> {
    let mut lengths = [0u8; 288];
    for (symbol, length) in lengths.iter_mut().enumerate() {
        *length = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    codes(bits, out, &Huffman::new(&lengths)?, &Huffman::new(&[5u8; 30])?)
}

fn dynamic(bits: &mut Bits, out: &mut Vec<u8>) -> Option<()> {
    let literals = usize::try_from(bits.need(5)? + 257).ok()?;
    let distances = usize::try_from(bits.need(5)? + 1).ok()?;
    let given = usize::try_from(bits.need(4)? + 4).ok()?;
    if literals > 286 || distances > 30 {
        return None;
    }
    let mut lengths = [0u8; 320];
    for at in ORDER.iter().take(given) {
        *lengths.get_mut(*at)? = u8::try_from(bits.need(3)?).ok()?;
    }
    let lengths_code = Huffman::new(lengths.get(..19)?)?;
    let total = literals + distances;
    let mut index = 0;
    while index < total {
        let symbol = lengths_code.decode(bits)?;
        if symbol < 16 {
            *lengths.get_mut(index)? = u8::try_from(symbol).ok()?;
            index += 1;
            continue;
        }
        let (length, repeat) = match symbol {
            16 => (*lengths.get(index.checked_sub(1)?)?, 3 + bits.need(2)?),
            17 => (0, 3 + bits.need(3)?),
            _ => (0, 11 + bits.need(7)?),
        };
        let repeat = usize::try_from(repeat).ok()?;
        if index + repeat > total {
            return None;
        }
        for _ in 0..repeat {
            *lengths.get_mut(index)? = length;
            index += 1;
        }
    }
    // A block without the code of its end cannot end.
    if lengths.get(256).copied()? == 0 {
        return None;
    }
    let literal_code = Huffman::new(lengths.get(..literals)?)?;
    let distance_code = Huffman::new(lengths.get(literals..total)?)?;
    codes(bits, out, &literal_code, &distance_code)
}

fn codes(bits: &mut Bits, out: &mut Vec<u8>, literal: &Huffman, distance: &Huffman) -> Option<()> {
    loop {
        let symbol = literal.decode(bits)?;
        match symbol {
            0..=255 => out.push(u8::try_from(symbol).ok()?),
            256 => return Some(()),
            _ => {
                let at = usize::from(symbol - 257);
                let length = usize::from(*LENGTH_BASE.get(at)?) + usize::try_from(bits.need(u32::from(*LENGTH_EXTRA.get(at)?))?).ok()?;
                let at = usize::from(distance.decode(bits)?);
                let back = usize::from(*DISTANCE_BASE.get(at)?) + usize::try_from(bits.need(u32::from(*DISTANCE_EXTRA.get(at)?))?).ok()?;
                let start = out.len().checked_sub(back)?;
                for i in 0..length {
                    let byte = *out.get(start + i)?;
                    out.push(byte);
                }
            }
        }
        if out.len() > MAX_OUT {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `zlib.compress(b"hello hello hello hello", 9)` in Python: fixed codes, a match that copies
    /// what it is writing.
    const HELLO: [u8; 16] = [0x78, 0xda, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01, 0x68, 0x03, 0x08, 0xb1];

    /// `zlib.compress(LINE * 20, 9)`: a block of dynamic codes.
    const LINE: &str = "Theoretische Informatik Digitaltechnik Algorithmieren und Programmieren ";
    const LINES: [u8; 86] = [
        0x78, 0xda, 0xed, 0xcc, 0x31, 0x0e, 0x80, 0x30, 0x08, 0x40, 0xd1, 0xab, 0x70, 0x15, 0x13, 0x17, 0x37, 0x87, 0x5e, 0xa0, 0x69, 0xb1, 0x10, 0x0b, 0x24, 0x88,
        0xf7, 0xd7, 0xc1, 0x23, 0x38, 0x32, 0xfe, 0x3f, 0xbc, 0x42, 0x68, 0x8e, 0xc1, 0x57, 0x23, 0x84, 0x4d, 0x0f, 0x73, 0xa9, 0xc1, 0x27, 0xac, 0x3c, 0x38, 0xea,
        0x0c, 0x6c, 0xa4, 0x6f, 0x2e, 0x73, 0x98, 0x73, 0x90, 0x30, 0x3a, 0x2a, 0xdc, 0xda, 0x61, 0x77, 0x1b, 0x5e, 0xe5, 0x3b, 0x25, 0x9d, 0x74, 0xd2, 0x49, 0x27,
        0x9d, 0x5f, 0x9d, 0x07, 0x19, 0x13, 0x2a, 0x7f,
    ];

    #[test]
    fn fixed_codes_with_a_match() {
        assert_eq!(zlib(&HELLO).as_deref(), Some(b"hello hello hello hello".as_slice()));
    }

    #[test]
    fn dynamic_codes() {
        assert_eq!(zlib(&LINES), Some(LINE.repeat(20).into_bytes()));
        // Cut short, what was there.
        let short = zlib(LINES.get(..40).unwrap_or_default()).unwrap_or_default();
        assert!(!short.is_empty() && LINE.repeat(20).as_bytes().starts_with(&short));
    }

    #[test]
    fn a_stored_block() {
        // BFINAL 1, BTYPE 00, then LEN 5, NLEN !5, the bytes.
        let data = [0x01, 0x05, 0x00, 0xfa, 0xff, b'a', b'b', b'c', b'd', b'e'];
        assert_eq!(inflate(&data).as_deref(), Some(b"abcde".as_slice()));
    }

    #[test]
    fn garbage_is_refused_or_short_never_a_panic() {
        for data in [&[][..], &[0x78], &[0x78, 0x9c, 0xff, 0xff, 0xff], &[0x07; 64], &[0xff; 300]] {
            let _ = zlib(data);
        }
    }
}
