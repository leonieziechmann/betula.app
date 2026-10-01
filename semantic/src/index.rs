//! What a query is compared with: one embedding per document (a module), each kept as 384 int8
//! values and a scale, with the document's id — 0.4 kB a module, 1.9 MB for the catalog.
//!
//! The server builds it (`Index::build`, from the module texts, `crate::module_text`) and hands
//! it to browsers as a file (`to_bytes`); both search it the same way (`search`).
//!
//! File layout (little endian):
//!
//! ```text
//! b"E5I2", u32 rows, u32 dims
//! rows × (u16 length, UTF-8 id)
//! rows × f32 scale
//! rows × dims × i8
//! ```

use crate::reader::Reader;
#[cfg(not(target_arch = "wasm32"))]
use crate::Model;

const MAGIC: &[u8; 4] = b"E5I2";

pub struct Index {
    dims: usize,
    ids: Vec<String>,
    scales: Vec<f32>,
    codes: Vec<i8>,
}

/// A document a query found: its id, its row in the index, and the cosine of the two
/// embeddings (1 the same direction; for e5, 0.8 and more is close).
#[derive(Clone, Debug, PartialEq)]
pub struct Hit<'a> {
    pub id: &'a str,
    pub row: usize,
    pub score: f32,
}

impl Index {
    /// An empty index for embeddings of `dims` values.
    pub fn new(dims: usize) -> Self {
        Self { dims, ids: Vec::new(), scales: Vec::new(), codes: Vec::new() }
    }

    /// Adds a document: its embedding as int8 with one scale (`quantize`).
    pub fn push(&mut self, id: impl Into<String>, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dims {
            return Err(format!("an embedding of {} values for an index of {}", embedding.len(), self.dims));
        }
        let (scale, codes) = quantize(embedding);
        self.codes.extend(codes);
        self.scales.push(scale);
        self.ids.push(id.into());
        Ok(())
    }

    /// Adds a document whose embedding is int8 already: a module's vector as Radix publishes it
    /// (`v_module_vector`, `catalog::queries::module_vectors`), `codes` × `scale`.
    pub fn push_codes(&mut self, id: impl Into<String>, scale: f32, codes: &[i8]) -> Result<(), String> {
        if codes.len() != self.dims {
            return Err(format!("a vector of {} values for an index of {}", codes.len(), self.dims));
        }
        if !(scale.is_finite() && scale > 0.0) {
            return Err(format!("a vector with the scale {scale}"));
        }
        self.codes.extend_from_slice(codes);
        self.scales.push(scale);
        self.ids.push(id.into());
        Ok(())
    }

    /// Embeds every `(id, text)` with `model` (as a passage, `Model::embed_passage`) on
    /// `threads` threads (at least one) and keeps the documents in the order given.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn build(model: &Model, documents: &[(String, String)], threads: usize) -> Result<Self, String> {
        let per_thread = documents.len().div_ceil(threads.max(1)).max(1);
        let embeddings: Vec<Vec<Vec<f32>>> = std::thread::scope(|scope| {
            let workers: Vec<_> = documents
                .chunks(per_thread)
                .map(|chunk| scope.spawn(move || chunk.iter().map(|(_, text)| model.embed_passage(text)).collect::<Vec<_>>()))
                .collect();
            workers.into_iter().map(|w| w.join().map_err(|_| "a thread embedding documents failed".to_string())).collect::<Result<_, _>>()
        })?;
        let mut index = Self::new(model.dims());
        for ((id, _), embedding) in documents.iter().zip(embeddings.iter().flatten()) {
            index.push(id.clone(), embedding)?;
        }
        Ok(index)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(bytes);
        if r.take(4)? != MAGIC {
            return Err("not an index (E5I2)".into());
        }
        let rows = r.usize()?;
        let dims = r.usize()?;
        let mut ids = Vec::with_capacity(rows);
        for _ in 0..rows {
            let len = usize::from(u16::from_le_bytes(r.take(2)?.try_into().map_err(|_| "an id's length")?));
            ids.push(std::str::from_utf8(r.take(len)?).map_err(|e| e.to_string())?.to_string());
        }
        let scales = r.f32s(rows)?;
        let codes = r.take(rows.checked_mul(dims).ok_or("an index beyond the address space")?)?.iter().map(|c| c.cast_signed()).collect();
        if r.at() != bytes.len() {
            return Err(format!("{} bytes left over", bytes.len() - r.at()));
        }
        Ok(Self { dims, ids, scales, codes })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(12 + self.ids.iter().map(|id| 2 + id.len()).sum::<usize>() + 4 * self.scales.len() + self.codes.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&u32::try_from(self.len()).map_err(|_| "too many documents")?.to_le_bytes());
        out.extend_from_slice(&u32::try_from(self.dims).map_err(|_| "too many dimensions")?.to_le_bytes());
        for id in &self.ids {
            out.extend_from_slice(&u16::try_from(id.len()).map_err(|_| format!("an id of {} bytes", id.len()))?.to_le_bytes());
            out.extend_from_slice(id.as_bytes());
        }
        for scale in &self.scales {
            out.extend_from_slice(&scale.to_le_bytes());
        }
        out.extend(self.codes.iter().map(|c| c.cast_unsigned()));
        Ok(out)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn dims(&self) -> usize {
        self.dims
    }

