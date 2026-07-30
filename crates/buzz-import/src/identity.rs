//! Jira accountId -> Nostr pubkey identity map.
//!
//! Mapped assignees/reporters become a `p` tag on the item. Unmapped people are
//! metadata-only (name in the body), never a `p` tag.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// One person's mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    /// Nostr public key (hex or npub).
    pub pubkey: String,
    /// Display name, used for metadata when unmapped elsewhere.
    pub display: String,
}

/// The full identity map, keyed by Jira accountId.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityMap {
    #[serde(flatten)]
    people: HashMap<String, Person>,
}

impl IdentityMap {
    /// Load the identity map from a JSON file.
    pub fn load(path: &Path) -> crate::error::Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            crate::error::ImportError::Input(format!("read identity map {path:?}: {e}"))
        })?;
        serde_json::from_str(&raw).map_err(|e| {
            crate::error::ImportError::Input(format!("parse identity map {path:?}: {e}"))
        })
    }

    /// Resolve a Jira accountId to a Nostr pubkey, if mapped.
    pub fn pubkey(&self, account_id: &str) -> Option<&str> {
        self.people.get(account_id).map(|p| p.pubkey.as_str())
    }

    /// Resolve a Jira accountId to a display name, if known.
    pub fn display(&self, account_id: &str) -> Option<&str> {
        self.people.get(account_id).map(|p| p.display.as_str())
    }
}
