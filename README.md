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

## Human vs Machine Output

The `pidx show` command is optimized for both human visual inspection and automated script pipelines:

* **Interactive Terminals (`--format human`, Default)**:
  Renders a beautiful, ANSI-colored, and boxed character card directly to `stderr`. The card automatically truncates long observations to clean terminal borders (width 76), and standard output is suppressed to prevent double-rendering in the terminal.
* **Piped/Redirected Output (`--format human`)**:
  If you pipe the output (e.g. `pidx show dakota | pbcopy` or `pidx show dakota > prompt.txt`), standard output is no longer a terminal. The CLI detects this and automatically outputs the **raw, unformatted text** directly to `stdout`, keeping prompt injection and tooling pipelines fully backward-compatible.
* **Machine Pipelines (`--format json` or `--format adapter`)**:
  Optimized for programmatic parsing by tools, downstream agents, or Tauri endpoints.

---

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

The server exposes 22 tools grouped as read, write, and lifecycle. See [pidx-agent-guide.md](pidx-mcp/pidx-agent-guide.md) for the agent-facing workflow guide.

## Project layout

```
pidx/               — core library + CLI binary
  src/
    models/         — profile schema (observation, evidence, decay, profile)
    ingestion.rs    — bridge packet routing + corroboration + decay
    output.rs       — tier-scaled rendering (nano/micro/standard/rich)
    storage.rs      — file-per-user JSON persistence
    main.rs         — CLI (clap derive, Human boxed display + TTY detection)
pidx-mcp/           — stdio MCP server (rust-mcp-sdk)
pidx-ui/            — Tauri desktop UI (pre-release build — SvelteKit 5 + Tailwind 4)
docs/               — schema spec + manual
profiles/           — example profiles (live profiles excluded via .gitignore)
```

## Status

**v0.3** — CLI and MCP server are stable. Schema `v0.3.0`. Cargo.lock committed.
The Tauri UI is a pre-release build — it works against the same profiles and is under active development.

## Bundled with mRAG

PIDX ships alongside [mRAG](https://github.com/dakotav0/mrag), a local-first associative memory engine with configurable decay and affect routing. Together:

| Layer | Tool | Role |
|---|---|---|
| **Identity** | PIDX | Structured observations, confidence tracking, profile diffing |
| **Memory** | [mRAG](https://github.com/dakotav0/mrag) | Associative retrieval, decay, affect routing |
| **Bridge** | `.bridge.json` | Cross-agent session packets |

See [on_agents.md](on_agents.md) for the design statement that ships with this bundle.

