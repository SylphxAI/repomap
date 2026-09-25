//! BM25 over code chunks (the Locus engine, now AST-chunked).

use crate::index::FileEntry;
use crate::parse::FileFacts;
use crate::tokenize::tokenize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub file: u32,
    pub start: u32,
    pub end: u32,
    /// Global symbol index when the chunk is a definition.
    pub symbol: Option<u32>,
    pub len: u32,
}

#[derive(Default)]
pub struct Bm25 {
    pub chunks: Vec<ChunkRef>,
    postings: HashMap<String, Vec<(u32, u16)>>,
    avg_len: f32,
}

pub struct Hit {
    pub chunk: u32,
    pub score: f32,
    pub matched: Vec<String>,
}

const K1: f32 = 1.2;
const B: f32 = 0.75;

impl Bm25 {
    pub fn build<'a>(files: impl Iterator<Item = (u32, &'a FileEntry, &'a FileFacts)>) -> Bm25 {
        let mut chunks = Vec::new();
        let mut postings: HashMap<String, Vec<(u32, u16)>> = HashMap::new();
        let mut total: u64 = 0;
        for (fi, entry, facts) in files {
            for c in &facts.chunks {
                let id = chunks.len() as u32;
                chunks.push(ChunkRef {
                    file: fi,
                    start: c.start,
                    end: c.end,
                    symbol: c.symbol.map(|s| s + entry.sym_start),
                    len: c.len,
                });
                total += c.len as u64;
                for (t, f) in c.terms.split(' ').zip(c.tfs.iter()) {
                    match postings.get_mut(t) {
                        Some(v) => v.push((id, *f)),
                        None => {
                            postings.insert(t.to_string(), vec![(id, *f)]);
                        }
                    }
                }
            }
        }
        let avg_len = if chunks.is_empty() { 1.0 } else { total as f32 / chunks.len() as f32 };
        Bm25 { chunks, postings, avg_len }
    }

    /// Files that contain `term` (lowercased token) anywhere in a chunk.
    pub fn files_with(&self, term: &str) -> std::collections::HashSet<u32> {
        self.postings
            .get(term)
            .map(|v| v.iter().map(|(c, _)| self.chunks[*c as usize].file).collect())
            .unwrap_or_default()
    }

    pub fn terms(&self) -> usize {
        self.postings.len()
    }

    pub fn search(&self, query: &str, limit: usize, filter: impl Fn(&ChunkRef) -> bool) -> Vec<Hit> {
        let mut qterms = tokenize(query);
        qterms.sort();
        qterms.dedup();
        let n = self.chunks.len() as f32;
        let mut scores: HashMap<u32, (f32, Vec<String>)> = HashMap::new();
        for t in &qterms {
            let Some(list) = self.postings.get(t) else { continue };
            let df = list.len() as f32;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            for &(cid, tf) in list {
                let c = &self.chunks[cid as usize];
                let tf = tf as f32;
                let norm = tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * c.len as f32 / self.avg_len));
                let e = scores.entry(cid).or_insert((0.0, Vec::new()));
                e.0 += idf * norm;
                e.1.push(t.clone());
            }
        }
        let mut hits: Vec<Hit> = scores
            .into_iter()
            .filter(|(cid, _)| filter(&self.chunks[*cid as usize]))
            .map(|(chunk, (score, matched))| {
                // Reward chunks that match more distinct query terms.
                let coverage = matched.len() as f32 / qterms.len().max(1) as f32;
                Hit { chunk, score: score * (0.5 + coverage), matched }
            })
            .collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then(a.chunk.cmp(&b.chunk)));
        hits.truncate(limit);
        hits
    }
}
