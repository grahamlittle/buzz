#![allow(dead_code)]

//! One-time Jira -> Buzz backlog importer.
//!
//! Scope: OPEN items only, one-way, idempotent, resumable. Closed history stays
//! in Jira. Sprints and fixVersions are not carried over; Buzz starts a fresh
//! sprint and version. See `05-RESOURCES/buzz-importer-spec.md` in the workhub
//! for the full design.
//!
//! Pipeline: [`jira`] extract -> [`transform`] -> [`emit`] load -> [`verify`].
//! All migrated events are signed by a single `buzz-import` key; ownership is
//! carried by a `p` tag, not the signer. Every emitted event is `now`-stamped
//! (the relay rejects `created_at` beyond +/-15 min).

pub mod config;
pub mod emit;
pub mod error;
pub mod identity;
pub mod jira;
pub mod ledger;
pub mod status_map;
pub mod transform;
pub mod verify;

pub use config::Config;
pub use error::{ExitCode, ImportError};
