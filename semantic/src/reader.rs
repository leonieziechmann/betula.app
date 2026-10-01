//! Reading the packed model, whose layout `python/pack.py` describes.

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Where the next read starts: tensors keep offsets into the file instead of copies.
    pub(crate) fn at(&self) -> usize {
        self.at
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(len).ok_or("a length beyond the address space")?;
        let part = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| format!("the file ends after {} bytes, {len} more wanted at {}", self.bytes.len(), self.at))?;
        self.at = end;
        Ok(part)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?.try_into().map_err(|_| "a short read".to_string())
    }

    pub(crate) fn u8(&mut self) -> Result<u8, String> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn usize(&mut self) -> Result<usize, String> {
        usize::try_from(self.u32()?).map_err(|e| e.to_string())
    }

    pub(crate) fn f32s(&mut self, count: usize) -> Result<Vec<f32>, String> {
        let raw = self.take(count.checked_mul(4).ok_or("too many floats")?)?;
        let (words, _) = raw.as_chunks::<4>();
        Ok(words.iter().map(|w| f32::from_le_bytes(*w)).collect())
    }
}

/// A half-precision float (the scales of the quantised blocks) as f32.
pub(crate) fn f16_to_f32(half: u16) -> f32 {
    let sign = u32::from(half >> 15) << 31;
    let exponent = u32::from((half >> 10) & 0x1f);
    let mantissa = u32::from(half & 0x3ff);
    let bits = match (exponent, mantissa) {
        (0, 0) => sign,
        (0, _) => {
            // Subnormal: shift the mantissa up until its leading one is the implicit bit.
            let mut e: u32 = 127 - 15 + 1;
            let mut m = mantissa;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x3ff) << 13)
        }
        (0x1f, _) => sign | 0x7f80_0000 | (mantissa << 13),
        _ => sign | ((exponent + 127 - 15) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::f16_to_f32;

    #[test]
    fn halves() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
        assert_eq!(f16_to_f32(0x0001), 5.960_464_5e-8);
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x7bff), 65504.0);
    }
}
