# PERSONALITY·IDX — Schema Specification
**Version:** 0.3.0  
**Status:** Current  
**Engine:** Claude-piloted. Not designed for local model execution.  
**Source of truth:** `src/models/*.rs` + `docs/pidx-schema.json` (generated via `cargo run --example emit_schema -- --write`)

---

## Design Axioms

1. **Every value is an Observation.** No raw strings or floats exist at the top level. Every stored value carries full provenance.
2. **Scores are always computed, never stored.** Register metrics are derived at read-time from their evidence pool. A stored score is always stale.
3. **Conflicts park, never merge.** When two orientations disagree, both survive and the field enters `delta` status. Nothing is auto-resolved.
4. **Rejected observations are permanent record.** A pattern of proposal → rejection is itself a signal. Nothing is deleted.
5. **User origination is the only ceiling.** A user annotation overrides engine confidence and is decay-exempt by default.
6. **Bridge trust is file-based.** No signing or verification required. The assumption is explicit.

---

## Core Types

### `ObservationValue`

The concrete value type stored inside every `Observation`. In the Python reference implementation this was a generic `Observation[T]`; in the Rust engine it is a sum-type enum so that values of different shapes can coexist in the same `Vec`.

```typescript
// Serialized with serde(untagged) — no wrapper key in JSON.
type ObservationValue =
  | string                          // Text(String)     → "hello"
  | DomainEntry                     // Domain(DomainEntry) → {"label":"...", "weight":0.6}
  | number;                         // Number(f64)      → 7.4
```

**Why not `Observation<T>`?** Rust generics are monomorphized — `Observation<String>` and `Observation<DomainEntry>` are incompatible types with no common base, so they cannot share a `Vec`. The enum is the idiomatic Rust equivalent and round-trips to identical JSON via `#[serde(untagged)]`.

---

### `Observation`

The atomic unit of every profile value.

```typescript
interface Observation {
  value:        ObservationValue;
  source:       ObservationSource;
  confidence:   number;           // 0.0–1.0, base from origination × orientation matrix
  weight:       number;           // 0.0–1.0, field-class decay modifier (default 1.0)
  status:       ObservationStatus;
  revision:     number;           // uint32, incremented on each in-place update
  decay_exempt: boolean;          // always true for user-originated identity/value fields
}

interface ObservationSource {
  origination: Origination;       // see enum below
  orientation: string;            // "claude.sonnet-4-6" | "local:gemma3:4b" | "algorithmic" | "user"
  session_ref: string;            // SHA-derived session identifier
  timestamp:   string;            // ISO 8601 (no timezone suffix — matches Python's utcnow().isoformat())
}

type ObservationStatus = "proposed" | "confirmed" | "rejected" | "delta" | "archived";

type Origination = "user" | "active" | "passive" | "sync" | "system";
```

**Status lifecycle:**
```
proposed ──► confirmed
         ──► rejected  (permanent, never deleted)
         ──► delta     (conflict parked for user resolution)

confirmed ──► delta    (if a later observation conflicts)
          ──► archived (if decay threshold crossed and cleanup runs)

archived  ──► (terminal, audit-only)
```

---

### `ObservationField`

A keyed collection of observations for a single profile field. Resolves to an active value on read.

```typescript
interface ObservationField {
  observations:   Observation[];
  proposal_count: number;         // uint32, how many times any value was proposed for this field
                                  // incremented instead of creating duplicate slots on re-proposal
                                  // starts at 1 (the first proposal); shown as "(×N)" in status output
}
```

**`proposal_count` note:** This replaces the implicit "count via array length" approach. When the same value arrives again in a subsequent bridge packet, the engine increments `proposal_count` on the existing slot rather than duplicating it. High repetition is a confidence signal.

**Resolution rule (active value):**
```
active_value = argmax over confirmed observations of:
  effective_confidence(obs) = obs.confidence × decay(obs.source.timestamp, field_class.λ)
```

`delta` entries never contribute to `active()`. If all observations are in `delta`, the field's active value is `null`.

---

### `DomainEntry`

