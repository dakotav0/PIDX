# PLAN — Unknown-Field Bucket + Loud Drop

**Status:** core implementation complete in the working tree (2026-07-31). The extra bucket, routing, status/show output, and CLI/MCP/Tauri ingest reporting are implemented. Remaining follow-up: skipped-value counts, v0.2 timestamp/target-profile regression coverage, and the later promotion command.

## Problem

`route_field()` in `src/ingestion.rs` (~line 93) is a closed whitelist:
`identity.*`, `domains`, `values`, `signals.{phrases,avoidances,rhythms,framings}`,
`working.{mode,pace,feedback,pattern}`, `register.evidence`.

Anything else returns `FieldRoute::Unknown` and `ingest_one_observation()` returns
**silently** — no log, no count, no trace. But the `pidx-pilot` skill (the protocol
every ensemble agent writes by) teaches `relational`, `moment`, `wellbeing`,
`creative`, `pattern`, `preference` as standard observation fields.

**Evidence:** ada's May-25 packet (5 observations, all `pattern`/`moment`) ingested
with `proposed: 0`. Months of ensemble observations discarded without a word.
The skill taught a vocabulary the engine doesn't speak; the failure mode was silence.

## Design

### 1. Catch-all bucket

Add to `ProfileDocument` (src/models/profile.rs):

```rust
/// Observations whose field path has no dedicated slot. Keyed by the raw
/// bridge-packet field name (e.g. "moment", "relational", "pattern").
/// Lossless holding pen — promote a key to a first-class field when it
/// earns one; migration is then a drain of that key.
#[serde(default)]
pub extra: std::collections::BTreeMap<String, Vec<ObservationField>>,
```

`BTreeMap` for stable serialization order. `#[serde(default)]` keeps every
existing `.pidx.json` loading unchanged (backward compatible, no migration).

### 2. Routing change

In `route_field()`, replace the `_ => FieldRoute::Unknown` arm:

```rust
other => {
    let list = profile.extra.entry(other.to_string()).or_default();
    let match_res = find_matching_field(list, incoming);
    let (idx, has_proposed) = match_res.unwrap_or_else(|| {
        list.push(ObservationField::default());
        (list.len() - 1, false)
    });
    if has_proposed {
        FieldRoute::DedupField(&mut list[idx])
    } else {
        FieldRoute::Field(&mut list[idx], FieldClass::Signal) // or new FieldClass::Extra
    }
}
```

Same dedup shape as `values`/`signals.*`. Borrow-checker note: the `entry()`
mutable borrow must not overlap other profile borrows in the match — it won't,
since each arm is exclusive, but the `FieldRoute<'a>` lifetime already handles this.

Decide: reuse `FieldClass::Signal` (cheap) vs add `FieldClass::Extra` (honest —
lets calibration assign its own decay rate/confidence weight later). Lean: `Extra`.

### 3. Loud drop → loud route

Nothing is dropped anymore, but announce the reroute so producers learn:

- In `cmd_watch` / mailbox scan / `pidx_ingest` result JSON: add
  `"extra_fields": ["moment", "pattern"]` listing bucket keys touched this packet.
- Human format: `  note: 2 observation(s) routed to extra bucket: moment, pattern`.
- Only truly unparseable values (arrays/bools/null in `parse_value`) remain
  skipped — count those too: `"skipped_values": n`. Silent discard ends entirely.

### 4. Surface in output tiers

- `status`: bucket keys appear as `extra.<key>` field paths (existing per-field
  count shape works as-is once they're real `ObservationField`s).
- `show` rich tier: append an `### EXTRA` section listing `key: text` for active
  observations. Standard/micro/nano tiers: omit (bucket is a holding pen, not
  canon). `confirm`/`reject`/`confirm-all` must resolve `extra.<key>` paths —
  extend the path parser in `main.rs` (~line 492 region).

### 5. Confidence math

`overall_confidence` currently averages known fields. Decide whether `extra`
participates. Lean: **exclude** from the overall score (unvetted holding pen)
but show per-observation confidence normally. One-line filter in the meta
recompute.

### 6. Tests (do these; the suite has zero coverage of ingestion routing today)

- `ingest_bridge_packet` with `field: "moment"` → lands in `extra["moment"]`, proposed=1.
- Duplicate value into same key → dedup (proposal_count increments, no new slot).
- Replay ada's real packet shape (5 obs, `pattern`/`moment`) → proposed=5.
- Old profile JSON without `extra` key → loads fine.
- v0.2 `observations_proposed` map with unknown keys → same bucket behavior.
- While here: unit tests for `effective_timestamp()` both packet shapes, and
  `target_profile` routing fallback (the 2026-07-20 fixes are only covered by
  ad-hoc evidence).

### 7. Promotion path (later, cheap)

`pidx promote <user> extra.<key> <canonical-path>` — drain a bucket key into a
first-class field once it earns one. Not needed for v1; the BTreeMap makes it a
mechanical drain-and-reingest.

## Touch list

| File | Change |
|---|---|
| `src/models/profile.rs` | `extra` map on `ProfileDocument`; maybe `FieldClass::Extra` |
| `src/ingestion.rs` | catch-all arm in `route_field`; touched-keys tracking |
| `src/main.rs` | path parser for `extra.<key>`; watch/ingest result fields |
| `src/output.rs` | rich-tier EXTRA section; status rows |
| `pidx-mcp/src/tools.rs` | mirror result fields in MCP ingest/scan outputs |
| `~/agents/skills/pidx-pilot/SKILL.md` | after landing: note that non-canonical fields route to `extra` (not dropped) |

## Non-goals

- No auto-promotion, no semantic merging of bucket keys.
- No delta detection inside `extra` v1 (append-only; conflicts are a promotion-time concern).
