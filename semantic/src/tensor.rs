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

/// How the matrices are multiplied (`Model::from_bytes_with`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Weights stay 4-bit; a row is expanded to floats every time it is used. Least memory
    /// (the file), slowest.
    Expand,
    /// Every matrix expanded to f32 once, when loading: 4 bytes a weight (85 MB more).
    F32,
    /// Every matrix expanded to int8 once (a byte a weight, 21 MB more); the tokens are
    /// quantised to int8 per block as well and multiplied in integers, as llama.cpp does
    /// (q4_0 × q8_0). Needs weights on uniform levels (`q4`, not `nf4`).
    Int8,
}

/// A matrix ready for one of the faster modes.
enum Unpacked {
    F32(Vec<f32>),
    /// Codes row by row, one scale per block of `block` codes. `unsigned`: the codes are the
    /// 4-bit codes as stored, 0…15, i.e. the weight + 8 — what relaxed SIMD's i8 × i7 dot product
    /// needs (its second operand must not have the top bit set; see `relaxed_product`).
    Int8 { codes: Vec<i8>, scales: Vec<f32>, block: usize, unsigned: bool },
}

/// A matrix and its bias: `y = x·Wᵀ + b` for every token of `x`.
pub(crate) struct Linear {
    pub(crate) weight: Tensor,
    pub(crate) bias: Vec<f32>,
    unpacked: Option<Unpacked>,
}

/// Room the products need, kept across calls.
#[derive(Default)]
pub(crate) struct Scratch {
    row: Vec<f32>,
    codes: Vec<i16>,
    #[cfg(all(target_arch = "wasm32", target_feature = "relaxed-simd"))]
    bytes: Vec<i8>,
    #[cfg(all(target_arch = "wasm32", target_feature = "relaxed-simd"))]
    sums: Vec<i32>,
    scales: Vec<f32>,
    wide: Vec<i16>,
}

impl Linear {
    pub(crate) fn read(r: &mut Reader, rows: usize, cols: usize) -> Result<Self, String> {
        let weight = Tensor::read(r)?;
        if weight.shape() != (rows, cols) {
            return Err(format!("expected a {rows}×{cols} matrix, found {:?}", weight.shape()));
        }
        let bias = Tensor::read(r)?.into_vector(rows)?;
        Ok(Self { weight, bias, unpacked: None })
    }

    /// Expands the matrix for `mode`, once.
    pub(crate) fn unpack(&mut self, bytes: &[u8], mode: Mode) -> Result<(), String> {
        let (rows, cols) = self.weight.shape();
        self.unpacked = match mode {
            Mode::Expand => None,
            Mode::F32 => {
                let mut all = vec![0.0f32; rows * cols];
                for (o, row) in all.chunks_exact_mut(cols).enumerate() {
                    self.weight.row(bytes, o, row);
                }
                Some(Unpacked::F32(all))
            }
            Mode::Int8 => Some(self.int8(bytes)?),
        };
        Ok(())
    }

    fn int8(&self, bytes: &[u8]) -> Result<Unpacked, String> {
        let (rows, cols) = self.weight.shape();
        match &self.weight {
            Tensor::Q4 { block, levels, scales, codes, .. } => {
                // Uniform levels are k·step: the code minus 8 is the integer, step·scale the scale.
                let step = levels.get(1).zip(levels.first()).map_or(0.0, |(b, a)| b - a);
                let uniform = levels.iter().enumerate().all(|(k, l)| (l - (k as f32 - 8.0) * step).abs() < 1e-6);
                if !uniform || step <= 0.0 {
                    return Err("int8 needs weights on uniform levels (q4)".into());
                }
                let unsigned = cfg!(all(target_arch = "wasm32", target_feature = "relaxed-simd"))
                    && block % 16 == 0
                    && rows % 4 == 0;
                let offset = if unsigned { 0 } else { 8 };
                let packed = bytes.get(*codes..codes + rows * cols / 2).ok_or("codes")?;
                let mut out = Vec::with_capacity(rows * cols);
                for byte in packed {
                    out.push((byte & 0x0f).cast_signed() - offset);
                    out.push((byte >> 4).cast_signed() - offset);
                }
                let raw = bytes.get(*scales..scales + rows * cols / block * 2).ok_or("scales")?;
                let (halves, _) = raw.as_chunks::<2>();
                let scales = halves.iter().map(|h| f16_to_f32(u16::from_le_bytes(*h)) * step).collect();
                Ok(Unpacked::Int8 { codes: out, scales, block: *block, unsigned })
            }
            Tensor::Q8 { block, scales, codes, .. } => {
                let raw = bytes.get(*codes..codes + rows * cols).ok_or("codes")?;
                let out = raw.iter().map(|c| c.cast_signed()).collect();
                let raw = bytes.get(*scales..scales + rows * cols / block * 2).ok_or("scales")?;
                let (halves, _) = raw.as_chunks::<2>();
                let scales = halves.iter().map(|h| f16_to_f32(u16::from_le_bytes(*h))).collect();
                Ok(Unpacked::Int8 { codes: out, scales, block: *block, unsigned: false })
            }
            _ => Err("int8 needs quantised weights".into()),
        }
    }