The value type for `domains` observations.

```typescript
interface DomainEntry {
  label:       string;            // required
  weight:      number;            // default 0.60; relative weight in the overall cluster (0.0–1.0)
  proficiency: string | null;     // optional tier: "beginner" | "intermediate" | "expert" | "architect"
                                  // omitted from JSON entirely when absent — old profiles are safe
}
```

---

### `Evidence`

The raw substrate of Register metric scores. Score is computed from evidence, not stored.

Evidence is **additive** — unlike `Observation`, evidence items are never in conflict. When a new `BridgePacket` arrives, its register evidence is appended to the existing pool; no delta detection runs on it.

```typescript
interface Evidence {
  observed_at:   string;          // ISO 8601 (no timezone suffix)
  session_ref:   string;
  orientation:   string;          // which model or system produced this evidence item
  evidence_type: EvidenceType;
  raw:           string;          // verbatim phrase or pattern that was observed
  metric:        RegisterMetricName;
  signal:        -1 | 0 | 1;     // i8 — directional contribution: +1 (high), 0 (neutral), -1 (low)
  weight:        number;          // recency/strength weight. Convention: 0.3 isolated, 0.6 repeated, 0.9 sustained
  decay_exempt:  boolean;
}

type EvidenceType =
  | "hedging_phrase"
  | "direct_assertion"
  | "qualification_clause"
  | "question_pattern"
  | "ironic_understatement"
  | "technical_register"
  | "casual_register"
  | "humor_marker"
  | "abstract_framing"
  | "concrete_example";
```

**Score computation (per metric, at read-time):**
```
score(metric) =
  Σ( evidence.signal × evidence.weight × decay(evidence.observed_at) )
  ─────────────────────────────────────────────────────────────────────
  Σ( evidence.weight × decay(evidence.observed_at) )

  → normalized to [0, 10]; empty pool returns neutral 5.0
```

---

## Register & `RegisterMetricName`

The register block stores **six** communication dimensions:

```typescript
type RegisterMetricName =
  | "formality"
  | "directness"
  | "hedging"
  | "humor"
  | "abstraction"
  | "affect";        // ← v0.2 addition
```

**`affect`** measures emotional warmth / expressiveness vs. affective neutrality:
- **High (8+):** warm, emotionally present language
- **Low (2−):** detached, neutral, professionally distanced

> **JSON key mapping:** The register block serializes as **`"comm"`** in all profile files — not `"register"`. This matches the Python attribute name (`comm`) rather than its Pydantic alias (`register`). Code that writes `"register"` will fail to deserialize.

Each dimension is a `RegisterMetric`:

```typescript
interface RegisterMetric {
  evidence: Evidence[];   // additive pool; score computed at read-time
}
```

---

## Origination × Orientation Confidence Matrix

Base confidence applied to every incoming observation before analysis.  
Engine may adjust upward based on corroboration; never below base.

| Origination | Orientation        | Base Confidence    |
|-------------|--------------------|--------------------|
| `user`      | `user`             | 1.00 *(override)*  |
| `active`    | `claude.*`         | 0.91               |
| `passive`   | `claude.*`         | 0.78               |
| `passive`   | `local:*`          | 0.61               |
| `sync`      | `local:*`          | 0.55               |
| `system`    | `algorithmic`      | 0.45               |

**Corroboration bonus:** if the same value appears across ≥2 independent orientations with `confirmed` status, each observation receives +0.08 to effective confidence (capped at 1.0).

---

## Field Classes & Decay

Decay function:
```
effective_confidence(obs, λ) = base_confidence × e^(−λ × days_since_observation)
```

| Field Class  | Fields                              | λ       | Review Behavior          |
|--------------|-------------------------------------|---------|--------------------------|
| `identity`   | core traits                         | 0.0005  | prompted only            |
| `value`      | values, constraints                 | 0.0008  | prompted only            |
| `register`   | communication metrics (via evidence)| 0.0100  | background or prompted   |
| `domain`     | domain clusters                     | 0.0080  | background or prompted   |
| `working`    | collaboration style                 | 0.0070  | background or prompted   |
| `signal`     | phrases, rhythms, avoidances        | 0.0200  | background (fast-moving) |
| `annotation` | all annotations                     | n/a     | never decays             |

