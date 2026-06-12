# Fork divergence ledger

Every file this fork diverges from upstream in, with why. Update on every change;
consult during weekly upstream merges (`git merge upstream/master`) to triage conflicts fast.

## Additive files (no conflict risk)

| File | Purpose |
|---|---|
| `FORK_PLAN.md` | The fork's implementation plan and decisions (D1–D7). |
| `PATCHES.md` | This file. |
| `.github/workflows/fork-ci.yml` | Fork CI: checks the lean build on push + weekly cron. Upstream has no CI for non-default feature sets. |

## Modified upstream files

| File | Phase | What/why |
|---|---|---|
| `crates/warp_core/src/channel/config.rs` | 1 | `WarpServerConfig::offline()` + `OzConfig::offline()` appended at end of file (additive impl blocks; conflict only if upstream also appends at EOF). |
| `app/src/bin/oss.rs` | 1 | `production()` → `offline()` for `server_config` and `oz_config` (2 lines + comment). |
| `app/Cargo.toml` | 2 | `terminal_only` feature alias appended at end of `[features]` (26 features; conflict only if upstream appends at section end). |

Phase 2b (lib.rs background-service gates): **not needed** per measurements —
services are dormant with the lean feature set (see FORK_PLAN.md Phase 2a results).
