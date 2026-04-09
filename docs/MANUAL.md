# PIDX — CLI Manual

**Binary:** `pidx`  
**Format flag:** `--format json|human` (global, default: `human`)  
**Environment:** `PIDX_PROFILES_DIR` — override the profiles directory (default: `./profiles`)  
**Logging:** `RUST_LOG=info|debug|warn` — structured log output to stderr; stdout stays clean for agents

---

## Overview

PIDX is a personality indexer. It stores structured, confidence-weighted observations
about a user and renders them as tiered context blocks for LLM injection. All values
in a profile are `Observation` objects — never raw strings — carrying full provenance,
confidence, and decay state.

Agents use `--format json` throughout. Every write command emits `{ "ok": true, ... }`
on success or `{ "ok": false, "error": "..." }` on failure, always to stdout.
Human-readable output goes to stderr so it doesn't interfere with pipes.

---

## Commands

### `show` — Render a profile for LLM injection

```
pidx show <user_id> [--tier nano|micro|standard|rich]
```

Outputs a tier-scaled markdown context block containing only **confirmed** observations.
`delta`, `proposed`, `rejected`, and `archived` observations are invisible to output.

| Tier | ~Tokens | Content |
|------|---------|---------|
| `nano` | 180 | `identity.core` top 3 |
| `micro` | 550 | + register scores, `working.mode`, `working.feedback` |
| `standard` | 1,400 | + `domains`, `values`, `identity.reasoning`, full `working` |
| `rich` | 3,200 | + `signals`, pinned annotations, unresolved conflicts |

```bash
pidx show usr_default --tier standard
pidx show usr_default --tier nano --format json
```

**JSON response fields:** `tier`, `version`, `overall_confidence`, `core`, `register`,
`domains`, `values`, `reasoning`, `working`, `signals`, `annotations`, `unresolved_conflicts`

---

### `status` — Observation summary

```
pidx status <user_id>
```

Lists every field that has at least one observation, with per-field confirmed/proposed/delta
counts and a value preview. Shows queue lengths at the bottom.

```bash
pidx status usr_default
pidx status usr_default --format json
```

**JSON response fields:** `user_id`, `version`, `overall_confidence`, `updated`,
`fields[]`, `totals`, `delta_queue_open`, `review_queue_pending`, `bridge_log_processed`

---

### `ingest` — Process a bridge packet

```
pidx ingest <user_id> <path/to/file.bridge.json>
```

Reads a `.bridge.json` bridge packet and proposes its observations into the profile.
Observations are **never auto-confirmed** — all land as `proposed` or `delta`.
Appends an audit entry to `bridge_log`.

```bash
pidx ingest usr_default ./session.bridge.json
pidx ingest usr_default ./session.bridge.json --format json
# → { "ok": true, "observations_proposed": 7, "overall_confidence": 0.0 }
```

**Bridge packet format:**
```json
{
  "bridge_version": "0.1",
  "orientation": "local:gemma3:4b",
  "session_ref": "abc123",
  "timestamp": "2026-04-07T06:00:00+00:00",
  "observations": [
    { "field": "working.mode", "value": "sketch-first", "origination": "passive" }
  ]
}
```

Valid `field` paths: `identity.core`, `identity.reasoning.style`, `identity.reasoning.pattern`,
`identity.reasoning.intake`, `domains`, `values`, `signals.phrases`, `signals.avoidances`,
`signals.rhythms`, `working.mode`, `working.pace`, `working.feedback`, `working.pattern`,
`register.evidence`

---

### `confirm` — Flip a proposed observation to confirmed

```
pidx confirm <user_id> <field.path> [<index>]
```

`field.path` uses dot-notation. `index` defaults to 0 (the first observation in the field).
Only observations in `proposed` status can be confirmed.

```bash
pidx confirm usr_default working.mode 0
pidx confirm usr_default identity.core.0
pidx --format json confirm usr_default working.mode 0
# → { "ok": true, "field": "working.mode", "index": 0, "value": "...", "new_status": "confirmed" }
```

---

### `reject` — Permanently reject a proposed observation

```
pidx reject <user_id> <field.path> [<index>]
```

Same syntax as `confirm`. Rejected observations stay in the profile permanently — a
pattern of proposal → rejection is itself a signal. Status moves to `rejected` (terminal).

```bash
pidx reject usr_default working.mode 0
```

---

### `annotate` — Add a permanent note to a field

```
pidx annotate <user_id> <field.path> "<note>" [--pinned]
```

Annotations never decay. Pinned annotations appear in `rich`-tier output.
`author` is always `"user"` from the CLI.

```bash
pidx annotate usr_default working.mode "prefers async collaboration" --pinned
pidx annotate usr_default identity.core.0 "self-identified in session 12"
```

**JSON response:** `{ "ok": true, "annotation": { "id", "field", "note", "pinned" } }`

---

### `delta list` — List unresolved conflicts

```
pidx delta list <user_id>
```

Shows all open `DeltaItem` conflicts — fields where two observations disagree and neither
is active. Fields in delta are inert: their value is excluded from `show` output entirely.

