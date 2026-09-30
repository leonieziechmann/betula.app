//! The weight matrices as the file stores them, and the kernels the encoder needs.
//!
//! A quantised matrix stays in the file's bytes as it is (the model keeps the file, tensors
//! keep offsets into it), so its memory is the file's size. A row is expanded into floats when
//! it is used: one block of `block` values at a time, through a table of its 16 levels times
//! the block's scale. A matrix product expands each row once and multiplies it with every token.

use crate::reader::{f16_to_f32, Reader};

pub(crate) enum Tensor {
    F32 { rows: usize, cols: usize, data: Vec<f32> },
    F16 { rows: usize, cols: usize, at: usize },
    /// 4-bit codes into `levels`, two per byte (the first in the low nibble), one f16 scale per block.
    Q4 { rows: usize, cols: usize, block: usize, levels: [f32; 16], scales: usize, codes: usize },
    /// int8 codes, one f16 scale per block.
    Q8 { rows: usize, cols: usize, block: usize, scales: usize, codes: usize },
}

impl Tensor {
    pub(crate) fn read(r: &mut Reader) -> Result<Self, String> {
        let kind = r.u8()?;
        let rows = r.usize()?;
        let cols = r.usize()?;
        let count = rows.checked_mul(cols).ok_or("a tensor beyond the address space")?;
        Ok(match kind {
            0 => Tensor::F32 { rows, cols, data: r.f32s(count)? },
            1 | 2 => {
                let block = r.usize()?;
                if block == 0 || block % 2 != 0 || cols % block != 0 {
                    return Err(format!("blocks of {block} do not divide rows of {cols}"));
                }
                let levels = if kind == 1 {
                    let levels: [f32; 16] = r.f32s(16)?.try_into().map_err(|_| "16 levels")?;
                    Some(levels)
                } else {
                    None
                };
                let scales = r.at();
                r.take(count / block * 2)?;
                let codes = r.at();
                match levels {
                    Some(levels) => {
                        r.take(count / 2)?;
                        Tensor::Q4 { rows, cols, block, levels, scales, codes }
                    }
                    None => {
                        r.take(count)?;
                        Tensor::Q8 { rows, cols, block, scales, codes }
                    }
                }
            }
            3 => {
                let at = r.at();
                r.take(count * 2)?;
                Tensor::F16 { rows, cols, at }
            }
            other => return Err(format!("unknown tensor kind {other}")),
        })
    }

    pub(crate) fn shape(&self) -> (usize, usize) {
        match self {
            Tensor::F32 { rows, cols, .. }
            | Tensor::F16 { rows, cols, .. }
            | Tensor::Q4 { rows, cols, .. }
            | Tensor::Q8 { rows, cols, .. } => (*rows, *cols),
        }
    }

    /// A vector (a bias, a LayerNorm's weight) of `len` floats.
    pub(crate) fn into_vector(self, len: usize) -> Result<Vec<f32>, String> {
        match self {
            Tensor::F32 { data, .. } if data.len() == len => Ok(data),
            _ => Err(format!("expected a vector of {len} floats")),
        }
    }

    /// Row `row` as floats into `out` (as long as a row).
    pub(crate) fn row(&self, bytes: &[u8], row: usize, out: &mut [f32]) {
        match self {
            Tensor::F32 { cols, data, .. } => {
                let src = data.get(row * cols..(row + 1) * cols).unwrap_or_default();
                for (o, s) in out.iter_mut().zip(src) {
                    *o = *s;
                }
            }
            Tensor::F16 { cols, at, .. } => {
                let src = bytes.get(at + row * cols * 2..at + (row + 1) * cols * 2).unwrap_or_default();
                let (halves, _) = src.as_chunks::<2>();
                for (o, h) in out.iter_mut().zip(halves) {
                    *o = f16_to_f32(u16::from_le_bytes(*h));
                }
            }
            Tensor::Q4 { cols, block, levels, scales, codes, .. } => {
                let per_row = cols / block;
                let scale_bytes = bytes.get(scales + row * per_row * 2..scales + (row + 1) * per_row * 2).unwrap_or_default();
                let code_bytes = bytes.get(codes + row * cols / 2..codes + (row + 1) * cols / 2).unwrap_or_default();
                let (scale_halves, _) = scale_bytes.as_chunks::<2>();
                for ((dst, src), half) in out.chunks_exact_mut(*block).zip(code_bytes.chunks_exact(block / 2)).zip(scale_halves) {
                    let scale = f16_to_f32(u16::from_le_bytes(*half));
                    let table = levels.map(|level| level * scale);
                    let (pairs, _) = dst.as_chunks_mut::<2>();
                    for (pair, byte) in pairs.iter_mut().zip(src) {
                        let low = table.get(usize::from(byte & 0x0f)).copied().unwrap_or_default();
                        let high = table.get(usize::from(byte >> 4)).copied().unwrap_or_default();
                        *pair = [low, high];
                    }
                }
            }
            Tensor::Q8 { cols, block, scales, codes, .. } => {
                let per_row = cols / block;
                let scale_bytes = bytes.get(scales + row * per_row * 2..scales + (row + 1) * per_row * 2).unwrap_or_default();
                let code_bytes = bytes.get(codes + row * cols..codes + (row + 1) * cols).unwrap_or_default();
                let (scale_halves, _) = scale_bytes.as_chunks::<2>();
                for ((dst, src), half) in out.chunks_exact_mut(*block).zip(code_bytes.chunks_exact(*block)).zip(scale_halves) {
                    let scale = f16_to_f32(u16::from_le_bytes(*half));
                    for (d, c) in dst.iter_mut().zip(src) {
                        *d = f32::from(c.cast_signed()) * scale;
                    }
                }
            }
        }
    }
}

