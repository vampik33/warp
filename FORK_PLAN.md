# Warp Terminal-Only Fork — Implementation Plan

Goal: a personal fork of `warpdotdev/warp` that is a fast, low-resource Rust terminal
(tabs, panes, blocks, local completions) with **no AI/agent features, no user auth,
no network phone-home**, while staying mergeable with upstream (~90–190 commits/week).

Decisions below are final for v1 (chosen 2026-06-12 after codebase analysis).

---

## Decisions (made)

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | Strip strategy | **Minimal-diff: AI compiled-but-dormant.** Disable via channel config, cargo features, and registration gating. No structural deletion. | AI = ~25% of codebase; 321 files outside `app/src/ai` import from it; upstream velocity makes deletion unmergeable. |
| D2 | Optimization target | **Runtime RAM/CPU/startup first; binary size second.** | Resource cost comes from background services (MCP spawner, codebase indexer, FS watchers, ONNX load), not dead code. `cargo check -p warp --no-default-features` already passes (verified 2026-06-12), so feature-stripping is cheap and taken as a free win. |
| D3 | Keep-list | Terminal-first: tabs/panes/blocks, themes, vim mode, classic local completions, autosuggestions, command palette, kitty protocols, ligatures, session restore. **Drop:** ONNX input classifier, voice, MCP, codebase indexing, skills, code-review/editor panes, markdown/agent rendering, all cloud. | Code-editor features can be re-added later by editing one feature list line. |
| D4 | Login behavior | **Stay fully logged-out** (default-on `skip_firebase_anonymous_user`). Do NOT use `skip_login` (it fakes a logged-in test user; cloud features then error instead of hiding). | Logged-out state makes cloud features hide themselves cleanly. |
| D5 | Upstream sync | **Weekly `git merge upstream/master`** (merge, not rebase) + fork CI building the lean config. | Rebase at this velocity means re-resolving conflicts forever. |
| D6 | Completions | Keep `classic_completions` (already default), skip `completions_v2`. | v2 pulls the 78 MB `command-signatures-v2` crate; classic engine is fully local. |
| D7 | `gui` feature | Omit it. `gui = ["voice_input"]` and nothing in code checks `cfg(feature = "gui")` (verified). | Drops voice input for free. |

Diff budget: keep the fork's non-additive diff under ~10 files / ~300 lines.
Additive files (new feature alias, new CI workflow, this file, PATCHES.md) are free.

---

## Phase 0 — Fork setup + baseline (one evening)

1. Create the GitHub fork: `gh repo fork warpdotdev/warp --clone=false`
2. Convert the existing clone at `/home/vampik/warp`:
   ```bash
   git fetch --unshallow origin
   git remote rename origin upstream
   git remote add origin git@github.com:<you>/warp.git
   git branch upstream-master upstream/master   # pristine mirror for diffing
   git push -u origin master
   ```
3. System deps: `sudo pacman -S git-lfs` then `git lfs install && git lfs pull`
   (`crates/input_classifier/models/**` and `*.pdb` are LFS pointers).
   Run `./script/linux/bootstrap` manually-reviewed, or install deps by hand;
   pass `--skip-common-skills` / `WARP_SKIP_COMMON_SKILLS_INSTALL=1` to skip the
   agent-skills install. Note: bootstrap targets apt; on CachyOS map packages to pacman.
4. Baseline build (vanilla OSS channel — this is the path external contributors use,
   no private `warp-channel-config` needed):
   ```bash
   WARP_CHANNEL=oss cargo run --bin warp-oss --features gui
   ```
5. Record baseline evidence: binary size, time-to-window, idle RSS,
   child process tree (`pstree -p`), open FDs/watches.

**Done when:** vanilla `warp-oss` runs, baseline numbers recorded in this file.

### Baseline (recorded 2026-06-12, commit a30cc7a, dev profile, Linux/Wayland)

