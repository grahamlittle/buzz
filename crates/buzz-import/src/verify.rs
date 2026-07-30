//! Verify stage: reconcile Jira selection against what landed in Buzz.
//!
//! Checks counts (selected == ledger roots == `jira:`-tagged events), tag
//! presence (`jira:`, `type:`, `region:`; `p` on assigned items), and that each
//! item's workflow state matches its mapped Jira status.

use crate::error::{ImportError, Result};
use crate::ledger::Ledger;

/// Outcome of the verify pass.
#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    pub selected: usize,
    pub migrated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub mismatches: Vec<String>,
}

/// Run the verify pass over the ledger and the relay.
pub async fn run(_ledger: &Ledger, _selected: usize) -> Result<VerifyReport> {
    Err(ImportError::Other("verify::run not yet implemented".into()))
}