    /// The id of the document in row `row`.
    pub fn id(&self, row: usize) -> Option<&str> {
        self.ids.get(row).map(String::as_str)
    }

    /// The `k` documents closest to `query` (an embedding, `Model::embed_query`), best first.
    pub fn search(&self, query: &[f32], k: usize) -> Vec<Hit<'_>> {
        if query.len() != self.dims || self.dims == 0 || k == 0 {
            return Vec::new();
        }
        let mut scored: Vec<(f32, usize)> = self
            .codes
            .chunks_exact(self.dims)
            .zip(&self.scales)
            .enumerate()
            .map(|(row, (codes, scale))| (dot(codes, query) * scale, row))
            .collect();
        let best_first = |a: &(f32, usize), b: &(f32, usize)| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1));
        if k < scored.len() {
            scored.select_nth_unstable_by(k - 1, best_first);
            scored.truncate(k);
        }
        scored.sort_unstable_by(best_first);
        scored.into_iter().filter_map(|(score, row)| Some(Hit { id: self.id(row)?, row, score })).collect()
    }
}

/// An embedding as an index keeps it: int8 codes and one scale, the largest value ±127 (1 for
/// an embedding of zeros). Radix publishes the modules' vectors so (`v_module_vector`, computed
/// by this crate as WASM).
#[allow(clippy::cast_possible_truncation)] // rounded and clamped to ±127 first
pub fn quantize(embedding: &[f32]) -> (f32, Vec<i8>) {
    let largest = embedding.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let scale = if largest > 0.0 { largest / 127.0 } else { 1.0 };
    (scale, embedding.iter().map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8).collect())
}

/// Σ codes · query, on eight lanes (which the compiler vectorises).
fn dot(codes: &[i8], query: &[f32]) -> f32 {
    let (c8, c_rest) = codes.as_chunks::<8>();
    let (q8, q_rest) = query.as_chunks::<8>();
    let mut lanes = [0.0f32; 8];
    for (c, q) in c8.iter().zip(q8) {
        for ((lane, c), q) in lanes.iter_mut().zip(c).zip(q) {
            *lane += f32::from(*c) * q;
        }
    }
    lanes.iter().sum::<f32>() + c_rest.iter().zip(q_rest).map(|(c, q)| f32::from(*c) * q).sum::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(v: &[f32]) -> Vec<f32> {
        let length = v.iter().map(|a| a * a).sum::<f32>().sqrt();
        v.iter().map(|a| a / length).collect()
    }

    #[test]
    fn finds_the_closest_and_survives_the_file() {
        let mut index = Index::new(9);
        index.push("a", &unit(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])).unwrap();
        index.push("b", &unit(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.2])).unwrap();
        index.push("c", &unit(&[0.7, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])).unwrap();
        index.push("Ökologie", &[0.0; 9]).unwrap();
        assert!(index.push("short", &[1.0]).is_err());

        let index = Index::from_bytes(&index.to_bytes().unwrap()).unwrap();
        assert_eq!(index.len(), 4);
        let query = unit(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let hits = index.search(&query, 2);
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), ["b", "c"]);
        assert!((hits[0].score - 1.0 / 1.02f32.sqrt()).abs() < 0.01, "{hits:?}");
        assert_eq!(index.search(&query, 10).len(), 4);
        assert_eq!(index.search(&query, 10)[2].id, "a", "ties in the order of the rows");
        assert!(index.search(&[1.0], 3).is_empty());
        assert_eq!(index.id(3), Some("Ökologie"));
    }

    #[test]
    fn takes_published_vectors_as_they_are() {
        let mut built = Index::new(3);
        built.push("a", &unit(&[0.6, -0.8, 0.0])).unwrap();
        let mut published = Index::new(3);
        published.push_codes("a", 0.8 / 127.0, &[95, -127, 0]).unwrap();
        assert_eq!(built.to_bytes().unwrap(), published.to_bytes().unwrap());
        assert!(published.push_codes("b", 1.0, &[1, 2]).is_err());
        assert!(published.push_codes("b", 0.0, &[1, 2, 3]).is_err());
        assert!(published.push_codes("b", f32::NAN, &[1, 2, 3]).is_err());
        assert_eq!(published.len(), 1);
    }

    #[test]
    fn rejects_what_is_not_an_index() {
        assert!(Index::from_bytes(b"E5I1\0\0\0\0").is_err());
        let mut bytes = Index::new(2).to_bytes().unwrap();
        bytes.push(0);
        assert!(Index::from_bytes(&bytes).is_err());
    }
}
