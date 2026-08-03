# PIDX TODO

## Bugs

### 1. Status and mutation layers disagree on observation state
- `pidx_status` shows `proposed: 1` but `pidx_confirm` and `pidx_reject` return "observation is Rejected, not Proposed"
- Re-ingested observations land on same indices as old rejected shells — old state shadows new
- Found: 2026-07-30, unable to confirm/reject any re-ingested extra.* observations

### 2. No endpoint to list proposed observations
- `pidx_status` shows counts (proposed: N) per field but not their values
- `pidx_show` only shows confirmed observations
- No way to list what's proposed without reading bridge files from disk
- Needed for both agent and user: "what am I being asked to confirm?"
- Suggested: `pidx_proposed_list` returning field path, value, source, confidence per item

## Feature Requests

### Observation list endpoint (see bug #4 above)
Ideal shape: `pidx_proposed_list(user_id)` → list of `{path, index, value, source, confidence}`
For legibility and confirmation workflow.

## Resolved

### Extra-bucket review paths and bulk mutations (2026-07-31)
- `confirm_all` / `reject_all` now enumerate `extra.*` fields for both `extra` and `extra.` prefixes.
- Shared path resolution parses extra field slots from the right, so keys containing dots and hyphens resolve correctly.
- CLI, MCP, and both Tauri UIs now use the shared resolver; Tauri ingest results surface `extra_fields`.

### Provenance-aware `adap seal`
Currently seals ALL NDJSON packets regardless of whether the agent was in the room.
Need a way to scope seal to direct sessions only, or tag packets with provenance
so downstream ingestion can filter.

**Resolved 2026-08-03 — constraint gate.** `constraints.py` added to ada-packets:
seal quarantines sessions labeled for another agent (`cross-agent-delegation`,
`known-agent-label`, `session-id-form`, `tally-register-enum` are errors; legacy
`tally-` naming and field-hygiene hazards are warnings). Validate mode reports
cross-agent presence as drift (the store is a shared record by design — naomi/elia
profiles still write through the old ada-mcp into `~/.config/ada/packets`).
Delegation = re-seal with `--target <agent>`; `gated-reader` dev session relabeled
to `2026-07-29-ada`. Live store: `adap validate` exit 0 (warnings only).

### Observation list endpoint (see bug #2 above)
Ideal shape: `pidx_proposed_list(user_id)` → list of `{path, index, value, source, confidence}`
For legibility and confirmation workflow.

**Status 2026-08-03:** bug #2 confirmed live — `working.mode` (×2) and
`identity.core.17` (×1) have proposed observations with no read path. This is the
read-side of the quarantine loop (review what's blocked/pending). Next build.
