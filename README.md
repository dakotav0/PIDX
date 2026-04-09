# PIDX — Personality Indexer

> A structured personality profiling engine built to give AI systems persistent, decay-aware context about the people they work with.

PIDX stores observations about a person's working style, communication register, reasoning patterns, and core identity — organized into typed fields with provenance, confidence scores, and exponential decay. It's not a dashboard. It's a library and CLI first, with an MCP server so any AI client can read and update profiles directly.

The design goal: a profile an AI can actually use, not just a list of adjectives.

## Install

```bash
git clone https://github.com/dakotav0/pidx
cd pidx
cargo install --path .
```

## Quick start

```bash
# 1. Ingest a bridge packet from an AI session
pidx ingest dakota ./session.bridge.json

# 2. See what came in
pidx status dakota

# 3. Bulk-confirm observations under a prefix
pidx confirm-all dakota identity

# 4. Render a context block
pidx show dakota --tier standard

# 5. Watch a mailbox directory for new packets (drop-and-ingest)
pidx watch dakota
```

## MCP setup

Build and register the MCP server so AI clients can read and update profiles via tool calls.

```bash
cargo build --release -p pidx-mcp
```

Add to your MCP client config (e.g. `.vscode/mcp.json`):

```json
{
  "servers": {
    "pidx": {
      "type": "stdio",
      "command": "/path/to/target/release/pidx-mcp"
    }
  }
}
```

The server exposes 14 tools grouped as read, write, and lifecycle. See [SKILL.md](pidx-mcp/pidx-pilot/SKILL.md) for the agent-facing workflow guide.

## Project layout

```
pidx/               — core library + CLI binary
  src/
    models/         — profile schema (observation, evidence, decay, profile)
    ingestion.rs    — bridge packet routing + corroboration + decay
    output.rs       — tier-scaled rendering (T1/T2/T3)
    storage.rs      — file-per-user JSON persistence
    main.rs         — CLI (clap derive)
pidx-mcp/           — stdio MCP server (rust-mcp-sdk)
pidx-ui/            — Tauri desktop UI (experimental, not in v0)
docs/               — schema spec + manual
profiles/           — example profiles (live profiles excluded via .gitignore)
```

## Status

**v0** — CLI and MCP server are stable. Schema `v0.1.0`. Cargo.lock committed.
Tauri UI is experimental and excluded from v0.
