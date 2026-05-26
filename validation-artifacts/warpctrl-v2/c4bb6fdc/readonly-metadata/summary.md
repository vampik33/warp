# Warp Control CLI validation summary
- Agent: readonly-metadata
- Validated SHA: `c4bb6fdc670d667e78041a9318eda7c6778a22a8`
- SHA verification before validation: `True`
- Pass/fail/skip: 12/1/0
- Commands attempted: 13
- Screenshot coverage: 13 per-command screenshots plus final diagnostic screenshot

## Blockers
- $WARPCTRL --output-format json tab inspect --tab-index 999: Expected missing_target; observed 'stale_target'.

## Notes
- Validation used a real graphical Warp app under Xvfb/Openbox with an external xterm, staggered so the terminal command/output and target Warp UI are visible together.
- Outside-Warp scripting permissions were enabled by preseeding private local settings in the isolated validation profile before app launch; details are in `logs/permission_preseed.json`.
- No commands were skipped.
