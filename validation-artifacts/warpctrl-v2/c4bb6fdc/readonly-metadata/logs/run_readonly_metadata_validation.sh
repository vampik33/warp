#!/usr/bin/env bash
ARTIFACT_ROOT=/workspace/warpctrl-validation/readonly-metadata/validation-artifacts/warpctrl-v2/c4bb6fdc/readonly-metadata
LOG_DIR="$ARTIFACT_ROOT/logs"
SCREEN_DIR="$ARTIFACT_ROOT/screenshots"
WARPCTRL=/workspace/warpctrl-validation/readonly-metadata/target/debug/warpctrl
export WARPCTRL
mkdir -p "$LOG_DIR" "$SCREEN_DIR"
printf "Starting readonly metadata command validation at %s\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee "$LOG_DIR/command_runner.log"
for i in $(seq 1 90); do
  if ls "$WARP_LOCAL_CONTROL_DISCOVERY_DIR"/*.json >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
ls -la "$WARP_LOCAL_CONTROL_DISCOVERY_DIR" > "$LOG_DIR/discovery_dir_listing_before_commands.log" 2>&1
if ls "$WARP_LOCAL_CONTROL_DISCOVERY_DIR"/*.json >/dev/null 2>&1; then
  cp "$WARP_LOCAL_CONTROL_DISCOVERY_DIR"/*.json "$LOG_DIR/" 2>/dev/null || true
fi
clear
printf "warpctrl validation 001: window_list\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json window list\n"
{ $WARPCTRL --output-format json window list; } > >(tee "$LOG_DIR/001__outside-staggered__metadata__window_list.stdout.log") 2> >(tee "$LOG_DIR/001__outside-staggered__metadata__window_list.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/001__outside-staggered__metadata__window_list.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/001__outside-staggered__metadata__window_list__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/001__outside-staggered__metadata__window_list__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 002: window_inspect_active\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json window inspect --window active\n"
{ $WARPCTRL --output-format json window inspect --window active; } > >(tee "$LOG_DIR/002__outside-staggered__metadata__window_inspect_active.stdout.log") 2> >(tee "$LOG_DIR/002__outside-staggered__metadata__window_inspect_active.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/002__outside-staggered__metadata__window_inspect_active.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/002__outside-staggered__metadata__window_inspect_active__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/002__outside-staggered__metadata__window_inspect_active__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 003: tab_list\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json tab list\n"
{ $WARPCTRL --output-format json tab list; } > >(tee "$LOG_DIR/003__outside-staggered__metadata__tab_list.stdout.log") 2> >(tee "$LOG_DIR/003__outside-staggered__metadata__tab_list.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/003__outside-staggered__metadata__tab_list.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/003__outside-staggered__metadata__tab_list__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/003__outside-staggered__metadata__tab_list__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 004: tab_inspect_active\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json tab inspect --tab active\n"
{ $WARPCTRL --output-format json tab inspect --tab active; } > >(tee "$LOG_DIR/004__outside-staggered__metadata__tab_inspect_active.stdout.log") 2> >(tee "$LOG_DIR/004__outside-staggered__metadata__tab_inspect_active.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/004__outside-staggered__metadata__tab_inspect_active.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/004__outside-staggered__metadata__tab_inspect_active__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/004__outside-staggered__metadata__tab_inspect_active__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 005: tab_inspect_index_0\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json tab inspect --tab-index 0\n"
{ $WARPCTRL --output-format json tab inspect --tab-index 0; } > >(tee "$LOG_DIR/005__outside-staggered__metadata__tab_inspect_index_0.stdout.log") 2> >(tee "$LOG_DIR/005__outside-staggered__metadata__tab_inspect_index_0.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/005__outside-staggered__metadata__tab_inspect_index_0.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/005__outside-staggered__metadata__tab_inspect_index_0__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/005__outside-staggered__metadata__tab_inspect_index_0__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 006: pane_list\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json pane list\n"
{ $WARPCTRL --output-format json pane list; } > >(tee "$LOG_DIR/006__outside-staggered__metadata__pane_list.stdout.log") 2> >(tee "$LOG_DIR/006__outside-staggered__metadata__pane_list.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/006__outside-staggered__metadata__pane_list.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/006__outside-staggered__metadata__pane_list__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/006__outside-staggered__metadata__pane_list__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 007: pane_inspect_active\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json pane inspect --pane active\n"
{ $WARPCTRL --output-format json pane inspect --pane active; } > >(tee "$LOG_DIR/007__outside-staggered__metadata__pane_inspect_active.stdout.log") 2> >(tee "$LOG_DIR/007__outside-staggered__metadata__pane_inspect_active.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/007__outside-staggered__metadata__pane_inspect_active.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/007__outside-staggered__metadata__pane_inspect_active__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/007__outside-staggered__metadata__pane_inspect_active__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 008: pane_inspect_index_0\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json pane inspect --pane-index 0\n"
{ $WARPCTRL --output-format json pane inspect --pane-index 0; } > >(tee "$LOG_DIR/008__outside-staggered__metadata__pane_inspect_index_0.stdout.log") 2> >(tee "$LOG_DIR/008__outside-staggered__metadata__pane_inspect_index_0.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/008__outside-staggered__metadata__pane_inspect_index_0.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/008__outside-staggered__metadata__pane_inspect_index_0__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/008__outside-staggered__metadata__pane_inspect_index_0__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 009: session_list\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json session list\n"
{ $WARPCTRL --output-format json session list; } > >(tee "$LOG_DIR/009__outside-staggered__metadata__session_list.stdout.log") 2> >(tee "$LOG_DIR/009__outside-staggered__metadata__session_list.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/009__outside-staggered__metadata__session_list.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/009__outside-staggered__metadata__session_list__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/009__outside-staggered__metadata__session_list__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 010: session_inspect_active\n"
printf "context: outside-staggered; expected: success\n"
printf "$ $WARPCTRL --output-format json session inspect --session active\n"
{ $WARPCTRL --output-format json session inspect --session active; } > >(tee "$LOG_DIR/010__outside-staggered__metadata__session_inspect_active.stdout.log") 2> >(tee "$LOG_DIR/010__outside-staggered__metadata__session_inspect_active.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/010__outside-staggered__metadata__session_inspect_active.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/010__outside-staggered__metadata__session_inspect_active__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/010__outside-staggered__metadata__session_inspect_active__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 011: window_inspect_missing_id\n"
printf "context: outside-staggered; expected: missing_or_stale_target_error\n"
printf "$ $WARPCTRL --output-format json window inspect --window missing-window-id\n"
{ $WARPCTRL --output-format json window inspect --window missing-window-id; } > >(tee "$LOG_DIR/011__outside-staggered__selector_edge__window_inspect_missing_id.stdout.log") 2> >(tee "$LOG_DIR/011__outside-staggered__selector_edge__window_inspect_missing_id.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/011__outside-staggered__selector_edge__window_inspect_missing_id.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/011__outside-staggered__selector_edge__window_inspect_missing_id__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/011__outside-staggered__selector_edge__window_inspect_missing_id__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 012: tab_inspect_index_999\n"
printf "context: outside-staggered; expected: missing_target_error\n"
printf "$ $WARPCTRL --output-format json tab inspect --tab-index 999\n"
{ $WARPCTRL --output-format json tab inspect --tab-index 999; } > >(tee "$LOG_DIR/012__outside-staggered__selector_edge__tab_inspect_index_999.stdout.log") 2> >(tee "$LOG_DIR/012__outside-staggered__selector_edge__tab_inspect_index_999.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/012__outside-staggered__selector_edge__tab_inspect_index_999.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/012__outside-staggered__selector_edge__tab_inspect_index_999__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/012__outside-staggered__selector_edge__tab_inspect_index_999__terminal_ui.png" || true
sleep 1
clear
printf "warpctrl validation 013: window_inspect_conflict\n"
printf "context: outside-staggered; expected: cli_conflict_error\n"
printf "$ $WARPCTRL --output-format json window inspect --window active --window-index 0\n"
{ $WARPCTRL --output-format json window inspect --window active --window-index 0; } > >(tee "$LOG_DIR/013__outside-staggered__selector_edge__window_inspect_conflict.stdout.log") 2> >(tee "$LOG_DIR/013__outside-staggered__selector_edge__window_inspect_conflict.stderr.log" >&2)
cmd_status=$?
printf "%s\n" "$cmd_status" > "$LOG_DIR/013__outside-staggered__selector_edge__window_inspect_conflict.exit_code.txt"
printf "exit_status=%s\n" "$cmd_status"
printf "screenshot: $SCREEN_DIR/013__outside-staggered__selector_edge__window_inspect_conflict__terminal_ui.png\n"
sleep 1
scrot "$SCREEN_DIR/013__outside-staggered__selector_edge__window_inspect_conflict__terminal_ui.png" || true
sleep 1
printf "Completed readonly metadata command validation at %s\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" | tee -a "$LOG_DIR/command_runner.log"
sleep 300
