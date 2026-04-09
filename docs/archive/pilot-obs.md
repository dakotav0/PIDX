# Pilot Observations

Session notes from early CLI + MCP testing. Verified commands are listed below.

---

## Verified commands

### Ingest a bridge packet
```bash
pidx ingest usr_default ./session.bridge.json --format json
# → {"ok": true, "observations_proposed": 7, "overall_confidence": 0.0}
```

### Read status
```bash
pidx status usr_default --format json
# → {"fields": [...], "totals": {"confirmed": 0, "proposed": 7, ...}, ...}
```

### Confirm one observation
```bash
pidx confirm usr_default working.mode 0 --format json
# → {"ok": true, "field": "working.mode", "index": 0, "value": "...", "new_status": "confirmed"}
```

### Bulk-confirm under a prefix
```bash
pidx confirm-all usr_default identity
# → {"ok": true, "confirmed_count": 3, "fields": ["identity.core.0", ...]}
```

### Render a context block
```bash
pidx show usr_default --tier standard --format json
# → {"tier": "standard", "core": [...], "domains": [...], ...}
```

### Annotate a field
```bash
pidx annotate usr_default working.mode "prefers async over meetings" --pinned
```

### Review low-confidence observations
```bash
pidx decay usr_default --threshold 0.30
pidx review list usr_default
pidx review process usr_default <id> --action solidify
pidx review process usr_default <id> --action discard
```

### Compare two profiles
```bash
pidx diff --user usr_a --user usr_b --format json
```

### Watch mailbox for new packets
```bash
pidx watch usr_default --dir ~/.local/share/pidx/mailbox
```

---

## Design observations

**Conflicts park, never merge.** The delta system (park → resolve a or b) is not a nice-to-have — it's table-stakes for MCP. Any LLM ingesting observations will create deltas and has no way to resolve them without CLI access if the MCP tools are missing. This was the primary functional blocker for v0.

**SKILL schema drift prevention.** `SKILL.md` references `docs/pidx-schema-spec.md` by version at the top, rather than restating field definitions. A schema bump becomes a one-line version reference change instead of a hunt through a duplicate field list.

**Stable v0 target:** lib + CLI + pidx-mcp + schema spec + manual + example packet, Cargo.lock committed, UI held back. MCP tool gap (confirm_all, delta, review, annotate, diff, decay) closed in Session 10.

---

## Resolved

- **MCP tool gap** — all 14 tools implemented in `pidx-mcp/src/tools.rs` (Session 10)
- **Tauri sync** — commands.rs updated for stance, framings, confirm_all, annotate, decay (Session 9)
- **Platform dirs** — `ProfileStore::default_dir()` uses `dirs::data_dir()` on Windows/Linux (Session 8)