| Metric | Value |
|---|---|
| Build time (cold deps cached from check) | 3m 06s |
| Binary size (debug, with debuginfo) | 922 MB |
| Idle RSS @ T+18s | ~1,007 MB |
| Threads / open FDs | 74 / 72 |
| Child processes | 2 × warp-oss helpers |
| Idle CPU (5s sample) | 21 ticks ≈ 4% of one core |
| **Outbound network at idle** | **1 ESTABLISHED TLS conn → 34.117.41.85:443 (app.warp.dev / GCLB)** |

The outbound connection is the Phase 1 target: it must be zero after the offline config.

## Phase 1 — Fully offline, zero auth (~1 day, ~5 files)

The OSS binary already sets `telemetry_config / crash_reporting_config /
autoupdate_config / mcp_static_config = None` (`app/src/bin/oss.rs:14-25`) and
skips login. Remaining gap: hardcoded production endpoints.

Edits:
1. `crates/warp_core/src/channel/config.rs` — add `WarpServerConfig::offline()`
   and `OzConfig::offline()`. NOTE (verified 2026-06-12): URLs must be parseable —
   empty strings panic at startup (`warp_server_client/src/auth/session.rs:200`,
   "Server root URL must be valid: RelativeUrlWithoutBase"). Use
   `http://127.0.0.1:1` / `ws://127.0.0.1:1`: parses fine, connection-refused
   instantly, zero packets leave the machine. `session_sharing_server_url: None`,
   `firebase_auth_api_key: ""`.
   Replaces: `https://app.warp.dev`, `wss://rtc.app.warp.dev/graphql/v2`,
   `wss://sessions.app.warp.dev`, Firebase key (config.rs:57-66), `https://oz.warp.dev` (config.rs:82).
2. `app/src/bin/oss.rs` — use the offline configs.
3. Optional cosmetic: hide login/logout menu items (`app/src/app_menus.rs:235-240,884-885`);
   skip auth-view init (`app/src/auth/mod.rs:56-62`).

Notes: `font_fallback.rs` is WASM-only (`app/src/lib.rs:38-39`) — no action needed.
OTLP tracing is opt-in via env var — no action needed.

**Done when:** app runs with shell + tabs working and **zero outbound connections**:
`ss -tnp | grep warp` empty over a 10-minute session incl. startup; no DNS lookups
(`resolvectl monitor` or `strace -f -e trace=network` spot-check).

## Phase 2 — De-AI the build and runtime (a few days, iterative)

### 2a. Lean feature set (compile-time)
Append (do not edit upstream lists — additive = conflict-free) to `app/Cargo.toml`:

```toml
# Fork-only alias. Build: cargo build --bin warp-oss --no-default-features --features terminal_only
terminal_only = [
  # terminal/UX
  "ligatures", "rect_selection", "kitty_images", "kitty_keyboard_protocol",
  # (iterm_images, local_tty, local_fs are auto-added by app/build.rs — not listed here)
  "minimalist_ui", "full_screen_zen_mode", "shell_selector", "ui_zoom",
  "settings_file", "trim_trailing_blank_lines", "directory_tab_colors",
  "new_tab_styling", "vertical_tabs", "vertical_tabs_summary_mode", "tab_configs",
  "tab_close_button_on_left", "undo_closed_panes", "drag_tabs_to_windows",
  # local input intelligence (no AI/network)
  "classic_completions", "force_classic_completions",
  "validate_autosuggestions", "clear_autosuggestion_on_escape",
  "command_correction_key",
  # search/navigation (local)
  "global_search", "command_palette_file_search", "async_find",
]
```
(`local_tty` / `local_fs` are injected by `app/build.rs:225-238` automatically.
This list is a v1 hypothesis — iterate empirically: build, smoke-test, adjust.
Baseline risk is low: the all-features-off check already compiles.)

