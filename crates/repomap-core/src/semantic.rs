//! Semantic search: one embedding per chunk from a small static code model
//! (potion-code-16M-v2, through `mcp_kit::embed`). The model is used when it is
//! installed and `REPOMAP_EMBED` is not `0`; otherwise search is keyword-only.

use mcp_kit::embed::{self, Model, Spec, Vec8};
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub const SPEC: Spec = embed::POTION_CODE_16M;

/// Embeddings are on unless `REPOMAP_EMBED=0` (or `false`, `off`).
pub fn enabled() -> bool {
    !matches!(std::env::var("REPOMAP_EMBED").as_deref(), Ok("0") | Ok("false") | Ok("off"))
}

/// The model, loaded once per process, when it is enabled and installed. While
/// it is missing, this checks again at most once a second, so a server picks
/// it up once a background download finishes.
pub fn model() -> Option<&'static Model> {
    static MODEL: OnceLock<Option<Model>> = OnceLock::new();
    static LOADING: Mutex<()> = Mutex::new(());
    static NEXT_CHECK: AtomicU64 = AtomicU64::new(0);
    if let Some(m) = MODEL.get() {
        return m.as_ref();
    }
    if !enabled() {
        return None;
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    if now < NEXT_CHECK.load(Ordering::Relaxed) {
        return None;
    }
    let _guard = LOADING.lock().unwrap();
    if let Some(m) = MODEL.get() {
        return m.as_ref();
    }
    if !embed::installed(&SPEC) {
        NEXT_CHECK.store(now + 1000, Ordering::Relaxed);
        return None;
    }
    let loaded = match Model::load(&SPEC) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!("repomap: cannot load the embedding model ({e:#}); search is keyword-only.");
            None
        }
    };
    MODEL.get_or_init(|| loaded).as_ref()
}

/// The id stored with cached facts, so a cache built without the model (or
/// with another one) is rebuilt.
pub fn model_id() -> &'static str {
    if model().is_some() { SPEC.id } else { "" }
}

/// Download the model if it is missing (once, with a message on stderr).
/// Returns false when search stays keyword-only.
pub fn ensure() -> bool {
    if !enabled() {
        return false;
    }
    match embed::ensure(&SPEC, "repomap", "Set REPOMAP_EMBED=0 to stay keyword-only.") {
        Ok(()) => true,
        Err(e) => {
            eprintln!("repomap: embedding model unavailable ({e:#}); search is keyword-only.");
            false
        }
    }
}

/// What a chunk means, for the embedding: its file path, then its code.
pub fn chunk_text(path: &str, body: &str) -> String {
    let mut s = String::with_capacity(path.len() + 1 + body.len().min(4096));
    s.push_str(path);
    s.push('\n');
    s.push_str(body);
    s
}

/// Chunk embeddings, parallel to `Bm25::chunks` (int8 rows, one scale each;
/// a zero scale means the chunk has no embedding).
#[derive(Default)]
pub struct Dense {
    pub dims: usize,
    q: Vec<i8>,
    s: Vec<f32>,
}

impl Dense {
    pub fn new(dims: usize) -> Dense {
        Dense { dims, ..Default::default() }
    }

    pub fn push(&mut self, v: Option<&Vec8>) {
        match v {
            Some(v) if v.q.len() == self.dims => {
                self.q.extend_from_slice(&v.q);
                self.s.push(v.s);
            }
            _ => {
                self.q.extend(std::iter::repeat_n(0, self.dims));
                self.s.push(0.0);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.dims == 0 || self.s.iter().all(|s| *s == 0.0)
    }

    /// The `limit` chunks most similar to the unit vector `query` among those
    /// `keep` accepts, best first.
    pub fn search(&self, query: &[f32], limit: usize, keep: impl Fn(u32) -> bool + Sync) -> Vec<(u32, f32)> {
        if query.len() != self.dims || self.dims == 0 {
            return Vec::new();
        }
        let score = |i: usize| (i as u32, embed::cosine_q(query, &self.q[i * self.dims..(i + 1) * self.dims], self.s[i]));
        let wanted = |i: &usize| self.s[*i] != 0.0 && keep(*i as u32);
        // Large repositories score their chunks on every core.
        let mut scored: Vec<(u32, f32)> = if self.s.len() > 20_000 {
            (0..self.s.len()).into_par_iter().filter(wanted).map(score).collect()
        } else {
            (0..self.s.len()).filter(wanted).map(score).collect()
        };
        let limit = limit.min(scored.len());
        if limit == 0 {
            return Vec::new();
        }
        scored.select_nth_unstable_by(limit - 1, |a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit);
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_search_ranks_by_cosine_and_skips_missing() {
        let mut d = Dense::new(2);
        d.push(Some(&embed::quantize(&[1.0, 0.0])));
        d.push(None);
        d.push(Some(&embed::quantize(&[0.6, 0.8])));
        d.push(Some(&embed::quantize(&[0.0, 1.0])));
        let hits = d.search(&[0.0, 1.0], 2, |_| true);
        assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), vec![3, 2]);
        assert!((hits[0].1 - 1.0).abs() < 0.02);
        // Filtered chunks and chunks without an embedding never come back.
        let hits = d.search(&[0.0, 1.0], 10, |c| c != 3);
        assert_eq!(hits.iter().map(|h| h.0).collect::<Vec<_>>(), vec![2, 0]);
        assert!(Dense::new(2).is_empty() && !d.is_empty());
    }
}
