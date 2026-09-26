//! repomap core: a map of your codebase for AI agents.
//!
//! Walks a repository (respecting `.gitignore`), parses code with
//! tree-sitter, links imports and calls into a graph, ranks files with
//! PageRank, groups them into Louvain communities, and indexes AST chunks
//! with BM25 and, when the model is installed, a static code embedding. Queries: map, search, context, trace, impact.

pub mod bm25;
pub mod db;
pub mod export;
pub mod graph;
pub mod index;
pub mod lang;
pub mod parse;
pub mod query;
pub mod score;
pub mod semantic;
pub mod tokenize;

pub use index::{BuildOptions, Index};
pub use query::{ContextOptions, Direction, ImpactOptions, MapOptions, SearchOptions, TraceOptions};
