//! Append-only ledger for idempotency and resume.
//!
//! One JSON line per item. On restart, items whose stage is `Done` are skipped;
//! partial items resume from their last completed sub-stage.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Per-item migration stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Root,
    Comments,
    Attachments,
    State,
    Done,
    Failed,
}

/// One ledger row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub jira: String,
    pub buzz_item: Option<String>,
    pub stage: Stage,
    pub state: Option<String>,
    pub comments: usize,
    pub attachments: usize,
}

/// Append-only ledger backed by a JSONL file.
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// Open (or create) a ledger at `path`.
    pub fn open(path: &Path) -> crate::error::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Read all entries, keyed by Jira key (last write wins).
    pub fn load(&self) -> crate::error::Result<Vec<Entry>> {
        let raw = match std::fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(crate::error::ImportError::Other(format!(
                    "read ledger {:?}: {e}",
                    self.path
                )))
            }
        };
        let mut out = Vec::new();
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            let entry: Entry = serde_json::from_str(line)
                .map_err(|e| crate::error::ImportError::Other(format!("parse ledger line: {e}")))?;
            out.push(entry);
        }
        Ok(out)
    }

    /// Append one entry.
    pub fn append(&self, entry: &Entry) -> crate::error::Result<()> {
        let line = serde_json::to_string(entry).map_err(|e| {
            crate::error::ImportError::Other(format!("serialize ledger entry: {e}"))
        })?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| {
                crate::error::ImportError::Other(format!("open ledger {:?}: {e}", self.path))
            })?;
        writeln!(file, "{line}")
            .map_err(|e| crate::error::ImportError::Other(format!("write ledger: {e}")))
    }
}
