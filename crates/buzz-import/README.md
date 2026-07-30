# buzz-import

One-time importer that migrates the **open** Jira backlog into Buzz for a clean cutover.

Scope: open items only, one-way, idempotent, resumable. Closed history stays in Jira. Sprints and fixVersions are not carried over — Buzz starts a fresh sprint and version. Full design: `05-RESOURCES/buzz-importer-spec.md` in the workhub.

## Pipeline

```
jira (extract) -> transform -> emit (load) -> verify
```

- **jira** — pulls the open issue set (selection JQL), plus comments and attachments. Current status only.
- **transform** — deterministic Jira issue -> Buzz event payloads. Tags: `jira:<KEY>`, `type:`, `epic:`, `region:`, `orig-created:`, `label:`, plus a `p` assignee tag and the NIP-29 `h` channel tag.
- **emit** — signs with the single `buzz-import` key, emits the root + history reply via the HTTP bridge (`POST /events`). Dedup via a relay REQ (`{ "kinds":[...], "#t":["jira:<KEY>"] }` — `kinds` is mandatory or the query 403s). Attachments are uploaded to Blossom (`PUT /upload`, BUD-02 kind-24242 auth) and referenced from a threaded reply.
- **verify** — reconciles Jira selection against the ledger and the relay.

Idempotency + resume via an append-only `import-ledger.jsonl` (dedup on the `jira:<KEY>` tag).

## Design invariants

- **Single import key.** All migrated events are signed by one `buzz-import` identity; ownership is carried by the `p` tag, not the signer. Native provenance begins at each item's first real transition after import.
- **`now`-stamped events.** The relay rejects `created_at` beyond +/-15 min, so backdating is impossible; original dates live in the `orig-created:` tag and the body.
- **One-way.** No back-sync to Jira (that would reintroduce a split source of truth).

## Usage

```bash
buzz-import --config buzz-import.json dry-run   # mandatory first pass
buzz-import --config buzz-import.json run
buzz-import --config buzz-import.json verify
```

`JIRA_BASE_URL`, `JIRA_EMAIL`, `JIRA_API_TOKEN` (Jira Basic auth) and `BUZZ_PRIVATE_KEY` (the import key) are read from the environment.

## Status

The full pipeline is implemented: `jira` (extract), `transform`, `emit` (root + history reply + Blossom attachment upload + dedup), and `verify`. Status seeding (a workflow trigger) is the remaining subsystem and is not yet wired. `#![allow(dead_code)]` is still set at the crate root and comes out once that lands.