`identity` and `value` class observations with `user` origination are **always decay-exempt**.

---

## Cleanup Policy

User-configured. Stored in `meta.cleanup_policy`.

```typescript
interface CleanupPolicy {
  threshold: number;           // default 0.20 — effective_confidence below which obs is flagged
  mode:      "prompted" | "background";
  cadence:   "event" | "session" | "weekly" | "monthly";
}
```

**`prompted` mode:**  
Flagged observations surface in a `review_queue`. At session start (or on cadence), the engine presents them to the user: *"This observation has decayed. Solidify or discard?"*
- Solidify → status remains `confirmed`, `decay_exempt` set to `true`, weight reset to 1.0
- Discard → status moves to `archived`

**`background` mode:**  
Flagged observations are auto-archived without user prompt. Appropriate for `signal` class fields. User can still inspect `archived` entries.

**`event`-triggered cadence:** cleanup runs whenever any observation crosses the threshold. Other cadences batch-process on schedule.

---

## Bridge Format (Inbound)

File-based. The indexer watches a configured directory for `.bridge.json` files.  
Files are consumed (moved to `bridge_log/processed/`) after ingestion.

The engine accepts two packet formats. Both can coexist in a single file.

### v0.1 Format (flat)

```typescript
interface BridgePacket_v1 {
  bridge_version: string;           // "0.1" — also accepted as `bridge_format_version`
  orientation:    string;           // "local:gemma3:4b"
  session_ref:    string;           // SHA hash provided by cron/script
  timestamp:      string;           // ISO 8601 session start
  observations:   BridgeObservation[];
}

interface BridgeObservation {
  field:       string;              // dot-path: "signals.phrases", "domains", "comm.formality"
  value:       any;                 // string | number | DomainEntry | Evidence
  origination: "passive" | "sync" | "active"; // defaults to "passive" on unknown string
  raw?:        string;              // optional: source text that produced this observation
}
```

### v0.2 Format (structured)

```typescript
interface BridgePacket_v2 {
  bridge_format_version: string;   // "0.2" — alias: bridge_version also accepted
  source:      BridgeSource;       // replaces flat orientation/session_ref/timestamp

  target_profile?:  string;        // profile ID — skips requiring explicit user_id from caller
  target_version?:  string;        // informational only; no optimistic-locking
  previous_version?: string;

  // Both arrays may be present in a migrating packet; engine handles both
  observations?:          BridgeObservation[];
  observations_proposed?: { [field: string]: BridgeObservationV2[] };

  deltas_flagged?: BridgeDeltaFlags;
  dyadic_notes?:   BridgeDyadicNotes;
}

interface BridgeSource {
  type:        string;             // classifier, e.g. "session_analysis"
  origination: "active" | "passive" | "sync"; // defaults to "passive" on unknown string
  orientation: string;
  session_ref: string;
  timestamp:   string;            // ISO 8601
}

interface BridgeObservationV2 {
  value:  any;                    // string | number | DomainEntry
  source: BridgeSource;
  // confidence, weight, status, revision, decay_exempt are accepted but silently dropped —
  // the engine always computes these server-side (axiom 2)
}
```

**`BridgeOrigination` permitted values:**
- `"active"` — structured elicitation, maps to `Active × claude.*` → 0.91 base confidence. Available in v0.2 only.
- `"passive"` — passive inference from conversation. Default fallback for any unrecognized string.
- `"sync"` — synced from a local model bridge session.
- `"user"` origination is **forbidden** at the bridge layer — only the user can set this via CLI/MCP annotate.

---

### `BridgeDeltaFlags`

Source hints about which observations to act on after ingestion. Treated as advisory, not authoritative.

```typescript
interface BridgeDeltaFlags {
  confirm:    string[];  // field prefixes to auto-confirm proposed observations
                         // treated like confirm_all_proposed(prefix) — engine trusts these
  revise:     string[];  // logged as intent; not auto-actioned (trust boundary)
  deprecate:  string[];  // logged as intent; not auto-actioned (trust boundary)
}
```

