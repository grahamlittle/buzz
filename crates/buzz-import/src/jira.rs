//! Jira extract stage.
//!
//! Pulls the OPEN issue set for a project (via the selection JQL), plus each
//! issue's comments and attachments. Current status only — the full changelog is
//! not fetched (no transition replay).

use crate::error::{ImportError, Result};

/// A single Jira issue in the open backlog.
#[derive(Debug, Clone)]
pub struct JiraIssue {
    pub key: String,
    pub summary: String,
    pub description: String,
    pub issue_type: String,
    pub status: String,
    pub assignee_account_id: Option<String>,
    pub reporter_account_id: Option<String>,
    pub labels: Vec<String>,
    pub components: Vec<String>,
    pub epic_key: Option<String>,
    pub epic_name: Option<String>,
    pub created: String,
    pub comments: Vec<JiraComment>,
    pub attachments: Vec<JiraAttachment>,
}

/// A Jira comment.
#[derive(Debug, Clone)]
pub struct JiraComment {
    pub author_account_id: Option<String>,
    pub author_display: String,
    pub created: String,
    pub body: String,
}

/// A Jira attachment reference.
#[derive(Debug, Clone)]
pub struct JiraAttachment {
    pub filename: String,
    pub mime_type: String,
    pub content_url: String,
}

/// Client over the Jira REST API.
pub struct JiraClient {
    base_url: String,
    http: reqwest::Client,
}

impl JiraClient {
    /// Build a client for the given Jira base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::new(),
        }
    }

    /// Fetch all open issues for a project, paginated, including comments and
    /// attachments.
    pub async fn fetch_open_issues(
        &self,
        _project_key: &str,
        _selection_jql: &str,
    ) -> Result<Vec<JiraIssue>> {
        let _ = (&self.base_url, &self.http);
        Err(ImportError::Other(
            "jira::fetch_open_issues not yet implemented".into(),
        ))
    }
}
