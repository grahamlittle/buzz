//! Load stage: sign and emit events, and dedup via the relay.
//!
//! Events are built with `buzz-sdk` and signed by the single `buzz-import` key.
//! Transport is the HTTP bridge (`POST /events` to emit, `POST /query` to dedup)
//! or `buzz-ws-client`. Dedup queries MUST include `kinds` — an open-ended REQ
//! hits the relay p-gate and returns 403.

use crate::config::{Config, Transport};
use crate::error::{ImportError, Result};
use crate::transform::ItemPayload;

/// Emits events and performs dedup reads against a Buzz relay.
pub struct Emitter {
    relay_url: String,
    transport: Transport,
    dry_run: bool,
    http: reqwest::Client,
}

/// Result of emitting one item's payloads.
#[derive(Debug, Clone)]
pub struct EmitOutcome {
    /// The root event id (the Buzz item id).
    pub item_id: String,
    pub comments: usize,
    pub attachments: usize,
    pub seeded_state: Option<String>,
}

impl Emitter {
    /// Build an emitter from config.
    pub fn new(config: &Config) -> Self {
        Self {
            relay_url: config.relay_url.clone(),
            transport: config.transport,
            dry_run: config.dry_run,
            http: reqwest::Client::new(),
        }
    }

    /// Return the existing Buzz item id for a Jira key, if already imported.
    ///
    /// Dedup filter is `{ "kinds":[<message-kind>], "#t":["jira:<KEY>"] }`.
    pub async fn find_existing(&self, _jira_key: &str) -> Result<Option<String>> {
        let _ = (&self.relay_url, &self.http, self.transport);
        Err(ImportError::Other(
            "emit::find_existing not yet implemented".into(),
        ))
    }

    /// Sign and emit all payloads for one item, in dependency order:
    /// root -> status seed -> comments -> attachments.
    pub async fn emit_item(&self, _payload: &ItemPayload) -> Result<EmitOutcome> {
        if self.dry_run {
            return Err(ImportError::Other(
                "emit::emit_item dry-run rendering not yet implemented".into(),
            ));
        }
        Err(ImportError::Other(
            "emit::emit_item not yet implemented".into(),
        ))
    }
}