Only `confirm` entries are acted on automatically. `revise` and `deprecate` are logged in the bridge audit record but require explicit user or engine action — the engine does not self-revise or self-deprecate based on source hints alone.

---

### `BridgeDyadicNotes`

Relational metadata about a specific pairing between two profiles. Stored as a decay-exempt annotation on the target profile — a dedicated `dyadic` document type is deferred to v0.3.

```typescript
interface BridgeDyadicNotes {
  pairing:                  string;         // e.g. "dakota–naomi"
  complementarity_finding:  string;         // defaults to "" if absent
  risk_flag?:               string | null;  // omitted if absent
}
```

---

### Ingestion Behavior

1. Engine receives packet; determines format from presence of `source` object vs. flat fields
2. Stamps each `BridgeObservation` / `BridgeObservationV2` with full `Observation` envelope
3. Applies matrix confidence from the origination × orientation table
4. For each field, checks for existing `confirmed` observations:
   - Compatible → new observation enters as `proposed`, awaits corroboration or user confirmation
   - Conflicting → both existing and new enter `delta` status, field becomes inert, added to `delta_queue`
5. Register evidence (dot-path `comm.*`) is **appended**, never delta-checked
6. `deltas_flagged.confirm` prefixes auto-confirm matching proposed observations
7. `dyadic_notes` if present is stored as a decay-exempt `system` annotation on the target profile
8. Packet is moved to `bridge_log/processed/`; a `BridgeLogEntry` is written

---

### `BridgeLogEntry`

Audit record written for each processed packet. Stored in `bridge_log.processed[]`.

```typescript
interface BridgeLogEntry {
  filename:              string;   // required — original filename consumed
  ingested_at:           string;   // ISO 8601 (default epoch if absent)
  observations_proposed: number;   // uint32, count of observations created
  deltas_flagged:        number;   // uint32, count of delta conflicts triggered
}
```

The `bridge_log.pending_filenames` array holds filenames queued for ingestion but not yet processed.

---

## Profile Document Structure

```typescript
interface ProfileDocument {
  meta: ProfileMeta;                     // required

  identity: Identity;
  comm:     Register;                    // JSON key is "comm" — not "register"
  domains:  ObservationField[];          // each element is one domain cluster
  values:   ObservationField[];
  signals:  Signals;
  working:  Working;

  annotations:  Annotation[];
  delta_queue:  DeltaItem[];
  review_queue: ReviewItem[];
  bridge_log:   BridgeLog;
}

interface ProfileMeta {
  id:                 string;            // required; "usr_" + 6-char hash
  version:            string;            // default "0.1.0"; semver, increments on confirmed change
  schema_version:     string;            // default "0.1.0"
  created:            string;            // RFC 3339 UTC from Rust; naive ISO 8601 from Python — both valid
  updated:            string;
  cleanup_policy:     CleanupPolicy;
  overall_confidence: number;            // mean effective_confidence across confirmed observations
}
```

### `Identity`

```typescript
interface Identity {
  core:      ObservationField[];         // each element tracks one distinct core personality trait
  reasoning: IdentityReasoning;
}

interface IdentityReasoning {
  style:   ObservationField;
  pattern: ObservationField;
  intake:  ObservationField;
  stance:  ObservationField;             // ← v0.2 addition
}
```

**`reasoning.stance`** — epistemic/affective orientation toward uncertainty. Singleton field; conflicting observations trigger delta detection. Examples: `"skeptical-by-default"`, `"curious-first"`, `"deferential until evidence"`.

### `Signals`

```typescript
interface Signals {
  phrases:    ObservationField[];
  avoidances: ObservationField[];
  rhythms:    ObservationField[];
  framings:   ObservationField[];        // ← v0.2 addition
}
```

**`signals.framings`** — conceptual scaffolds the user reaches for: systems-first, narrative-first, empirical-first, relational-first. Qualitatively different from surface `phrases` — these describe how the user structures thought, not what they say.

