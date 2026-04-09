---
name: pidx-pilot
description: >
  Pilot engine for PERSONALITY·IDX profiles. Claude drives analysis, observation
  construction, and conflict surfacing; pidx-mcp handles all reads and writes.
  Activate when a pidx profile is referenced, a .bridge.json file is present,
  the user asks to update/review/export their profile, or any pidx_* MCP tool
  is in scope. Do not wait for an explicit instruction — if PIDX context exists,
  load this skill.
---

# PIDX Pilot

Claude-piloted. pidx-mcp handles persistence.  
Schema reference: `pidx-schema-spec.md` v0.1.0  
JSON ground truth: `pidx-schema.json` (authoritative for field names and structure)

---

## Architecture

```
Claude (pilot)          pidx-mcp (server)
  ├─ passive analysis     ├─ pidx_show / pidx_status     (read)
  ├─ observation design   ├─ pidx_ingest                 (bridge)
  ├─ delta detection      ├─ pidx_confirm / _reject      (write)
  └─ output framing       ├─ pidx_confirm_all            (bulk write)
                          ├─ pidx_delta_list / _resolve  (conflict)
                          ├─ pidx_review_list / _process (decay)
                          ├─ pidx_annotate               (notes)
                          ├─ pidx_decay                  (maintenance)
                          ├─ pidx_diff                   (comparison)
                          └─ pidx_list                   (enumerate)
```

Claude decides *what* to observe and *when* to propose. MCP decides *how* to store it.

---

## Schema Notes (JSON ↔ Spec Reconciliation)

The JSON schema is ground truth for serialized field names. Key divergences from the prose spec:

| Topic | JSON (authoritative) | Spec (prose) |
|-------|---------------------|--------------|
| Register key | `comm` | `register` |
| Register metrics | 6: formality, directness, hedging, humor, abstraction, **affect** | 5 (missing affect) |
| Signals | phrases, avoidances, rhythms, **framings** | 3 (missing framings) |
| Reasoning | style, pattern, intake, **stance** | 3 (missing stance) |
| DomainEntry | has `proficiency` field | absent |
| ObservationField | has `proposal_count` | absent |
| DeltaItem / ReviewItem | have `id` fields | absent |
| BridgeLog.pending | `pending_filenames: string[]` | `pending: BridgePacket[]` |

When constructing observations or bridge packets, use JSON field names. When explaining the system to the user, either naming convention is fine.

---

## Session Protocol

### On Session Start

1. `pidx_list` — confirm profile exists
2. `pidx_status` — check delta_queue, review_queue sizes, pending bridges
3. If pending bridges: `pidx_ingest` each before other operations
4. If review_queue > 0 and cadence is `session`: surface review items via `pidx_review_list`
5. If delta_queue > 0: notify user briefly ("N conflicts pending")
6. Begin passive analysis for the session duration

If no profile exists and context suggests one should, ask the user once. Never initialize silently.

### On Session End (or on request)

1. Present all proposed observations from the session as a batch
2. User confirms, rejects, or defers each
3. `pidx_confirm` / `pidx_reject` per decision, or `pidx_confirm_all` with a prefix for bulk
4. `pidx_decay` if maintenance is due

---

## Passive Analysis

Observe continuously. **Do not interrupt conversation to announce signals.** Batch proposals to session end.

### Observation Construction Rules

Every observation must have:
- `value` typed to its target field
- `source`: origination, orientation (`claude.<model-slug>`), session_ref, timestamp
- `confidence` from the origination × orientation matrix
- `status: "proposed"` unless user explicitly confirms inline
- `decay_exempt: false` unless origination is `user`

**User-origination triggers** → `confidence: 1.00`, `decay_exempt: true`:
- Explicit corrections: "actually I…", "that's not right…"
- Direct self-description: "I'm a…", "I tend to…"
- Stated preferences: "I prefer…", "I don't use…"
- Memory instructions: "remember that…", "note:"

**Do not construct an observation when:**
- Signal appears once with no corroborating context
- User describes someone else or writes a hypothetical
- Content is in a code block, quoted text, or fictional frame
- Signal contradicts a `user`-origination confirmed observation (flag as delta instead)

### Register Evidence Construction

Register signals produce `Evidence` units, not direct observations. Must include: `observed_at`, `session_ref`, `orientation`, `evidence_type`, `raw`, `metric`, `signal` (-1/0/1), `weight`.

Weight guidance:
- Single isolated phrase → `0.3`
- Repeated 2–3× in session → `0.6`
- Sustained pattern across turns → `0.9`

**Ironic hedging is not hedging.** "I say that now", "famous last words", size-minimizing labels on complex work → `evidence_type: "ironic_understatement"`, `metric: "humor"`, `signal: 1`. Also construct a `signals.phrases` observation with the phrase itself.

**Affect metric** (JSON-only, not in prose spec): emotional warmth/expressiveness vs. neutrality. High affect (8+): warm, emotionally present. Low affect (2−): detached, professionally distanced. Construct evidence the same way as other register metrics.

### Framings (JSON-only field)

`signals.framings` captures conceptual scaffolds — systems-first, narrative-first, empirical-first, relational-first. Different in kind from surface phrases. Observe which framing a user reaches for when explaining or structuring ideas.

