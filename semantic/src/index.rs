//! What a query is compared with: one embedding per document (a module), each kept as 384
//! values of 4 bits and a scale, with the document's id — 0.2 kB a module, 1 MB for the catalog.
//!
//! Radix computes the modules' vectors so (`quantize`, this crate as WASM) and publishes them in
//! the snapshot (`v_module_vector`); the server and the browser make an index of them
//! (`push_codes`, or the file `to_bytes` writes) and search it the same way (`search`).
//!
//! A vector is packed (`quantize`): a value v of -7..7 is the nibble v + 8, two to a byte, the
//! first in the low nibble; the vector is the values × the scale. 4 bits rather than 8 halve the
//! vectors in the snapshot for 2.6 points of the first 10 (semantic/README.md, „Quality“).
//!
//! File layout (little endian):
//!
//! ```text
//! b"E5I3", u32 rows, u32 dims
//! rows × (u16 length, UTF-8 id)
//! rows × f32 scale
//! rows × dims / 2 bytes, the packed values
//! ```

use crate::reader::Reader;
#[cfg(not(target_arch = "wasm32"))]
use crate::Model;

const MAGIC: &[u8; 4] = b"E5I3";

/// The largest value of a packed vector: 4 bits, symmetric around 0.
const LEVELS: f32 = 7.0;

pub struct Index {
    dims: usize,
    ids: Vec<String>,
    scales: Vec<f32>,
    codes: Vec<i8>, // unpacked: a byte a value, for the dot products
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
    /// An empty index for embeddings of `dims` values (an even number: two a byte).
    pub fn new(dims: usize) -> Self {
        Self { dims, ids: Vec::new(), scales: Vec::new(), codes: Vec::new() }
    }

    /// Adds a document: its embedding packed (`quantize`), as Radix publishes it.
    pub fn push(&mut self, id: impl Into<String>, embedding: &[f32]) -> Result<(), String> {
        if embedding.len() != self.dims {
            return Err(format!("an embedding of {} values for an index of {}", embedding.len(), self.dims));
        }
        let (scale, packed) = quantize(embedding);
        self.push_codes(id, scale, &packed)
    }