    /// `x` holds the tokens one after the other (each as long as a row of the matrix), `y`
    /// receives them (each as long as a column).
    pub(crate) fn apply(&self, bytes: &[u8], x: &[f32], y: &mut [f32], scratch: &mut Scratch) {
        let (rows, cols) = self.weight.shape();
        match &self.unpacked {
            None => {
                let row = &mut scratch.row;
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
            Some(Unpacked::F32(all)) => {
                for ((o, b), row) in self.bias.iter().enumerate().zip(all.chunks_exact(cols)) {
                    for (xt, yt) in x.chunks_exact(cols).zip(y.chunks_exact_mut(rows)) {
                        if let Some(v) = yt.get_mut(o) {
                            *v = dot(xt, row) + b;
                        }
                    }
                }
            }
            Some(Unpacked::Int8 { codes, scales, block, unsigned }) => {
                #[cfg(all(target_arch = "wasm32", target_feature = "relaxed-simd"))]
                if *unsigned {
                    quantise(x, *block, &mut scratch.bytes, &mut scratch.scales);
                    // 8 · Σ x of each block of each token, the part of Σ x·(w + 8) that is not Σ x·w;
                    // a quarter of it off each of the four lanes.
                    scratch.sums.clear();
                    scratch.sums.extend(scratch.bytes.chunks_exact(*block).map(|b| 2 * b.iter().map(|v| i32::from(*v)).sum::<i32>()));
                    relaxed_product(&scratch.bytes, &scratch.scales, &scratch.sums, codes, scales, &self.bias, *block, cols, y);
                    return;
                }
                let _ = unsigned;
                quantise(x, *block, &mut scratch.codes, &mut scratch.scales);
                int8_product(&scratch.codes, &scratch.scales, codes, scales, &self.bias, *block, cols, y, &mut scratch.wide);
            }
        }
    }
}

/// `y = x·Wᵀ + b` with int8 tokens `xq` (scales `xs`) and int8 rows `w` (scales `ws`): four rows
/// widened to i16 at a time, then multiplied with the tokens two at a time, so that every vector
/// loaded is used several times (`tile`).
#[allow(clippy::too_many_arguments)]
fn int8_product(xq: &[i16], xs: &[f32], w: &[i8], ws: &[f32], bias: &[f32], block: usize, cols: usize, y: &mut [f32], wide: &mut Vec<i16>) {
    let rows = bias.len();
    let per_row = cols / block;
    let tokens = xq.len() / cols;
    for ((group, w4), ws4) in w.chunks(4 * cols).enumerate().zip(ws.chunks(4 * per_row)) {
        wide.clear();
        wide.extend(w4.iter().map(|c| i16::from(*c)));
        let first = group * 4;
        let (wr, wsr): (Vec<&[i16]>, Vec<&[f32]>) = (wide.chunks(cols).collect(), ws4.chunks(per_row).collect());
        let mut t = 0;
        while t < tokens {
            let pair = (t + 1 < tokens) as usize + 1;
            let xt: Vec<&[i16]> = (t..t + pair).filter_map(|u| xq.get(u * cols..(u + 1) * cols)).collect();
            let xst: Vec<&[f32]> = (t..t + pair).filter_map(|u| xs.get(u * per_row..(u + 1) * per_row)).collect();
            let sums = tile(&xt, &xst, &wr, &wsr, block);
            for (r, row_sums) in sums.iter().enumerate().take(wr.len()) {
                let b = bias.get(first + r).copied().unwrap_or_default();
                for (u, sum) in row_sums.iter().enumerate().take(pair) {
                    if let Some(v) = y.get_mut((t + u) * rows + first + r) {
                        *v = sum + b;
                    }
                }
            }
            t += pair;
        }
    }
}

/// Σ over the blocks of (Σ x·w in integers) · (x's scale · w's scale), for up to 4 rows × 2
/// tokens: the definition every build computes bit for bit (`tile_scalar`). The integer sum of
/// a block is exact however it is added up (and below 2²⁴, so exact as a float too); the floats
/// are then added block after block. That is what makes the server (native) and every browser
/// (WASM SIMD, relaxed SIMD) find the same modules with the same scores for the same query.
fn tile_scalar(x: &[&[i16]], xs: &[&[f32]], w: &[&[i16]], ws: &[&[f32]], block: usize) -> [[f32; 2]; 4] {
    let mut out = [[0.0f32; 2]; 4];
    for ((o, wr), wsr) in out.iter_mut().zip(w).zip(ws) {
        for ((cell, xt), xst) in o.iter_mut().zip(x).zip(xs) {
            for (((xb, wb), xd), wd) in xt.chunks_exact(block).zip(wr.chunks_exact(block)).zip(*xst).zip(*wsr) {
                let dot: i32 = xb.iter().zip(wb).map(|(a, b)| i32::from(*a) * i32::from(*b)).sum();
                *cell += dot as f32 * (xd * wd);
            }
        }
    }
    out
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn tile(x: &[&[i16]], xs: &[&[f32]], w: &[&[i16]], ws: &[&[f32]], block: usize) -> [[f32; 2]; 4] {
    tile_scalar(x, xs, w, ws, block)
}

/// The four lanes of each of `a`…`d` added up, exactly (integers): [Σa, Σb, Σc, Σd].
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn lane_sums(a: core::arch::wasm32::v128, b: core::arch::wasm32::v128, c: core::arch::wasm32::v128, d: core::arch::wasm32::v128) -> core::arch::wasm32::v128 {
    use core::arch::wasm32::*;
    let ab = i32x4_add(i32x4_shuffle::<0, 4, 1, 5>(a, b), i32x4_shuffle::<2, 6, 3, 7>(a, b)); // a0+a2 b0+b2 a1+a3 b1+b3
    let cd = i32x4_add(i32x4_shuffle::<0, 4, 1, 5>(c, d), i32x4_shuffle::<2, 6, 3, 7>(c, d));
    i32x4_add(i32x4_shuffle::<0, 1, 4, 5>(ab, cd), i32x4_shuffle::<2, 3, 6, 7>(ab, cd))
}

/// `tile_scalar` in WebAssembly SIMD, bit for bit. `i32x4.dot_i16x8_s` multiplies eight pairs
/// and adds them pairwise into four i32 lanes; per block, the lanes of the four rows are added up
/// exactly (`lane_sums`), and the four sums scaled and added to the rows' floats in one vector.
/// Two tokens and four rows share every load.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[allow(clippy::indexing_slicing)] // every index is below the lengths checked on entry
fn tile(x: &[&[i16]], xs: &[&[f32]], w: &[&[i16]], ws: &[&[f32]], block: usize) -> [[f32; 2]; 4] {
    use core::arch::wasm32::*;
    let mut out = [[0.0f32; 2]; 4];
    let (Some(first), true) = (x.first(), w.len() == 4 && (x.len() == 1 || x.len() == 2)) else {
        // Fewer than four rows (never with 384 or 1536 of them).
        return tile_scalar(x, xs, w, ws, block);
    };
    let cols = first.len();
    let second = if x.len() == 2 { x[1] } else { x[0] };
    let second_scales = if xs.len() == 2 { xs[1] } else { xs[0] };
    let blocks = cols / block;
    if w.iter().any(|r| r.len() < cols) || ws.iter().any(|r| r.len() < blocks) || second.len() < cols
        || xs[0].len() < blocks || second_scales.len() < blocks || !block.is_multiple_of(8) {
        return out;
    }
    // SAFETY: every load reads 8 i16 at an offset + 8 ≤ cols, within slices checked above.
    let load = |s: &[i16], at: usize| unsafe { v128_load(s.as_ptr().add(at).cast()) };
    // acc[u]: token u, lane r: row r.
    let mut acc = [f32x4_splat(0.0); 2];
    for b in 0..blocks {
        let mut dot = [[i32x4_splat(0); 4]; 2];
        let mut at = b * block;
        while at < (b + 1) * block {
            let (a0, a1) = (load(first, at), load(second, at));
            for r in 0..4 {
                let wv = load(w[r], at);
                dot[0][r] = i32x4_add(dot[0][r], i32x4_dot_i16x8(a0, wv));
                dot[1][r] = i32x4_add(dot[1][r], i32x4_dot_i16x8(a1, wv));
            }
            at += 8;
        }
        let wd = f32x4(ws[0][b], ws[1][b], ws[2][b], ws[3][b]);
        for (u, d) in [xs[0][b], second_scales[b]].into_iter().enumerate() {
            let [d0, d1, d2, d3] = dot[u];
            let sums = f32x4_convert_i32x4(lane_sums(d0, d1, d2, d3));
            acc[u] = f32x4_add(acc[u], f32x4_mul(sums, f32x4_mul(f32x4_splat(d), wd)));
        }
    }
    for (u, a) in acc.iter().enumerate() {
        let lanes = [f32x4_extract_lane::<0>(*a), f32x4_extract_lane::<1>(*a), f32x4_extract_lane::<2>(*a), f32x4_extract_lane::<3>(*a)];
        for (r, lane) in lanes.into_iter().enumerate() {
            out[r][u] = lane;
        }
    }
    out
}

/// `int8_product` with relaxed SIMD's `i32x4.relaxed_dot_i8x16_i7x16_add`: sixteen i8 × i7
/// products added into four i32 lanes by one instruction, the weights as they are (no i16).
/// The i7 operand is deterministic only without its top bit — V8 lowers the instruction to x86's
/// `pmaddubsw`, which reads it unsigned — so the weights are the unsigned codes w + 8 and
/// 8 · Σ x per block (`sums`, doubled per lane) is taken off again. Chrome 114+, Firefox 120+;
/// not in Safari, which gets the simd128 build (`demo/e5.js` picks).
#[cfg(all(target_arch = "wasm32", target_feature = "relaxed-simd"))]
#[allow(clippy::too_many_arguments)]
fn relaxed_product(xq: &[i8], xs: &[f32], sums: &[i32], w: &[i8], ws: &[f32], bias: &[f32], block: usize, cols: usize, y: &mut [f32]) {
    let rows = bias.len();
    let per_row = cols / block;
    let tokens = xq.len() / cols;
    for ((group, w4), ws4) in w.chunks_exact(4 * cols).enumerate().zip(ws.chunks_exact(4 * per_row)) {
        let first = group * 4;
        let mut t = 0;
        while t < tokens {
            let pair = (t + 1 < tokens) as usize + 1;
            let x0 = xq.get(t * cols..(t + 1) * cols).unwrap_or_default();
            let x1 = xq.get((t + pair - 1) * cols..(t + pair) * cols).unwrap_or_default();
            let s0 = xs.get(t * per_row..(t + 1) * per_row).unwrap_or_default();
            let s1 = xs.get((t + pair - 1) * per_row..(t + pair) * per_row).unwrap_or_default();
            let c0 = sums.get(t * per_row..(t + 1) * per_row).unwrap_or_default();
            let c1 = sums.get((t + pair - 1) * per_row..(t + pair) * per_row).unwrap_or_default();
            let results = tile_relaxed([x0, x1], [s0, s1], [c0, c1], w4, ws4, cols, block);
            for (r, row_sums) in results.iter().enumerate() {
                let b = bias.get(first + r).copied().unwrap_or_default();
                for (u, sum) in row_sums.iter().enumerate().take(pair) {
                    if let Some(v) = y.get_mut((t + u) * rows + first + r) {
                        *v = sum + b;
                    }
                }
            }
            t += pair;
        }
    }
}

/// Four rows (`w`, one after the other) × two tokens, per block of `block` values.
#[cfg(all(target_arch = "wasm32", target_feature = "relaxed-simd"))]
#[allow(clippy::indexing_slicing, clippy::needless_range_loop)] // every index is below the lengths checked on entry
fn tile_relaxed(x: [&[i8]; 2], xs: [&[f32]; 2], sums: [&[i32]; 2], w: &[i8], ws: &[f32], cols: usize, block: usize) -> [[f32; 2]; 4] {
    use core::arch::wasm32::*;
    let mut out = [[0.0f32; 2]; 4];
    let blocks = cols / block;
    if x.iter().any(|t| t.len() < cols) || xs.iter().any(|s| s.len() < blocks) || sums.iter().any(|s| s.len() < blocks) || w.len() < 4 * cols || ws.len() < 4 * blocks {
        return out;
    }
    // SAFETY: every load reads 16 bytes at an offset + 16 ≤ the length checked above.
    let load = |s: &[i8], at: usize| unsafe { v128_load(s.as_ptr().add(at).cast()) };
    // acc[u]: token u, lane r: row r — as in the simd128 `tile`, so both equal `tile_scalar`.
    let mut acc = [f32x4_splat(0.0); 2];
    for b in 0..blocks {
        let mut dot = [[i32x4_splat(0); 4]; 2];
        let mut at = b * block;
        while at < (b + 1) * block {
            let (a0, a1) = (load(x[0], at), load(x[1], at));
            for r in 0..4 {
                let wv = load(w, r * cols + at);
                dot[0][r] = i32x4_relaxed_dot_i8x16_i7x16_add(a0, wv, dot[0][r]);
                dot[1][r] = i32x4_relaxed_dot_i8x16_i7x16_add(a1, wv, dot[1][r]);
            }
            at += 16;
        }
        let wd = f32x4(ws[b], ws[blocks + b], ws[2 * blocks + b], ws[3 * blocks + b]);
        for u in 0..2 {
            // Σ x·(w + 8) − 8·Σ x = Σ x·w, in integers: exact.
            let offset = i32x4_splat(sums[u][b]);
            let [d0, d1, d2, d3] = dot[u].map(|d| i32x4_sub(d, offset));
            let exact = f32x4_convert_i32x4(lane_sums(d0, d1, d2, d3));
            acc[u] = f32x4_add(acc[u], f32x4_mul(exact, f32x4_mul(f32x4_splat(xs[u][b]), wd)));
        }
    }
    for (u, a) in acc.iter().enumerate() {
        let lanes = [f32x4_extract_lane::<0>(*a), f32x4_extract_lane::<1>(*a), f32x4_extract_lane::<2>(*a), f32x4_extract_lane::<3>(*a)];
        for (r, lane) in lanes.into_iter().enumerate() {
            out[r][u] = lane;
        }
    }
    out
}

/// Every block of `block` values of `x` as int8 codes (as i8, or kept in i16 ready to be
/// multiplied) and a scale (its largest magnitude / 127).
fn quantise<T: From<i8>>(x: &[f32], block: usize, codes: &mut Vec<T>, scales: &mut Vec<f32>) {
    codes.clear();
    scales.clear();
    for b in x.chunks_exact(block) {
        let max = b.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let scale = max / 127.0;
        let inverse = if scale > 0.0 { 1.0 / scale } else { 0.0 };
        codes.extend(b.iter().map(|v| T::from((v * inverse).round() as i8)));
        scales.push(scale);
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

/// erf as a rational function of x² on [-4, 4] (Eigen's and XLA's float erf; beyond ±4 it is
/// ±1 to float precision): only multiplications, additions and one division, which SIMD does —
/// `exp` would be a library call in WebAssembly, for every value of the feed-forward part.
fn erf(x: f32) -> f32 {
    const ALPHA: [f32; 7] = [-2.726_142_3e-10, 2.770_681_4e-8, -2.101_024e-6, -5.692_506_6e-5, -7.349_906_3e-4, -2.954_600_1e-3, -1.609_603_3e-2];
    const BETA: [f32; 5] = [-1.456_607_2e-5, -2.133_740_6e-4, -1.682_827e-3, -7.373_329_2e-3, -1.426_474e-2];
    let x = x.clamp(-4.0, 4.0);
    let x2 = x * x;
    let p = ALPHA.iter().fold(0.0f32, |p, a| p * x2 + a) * x;
    let q = BETA.iter().fold(0.0f32, |q, b| q * x2 + b);
    p / q
}

/// eˣ for x ≤ 0 from additions and multiplications only, the same on every platform: `f32::exp`
/// is the platform's libm natively and Rust's own in WebAssembly, which can differ in the last
/// bit — and the server and the browser must compute the same embeddings to the bit.
/// x = n·ln 2 + r with |r| ≤ ln 2 / 2, eʳ by its Taylor series to r⁷ (relative error < 1e-7),
/// 2ⁿ from the exponent bits.
pub(crate) fn exp(x: f32) -> f32 {
    const LN2_HI: f32 = f32::from_bits(0x3f31_7200); // ln 2 to 16 bits (0.693145751953125): n · LN2_HI is exact
    const LN2_LO: f32 = 1.428_606_8e-6;
    if x < -87.0 {
        return 0.0;
    }
    let x = x.min(0.0);
    let n = (x * std::f32::consts::LOG2_E).round();
    let r = (x - n * LN2_HI) - n * LN2_LO;
    let p = 1.0 + r * (1.0 + r * (0.5 + r * (1.0 / 6.0 + r * (1.0 / 24.0 + r * (1.0 / 120.0 + r * (1.0 / 720.0 + r * (1.0 / 5040.0)))))));
    // n ∈ [-126, 0] here: a normal power of two.
    p * f32::from_bits(((n as i32 + 127) as u32) << 23)
}

pub(crate) fn softmax(x: &mut [f32]) {
    let max = x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0;
    for v in x.iter_mut() {
        *v = exp(*v - max);
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
    fn exp_is_close_to_libm() {
        for i in 0..=20_000 {
            let x = -(i as f32) * 0.0045;
            let (ours, libm) = (exp(x), x.exp());
            assert!((ours - libm).abs() <= 2e-7 * libm + 1e-37, "exp({x}) = {ours} ≠ {libm}");
        }
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(exp(-100.0), 0.0);
    }

    #[test]
    fn erf_matches_known_values() {
        for (x, want) in [(0.0f32, 0.0f32), (0.5, 0.520_499_9), (1.0, 0.842_700_8), (-1.5, -0.966_105_16), (3.0, 0.999_977_9), (5.0, 1.0), (-0.1, -0.112_462_92)] {
            assert!((erf(x) - want).abs() < 3e-7, "erf({x}) = {} ≠ {want}", erf(x));
        }
    }

    #[test]
    fn integer_tiles() {
        let x: Vec<i16> = (0..128).map(|i| (i * 7 % 255 - 127) as i16).collect();
        let w: Vec<i16> = (0..256).map(|i| (i * 13 % 17 - 8) as i16).collect();
        let (x0, x1) = x.split_at(64);
        let rows: Vec<&[i16]> = w.chunks(64).collect();
        let (xs, ws) = ([0.5f32, 0.25], [2.0f32, 4.0]);
        let (xs, ws): (&[f32], &[f32]) = (&xs, &ws);
        let sums = tile(&[x0, x1], &[xs, xs], &rows, &[ws; 4], 32);
        let block = |a: &[i16], b: &[i16], k: usize| {
            a.iter().zip(b).skip(32 * k).take(32).map(|(p, q)| i32::from(*p) * i32::from(*q)).sum::<i32>() as f32
        };
        for (r, row) in rows.iter().enumerate() {
            for (u, xt) in [x0, x1].iter().enumerate() {
                assert_eq!(sums[r][u], block(xt, row, 0) + block(xt, row, 1), "row {r}, token {u}");
            }
        }
    }

    #[test]
    fn dot_with_a_remainder() {
        let a: Vec<f32> = (0..19).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..19).map(|i| (i % 3) as f32).collect();
        assert_eq!(dot(&a, &b), a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>());
    }
}
