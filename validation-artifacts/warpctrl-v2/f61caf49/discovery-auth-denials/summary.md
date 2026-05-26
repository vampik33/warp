# Warp Control CLI validation summary: discovery-auth-denials
Validated SHA: `f61caf49400dc5c0d37d57a553d27733700e5204`
Validation ref: `zach/warpctrl-validation/f61caf49`
Artifact directory: `validation-artifacts/warpctrl-v2/f61caf49/discovery-auth-denials`

## Result counts
Final required validation cases: 14 pass, 0 fail, 0 skip. There were 3 blocked retry/setup attempts that were superseded by later successful evidence.
All manifest command attempts: 16 pass, 0 fail, 11 blocked, 0 skip.

## Passed required coverage
- `instance list` discovered the default-off instance and showed `outside_warp_control_enabled: false`.
- Default-off `instance inspect`, `app ping`, `app version`, `app active`, and `tab create` returned `local_control_disabled` and left the visible Warp state unchanged.
- Enabled explicit-instance `instance inspect`, `app ping`, `app version`, and `app active` succeeded against `inst_ecbfdff4cf1e48cc9cdab9b34567cc8d`.
- Enabled `tab create` succeeded after focusing Warp during execution; before/after screenshots show the tab strip changing from one tab to two tabs.
- `input run "printf warpctrl-validation"` denied with `execution_context_not_allowed`; the visible target terminal did not run/display the printf output.
- `drive list --type workflow` denied with `execution_context_not_allowed`; no authenticated Drive data was exposed.
- Inside-Warp `app ping` current implemented behavior was success after stale discovery cleanup; screenshot `screenshots/031__inside-warp__app_ping_visible__terminal_ui.png` shows the command/output in the Warp terminal.

## Visual-inspection blockers / retry notes
- `025` enabled `tab create` initially returned `missing_target` because xterm had OS focus; retry `026` focused Warp during execution and visually proved the new tab.
- `029` inside-Warp `app ping` initially returned `ambiguous_instance` due to a stale zombie discovery record.
- `030` inside-Warp `app ping` returned success in the log after stale cleanup, but the screenshot did not clearly prove the fresh output; repeat `031` captured clear visible success.

## Non-blocking environment notes
- Onboarding/sign-in made manual Settings UI toggling unreliable in the headless run, so isolated preferences were edited to reflect Settings > Scripting permission states; the preference excerpt is in `logs/permissions_enabled_preferences_excerpt.json`.
- A stale discovery record from defunct pid 6523 was archived under `logs/stale_local_control_records/`.
- `apt-get install` reported a `libc-bin` post-install error for missing `/sbin/ldconfig.real`, but installed X11 tools were usable.

## Skipped commands
None.

## Blockers
None remaining.
