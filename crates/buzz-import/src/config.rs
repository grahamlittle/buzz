//! Importer configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level importer config, loaded from a JSON/YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Products to migrate: Jira project key -> Buzz channel id.
    pub products: Vec<ProductMap>,

    /// JQL selecting the OPEN items to migrate. Default excludes Done.
    #[serde(default = "default_selection_jql")]
    pub selection_jql: String,

    /// Path to the Jira accountId -> Nostr pubkey identity map.
    pub identity_map: PathBuf,

    /// Path to the Jira status -> Buzz workflow-state map.
    pub status_map: PathBuf,

    /// Relay base URL. HTTP bridge (`POST /events`, `/query`) or WS origin.
    pub relay_url: String,

    /// Dedup read strategy.
    #[serde(default)]
    pub dedup: DedupMode,

    /// Transport for emit + dedup.
    #[serde(default)]
    pub transport: Transport,

    /// Concurrency + backoff.
    #[serde(default)]
    pub throttle: Throttle,

    /// When true, compute and log payloads but emit nothing.
    #[serde(default = "default_true")]
    pub dry_run: bool,

    /// Append-only ledger path (idempotency + resume).
    #[serde(default = "default_ledger")]
    pub ledger: PathBuf,

    /// Workflow-state name -> Buzz workflow UUID, for status seeding. A state
    /// with no mapping is skipped (workflows are UUID-keyed, not name-keyed).
    #[serde(default)]
    pub workflow_ids: HashMap<String, String>,
}

/// One product's Jira project -> Buzz channel mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductMap {
    /// Jira project key, e.g. "UFX".
    pub jira: String,
    /// Buzz channel id the items root into.
    pub channel: String,
}

/// Dedup read strategy. Relay REQ is the default (single source of truth);
/// Postgres is a fallback only.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DedupMode {
    /// Relay REQ with `kinds` + `#t` filter (via `POST /query` or WS).
    #[default]
    Req,
    /// Direct read-only Postgres containment query. Fallback only.
    Postgres,
}

/// Emit + dedup transport.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// HTTP bridge: `POST /events`, `POST /query`, `POST /count`.
    #[default]
    Http,
    /// WebSocket via `buzz-ws-client`.
    Ws,
}

/// Concurrency and backoff knobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Throttle {
    pub concurrency: usize,
    pub backoff_ms: u64,
    pub max_retries: u32,
}

impl Default for Throttle {
    fn default() -> Self {
        Self {
            concurrency: 2,
            backoff_ms: 500,
            max_retries: 5,
        }
    }
}

fn default_selection_jql() -> String {
    "statusCategory != Done ORDER BY created ASC".to_string()
}

fn default_ledger() -> PathBuf {
    PathBuf::from("import-ledger.jsonl")
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load config from a JSON file.
    pub fn load(path: &std::path::Path) -> crate::error::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| crate::error::ImportError::Input(format!("read config {path:?}: {e}")))?;
        serde_json::from_str(&raw)
            .map_err(|e| crate::error::ImportError::Input(format!("parse config {path:?}: {e}")))
    }
}
