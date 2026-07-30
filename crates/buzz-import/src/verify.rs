//! Verify stage: reconcile the Jira selection against what landed in Buzz.
//!
//! For every open Jira key: confirm a `jira:<KEY>`-tagged event exists in Buzz
//! and carries the required tags (`jira:`, `type:`, `region:`). Cross-checks the
//! ledger's done/failed counts. Workflow-state verification is deferred until
//! status seeding is wired.

use crate::config::Config;
use crate::emit::Emitter;
use crate::error::Result;
use crate::jira::JiraClient;
use crate::ledger::{Ledger, Stage};

const REQUIRED_TAG_PREFIXES: &[&str] = &["jira:", "type:", "region:"];

/// Outcome of the verify pass.
#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    /// Open Jira items selected.
    pub selected: usize,
    /// Items found in Buzz with all required tags.
    pub present: usize,
    /// Keys selected in Jira but absent from Buzz.
    pub missing: Vec<String>,
    /// Keys present in Buzz but missing a required tag.
    pub tag_mismatches: Vec<String>,
    /// Ledger rows at stage `done`.
    pub ledger_done: usize,
    /// Ledger rows at stage `failed`.
    pub ledger_failed: usize,
}

impl VerifyReport {
    /// True when every selected item is present, correctly tagged, and no ledger
    /// failures remain.
    pub fn ok(&self) -> bool {
        self.missing.is_empty()
            && self.tag_mismatches.is_empty()
            && self.ledger_failed == 0
            && self.present == self.selected
    }
}

/// Run the verify pass over Jira, the relay, and the ledger.
pub async fn run(
    config: &Config,
    jira: &JiraClient,
    emitter: &Emitter,
    ledger: &Ledger,
) -> Result<VerifyReport> {
    let mut report = VerifyReport::default();

    for product in &config.products {
        let keys = jira
            .fetch_open_keys(&product.jira, &config.selection_jql)
            .await?;
        report.selected += keys.len();

        for key in keys {
            match emitter.get_item(&key).await? {
                None => report.missing.push(key),
                Some(event) => {
                    if has_required_tags(&event) {
                        report.present += 1;
                    } else {
                        report.tag_mismatches.push(key);
                    }
                }
            }
        }
    }

    for entry in ledger.load()? {
        match entry.stage {
            Stage::Done => report.ledger_done += 1,
            Stage::Failed => report.ledger_failed += 1,
            _ => {}
        }
    }

    Ok(report)
}

/// True when the event carries every required `t` tag prefix.
fn has_required_tags(event: &serde_json::Value) -> bool {
    let tags = match event.get("tags").and_then(|t| t.as_array()) {
        Some(t) => t,
        None => return false,
    };
    let t_values: Vec<&str> = tags
        .iter()
        .filter_map(|tag| tag.as_array())
        .filter(|parts| parts.first().and_then(|k| k.as_str()) == Some("t"))
        .filter_map(|parts| parts.get(1).and_then(|v| v.as_str()))
        .collect();

    REQUIRED_TAG_PREFIXES
        .iter()
        .all(|prefix| t_values.iter().any(|v| v.starts_with(prefix)))
}
