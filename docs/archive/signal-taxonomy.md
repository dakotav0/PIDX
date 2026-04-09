# Signal Taxonomy Reference
**For:** PERSONALITY·IDX Engine  
**Read when:** Beginning a passive analysis pass or constructing register Evidence

---

## Communication Register Signals

### Formality

| Signal | Evidence Type | Metric | Direction |
|--------|---------------|--------|-----------|
| Contractions ("it's", "I've", "won't") | `casual_register` | formality | -1 |
| Sentence fragments as complete thoughts | `casual_register` | formality | -1 |
| Casual connectors as sentence starters ("So,", "And,", "Look,") | `casual_register` | formality | -1 |
| Emoji in non-ironic use | `casual_register` | formality | -1 |
| Full grammatical sentences throughout | `technical_register` | formality | +1 |
| Passive voice constructions | `technical_register` | formality | +1 |
| Structured enumeration (numbered lists in natural writing) | `technical_register` | formality | +1 |
| Domain jargon used precisely without definition | `technical_register` | formality | +1 |

---

### Directness

| Signal | Evidence Type | Metric | Direction |
|--------|---------------|--------|-----------|
| Conclusion-first sentence structure | `direct_assertion` | directness | +1 |
| Imperative requests ("do X", "make it Y") | `direct_assertion` | directness | +1 |
| Short declarative sentences stating a position | `direct_assertion` | directness | +1 |
| Explicit rejection/preference statements ("I don't use X") | `direct_assertion` | directness | +1 |
| Request buried after extensive context-setting | `qualification_clause` | directness | -1 |
| "I was thinking maybe..." framing | `qualification_clause` | directness | -1 |
| "Would it be possible to..." phrasing | `qualification_clause` | directness | -1 |
| Multi-clause qualification before the main point | `qualification_clause` | directness | -1 |

---

### Hedging

*Note: Distinguish genuine hedging (epistemic uncertainty) from ironic hedging (rhetorical self-awareness). See Ironic Hedging section below.*

| Signal | Evidence Type | Metric | Direction |
|--------|---------------|--------|-----------|
| "I think", "probably", "maybe", "perhaps" (genuine) | `hedging_phrase` | hedging | +1 |
| "I'm not sure but..." before a technical claim | `hedging_phrase` | hedging | +1 |
| Qualifying a position immediately after stating it | `qualification_clause` | hedging | +1 |
| Bare assertions without qualification | `direct_assertion` | hedging | -1 |
| "This is X" without epistemic marker | `direct_assertion` | hedging | -1 |
| Confident correction of prior content | `direct_assertion` | hedging | -1 |

**Ironic Hedging — DO NOT register as hedging:**

These are `evidence_type: "ironic_understatement"`, `metric: "humor"`, `signal: +1`.  
Also generate a `signals.phrases` Observation for the specific phrase.

| Pattern | Example |
|---------|---------|
| Size-minimizing label on complex work | "micro project (I say that now)" |
| Self-aware scope disclaimer | "famous last words" |
| Understatement of known complexity | "a small change" (on a major refactor) |
| Preemptive self-deprecation on ambitious claims | "wild idea, probably nothing" |

Ironic hedging is a high-confidence indicator of: humor register, self-awareness, experience with scope creep, and comfort with ambiguity. It signals the user knows the real complexity and is flagging it obliquely rather than genuinely uncertain.

---

### Humor

| Signal | Evidence Type | Metric | Direction |
|--------|---------------|--------|-----------|
| Ironic understatement (see above) | `ironic_understatement` | humor | +1 |
| Parenthetical aside that reframes the main statement | `humor_marker` | humor | +1 |
| Self-deprecating framing of own work or tendency | `humor_marker` | humor | +1 |
| Absurdist or unexpected naming ("bebop", "seed store") | `humor_marker` | humor | +1 |
| Dry observation about a shared frustration | `humor_marker` | humor | +1 |
| Purely earnest, no ironic register detected | `casual_register` | humor | -1 |

---

### Abstraction

| Signal | Evidence Type | Metric | Direction |
|--------|---------------|--------|-----------|
| Architectural/topological framing before specifics | `abstract_framing` | abstraction | +1 |
| Metaphor-first explanation ("like a bus lane") | `abstract_framing` | abstraction | +1 |
| Systems thinking language (pipeline, bridge, engine, topology) | `abstract_framing` | abstraction | +1 |
| Naming a concept before asking about implementation | `abstract_framing` | abstraction | +1 |
| Code or file path as first response content | `concrete_example` | abstraction | -1 |
| Specific variable/function names without framing | `concrete_example` | abstraction | -1 |
| Immediate "how do I do X in Y" without context framing | `concrete_example` | abstraction | -1 |

