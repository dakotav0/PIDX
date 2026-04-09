# a repo can't sneak past without one.

## pidx-ui tauri-src tasks

**1. The Gardener** — tending and curating their profile over time. This is the review/confirm/reject/decay loop. They want to feel like they're pruning a living thing, not managing a database. The key emotion here is *trust* — "does this still sound like me?" The UX for this should feel like a feed of cards you can swipe or tap, not a spreadsheet. Think: observation surfaces with the value front and center, source/confidence as secondary metadata, and swipe-to-confirm / swipe-to-reject as the primary gesture. The `review_queue` and `delta_queue` are basically inbox items. Treat them like that — badge count, clear-one-at-a-time flow, satisfying empty state.

**2. The Inspector** — wanting to see their profile as a coherent whole. "What does an LLM see when it loads my T3?" This is `pidx_show` territory. The UX here should render the *output* — not the internal schema, but the narrative block that a consuming model would get. Let them toggle tiers (T1/T2/T3/T4) and see the profile expand and contract. Register metrics want a radar chart or horizontal bar visualization — six axes, computed scores, maybe with evidence count as opacity/saturation. Identity core wants to feel like a mantra, not a bullet list.

**3. The Debugger** — the power-user mode you're already living in. Raw observation lists, dot-paths, confidence values, decay curves, bridge logs. This is the mode where `clear <index>` and `reject-all` matter. Table views, filters by status, sort by confidence or age. This is the mode that needs the CLI parity you're porting.

**Dashboard landing** — a single screen that answers "what happened since I was last here?" Show: pending proposals (if any), review queue count, delta count, overall confidence trend, last bridge ingestion. Basically your `pidx_status` output but warm.

**Profile explorer** — the identity/values/domains/working/signals sections as collapsible panels or a sidebar nav. Each section shows confirmed observations with their confidence as a subtle indicator (color fade or bar). Click into any observation to see its full source metadata, timestamp, origination. This is where annotations live too — inline, pinned ones visually distinct.

**Register viz** — this one deserves its own component. Six metrics, evidence-backed scores computed live. A radar chart is the obvious choice but honestly horizontal bars with the evidence count might be more informative. Let the user drill into any metric to see the raw evidence pool — the actual phrases and sessions that produced each data point.

**Delta resolver** — when conflicts exist, present them as A/B comparisons. Side by side, same field, different values. Make the user pick. This is a modal or a dedicated flow, not buried in a table.

**Bridge timeline** — a chronological view of ingested bridge packets. Which sessions fed in, how many observations each proposed, how many got confirmed vs. rejected. This gives a sense of how the profile evolved and which sources contributed.

**The missing CLI commands** you mentioned — `clear <index>` and `reject-all <prefix>` — map to two UX patterns: a destructive single-action (with confirmation), and a bulk-action toolbar that appears when you're in a filtered view. Like "viewing 12 proposed observations under `signals.phrases` → [Confirm All] [Reject All]."

The thing I'd push hardest on: **don't make the user think in dot-paths.** The schema is `identity.reasoning.stance` but the UI should say "Reasoning → Stance" with breadcrumbs or nested nav. The dot-path is the API language; the UI language should be the field's *meaning*. Same with origination codes — "you said this" vs. "Claude inferred this" vs. "your local model suggested this" is way more legible than `user` / `passive` / `sync`.

# first time?