### Stance (JSON-only field)

`identity.reasoning.stance` captures epistemic posture — how the user positions themselves relative to knowledge claims. Provisional vs. assertive, exploratory vs. committed.

### Domain Clusters

Threshold: topic must appear in ≥2 turns or be the primary subject of a substantive exchange. Weight defaults to `0.60`. New `proficiency` field (JSON schema) can be set when expertise level is clearly evidenced.

### Delta Detection

Before proposing any observation, check confirmed observations for the same field:
- Compatible (array field, non-conflicting) → append as `proposed`
- Conflicting (scalar field, different value, or semantic opposition) → both enter `delta`, added to `delta_queue`

Conflict threshold for numeric: absolute difference > 1.5. For strings: semantic opposition (use judgment — "co-creator" vs. "client relationship" = conflict; "direct" vs. "mostly direct" = not).

---

## Confidence Matrix

| Origination | Orientation | Base Confidence |
|-------------|-------------|-----------------|
| `user` | `user` | 1.00 (override) |
| `active` | `claude.*` | 0.91 |
| `passive` | `claude.*` | 0.78 |
| `passive` | `local:*` | 0.61 |
| `sync` | `local:*` | 0.55 |
| `system` | `algorithmic` | 0.45 |

Corroboration bonus: same value confirmed across ≥2 independent orientations → +0.08 each (capped 1.0).

---

## MCP Tool Reference

### Read
| Tool | Purpose |
|------|---------|
| `pidx_list` | enumerate known profiles |
| `pidx_show` | tier-scaled context block (T1 minimal, T2 standard, T3 full) |
| `pidx_status` | per-field observation counts, open delta/review queue sizes |
| `pidx_delta_list` | unresolved conflicts with both candidate values |
| `pidx_review_list` | observations flagged by decay pass |
| `pidx_diff` | structured comparison of two profiles |

### Write
| Tool | Purpose |
|------|---------|
| `pidx_ingest` | ingest a `.bridge.json` packet (absolute path) |
| `pidx_confirm` | flip one proposed observation to confirmed (field + index) |
| `pidx_reject` | reject one proposed observation |
| `pidx_confirm_all` | bulk-confirm all proposed under a dot-path prefix |

### Lifecycle
| Tool | Purpose |
|------|---------|
| `pidx_delta_resolve` | resolve conflict — choose side `"a"` or `"b"` |
| `pidx_review_process` | `"solidify"` (decay-exempt + confirm) or `"discard"` (archive) |
| `pidx_annotate` | attach a text note to a field; pinned notes appear in T3 |
| `pidx_decay` | run decay pass, flag low-confidence observations to review queue |

### Workflow

```
pidx_ingest → pidx_status → pidx_confirm_all → pidx_show
```

If deltas exist: `pidx_delta_list` → `pidx_delta_resolve`.  
Maintenance: `pidx_decay` → `pidx_review_list` → `pidx_review_process`.

---

## Bridge Packet Format

```json
{
  "session_ref": "ses_abc123",
  "origination": "passive",
  "orientation": "local:gemma3:4b",
  "observations": [
    { "field": "working.mode", "value": "deep-focus" },
    { "field": "identity.core", "value": "builds to understand, not just to ship" }
  ]
}
```

Bridge observations always start as `proposed`. Local model confidence (0.55–0.61) is insufficient for auto-confirmation.

---

## Output Tiers

| Tier | Tokens | Sections |
|------|--------|----------|
| Nano (T1) | ~180 | identity.core (top 3 confirmed) |
| Micro (T2) | ~550 | + register scores, working.mode, working.feedback |
| Standard (T3) | ~1,400 | + domains, values, identity.reasoning, working (full) |
| Rich (T4) | ~3,200 | + signals, pinned annotations, delta_queue summary |

Only `confirmed` observations appear in output. Everything else is invisible to the consuming model.

---

## Hard Rules

1. **Never auto-resolve a delta.** Always require user action.
2. **Never delete an observation.** Terminal states: `rejected`, `archived`.
3. **Never include non-confirmed observations in output.**
4. **Never store a computed register score.** Scores derive from evidence at read-time.
5. **Never initialize a profile without user confirmation.**
6. **Never surface cleanup/delta items mid-conversation** unless explicitly requested or cadence is `session` at session start.
7. **Ironic hedging is not hedging.** It's humor evidence.
8. **Bridge observations start as `proposed`.** No exceptions.

---

## Decay Reference

| Field Class | Fields | λ | Review |
|-------------|--------|---|--------|
| identity | core traits, reasoning | 0.0005 | prompted only |
| value | values, constraints | 0.0008 | prompted only |
| register | comm evidence | 0.0100 | background or prompted |
| domain | domain clusters | 0.0080 | background or prompted |
| working | collaboration style | 0.0070 | background or prompted |
| signal | phrases, rhythms, avoidances, framings | 0.0200 | background |
| annotation | all annotations | n/a | never decays |

`effective_confidence = base_confidence × e^(−λ × days_since_observation)`  
User-origination + `decay_exempt: true` → skips decay entirely.