```bash
pidx delta list usr_default
pidx delta list usr_default --format json
# → { "user_id", "open_count", "deltas": [{ "id", "field", "a", "b" }] }
```

---

### `delta resolve` — Resolve a conflict

```
pidx delta resolve <user_id> <delta_id> --keep a|b
```

Keeps one observation (`confirmed`), rejects the other (`rejected`). The field exits
delta and its active value is restored.

```bash
pidx delta resolve usr_default d1a2b3c4 --keep a
```

**JSON response:** `{ "ok": true, "delta_id", "kept": "a", "field" }`

---

### `review list` — List decayed observations awaiting review

```
pidx review list <user_id>
```

Shows observations whose effective confidence dropped below the cleanup threshold
(`meta.cleanup_policy.threshold`, default 0.20). Each item shows the field, index,
current confidence, and when it was flagged.

```bash
pidx review list usr_default
pidx review list usr_default --format json
# → { "user_id", "pending_count", "items": [{ "id", "field", "index", "confidence", "flagged_at" }] }
```

---

### `review process` — Solidify or discard a decayed observation

```
pidx review process <user_id> <review_id> --action solidify|discard
```

- `solidify` → sets `decay_exempt: true`, resets weight to 1.0, keeps `confirmed`
- `discard` → moves to `archived` (terminal, never deleted)

```bash
pidx review process usr_default r5e6f7a8 --action solidify
pidx review process usr_default r5e6f7a8 --action discard
```

**JSON response:** `{ "ok": true, "review_id", "action", "field" }`

---

### `diff` — Compare two profiles

```
pidx diff <user_a> <user_b>
```

Compares confirmed observations side-by-side: identity core, working mode, values,
and register metrics (only dimensions with >0.1 difference are shown).

```bash
pidx diff usr_alice usr_bob
pidx diff usr_alice usr_bob --format json
# → { "user_a", "user_b", "confidence_a", "confidence_b", "diffs": [...] }
```

---

### `list-users` — Enumerate all profiles

```
pidx list-users
```

Scans `PIDX_PROFILES_DIR` for `*.pidx.json` files and returns a sorted summary.
Useful for agents that don't know user IDs in advance.

```bash
pidx list-users
pidx list-users --format json
# → { "count": 2, "users": [{ "user_id", "version", "updated", "overall_confidence" }] }
```

---

## Field Paths (dot-notation)

Used by `confirm`, `reject`, `annotate`, `delta resolve`.

| Path | Type | Notes |
|------|------|-------|
| `identity.core.<n>` | list slot | `n` = index into core traits array |
| `identity.reasoning.style` | singleton | |
| `identity.reasoning.pattern` | singleton | |
| `identity.reasoning.intake` | singleton | |
| `domains.<n>` | list slot | |
| `values.<n>` | list slot | |
| `signals.phrases.<n>` | list slot | |
| `signals.avoidances.<n>` | list slot | |
| `signals.rhythms.<n>` | list slot | |
| `working.mode` | singleton | |
| `working.pace` | singleton | |
| `working.feedback` | singleton | |
| `working.pattern` | singleton | |

**Singleton fields** share one `ObservationField` — multiple conflicting observations
are possible and will enter delta. **List slot fields** each represent an independent
item (a distinct trait, phrase, domain); they never conflict with each other.

---

## Observation Status Lifecycle

```
proposed ──► confirmed ──► archived  (cleanup)
         ──► rejected               (permanent)
         ──► delta                  (conflict)

confirmed ──► delta    (incoming conflict)
```

- `proposed` — ingested or engine-observed, awaiting human review  
- `confirmed` — actively contributes to `show` output  
- `rejected` — permanent audit record, never shown, never deleted  
- `delta` — field is inert; two observations conflict and need resolution  
- `archived` — decayed below threshold and discarded via `review process --action discard`

---

## Confidence & Decay

Base confidence is determined by the origination × orientation matrix:

| Origination | Orientation | Base Confidence |
|-------------|-------------|-----------------|
| `user` | `user` | 1.00 *(decay-exempt)* |
| `active` | `claude.*` | 0.91 |
| `passive` | `claude.*` | 0.78 |
| `passive` | `local:*` | 0.61 |
| `sync` | `local:*` | 0.55 |
| `system` | `algorithmic` | 0.45 |

Effective confidence at read-time: `base × e^(−λ × days_since_observation)`  
Corroboration bonus: if ≥2 independent orientations confirm the same value → +0.08 each (capped at 1.0)

---

## Agent Usage Pattern

```bash
# 1. Ingest a local session packet
pidx ingest usr_default ./session.bridge.json --format json

# 2. Check what needs review
pidx status usr_default --format json | jq '.totals'

# 3. Confirm an observation by index
pidx --format json confirm usr_default working.mode 0

# 4. Pull context block for injection
pidx show usr_default --tier standard --format json | jq '.core'

# 5. List all users
pidx list-users --format json
```

All write commands return `{ "ok": true/false }` to stdout.  
All human-readable output (status tables, previews) goes to stderr.  
`RUST_LOG=info` enables structured span logging to stderr without affecting stdout.
