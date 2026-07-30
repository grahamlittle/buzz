//! Jira status -> Buzz workflow-state map.
//!
//! Current state only; transition history is not replayed. Gated transitions are
//! not auto-approved during import (that would fake approvals) — the item lands
//! at the state and its future transitions are native.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Maps a Jira status name to a Buzz per-transition workflow state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusMap {
    #[serde(flatten)]
    states: HashMap<String, String>,
}

impl StatusMap {
    /// Load the status map from a JSON file.
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            crate::error::ImportError::Input(format!("read status map {path:?}: {e}"))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            crate::error::ImportError::Input(format!("parse status map {path:?}: {e}"))
        })
    }

    /// Resolve a Jira status to a Buzz workflow state, if mapped.
    pub fn state(&self, jira_status: &str) -> Option<&str> {
        self.states.get(jira_status).map(String::as_str)
    }
}
