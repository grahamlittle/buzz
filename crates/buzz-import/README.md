# buzz-import

One-time importer that migrates the **open** Jira backlog into Buzz for a clean cutover.

Scope: open items only, one-way, idempotent, resumable. Closed history stays in Jira. Sprints and fixVersions are not carried over — Buzz starts a fresh sprint and version. Full design: `05-RESOURCES/buzz-importer-spec.md` in the workhub.

## Pipeline

```
jira (extract) -> transform -> emit (load) -> verify
```

- **jira** — pulls the open issue set (selection JQL), plus comments and attachments. Current status only.
- **transform** — deterministic Jira issue -> Buzz event payloads. Tags: `jira:<KEY>`, `type:`, `epic:`, `region:`, `orig-created:`, `label:`, plus a `p` assignee tag and the NIP-29 `h` channel tag.
- **emit** — builds events with `buzz-sdk`, signs with the single `buzz-import` key, emits via the HTTP bridge (`POST /events`) or `buzz-ws-client`. Dedup via a relay REQ (`{ "kinds":[...], "#t":["jira:<KEY>"] }` — `kinds` is mandatory or the query 403s).
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

`JIRA_BASE_URL` and `BUZZ_PRIVATE_KEY` (the import key) are read from the environment.

## Status

Scaffold. Module structure, config, error/exit-code contract, and the ledger are in place; the `jira`, `emit`, and `verify` stages are stubbed (`not yet implemented`). `#![allow(dead_code)]` is set at the crate root and comes out as the stages are filled.