    /// Adds a document whose vector is packed already: a module's as Radix publishes it
    /// (`v_module_vector`, `catalog::queries::module_vectors`), `packed` and its `scale`.
    pub fn push_codes(&mut self, id: impl Into<String>, scale: f32, packed: &[u8]) -> Result<(), String> {
        if packed.len() * 2 != self.dims {
            return Err(format!("a vector of {} bytes for an index of {} values", packed.len(), self.dims));
        }
        if !(scale.is_finite() && scale > 0.0) {
            // Without the number: formatting a float would pull its code into the WASM module.
            return Err("a vector whose scale is not a positive number".into());
        }
        if packed.iter().any(|b| b & 0x0f == 0 || b >> 4 == 0) {
            return Err("a packed value of -8, which quantize never writes".into());
        }
        self.codes.extend(unpack(packed));
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
            return Err("not an index (E5I3)".into());
        }
        let rows = r.usize()?;
        let dims = r.usize()?;
        if !dims.is_multiple_of(2) {
            return Err(format!("{dims} values a vector: not two a byte"));
        }
        let mut ids = Vec::with_capacity(rows.min(bytes.len() / 2)); // an id takes two bytes at least
        for _ in 0..rows {
            let len = usize::from(u16::from_le_bytes(r.take(2)?.try_into().map_err(|_| "an id's length")?));
            ids.push(std::str::from_utf8(r.take(len)?).map_err(|e| e.to_string())?.to_string());
        }
        let scales = r.f32s(rows)?;
        // What the header promises, against what the file holds, before anything is reserved.
        let packed = rows.checked_mul(dims / 2).ok_or("an index beyond the address space")?;
        if bytes.len() - r.at() != packed {
            return Err(format!("{} bytes of vectors, {packed} expected", bytes.len() - r.at()));
        }
        let mut index = Self { dims, ids: Vec::with_capacity(rows), scales: Vec::with_capacity(rows), codes: Vec::with_capacity(packed * 2) };
        for (id, scale) in ids.into_iter().zip(scales) {
            index.push_codes(id, scale, r.take(dims / 2)?)?;
        }
        if r.at() != bytes.len() {
            return Err(format!("{} bytes left over", bytes.len() - r.at()));
        }
        Ok(index)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(12 + self.ids.iter().map(|id| 2 + id.len()).sum::<usize>() + 4 * self.scales.len() + self.codes.len() / 2);
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
        out.extend(pack(&self.codes));
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

/// An embedding as the index keeps it and Radix publishes it (`v_module_vector`, computed by
/// this crate as WASM): values of -7..7 and one scale, the largest value ±7 (scale 1 for an
/// embedding of zeros), packed two to a byte (the module's documentation). An embedding of an
/// odd number of values gets a 0 at the end.
#[allow(clippy::cast_possible_truncation)] // rounded and clamped to ±7 first
pub fn quantize(embedding: &[f32]) -> (f32, Vec<u8>) {
    let largest = embedding.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let scale = if largest > 0.0 { largest / LEVELS } else { 1.0 };
    let codes: Vec<i8> = embedding.iter().map(|v| (v / scale).round().clamp(-LEVELS, LEVELS) as i8).collect();
    (scale, pack(&codes))
}

/// Values of -7..7, two to a byte: the nibble v + 8, the first in the low nibble.
fn pack(codes: &[i8]) -> Vec<u8> {
    codes
        .chunks(2)
        .map(|pair| {
            let nibble = |v: Option<&i8>| (v.copied().unwrap_or(0) + 8).cast_unsigned() & 0x0f;
            nibble(pair.first()) | nibble(pair.get(1)) << 4
        })
        .collect()
}

fn unpack(packed: &[u8]) -> impl Iterator<Item = i8> + '_ {
    packed.iter().flat_map(|b| [(b & 0x0f).cast_signed() - 8, (b >> 4).cast_signed() - 8])
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
        let mut index = Index::new(10);
        index.push("a", &unit(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])).unwrap();
        index.push("b", &unit(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.2, 0.0])).unwrap();
        index.push("c", &unit(&[0.7, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])).unwrap();
        index.push("Ökologie", &[0.0; 10]).unwrap();
        assert!(index.push("short", &[1.0]).is_err());

        let index = Index::from_bytes(&index.to_bytes().unwrap()).unwrap();
        assert_eq!(index.len(), 4);
        let query = unit(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let hits = index.search(&query, 2);
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), ["b", "c"]);
        assert!((hits[0].score - 1.0 / 1.04f32.sqrt()).abs() < 0.001, "{hits:?}");
        assert_eq!(index.search(&query, 10).len(), 4);
        assert_eq!(index.search(&query, 10)[2].id, "a", "ties in the order of the rows");
        assert!(index.search(&[1.0], 3).is_empty());
        assert_eq!(index.id(3), Some("Ökologie"));
    }

    #[test]
    fn packs_two_values_to_a_byte() {
        // 0.6 / (0.8 / 7) = 5.25 → 5, nibble 13; -0.8 → -7, nibble 1; 0 → nibble 8.
        let (scale, packed) = quantize(&unit(&[0.6, -0.8, 0.0, 0.0]));
        assert_eq!((scale, packed.as_slice()), (0.8 / 7.0, [0x1d, 0x88].as_slice()));
        assert_eq!(unpack(&packed).collect::<Vec<_>>(), [5, -7, 0, 0]);
        // 0.6 · 7 = 4.2 → 4 and 0.3 · 7 = 2.1 → 2: nibbles 12 and 10; -1 → -7, then the 0 an odd one gets.
        assert_eq!(quantize(&[0.6, 0.3, -1.0]).1, [0xac, 0x81]);
        assert_eq!(quantize(&[0.0; 4]), (1.0, vec![0x88, 0x88]));
    }

    #[test]
    fn takes_published_vectors_as_they_are() {
        let mut built = Index::new(4);
        built.push("a", &unit(&[0.6, -0.8, 0.0, 0.0])).unwrap();
        let mut published = Index::new(4);
        published.push_codes("a", 0.8 / 7.0, &[0x1d, 0x88]).unwrap();
        assert_eq!(built.to_bytes().unwrap(), published.to_bytes().unwrap());
        assert!(published.push_codes("b", 1.0, &[0x88]).is_err(), "too short");
        assert!(published.push_codes("b", 1.0, &[0x80, 0x88]).is_err(), "-8 is never written");
        assert!(published.push_codes("b", 0.0, &[0x88, 0x88]).is_err());
        assert!(published.push_codes("b", f32::NAN, &[0x88, 0x88]).is_err());
        assert!(Index::new(3).push_codes("b", 1.0, &[0x88, 0x88]).is_err(), "an odd number of values");
        assert_eq!(published.len(), 1);
    }

    #[test]
    fn rejects_what_is_not_an_index() {
        assert!(Index::from_bytes(b"E5I1\0\0\0\0").is_err());
        // A header that promises more than the file holds is an error, not an allocation.
        let mut huge = b"E5I3".to_vec();
        huge.extend_from_slice(&0u32.to_le_bytes());
        huge.extend_from_slice(&u32::MAX.wrapping_sub(1).to_le_bytes());
        assert!(Index::from_bytes(&huge).is_ok_and(|i| i.is_empty()), "no rows: nothing to reserve");
        let mut short = Index::new(4);
        short.push("a", &[0.5, 0.5, 0.5, 0.5]).unwrap();
        let bytes = short.to_bytes().unwrap();
        assert!(Index::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        let mut bytes = Index::new(2).to_bytes().unwrap();
        bytes.push(0);
        assert!(Index::from_bytes(&bytes).is_err());
    }
}
