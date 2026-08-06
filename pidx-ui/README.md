# PIDX UI

The PIDX desktop UI — a **pre-release** build of the personality indexer's front end.
SvelteKit 5 (runes) + Tailwind 4 + Tauri v2. It reads and writes the same profiles as
the CLI and MCP server; nothing here is a separate data store.

## Prerequisites

- Rust workspace at the repo root (`cargo build` — the UI crate is `pidx-ui-svelte`)
- Node 22+ (`npm ci` in this directory)

## Dev

```bash
npm ci
# profiles dir defaults to PIDX_PROFILES_DIR, then a walk-up `profiles/` dir,
# then `./profiles`. Point it at your real store for a live view:
PIDX_PROFILES_DIR="$HOME/Library/Application Support/pidx/profiles" npm run tauri dev
```

## Build

```bash
npm run build       # vite production build (also the CI gate)
npm run tauri build # standalone macOS app bundle
```

## Layout

```
src/routes/            / (profile list) · /profile/[user] (Profile · Review · Gardener · Inspector · Debugger) · /bridge · /diff
src/lib/ipc.ts         typed invoke() wrappers — one function per Tauri command
src/lib/profile.ts     view helpers (confirmed-observation selectors, register axes)
src/lib/components/    ProfileView (tiered centerpiece) · ProfileSection · RegisterRadar · ObservationTable · Inspector/Gardener/Debugger views
src-tauri/src/commands.rs  #[tauri::command] handlers — thin shims over the pidx library
```

## Notes

- **Tauri v2 invoke args are camelCase** (`userId`, `fieldPrefix`…) — keep `ipc.ts` in that
  shape; snake_case keys fail with `command … missing required key …`.
- Confidence is recomputed at read time everywhere (home list, status, profile) — the
  stored `meta.overall_confidence` may lag after writes.
- The profile page defaults to the Profile tab; Debugger is the fallback.
