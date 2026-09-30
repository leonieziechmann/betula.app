//! multilingual-e5-small for German and English, small enough for a browser: the tokenizer and
//! the encoder of the packed model `python/pack.py` writes (the vocabulary trimmed to the pieces
//! German and English need, the weights in 4 bits). No dependencies; native and wasm32.
//!
//! The encoder is BERT: token + position embedding, LayerNorm, then 12 times self-attention
//! (12 heads) and a feed-forward part (GELU), each added to its input and normalised; the
//! embedding of a text is the mean over its tokens, scaled to length 1. E5 expects every text
//! behind a prefix: „query: “ for what is searched for, „passage: “ for what is searched.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

mod reader;
mod tensor;
pub mod tokenizer;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::collections::HashMap;

use reader::Reader;
use tensor::{gelu, softmax, Linear, Norm, Scratch, Tensor};
pub use tensor::Mode;
pub use tokenizer::Tokenizer;

struct Layer {
    query: Linear,
    key: Linear,
    value: Linear,
    output: Linear,
    attention_norm: Norm,
    intermediate: Linear,
    out: Linear,
    output_norm: Norm,
}

/// What a packed model starts with: its dimensions and the tokenizer, before the tensors.
struct Header {
    hidden: usize,
    layers: usize,
    heads: usize,
    intermediate: usize,
    positions: usize,
    vocab: usize,
    tokenizer: Tokenizer,
}

impl Header {
    fn read(r: &mut Reader) -> Result<Self, String> {
        if r.take(4)? != b"E5Q1" {
            return Err("not a packed e5 model (E5Q1)".into());
        }
        let hidden = r.usize()?;
        let layers = r.usize()?;
        let heads = r.usize()?;
        let intermediate = r.usize()?;
        let positions = r.usize()?;
        let vocab = r.usize()?;
        if heads == 0 || hidden % heads != 0 {
            return Err(format!("{heads} heads do not divide {hidden}"));
        }

        let mut pieces = Vec::with_capacity(vocab);
        for _ in 0..vocab {
            let len = usize::from(r.u8()?);
            let piece = std::str::from_utf8(r.take(len)?).map_err(|e| e.to_string())?.to_string();
            let score = f32::from_le_bytes(r.take(4)?.try_into().map_err(|_| "a score")?);
            pieces.push((piece, score));
        }
        let mut replace = HashMap::new();
        for _ in 0..r.usize()? {
            let code = r.u32()?;
            let len = usize::from(r.u8()?);
            let text = std::str::from_utf8(r.take(len)?).map_err(|e| e.to_string())?;
            if let Some(c) = char::from_u32(code) {
                replace.insert(c, text.to_string());
            }
        }
        Ok(Self { hidden, layers, heads, intermediate, positions, vocab, tokenizer: Tokenizer::new(pieces, replace) })
    }
}

/// Only the tokenizer of a packed model: for an encoder that runs elsewhere (WebGPU,
/// `demo/e5-gpu.js`). `bytes` may end where the tensors begin.
pub fn tokenizer_from_bytes(bytes: &[u8]) -> Result<Tokenizer, String> {
    Ok(Header::read(&mut Reader::new(bytes))?.tokenizer)
}

pub struct Model {
    bytes: Vec<u8>,
    hidden: usize,
    heads: usize,
    intermediate: usize,
    positions: usize,
    tokenizer: Tokenizer,
    words: Tensor,
    position: Tensor,
    norm: Norm,
    layers: Vec<Layer>,
}

