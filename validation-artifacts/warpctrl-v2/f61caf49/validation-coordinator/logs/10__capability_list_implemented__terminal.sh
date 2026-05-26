printf 'Command: $WARPCTRL --output-format json capability list --implemented-only\n'
printf 'Context: outside-Warp external xterm, Warp app visible on right, exact SHA f61caf49400dc5c0d37d57a553d27733700e5204\n'
printf 'Permission state: parser/catalog command; Settings > Scripting private prefs preseeded true for outside-Warp categories in isolated profile.\n'
printf 'Exit code: 0 (expected 0)\n'
printf 'stdout log: logs/10__capability_list_implemented__stdout.txt\n'
printf 'stderr log: logs/10__capability_list_implemented__stderr.txt\n'
printf 'stdout lines/bytes: 1866/53701; stderr lines: 0\n'
printf 'JSON parse valid: True\n'
printf '\n--- terminal evidence ---\n'
printf 'Output too large for one screenshot; complete stdout/stderr are in logs. Showing first and last 25 lines.\n'
printf '\n--- stdout head ---\n'
sed -n '1,25p' /workspace/warpctrl-validation/validation-coordinator/validation-artifacts/warpctrl-v2/f61caf49/validation-coordinator/logs/10__capability_list_implemented__stdout.txt
printf '\n--- stdout tail ---\n'
tail -n 25 /workspace/warpctrl-validation/validation-coordinator/validation-artifacts/warpctrl-v2/f61caf49/validation-coordinator/logs/10__capability_list_implemented__stdout.txt
printf '\n--- stderr head/tail ---\n'
sed -n '1,10p' /workspace/warpctrl-validation/validation-coordinator/validation-artifacts/warpctrl-v2/f61caf49/validation-coordinator/logs/10__capability_list_implemented__stderr.txt
tail -n 10 /workspace/warpctrl-validation/validation-coordinator/validation-artifacts/warpctrl-v2/f61caf49/validation-coordinator/logs/10__capability_list_implemented__stderr.txt
printf '\n[terminal held for screenshot]\n'
sleep 300
