# PERSONALITY·IDX — Original Schema Specification
**Version:** 0.1.0  
**Status:** Finalized
**Engine:** Claude-piloted. Not designed for local model execution.

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

### `Observation<T>`

The atomic unit of every profile value.

```typescript
interface Observation<T> {
  value:      T;
  source: {
    origination:   "user" | "active" | "passive" | "sync" | "system";
    orientation:   string;       // "claude.sonnet-4-6" | "local:gemma3:4b" | "algorithmic" | "user"
    session_ref:   string;       // sha hash of originating session
    timestamp:     string;       // ISO 8601
  };
  confidence:  number;           // 0.0–1.0, base × matrix weight, recomputed on read
  weight:      number;           // 0.0–1.0, field-class decay modifier
  status:      ObservationStatus;
  revision:    number;           // increment on each update to this observation
  decay_exempt: boolean;         // default true for user origination, false otherwise
}

type ObservationStatus = "proposed" | "confirmed" | "rejected" | "delta" | "archived";
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

### `ObservationField<T>`

A keyed collection of observations for a single profile field. Resolves to an active value on read.

```typescript
interface ObservationField<T> {
  observations: Observation<T>[];
  active():     T | null;   // highest (confidence × weight) among `confirmed`; null if all delta
  delta():      Observation<T>[] | null;  // returns conflicting pair if field is in delta
}
```

Resolution rule:
```
active_value = argmax over confirmed observations of:
  effective_confidence(obs) = obs.confidence × decay(obs.source.timestamp, field_class.λ)
```

`delta` entries never contribute to `active()`.

---

### `Evidence`

The raw substrate of Register metric scores. Score is computed from evidence, not stored.

```typescript
interface Evidence {
  observed_at:    string;      // ISO 8601
  session_ref:    string;
  orientation:    string;
  evidence_type:  EvidenceType;
  raw:            string;      // the actual observed text or pattern fragment
  metric:         RegisterMetric;
  signal:         -1 | 0 | 1; // directional contribution to metric
  weight:         number;      // 0.0–1.0
  decay_exempt:   boolean;
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

type RegisterMetric =
  | "formality"
  | "directness"
  | "hedging"
  | "humor"
  | "abstraction";
```

**Score computation (per metric, at read-time):**
```
score(metric) =
  Σ( evidence.signal × evidence.weight × decay(evidence.observed_at) )
  ─────────────────────────────────────────────────────────────────────
  Σ( evidence.weight × decay(evidence.observed_at) )

  → normalized to [0, 10]
```

---

## Origination × Orientation Confidence Matrix

Base confidence applied to every incoming observation before analysis.  
Engine may adjust upward based on corroboration; never below base.

| Origination | Orientation        | Base Confidence |
|-------------|--------------------|-----------------|
| `user`      | `user`             | 1.00 *(override)* |
| `active`    | `claude.*`         | 0.91            |
| `passive`   | `claude.*`         | 0.78            |
| `passive`   | `local:*`          | 0.61            |
| `sync`      | `local:*`          | 0.55            |
| `system`    | `algorithmic`      | 0.45            |

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
  threshold:  number;           // effective_confidence below which obs is flagged (default: 0.20)
  mode:       "prompted" | "background";
  cadence:    "event" | "session" | "weekly" | "monthly";
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

```typescript
interface BridgePacket {
  bridge_version: string;          // "0.1"
  orientation:    string;          // "local:gemma3:4b"
  session_ref:    string;          // sha hash provided by cron/script
  timestamp:      string;          // ISO 8601 session start
  observations: BridgeObservation[];
}

interface BridgeObservation {
  field:       string;             // dot-path: "signals.phrases", "domains", "register.evidence"
  value:       any;                // string | number | Evidence (for register)
  origination: "passive" | "sync"; // local bridge is never "active" or "user"
  raw?:        string;             // optional: the source text that produced this observation
}
```

**Ingestion behavior:**
1. Engine receives packet, stamps each `BridgeObservation` with full `Observation` envelope
2. Applies matrix confidence: `local:*` passive = 0.61, sync = 0.55
3. Checks each field for existing `confirmed` observations
4. If compatible: new observation enters as `proposed`, awaits corroboration or user confirmation
5. If conflicting: both existing and new enter `delta` status, field becomes inert, added to `delta_queue`

---

## Profile Document Structure

```typescript
interface ProfileDocument {
  meta: {
    id:               string;      // "usr_" + 6-char hash
    version:          string;      // semver, increments on any confirmed change
    schema_version:   string;      // "0.1.0"
    created:          string;
    updated:          string;
    cleanup_policy:   CleanupPolicy;
    overall_confidence: number;    // mean effective_confidence across confirmed observations
  };

