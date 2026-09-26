//! In-memory indexes per repository root, refreshed when files change.

use anyhow::Result;
use repomap_core::{BuildOptions, Index};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Entry {
    index: Arc<Index>,
    checked: Instant,
}

#[derive(Default)]
pub struct Workspace {
    entries: Mutex<HashMap<PathBuf, Entry>>,
}

const RECHECK: Duration = Duration::from_millis(1500);

impl Workspace {
    pub fn get(&self, root: &Path) -> Result<Arc<Index>> {
        let root = root.canonicalize()?;
        let mut map = self.entries.lock().unwrap();
        if let Some(e) = map.get_mut(&root) {
            if e.checked.elapsed() < RECHECK {
                return Ok(e.index.clone());
            }
            // Rebuild when files changed or the embedding model arrived.
            if repomap_core::index::fingerprint(&root).ok() == Some(e.index.fingerprint)
                && e.index.model_id == repomap_core::semantic::model_id()
            {
                e.checked = Instant::now();
                return Ok(e.index.clone());
            }
        }
        let index = Arc::new(Index::build(&root, &BuildOptions::default())?);
        map.insert(root, Entry { index: index.clone(), checked: Instant::now() });
        Ok(index)
    }
}