/// A matrix and its bias: `y = x·Wᵀ + b` for every token of `x`.
pub(crate) struct Linear {
    pub(crate) weight: Tensor,
    pub(crate) bias: Vec<f32>,
}

impl Linear {
    pub(crate) fn read(r: &mut Reader, rows: usize, cols: usize) -> Result<Self, String> {
        let weight = Tensor::read(r)?;
        if weight.shape() != (rows, cols) {
            return Err(format!("expected a {rows}×{cols} matrix, found {:?}", weight.shape()));
        }
        let bias = Tensor::read(r)?.into_vector(rows)?;
        Ok(Self { weight, bias })
    }

    /// `x` holds the tokens one after the other (each as long as a row of the matrix), `y`
    /// receives them (each as long as a column); `row` is room for one expanded row.
    pub(crate) fn apply(&self, bytes: &[u8], x: &[f32], y: &mut [f32], row: &mut Vec<f32>) {
        let (rows, cols) = self.weight.shape();
        row.resize(cols, 0.0);
        for (o, b) in self.bias.iter().enumerate() {
            self.weight.row(bytes, o, row);
            for (xt, yt) in x.chunks_exact(cols).zip(y.chunks_exact_mut(rows)) {
                if let Some(v) = yt.get_mut(o) {
                    *v = dot(xt, row) + b;
                }
            }
        }
    }
}

/// Eight sums side by side, so that the compiler turns the loop into SIMD (wasm simd128, SSE,
/// AVX) without being allowed to reorder floating-point additions itself.
pub(crate) fn dot(a: &[f32], b: &[f32]) -> f32 {
    let (a8, a_rest) = a.as_chunks::<8>();
    let (b8, b_rest) = b.as_chunks::<8>();
    let mut acc = [0.0f32; 8];
    for (x, y) in a8.iter().zip(b8) {
        for ((s, x), y) in acc.iter_mut().zip(x).zip(y) {
            *s += x * y;
        }
    }
    let mut sum: f32 = acc.iter().sum();
    for (x, y) in a_rest.iter().zip(b_rest) {
        sum += x * y;
    }
    sum
}

pub(crate) struct Norm {
    pub(crate) weight: Vec<f32>,
    pub(crate) bias: Vec<f32>,
}

impl Norm {
    pub(crate) fn read(r: &mut Reader, len: usize) -> Result<Self, String> {
        Ok(Self { weight: Tensor::read(r)?.into_vector(len)?, bias: Tensor::read(r)?.into_vector(len)? })
    }

    /// LayerNorm of every token of `x` in place (ε = 1e-12, as the model was trained).
    pub(crate) fn apply(&self, x: &mut [f32]) {
        let len = self.weight.len();
        for v in x.chunks_exact_mut(len) {
            let mean = v.iter().sum::<f32>() / len as f32;
            let variance = v.iter().map(|a| (a - mean) * (a - mean)).sum::<f32>() / len as f32;
            let inverse = 1.0 / (variance + 1e-12).sqrt();
            for ((a, w), b) in v.iter_mut().zip(&self.weight).zip(&self.bias) {
                *a = (*a - mean) * inverse * w + b;
            }
        }
    }
}

/// GELU as BERT computes it, with erf (not the tanh approximation).
pub(crate) fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + erf(x / std::f32::consts::SQRT_2))
}

/// erf through the complementary error function of Numerical Recipes (`erfcc`, relative error
/// below 1.2e-7 everywhere).
fn erf(x: f32) -> f32 {
    let z = f64::from(x).abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let poly = -z * z - 1.265_512_23
        + t * (1.000_023_68
            + t * (0.374_091_96
                + t * (0.096_784_18
                    + t * (-0.186_288_06
                        + t * (0.278_868_07 + t * (-1.135_203_98 + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77))))))));
    let erfc = t * poly.exp();
    (if x >= 0.0 { 1.0 - erfc } else { erfc - 1.0 }) as f32
}

pub(crate) fn softmax(x: &mut [f32]) {
    let max = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0;
    for v in x.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }
    for v in x.iter_mut() {
        *v /= sum;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erf_matches_known_values() {
        for (x, want) in [(0.0f32, 0.0f32), (0.5, 0.520_499_9), (1.0, 0.842_700_8), (-1.5, -0.966_105_16), (3.0, 0.999_977_9)] {
            assert!((erf(x) - want).abs() < 2e-7, "erf({x}) = {} ≠ {want}", erf(x));
        }
    }

    #[test]
    fn dot_with_a_remainder() {
        let a: Vec<f32> = (0..19).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..19).map(|i| (i % 3) as f32).collect();
        assert_eq!(dot(&a, &b), a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>());
    }
}