  identity: {
    core: ObservationField<string>[];
    reasoning: {
      style:   ObservationField<string>;
      pattern: ObservationField<string>;
      intake:  ObservationField<string>;
    };
  };

  register: {
    [metric in RegisterMetric]: {
      evidence:    Evidence[];
      score():     number;         // computed at read-time from evidence pool
    }
  };

  domains:    ObservationField<{ label: string; weight: number }>[];
  values:     ObservationField<string>[];

  signals: {
    phrases:    ObservationField<string>[];
    avoidances: ObservationField<string>[];
    rhythms:    ObservationField<string>[];
  };

  working: {
    mode:     ObservationField<string>;
    pace:     ObservationField<string>;
    feedback: ObservationField<string>;
    pattern:  ObservationField<string>;
  };

  annotations: Annotation[];       // never decay, permanent
  delta_queue: DeltaItem[];        // conflicts awaiting user resolution
  review_queue: ReviewItem[];      // decayed observations awaiting prompted review
  bridge_log: {
    processed: BridgePacket[];
    pending:   BridgePacket[];
  };
}
```

---

## Delta Resolution

When a field enters `delta` status:

```typescript
interface DeltaItem {
  field:       string;             // dot-path to the conflicting field
  a:           Observation<any>;   // existing confirmed observation
  b:           Observation<any>;   // incoming conflicting observation
  created_at:  string;
  resolved:    boolean;
}
```

**Resolution actions (user-only):**
- **Confirm A:** `a.status = "confirmed"`, `b.status = "rejected"`, field exits delta
- **Confirm B:** `b.status = "confirmed"`, `a.status = "rejected"`, field exits delta
- **Confirm Both:** allowed for array fields (e.g., `signals.phrases`) — both enter `confirmed`, no conflict
- **Reject Both:** both enter `rejected`, field returns to `null` active value

Rejected observations remain in the observation array with `status: "rejected"`. Never removed.

---

## Annotation Structure

```typescript
interface Annotation {
  id:          string;
  field:       string;             // dot-path reference
  note:        string;
  author:      "user" | "system";
  created_at:  string;
  pinned:      boolean;            // pinned annotations always appear in Rich tier output
}
```

Annotations never decay. `system` annotations are engine-generated observations about patterns (e.g., "this field has been proposed and rejected 3 times from local orientations"). `user` annotations are free-form and take precedence in display.

---

## Output Resolution (Tier Scaling)

At render-time, the engine walks the profile and applies tier filters:

| Tier     | Tokens  | Includes                                                                 |
|----------|---------|--------------------------------------------------------------------------|
| Nano     | ~180    | `identity.core` (top 3 confirmed only)                                   |
| Micro    | ~550    | + `register` (computed scores), `working.mode`, `working.feedback`       |
| Standard | ~1,400  | + `domains`, `values`, `identity.reasoning`, `working` (full)           |
| Rich     | ~3,200  | + `signals`, pinned `annotations`, `delta_queue` summary                 |

Only `confirmed` observations contribute to output. `delta`, `proposed`, `rejected`, `archived` are invisible to the consuming model.

---

*Next: SKILL.md — engine rules, analysis patterns, passive signal taxonomy, session integration protocol.*