### 2b. Kill remaining background services (runtime) — measure first, then gate
With the lean build running, measure what is STILL active (per CLAUDE.md: observe,
don't guess — many services may already be inert when flags are off and logged out):
child processes, FS watchers (`/proc/<pid>/fd`, `/proc/<pid>/fdinfo` inotify), CPU wakeups, RSS.

Known suspects (unconditional registrations in `app/src/lib.rs`, line refs as of a30cc7a):
| Service | Site | Cost |
|---|---|---|
| `TemplatableMCPServerManager` | lib.rs:1889 | spawns MCP child processes |
| `FileMCPWatcher` / `FileBasedMCPManager` | lib.rs:1884-1885 | inotify watches |
| `SkillManager` | lib.rs:1903 | SKILL.md FS watcher |
| `RepoOutlines` + `SyncQueue` | lib.rs:1829-1835 | repo indexing |
| `CodebaseIndexManager` | lib.rs:2060 | FS watcher + embeddings |
| `InputClassifierModel` | lib.rs:2085 | ONNX model load (RAM) |
| `LLMPreferences` | lib.rs:1973 | network on auth/net events (inert offline, verify) |

For each one still measurably active: gate the registration or its internal start
condition behind an existing disabled FeatureFlag. Prefer gating the *work* over
removing the *registration* (other code may `get_singleton` and panic).
This is the highest merge-friction area of the fork (lib.rs churns upstream) —
keep each gate to 1–3 lines and list every one in PATCHES.md.

**Done when:** lean build runs daily-driver clean; idle: 0 unexpected child
processes, no AI-related FS watchers, RSS and startup measurably ≤ baseline;
agent UI absent (no agent panes/palette entries/settings pages).

## Phase 3 — Leaf pruning (DEFERRED — only if Phase 2 numbers justify)

Candidates (low conflict risk): `crates/voice_input` (already off without `gui`),
`crates/computer_use`, `crates/node_runtime`. Default: don't. Every deletion is
permanent merge friction; dormant code that costs nothing measurable stays.

## Phase 4 — Upstream tracking (ongoing, ~1–2 h/week)

- Weekly: `git fetch upstream && git merge upstream/master`, resolve, build lean, smoke.
- Fork CI (`.github/workflows/fork-ci.yml`, additive file): on push + weekly cron —
  `cargo check --bin warp-oss --no-default-features --features terminal_only`
  plus `cargo nextest run` on core crates (`warp_terminal`, `warp_completer`, `editor`).
  Upstream has no CI for non-default feature sets; this catches silent breakage.
- `PATCHES.md` (additive): one line per diverged file → makes conflict triage mechanical.
- If cadence slips > ~1 month, merge debt at 100+ commits/week becomes painful. Cadence is the discipline.

## Licensing / distribution

AGPL-3.0 (UI framework crates MIT). Private use: no obligations. If binaries are
ever distributed: publish fork source (already public on GitHub = compliant) and
rebrand (name + `app/channels/*/icon` assets are Warp trademarks). Defer until relevant.

## Risk register

| Risk | Mitigation |
|---|---|
| lib.rs gates conflict on merges | keep gates 1–3 lines, tracked in PATCHES.md |
| Feature subset breaks at runtime (flags assumed on) | empirical iteration in 2a/2b + smoke checklist |
| Upstream changes channel/config plumbing | offline() lives in small, stable config.rs; pristine `upstream-master` branch for diffing |
| Logged-out paths regress upstream (they test logged-in) | fork CI smoke + weekly manual run |

## Verified facts this plan relies on (2026-06-12, commit a30cc7a)

- `cargo check -p warp --no-default-features` → clean (3m16s).
- OSS channel: telemetry/crash/autoupdate/MCP configs all `None`; login not required
  (`app/src/root_view.rs:1692`); URL overrides disallowed for Oss channel.
- `gui = ["voice_input"]`, unused in code → omit.
- `completions_v2` not in defaults; classic completions fully local.
- `font_fallback` WASM-only. OTLP opt-in via env.
- AI coupling: `ai` crate mandatory; 321 files outside `app/src/ai` import it.
