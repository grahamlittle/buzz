//! Transform stage: Jira issue -> Buzz event payloads.
//!
//! Deterministic: the same issue always produces the same logical payloads, so
//! re-runs are stable and dedup works. Every item carries `jira:<KEY>`,
//! `type:`, `epic:`, `region:` and (if mapped) a `p` assignee tag, plus the
//! NIP-29 `h` channel tag. Original date is an `orig-created:` tag, never the
//! event timestamp (the relay caps drift at +/-15 min).

use crate::identity::IdentityMap;
use crate::jira::JiraIssue;
use crate::status_map::StatusMap;

/// A Nostr tag as a flat string vector (e.g. `["t","jira:UFX-1"]`).
pub type Tag = Vec<String>;

/// The full set of payloads to emit for one migrated item.
#[derive(Debug, Clone)]
pub struct ItemPayload {
    /// Root event content (body) and tags.
    pub root_body: String,
    pub root_tags: Vec<Tag>,
    /// One consolidated "Imported Jira history" reply body, if there are comments.
    pub history_reply: Option<String>,
    /// Attachments to upload + reference.
    pub attachments: Vec<AttachmentRef>,
    /// Buzz workflow state to seed (from the status map), if resolvable.
    pub seed_state: Option<String>,
}

/// An attachment to re-upload and reference from a reply.
#[derive(Debug, Clone)]
pub struct AttachmentRef {
    pub filename: String,
    pub mime_type: String,
    pub source_url: String,
}

/// Classify region from the epic name (`MVP LDN...` -> EMEA, else Asia).
pub fn region_for(epic_name: Option<&str>) -> &'static str {
    match epic_name {
        Some(name) if name.starts_with("MVP LDN") => "EMEA",
        _ => "Asia",
    }
}

/// Build the payloads for one issue, given the channel it roots into.
pub fn build_item(
    issue: &JiraIssue,
    channel_id: &str,
    identity: &IdentityMap,
    status: &StatusMap,
) -> ItemPayload {
    let mut root_tags: Vec<Tag> = vec![
        vec!["h".into(), channel_id.into()],
        vec!["t".into(), format!("jira:{}", issue.key)],
        vec![
            "t".into(),
            format!("type:{}", issue.issue_type.to_lowercase()),
        ],
        vec![
            "t".into(),
            format!("region:{}", region_for(issue.epic_name.as_deref())),
        ],
        vec!["t".into(), format!("orig-created:{}", issue.created)],
    ];
    if let Some(epic) = &issue.epic_key {
        root_tags.push(vec!["t".into(), format!("epic:{epic}")]);
    }
    for label in issue.labels.iter().chain(issue.components.iter()) {
        root_tags.push(vec!["t".into(), format!("label:{label}")]);
    }
    if let Some(acct) = &issue.assignee_account_id {
        if let Some(pk) = identity.pubkey(acct) {
            root_tags.push(vec!["p".into(), pk.into()]);
        }
    }

    let history_reply = if issue.comments.is_empty() {
        None
    } else {
        let mut out = format!("Imported Jira history - {}\n", issue.key);
        for c in &issue.comments {
            out.push_str(&format!(
                "\n---\norig-author: {} - {}\n{}\n",
                c.author_display, c.created, c.body
            ));
        }
        Some(out)
    };

    let attachments = issue
        .attachments
        .iter()
        .map(|a| AttachmentRef {
            filename: a.filename.clone(),
            mime_type: a.mime_type.clone(),
            source_url: a.content_url.clone(),
        })
        .collect();

    ItemPayload {
        root_body: render_body(issue),
        root_tags,
        history_reply,
        attachments,
        seed_state: status.state(&issue.status).map(str::to_string),
    }
}

/// Render the root body in the workhub house-style format.
fn render_body(issue: &JiraIssue) -> String {
    format!(
        "[{}] {}\n\n{}\n\n---\njira: {}\norig-created: {}",
        issue.issue_type.to_uppercase(),
        issue.summary,
        issue.description,
        issue.key,
        issue.created,
    )
}
