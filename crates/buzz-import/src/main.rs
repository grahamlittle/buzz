//! `buzz-import` — one-time Jira -> Buzz backlog importer (open items, cutover).

use std::path::PathBuf;
use std::process::ExitCode as ProcExit;

use clap::{Parser, Subcommand};

use buzz_import::config::Config;
use buzz_import::emit::Emitter;
use buzz_import::identity::IdentityMap;
use buzz_import::jira::JiraClient;
use buzz_import::ledger::Ledger;
use buzz_import::status_map::StatusMap;
use buzz_import::transform::build_item;
use buzz_import::{verify, ImportError};

#[derive(Parser)]
#[command(
    name = "buzz-import",
    about = "Migrate the open Jira backlog into Buzz"
)]
struct Cli {
    /// Path to the importer config (JSON).
    #[arg(long, env = "BUZZ_IMPORT_CONFIG", default_value = "buzz-import.json")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compute and log all payloads without emitting (mandatory first pass).
    DryRun,
    /// Run the migration for real.
    Run,
    /// Verify a completed run against Jira and the relay.
    Verify,
}

#[tokio::main]
async fn main() -> ProcExit {
    let _ = rustls::crypto::ring::default_provider().install_default();
    match run().await {
        Ok(()) => ProcExit::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ProcExit::from(e.exit_code() as u8)
        }
    }
}

async fn run() -> buzz_import::error::Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load(&cli.config)?;

    match cli.command {
        Command::DryRun => config.dry_run = true,
        Command::Run => config.dry_run = false,
        Command::Verify => {
            let ledger = Ledger::open(&config.ledger)?;
            let report = verify::run(&ledger, 0).await?;
            println!("{report:?}");
            return Ok(());
        }
    }

    let identity = IdentityMap::load(&config.identity_map)?;
    let status = StatusMap::load(&config.status_map)?;
    let ledger = Ledger::open(&config.ledger)?;
    let emitter = Emitter::new(&config)?;
    let jira = JiraClient::new(jira_base_url()?);

    for product in &config.products {
        let issues = jira
            .fetch_open_issues(&product.jira, &config.selection_jql)
            .await?;
        for issue in &issues {
            if emitter.find_existing(&issue.key).await?.is_some() {
                continue;
            }
            let payload = build_item(issue, &product.channel, &identity, &status);
            let outcome = emitter.emit_item(&payload).await?;
            ledger.append(&buzz_import::ledger::Entry {
                jira: issue.key.clone(),
                buzz_item: Some(outcome.item_id),
                stage: buzz_import::ledger::Stage::Done,
                state: outcome.seeded_state,
                comments: outcome.comments,
                attachments: outcome.attachments,
            })?;
        }
    }

    Ok(())
}

fn jira_base_url() -> buzz_import::error::Result<String> {
    std::env::var("JIRA_BASE_URL").map_err(|_| ImportError::Input("JIRA_BASE_URL not set".into()))
}