### `Working`

```typescript
interface Working {
  mode:     ObservationField;
  pace:     ObservationField;
  feedback: ObservationField;
  pattern:  ObservationField;
}
```

### `BridgeLog`

```typescript
interface BridgeLog {
  processed:         BridgeLogEntry[];   // audit records of consumed packets
  pending_filenames: string[];           // queued but not yet processed
}
```

---

## Delta Resolution

When a field enters `delta` status:

```typescript
interface DeltaItem {
  id:         string;
  field:      string;             // dot-path to the conflicting field, e.g. "identity.core[0]"
  a:          Observation;        // existing confirmed observation
  b:          Observation;        // incoming conflicting observation
  created_at: string;
  resolved:   boolean;
}
```

**Resolution actions (user-only):**
- **Confirm A:** `a.status = "confirmed"`, `b.status = "rejected"`, field exits delta
- **Confirm B:** `b.status = "confirmed"`, `a.status = "rejected"`, field exits delta
- **Confirm Both:** allowed for array fields (e.g., `signals.phrases`) — both enter `confirmed`, no conflict
- **Reject Both:** both enter `rejected`, field returns to `null` active value

Rejected observations remain in the observation array with `status: "rejected"`. Never removed.

---

## Review Queue

```typescript
interface ReviewItem {
  id:                  string;
  field:               string;
  observation_index:   number;   // uint index into ObservationField.observations
  effective_confidence: number;
  flagged_at:          string;
  resolved:            boolean;
}
```

---

## Annotation Structure

```typescript
interface Annotation {
  id:         string;
  field:      string;            // dot-path reference
  note:       string;
  author:     "user" | "system";
  created_at: string;
  pinned:     boolean;           // pinned annotations always appear in Rich-tier output
}
```

Annotations never decay. `system` annotations are engine-generated notes about patterns (e.g., "this field has been proposed and rejected 3 times from local orientations"). `user` annotations are free-form and take precedence in display.

---

## Output Resolution (Tier Scaling)

At render-time, the engine walks the profile and applies tier filters:

| Tier     | Tokens  | Includes                                                                        |
|----------|---------|---------------------------------------------------------------------------------|
| Nano     | ~180    | `identity.core` (top 3 confirmed only)                                          |
| Micro    | ~550    | + `comm` (computed scores), `working.mode`, `working.feedback`                  |
| Standard | ~1,400  | + `domains`, `values`, `identity.reasoning`, `working` (full)                  |
| Rich     | ~3,200  | + `signals`, pinned `annotations`, `delta_queue` summary                        |

Only `confirmed` observations contribute to output. `delta`, `proposed`, `rejected`, `archived` are invisible to the consuming model.

---

## v0.1 → v0.2 Delta Summary

| Area | v0.1 Spec | v0.2 Reality |
|---|---|---|
| Observation value type | Generic `Observation<T>` | Sum-type enum `ObservationValue` (`Text`, `Domain`, `Number`) |
| `DomainEntry` | `{label, weight}` | `{label, weight, proficiency?}` |
| `identity.reasoning` | `style`, `pattern`, `intake` | + `stance` (epistemic orientation) |
| `signals` | `phrases`, `avoidances`, `rhythms` | + `framings` (conceptual scaffolds) |
| Register JSON key | `"register"` | **`"comm"`** |
| Register dimensions | 5 (`formality`…`abstraction`) | 6 — + `affect` |
| `ObservationField` | bare `{observations[]}` | + `proposal_count: uint32` |
| Bridge origination | `"passive"` \| `"sync"` only | + `"active"` (0.91 confidence, v0.2 only) |
| Bridge packet | flat fields only | + v0.2 nested `source`, `observations_proposed`, `deltas_flagged`, `dyadic_notes` |
| Bridge log | `{processed[], pending[]}` | `processed[]` is `BridgeLogEntry[]` with `observations_proposed`, `deltas_flagged` counts |

---

*Next: SKILL.md — engine rules, analysis patterns, passive signal taxonomy, session integration protocol.*
