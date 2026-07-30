//! Live smoke test of the buzz-import relay-facing code against a running relay.
//!
//! Env: BUZZ_RELAY_URL, BUZZ_PRIVATE_KEY, CHANNEL_ID (a stream channel the key
//! can post to). Exercises transform + emit_item + find_existing/get_item
//! (dedup) + emit_attachments (Blossom) + seed_status (workflow trigger).

use std::collections::HashMap;
use std::path::PathBuf;

use buzz_import::config::{Config, Throttle};
use buzz_import::emit::Emitter;
use buzz_import::identity::IdentityMap;
use buzz_import::jira::{JiraComment, JiraIssue};
use buzz_import::status_map::StatusMap;
use buzz_import::transform::build_item;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let relay_url = std::env::var("BUZZ_RELAY_URL")?;
    let channel = std::env::var("CHANNEL_ID")?;

    let config = Config {
        products: vec![],
        selection_jql: String::new(),
        identity_map: PathBuf::new(),
        status_map: PathBuf::new(),
        relay_url,
        dedup: Default::default(),
        transport: Default::default(),
        throttle: Throttle::default(),
        dry_run: false,
        ledger: PathBuf::new(),
        workflow_ids: HashMap::new(),
    };
    let emitter = Emitter::new(&config)?;

    let issue = JiraIssue {
        key: "TEST-1".into(),
        summary: "Live test story".into(),
        description: "Body from the live test.".into(),
        issue_type: "Story".into(),
        status: "To Do".into(),
        assignee_account_id: None,
        reporter_account_id: None,
        labels: vec!["migrated".into()],
        components: vec![],
        epic_key: Some("MVP-LDN-1".into()),
        epic_name: Some("MVP LDN Swaps".into()),
        created: "2026-07-01T09:00:00.000+0000".into(),
        comments: vec![JiraComment {
            author_account_id: None,
            author_display: "Tester".into(),
            created: "2026-07-02T10:00:00.000+0000".into(),
            body: "First comment for history.".into(),
        }],
        attachments: vec![],
    };

    let payload = build_item(&issue, &channel, &IdentityMap::default(), &StatusMap::default());
    println!("== transform ==");
    println!("root_tags: {:?}", payload.root_tags);
    println!("history_reply present: {}", payload.history_reply.is_some());

    println!("== emit_item ==");
    let out = emitter.emit_item(&payload).await?;
    println!("item_id: {}  comments: {}", out.item_id, out.comments);
    assert!(!out.item_id.is_empty(), "empty item id");

    println!("== find_existing (dedup /query, retry for index lag) ==");
    let mut found = None;
    for i in 0..30 {
        found = emitter.find_existing("TEST-1").await?;
        if found.is_some() {
            println!("(found after ~{}s — tag-index lag)", i);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    println!("found: {found:?}");
    assert!(found.is_some(), "dedup query never returned the item within 30s");

    println!("== get_item tags ==");
    let ev = emitter.get_item("TEST-1").await?.expect("item not found");
    let tags = ev.get("tags").and_then(|t| t.as_array()).cloned().unwrap_or_default();
    let has = |p: &str| {
        tags.iter().filter_map(|t| t.as_array()).any(|a| {
            a.first().and_then(|k| k.as_str()) == Some("t")
                && a.get(1).and_then(|v| v.as_str()).map(|v| v.starts_with(p)) == Some(true)
        })
    };
    for p in ["jira:", "type:", "region:", "epic:", "label:"] {
        println!("has t {p:<8} {}", has(p));
        assert!(has(p), "missing tag prefix {p}");
    }

    println!("== emit_attachments (Blossom PUT /upload) ==");
    let n = emitter
        .emit_attachments(
            &out.item_id,
            &payload.root_tags,
            &[("test.txt".into(), "text/plain".into(), b"hello blob".to_vec())],
        )
        .await?;
    println!("attachments uploaded: {n}");
    assert_eq!(n, 1);

    println!("== seed_status (workflow trigger kind 46020) ==");
    let wf = std::env::var("WORKFLOW_ID").unwrap_or_else(|_| "11111111-2222-3333-4444-555555555555".into());
    emitter.seed_status(&out.item_id, &wf).await?;
    println!("trigger emitted for workflow {wf}");

    println!("\nALL LIVE CHECKS PASSED");
    Ok(())
}