impl Model {
    /// Takes the packed model's bytes over; its quantised weights are used where they are
    /// (`Mode::Expand`).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        Self::from_bytes_with(bytes, Mode::Expand)
    }

    /// Takes the packed model's bytes over and prepares the matrices for `mode`.
    pub fn from_bytes_with(bytes: Vec<u8>, mode: Mode) -> Result<Self, String> {
        let mut r = Reader::new(&bytes);
        let Header { hidden, layers: layer_count, heads, intermediate, positions, vocab, tokenizer } = Header::read(&mut r)?;

        let words = Tensor::read(&mut r)?;
        let position = Tensor::read(&mut r)?;
        if words.shape() != (vocab, hidden) || position.shape() != (positions, hidden) {
            return Err("the embeddings do not fit the vocabulary".into());
        }
        let norm = Norm::read(&mut r, hidden)?;
        let mut layers = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            layers.push(Layer {
                query: Linear::read(&mut r, hidden, hidden)?,
                key: Linear::read(&mut r, hidden, hidden)?,
                value: Linear::read(&mut r, hidden, hidden)?,
                output: Linear::read(&mut r, hidden, hidden)?,
                attention_norm: Norm::read(&mut r, hidden)?,
                intermediate: Linear::read(&mut r, intermediate, hidden)?,
                out: Linear::read(&mut r, hidden, intermediate)?,
                output_norm: Norm::read(&mut r, hidden)?,
            });
        }
        if r.at() != bytes.len() {
            return Err(format!("{} bytes left over", bytes.len() - r.at()));
        }
        for layer in &mut layers {
            for linear in [&mut layer.query, &mut layer.key, &mut layer.value, &mut layer.output, &mut layer.intermediate, &mut layer.out] {
                linear.unpack(&bytes, mode)?;
            }
        }
        Ok(Self { bytes, hidden, heads, intermediate, positions, tokenizer, words, position, norm, layers })
    }

    pub fn dims(&self) -> usize {
        self.hidden
    }

    pub fn tokenizer(&self) -> &Tokenizer {
        &self.tokenizer
    }

    /// The embedding of `text`, prefix included („query: …“).
    pub fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_ids(&self.tokenizer.encode(text))
    }

    /// The embedding of tokens as `Tokenizer::encode` gives them; beyond the positions the file
    /// keeps (128), the rest is cut off and `</s>` put at the end.
    pub fn embed_ids(&self, ids: &[u32]) -> Vec<f32> {
        let mut ids = ids.to_vec();
        if ids.len() > self.positions {
            ids.truncate(self.positions.saturating_sub(1));
            ids.push(tokenizer::EOS);
        }
        let (h, n) = (self.hidden, ids.len());
        let bytes = self.bytes.as_slice();

        let mut x = vec![0.0f32; n * h];
        let mut row = vec![0.0f32; h];
        let mut scratch = Scratch::default();
        for (t, (xt, id)) in x.chunks_exact_mut(h).zip(&ids).enumerate() {
            self.words.row(bytes, *id as usize, xt);
            self.position.row(bytes, t, &mut row);
            for (a, p) in xt.iter_mut().zip(&row) {
                *a += p;
            }
        }
        self.norm.apply(&mut x);

        let mut q = vec![0.0f32; n * h];
        let mut k = vec![0.0f32; n * h];
        let mut v = vec![0.0f32; n * h];
        let mut context = vec![0.0f32; n * h];
        let mut attended = vec![0.0f32; n * h];
        let mut inner = vec![0.0f32; n * self.intermediate];
        let mut out = vec![0.0f32; n * h];
        for layer in &self.layers {
            layer.query.apply(bytes, &x, &mut q, &mut scratch);
            layer.key.apply(bytes, &x, &mut k, &mut scratch);
            layer.value.apply(bytes, &x, &mut v, &mut scratch);
            self.attention(&q, &k, &v, &mut context);
            layer.output.apply(bytes, &context, &mut attended, &mut scratch);
            for (a, b) in attended.iter_mut().zip(&x) {
                *a += b;
            }
            layer.attention_norm.apply(&mut attended);
            layer.intermediate.apply(bytes, &attended, &mut inner, &mut scratch);
            for a in inner.iter_mut() {
                *a = gelu(*a);
            }
            layer.out.apply(bytes, &inner, &mut out, &mut scratch);
            for (a, b) in out.iter_mut().zip(&attended) {
                *a += b;
            }
            layer.output_norm.apply(&mut out);
            std::mem::swap(&mut x, &mut out);
        }

        // The mean over the tokens, scaled to length 1 (the mean's 1/n cancels out).
        let mut pooled = vec![0.0f32; h];
        for xt in x.chunks_exact(h) {
            for (p, a) in pooled.iter_mut().zip(xt) {
                *p += a;
            }
        }
        let length = pooled.iter().map(|a| a * a).sum::<f32>().sqrt().max(1e-12);
        for p in pooled.iter_mut() {
            *p /= length;
        }
        pooled
    }

    /// Scaled dot-product attention, every head over every token (a query is one sequence,
    /// so nothing is masked).
    fn attention(&self, q: &[f32], k: &[f32], v: &[f32], context: &mut [f32]) {
        let h = self.hidden;
        let d = h / self.heads;
        let scale = 1.0 / (d as f32).sqrt();
        let mut scores = vec![0.0f32; q.len() / h];
        for head in 0..self.heads {
            let part = head * d..(head + 1) * d;
            for (qt, ct) in q.chunks_exact(h).zip(context.chunks_exact_mut(h)) {
                let (Some(qh), Some(ch)) = (qt.get(part.clone()), ct.get_mut(part.clone())) else { continue };
                for (s, kt) in scores.iter_mut().zip(k.chunks_exact(h)) {
                    *s = kt.get(part.clone()).map_or(0.0, |kh| tensor::dot(qh, kh)) * scale;
                }
                softmax(&mut scores);
                ch.fill(0.0);
                for (s, vt) in scores.iter().zip(v.chunks_exact(h)) {
                    if let Some(vh) = vt.get(part.clone()) {
                        for (c, a) in ch.iter_mut().zip(vh) {
                            *c += s * a;
                        }
                    }
                }
            }
        }
    }
}