---

## Domain Cluster Signals

Threshold: must appear in ≥2 turns or be the primary subject of a substantive exchange.

| Signal Type | What to observe | Notes |
|-------------|-----------------|-------|
| Named technologies | Framework names, libraries, tools explicitly used or discussed | Distinguish "mentioned in passing" from "working with" |
| Named projects | User's own project names and codenames | High-weight domain signal |
| Proper nouns (institutional) | Organizations, programs, communities referenced | May indicate domain or values both |
| Assumed knowledge | Questions that presuppose deep familiarity | Implies active domain membership |
| Vocabulary precision | Domain-specific terms used correctly without definition | User is fluent in this domain |
| Named people | Researchers, authors, practitioners cited | Cross-reference with values signals |

---

## Value Signals

Values signals often appear alongside domain signals but are distinct — they carry normative weight.

| Signal | Observation Target | Notes |
|--------|--------------------|-------|
| Explicit refusal of a tool/platform | `signals.avoidances` + `values` | Include reason if stated |
| Technology choice explained ethically | `values` | Treat as high-confidence |
| Community/collective framing of a personal project | `values` | "community resource" vs "my tool" |
| Critical reference to extractive patterns | `values` | Also generates `signals.avoidances` |
| Approval of a practice based on stated principles | `values` | Lower confidence — infer carefully |
| Data sovereignty, ownership, or access framing | `values` | Generate specific Observation |

---

## Identity / Core Signals

These inform `identity.core` — stable, slow-decay observations.  
Only generate from strong, unambiguous signals. Do not infer core traits from single exchanges.

| Signal | Evidence | Notes |
|--------|----------|-------|
| Explicit self-description ("I'm a viber", "I'm a poet") | High confidence | User-origination if phrased as "I am" |
| Named personal methodology | `identity.core` + `annotation` | e.g., "my roundtable methodology" |
| Recurring self-framing across multiple sessions | High confidence after corroboration | Pattern across sessions elevates confidence |
| How user names their own projects | `signals.rhythms` | Naming style is a signature |
| Values-technology alignment (choosing tools by values) | `identity.core` | "values-aligned builder" type trait |

---

## Working Style Signals

Inform `working.*` fields — medium decay, session-behavior patterns.

| Signal | Field | What to observe |
|--------|-------|-----------------|
| Sketch-level prompt with expectation of extrapolation | `working.mode` | Short, high-trust prompts without specs |
| Spec-level prompt with explicit constraints | `working.pattern` | Detailed, leaves little to inference |
| Corrects structure, not just content | `working.mode` | User reshapes the frame, not just the output |
| Asks for options | `working.pattern` | vs. gives direction |
| Responds to drafts by extending, not rewriting | `working.feedback` | Signal of co-creation mode |
| Requests direct critique explicitly | `working.feedback` | High-confidence "no sugar-coating" signal |
| Returns to the same session with continuation framing | `working.pace` | "continuing from..." or implicit continuity |
| Single deep-dive sessions | `working.pace` | Burst-capable signal |

---

## Signal Phrase Patterns

These feed `signals.phrases` as Observation arrays. Each unique recurring phrase gets its own Observation.

**What qualifies as a signal phrase:**
- Idiosyncratic vocabulary used in place of a common term ("topography" for structure/shape)
- Project-specific coinages ("seed store", "bebop", "ambient navigator")
- Rhetorical signatures (ironic hedging phrases, recurring sentence openers)
- Self-referential labels ("viber", "roundtable")

**What does not qualify:**
- Common technical vocabulary used conventionally
- One-time usage with no recurrence
- Phrases clearly borrowed from a source being discussed

---

## Rhythm Signals

These feed `signals.rhythms` — patterns of *when and how* the user works.

| Pattern | Observation |
|---------|-------------|
| Deep-dive sessions that expand scope over time | "late-session scope expansion" |
| Aesthetic/design framing precedes technical framing | "aesthetic-first framing" |
| Naming and labeling before implementing | "deliberate naming" |
| Returning to finalize things started in a prior session | "async-continuation pattern" |
| Starting micro then discovering macro | "scope discovery tendency" |
