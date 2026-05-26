printf 'warpctrl validation setup\n'
printf 'HEAD: f61caf49400dc5c0d37d57a553d27733700e5204\n'
printf 'WARPCTRL: /workspace/warpctrl-validation/validation-coordinator/target/debug/warpctrl\n'
printf 'Warp app: /workspace/warpctrl-validation/validation-coordinator/target/debug/warp-oss\n'
printf 'Display: :94\n'
printf 'Discovery dir: /tmp/warpctrl-validation/validation-coordinator/home/discovery\n'
printf 'Permission state: outside Warp control + all granular categories preseeded true in isolated Settings > Scripting private preferences.\n'
printf '\nWarp windows from wmctrl:\n'
wmctrl -l || true
printf '\nDiscovery records:\n'
ls -l /tmp/warpctrl-validation/validation-coordinator/home/discovery || true
printf '\nKeep this terminal visible for setup evidence.\n'
sleep 300
