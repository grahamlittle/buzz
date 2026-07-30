//! Jira extract stage.
//!
//! Pulls the OPEN issue set for a project (via the selection JQL), plus each
//! issue's comments and attachments. Current status only — the full changelog is
//! not fetched (no transition replay).
//!
//! Uses the Jira Cloud REST v3 enhanced search (`POST /rest/api/3/search/jql`,
//! token-paginated). Auth is HTTP Basic from `JIRA_EMAIL` + `JIRA_API_TOKEN`.
//! Rich-text fields (description, comment bodies) arrive as ADF (Atlassian
//! Document Format) and are flattened to plain text.

use base64::{engine::general_purpose::STANDARD as B64, Engine};

use crate::error::{ImportError, Result};

const ISSUE_FIELDS: &[&str] = &[
    "summary",
    "issuetype",
    "status",
    "assignee",
    "reporter",
    "labels",
    "components",
    "parent",
    "created",
    "description",
    "attachment",
];

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

/// Client over the Jira Cloud REST v3 API.
pub struct JiraClient {
    base_url: String,
    http: reqwest::Client,
    auth: String,
}

impl JiraClient {
    /// Build a client for the given Jira base URL.
    ///
    /// Credentials are read from `JIRA_EMAIL` + `JIRA_API_TOKEN` (HTTP Basic).
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let email = std::env::var("JIRA_EMAIL")
            .map_err(|_| ImportError::Auth("JIRA_EMAIL not set".into()))?;
        let token = std::env::var("JIRA_API_TOKEN")
            .map_err(|_| ImportError::Auth("JIRA_API_TOKEN not set".into()))?;
        let auth = format!("Basic {}", B64.encode(format!("{email}:{token}")));
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
            auth,
        })
    }

    /// Fetch all open issues for a project, paginated, including comments and
    /// attachments.
    pub async fn fetch_open_issues(
        &self,
        project_key: &str,
        selection_jql: &str,
    ) -> Result<Vec<JiraIssue>> {
        let jql = format!("project = \"{project_key}\" AND {selection_jql}");
        let mut issues = Vec::new();
        let mut next_page: Option<String> = None;

        loop {
            let mut body = serde_json::json!({
                "jql": jql,
                "maxResults": 100,
                "fields": ISSUE_FIELDS,
            });
            if let Some(token) = &next_page {
                body["nextPageToken"] = serde_json::json!(token);
            }

            let page = self.post_json("/rest/api/3/search/jql", &body).await?;

            if let Some(arr) = page.get("issues").and_then(|v| v.as_array()) {
                for raw in arr {
                    let key = str_at(raw, &["key"]).unwrap_or_default();
                    let comments = self.fetch_comments(&key).await?;
                    issues.push(map_issue(raw, comments));
                }
            }

            match page.get("nextPageToken").and_then(|v| v.as_str()) {
                Some(token) if page.get("isLast").and_then(|v| v.as_bool()) != Some(true) => {
                    next_page = Some(token.to_string());
                }
                _ => break,
            }
        }

        Ok(issues)
    }

    /// Fetch all comments for an issue, paginated.
    async fn fetch_comments(&self, issue_key: &str) -> Result<Vec<JiraComment>> {
        let mut out = Vec::new();
        let mut start_at = 0u64;

        loop {
            let path =
                format!("/rest/api/3/issue/{issue_key}/comment?startAt={start_at}&maxResults=100");
            let page = self.get_json(&path).await?;

            let batch = page.get("comments").and_then(|v| v.as_array());
            if let Some(batch) = batch {
                for c in batch {
                    out.push(JiraComment {
                        author_account_id: str_at(c, &["author", "accountId"]),
                        author_display: str_at(c, &["author", "displayName"])
                            .unwrap_or_else(|| "unknown".into()),
                        created: str_at(c, &["created"]).unwrap_or_default(),
                        body: adf_to_text(c.get("body")),
                    });
                }
            }

            let total = page.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            let fetched = batch.map(|b| b.len() as u64).unwrap_or(0);
            start_at += fetched;
            if fetched == 0 || start_at >= total {
                break;
            }
        }

        Ok(out)
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        self.parse(url, resp).await
    }

    async fn get_json(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        self.parse(url, resp).await
    }

    async fn parse(&self, url: String, resp: reqwest::Response) -> Result<serde_json::Value> {
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ImportError::Network(e.to_string()))?;
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ImportError::Auth(format!("{url} {status}: {text}")));
        }
        if !status.is_success() {
            return Err(ImportError::Network(format!("{url} {status}: {text}")));
        }
        serde_json::from_str(&text).map_err(|e| ImportError::Other(format!("parse {url}: {e}")))
    }
}

/// Map one raw issue JSON into a [`JiraIssue`].
fn map_issue(raw: &serde_json::Value, comments: Vec<JiraComment>) -> JiraIssue {
    let f = |p: &[&str]| str_at(raw, p);
    JiraIssue {
        key: f(&["key"]).unwrap_or_default(),
        summary: f(&["fields", "summary"]).unwrap_or_default(),
        description: adf_to_text(raw.pointer("/fields/description")),
        issue_type: f(&["fields", "issuetype", "name"]).unwrap_or_default(),
        status: f(&["fields", "status", "name"]).unwrap_or_default(),
        assignee_account_id: f(&["fields", "assignee", "accountId"]),
        reporter_account_id: f(&["fields", "reporter", "accountId"]),
        labels: str_array(raw.pointer("/fields/labels")),
        components: named_array(raw.pointer("/fields/components")),
        epic_key: f(&["fields", "parent", "key"]),
        epic_name: f(&["fields", "parent", "fields", "summary"]),
        created: f(&["fields", "created"]).unwrap_or_default(),
        comments,
        attachments: map_attachments(raw.pointer("/fields/attachment")),
    }
}

fn map_attachments(v: Option<&serde_json::Value>) -> Vec<JiraAttachment> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| JiraAttachment {
                    filename: str_at(a, &["filename"]).unwrap_or_default(),
                    mime_type: str_at(a, &["mimeType"]).unwrap_or_default(),
                    content_url: str_at(a, &["content"]).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Read a string at a nested key path.
fn str_at(v: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for k in path {
        cur = cur.get(k)?;
    }
    cur.as_str().map(str::to_string)
}

/// Read an array of plain strings.
fn str_array(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Read an array of objects, taking each `name` field.
fn named_array(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Flatten an Atlassian Document Format node to plain text.
///
/// Walks the ADF tree collecting `text` leaves; block nodes (paragraph, heading,
/// list item) are newline-separated. Non-text nodes (media, mentions) are
/// skipped. Good enough to preserve the readable content of a description or
/// comment; not a full ADF renderer.
fn adf_to_text(v: Option<&serde_json::Value>) -> String {
    fn walk(node: &serde_json::Value, out: &mut String) {
        if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
            out.push_str(text);
        }
        if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
            for child in children {
                walk(child, out);
            }
            let block = matches!(
                node.get("type").and_then(|t| t.as_str()),
                Some("paragraph" | "heading" | "listItem" | "blockquote" | "codeBlock")
            );
            if block {
                out.push('\n');
            }
        }
    }

    let mut out = String::new();
    if let Some(v) = v {
        walk(v, &mut out);
    }
    out.trim_end().to_string()
}
