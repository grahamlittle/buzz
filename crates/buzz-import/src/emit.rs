//! Load stage: sign and emit events, and dedup via the relay.
//!
//! Events are built as `Kind::Custom(9)` stream messages (the same kind
//! `buzz-sdk::build_message` uses) and signed by the single `buzz-import` key
//! (`BUZZ_PRIVATE_KEY`). Transport is the HTTP bridge: `POST /events` to emit,
//! `POST /query` to dedup, each authenticated with a NIP-98 `Authorization`
//! header. Dedup queries MUST include `kinds` — an open-ended REQ hits the
//! relay p-gate and returns 403.
//!
//! Attachment upload (Blossom) and status seeding (a workflow trigger) are
//! separate subsystems and are not performed here; `emit_item` handles the root
//! message and the consolidated history reply.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use nostr::{EventBuilder, JsonUtil, Keys, Kind, Tag};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::error::{ImportError, Result};
use crate::transform::{ItemPayload, Tag as RawTag};

const MESSAGE_KIND: u16 = 9;
const NIP98_KIND: u16 = 27235;

/// Emits events and performs dedup reads against a Buzz relay over the HTTP bridge.
pub struct Emitter {
    relay_url: String,
    keys: Keys,
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
    /// Build an emitter from config. The import key is read from `BUZZ_PRIVATE_KEY`.
    pub fn new(config: &Config) -> Result<Self> {
        let secret = std::env::var("BUZZ_PRIVATE_KEY")
            .map_err(|_| ImportError::Auth("BUZZ_PRIVATE_KEY not set".into()))?;
        let keys = Keys::parse(&secret)
            .map_err(|e| ImportError::Auth(format!("invalid BUZZ_PRIVATE_KEY: {e}")))?;
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| ImportError::Other(format!("http client: {e}")))?;
        Ok(Self {
            relay_url: config.relay_url.trim_end_matches('/').to_string(),
            keys,
            dry_run: config.dry_run,
            http,
        })
    }

    /// Return the existing Buzz item event for a Jira key, if already imported.
    ///
    /// Dedup filter is `{ "kinds":[9], "#t":["jira:<KEY>"], "limit":1 }`.
    pub async fn get_item(&self, jira_key: &str) -> Result<Option<serde_json::Value>> {
        let filter = serde_json::json!([{
            "kinds": [MESSAGE_KIND],
            "#t": [format!("jira:{jira_key}")],
            "limit": 1,
        }]);
        Ok(self.query(&filter).await?.into_iter().next())
    }

    /// Return the existing Buzz item id for a Jira key, if already imported.
    pub async fn find_existing(&self, jira_key: &str) -> Result<Option<String>> {
        Ok(self
            .get_item(jira_key)
            .await?
            .and_then(|e| e.get("id").and_then(|id| id.as_str()).map(str::to_string)))
    }

    /// Sign and emit all payloads for one item, in dependency order:
    /// root -> comments (one consolidated history reply).
    ///
    /// Attachment upload and status seeding are handled by their own stages.
    pub async fn emit_item(&self, payload: &ItemPayload) -> Result<EmitOutcome> {
        let root = self.build_event(&payload.root_body, &payload.root_tags)?;
        let item_id = self.emit_event(&root).await?;

        let mut comments = 0;
        if let Some(body) = &payload.history_reply {
            let reply_tags = self.reply_tags(&payload.root_tags, &item_id)?;
            let reply = self.build_event(body, &reply_tags)?;
            self.emit_event(&reply).await?;
            comments = 1;
        }

        Ok(EmitOutcome {
            item_id,
            comments,
            attachments: payload.attachments.len(),
            seeded_state: payload.seed_state.clone(),
        })
    }

    /// Build and sign a `Kind::Custom(9)` event from raw tags.
    fn build_event(&self, content: &str, raw_tags: &[RawTag]) -> Result<nostr::Event> {
        let tags = parse_tags(raw_tags)?;
        EventBuilder::new(Kind::Custom(MESSAGE_KIND), content)
            .tags(tags)
            .sign_with_keys(&self.keys)
            .map_err(|e| ImportError::Other(format!("event signing failed: {e}")))
    }

    /// Reply tags: the channel `h` tag (copied from the root) + a NIP-10 `e` tag
    /// referencing the root event.
    fn reply_tags(&self, root_tags: &[RawTag], root_id: &str) -> Result<Vec<RawTag>> {
        let h = root_tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("h"))
            .cloned()
            .ok_or_else(|| ImportError::Other("root tags missing h (channel) tag".into()))?;
        Ok(vec![
            h,
            vec!["e".into(), root_id.into(), String::new(), "root".into()],
        ])
    }

    /// Emit a signed event via `POST /events`. Honours dry-run (logs, no send).
    async fn emit_event(&self, event: &nostr::Event) -> Result<String> {
        let body = serde_json::to_vec(event)
            .map_err(|e| ImportError::Other(format!("event serialization: {e}")))?;
        if self.dry_run {
            println!("{}", String::from_utf8_lossy(&body));
            return Ok(event.id.to_hex());
        }
        let url = format!("{}/events", self.relay_url);
        let auth = self.nip98_auth("POST", &url, Some(&body))?;
        let resp = self
            .http
            .post(&url)
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        if status == reqwest::StatusCode::CONFLICT {
            return Err(ImportError::WriteConflict(text));
        }
        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ImportError::Auth(text));
        }
        if !status.is_success() {
            return Err(ImportError::Network(format!(
                "POST /events {status}: {text}"
            )));
        }
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| ImportError::Other(format!("parse /events response: {e}")))?;
        if v.get("accepted").and_then(serde_json::Value::as_bool) == Some(false) {
            return Err(ImportError::WriteConflict(text));
        }
        Ok(v.get("event_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// One-shot REQ via `POST /query`. Returns the array of matched events.
    async fn query(&self, filter: &serde_json::Value) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/query", self.relay_url);
        let body = serde_json::to_vec(filter)
            .map_err(|e| ImportError::Other(format!("filter serialization: {e}")))?;
        let auth = self.nip98_auth("POST", &url, Some(&body))?;
        let resp = self
            .http
            .post(&url)
            .header("Authorization", auth)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        if !status.is_success() {
            return Err(ImportError::Network(format!(
                "POST /query {status}: {text}"
            )));
        }
        serde_json::from_str(&text)
            .map_err(|e| ImportError::Other(format!("parse /query response: {e}")))
    }

    /// Build a NIP-98 `Authorization: Nostr <base64>` header for an HTTP request.
    fn nip98_auth(&self, method: &str, url: &str, body: Option<&[u8]>) -> Result<String> {
        let mut tags = vec![
            parse_tag(&["u", url])?,
            parse_tag(&["method", method])?,
            parse_tag(&["nonce", &uuid::Uuid::new_v4().to_string()])?,
        ];
        if let Some(b) = body {
            let hash = hex::encode(Sha256::digest(b));
            tags.push(parse_tag(&["payload", &hash])?);
        }
        let event = EventBuilder::new(Kind::Custom(NIP98_KIND), "")
            .tags(tags)
            .sign_with_keys(&self.keys)
            .map_err(|e| ImportError::Auth(format!("NIP-98 signing failed: {e}")))?;
        Ok(format!("Nostr {}", B64.encode(event.as_json().as_bytes())))
    }
}

/// Parse a slice of raw string-vector tags into `nostr::Tag`s.
fn parse_tags(raw: &[RawTag]) -> Result<Vec<Tag>> {
    raw.iter()
        .map(|t| Tag::parse(t.iter().map(String::as_str)))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| ImportError::Other(format!("invalid tag: {e}")))
}

/// Parse a single tag from string parts.
fn parse_tag(parts: &[&str]) -> Result<Tag> {
    Tag::parse(parts.iter().copied()).map_err(|e| ImportError::Other(format!("invalid tag: {e}")))
}
