# PIDX — Agent Skill

**Schema version:** `0.1.0`  (field definitions live in [`../docs/pidx-schema-spec.md`](../docs/pidx-schema-spec.md))

## What PIDX is

PIDX stores structured personality profiles built from timestamped observations. Each observation has a type, confidence score, origination source, and decays over time. Profiles are built incrementally across sessions — you don't need a complete picture on the first pass.

## When to read vs ingest

- **Read first** (`pidx_show`, `pidx_status`) — check what's already confirmed before proposing observations. Avoid re-proposing values that are already confirmed.
- **Ingest** when you have new session evidence to add. Drop a `.bridge.json` packet via `pidx_ingest` or the mailbox watcher.
- **Confirm** proposed observations (`pidx_confirm`, `pidx_confirm_all`) only when you have enough context to vouch for them. Bulk-confirm under a prefix after a dense session.

## MCP tool reference

**Read:**
- `pidx_list` — list all known profiles
- `pidx_show` — fetch a tier-scaled context block (`nano` ~180t / `micro` ~550t / `standard` ~1400t / `rich` ~3200t)
- `pidx_status` — per-field observation counts + open delta/review queue sizes
- `pidx_delta_list` — list unresolved delta conflicts with both candidate values
- `pidx_review_list` — list observations flagged by the decay pass
- `pidx_diff` — structured comparison of two profiles

**Write:**
- `pidx_ingest` — ingest a `.bridge.json` packet (absolute path required)
- `pidx_confirm` — flip one proposed observation to confirmed (field + zero-based index)
- `pidx_reject` — reject one proposed observation
- `pidx_confirm_all` — bulk-confirm all proposed obs under a dot-path prefix

**Lifecycle:**
- `pidx_delta_resolve` — resolve a conflict by choosing side `"a"` or `"b"`
- `pidx_review_process` — `"solidify"` (decay-exempt + confirm) or `"discard"` (archive)
- `pidx_annotate` — attach a text note to a field; pinned notes appear in `rich` output
- `pidx_decay` — run a decay pass; flags low-confidence obs to the review queue

## Bridge packet format (minimal example)

```json
{
  "session_ref": "ses_abc123",
  "origination": "passive",
  "orientation": "claude.sonnet-4-6",
  "observations": [
    { "field": "working.mode", "value": "deep-focus" },
    { "field": "identity.core", "value": "builds to understand, not just to ship" }
  ]
}
```

Field definitions and valid paths are in `docs/pidx-schema-spec.md` v0.1.0. Do not restate them here — bump the version reference above when the schema changes.

## Workflow

```
pidx_ingest → pidx_status → pidx_confirm_all → pidx_show
```

If `pidx_status` shows open deltas: `pidx_delta_list` → `pidx_delta_resolve`.
Periodic maintenance: `pidx_decay` → `pidx_review_list` → `pidx_review_process`.
