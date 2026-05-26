import json
import os
import shutil
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path('/workspace/warpctrl-validation/validation-coordinator')
ART = ROOT / 'validation-artifacts/warpctrl-v2/f61caf49/validation-coordinator'
LOGS = ART / 'logs'
SHOTS = ART / 'screenshots'
LOGS.mkdir(parents=True, exist_ok=True)
SHOTS.mkdir(parents=True, exist_ok=True)

EXPECTED_SHA = 'f61caf49400dc5c0d37d57a553d27733700e5204'
HEAD = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
if HEAD != EXPECTED_SHA:
    raise RuntimeError(f'HEAD mismatch: {HEAD} != {EXPECTED_SHA}')

DISPLAY = ':94'
HOME = Path('/tmp/warpctrl-validation/validation-coordinator/home')
RUNTIME = HOME / 'runtime'
DISCOVERY = HOME / 'discovery'
PROFILE = 'validation-coordinator-f61caf49'
for p in [HOME, RUNTIME, DISCOVERY]:
    p.mkdir(parents=True, exist_ok=True)
os.chmod(RUNTIME, 0o700)

prefs_dir = HOME / f'.config/warp-oss-{PROFILE}'
prefs_dir.mkdir(parents=True, exist_ok=True)
prefs = {
    'prefs': {
        'LocalControlAllowOutsideWarp': 'true',
        'LocalControlOutsideWarpMetadataReads': 'true',
        'LocalControlOutsideWarpUnderlyingDataReads': 'true',
        'LocalControlOutsideWarpAppStateMutations': 'true',
        'LocalControlOutsideWarpMetadataConfigurationMutations': 'true',
        'LocalControlOutsideWarpUnderlyingDataMutations': 'true',
        'LocalControlInsideWarpAuthenticatedUserActions': 'true',
        'HasCompletedOnboarding': 'true'
    }
}
(prefs_dir / 'user_preferences.json').write_text(json.dumps(prefs, indent=2))

common_env = os.environ.copy()
common_env.update({
    'DISPLAY': DISPLAY,
    'HOME': str(HOME),
    'XDG_RUNTIME_DIR': str(RUNTIME),
    'WARP_DATA_PROFILE': PROFILE,
    'WARP_LOCAL_CONTROL_DISCOVERY_DIR': str(DISCOVERY),
    'RUST_LOG': 'warn,local_control=debug',
    'LIBGL_ALWAYS_SOFTWARE': '1',
    'WARP_DISABLE_GPU': '1',
})

def safe_run(args, timeout=20, env=common_env):
    return subprocess.run(args, env=env, cwd=str(ROOT), text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)

for pattern in ['Xvfb :94', 'openbox', 'target/debug/warp-oss']:
    subprocess.run(['pkill', '-f', pattern], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

procs = []
def start(name, args, stdout_path, env=common_env):
    f = open(stdout_path, 'ab')
    p = subprocess.Popen(args, stdout=f, stderr=subprocess.STDOUT, env=env, cwd=str(ROOT))
    procs.append((name, p, f))
    return p

start('xvfb', ['Xvfb', DISPLAY, '-screen', '0', '2400x1600x24'], LOGS / 'xvfb.log')
time.sleep(1.0)
start('openbox', ['openbox'], LOGS / 'openbox.log')
time.sleep(1.5)
warp = start('warp', [str(ROOT / 'target/debug/warp-oss')], LOGS / 'warp_app.log')

warp_ready = False
for _ in range(90):
    wm = safe_run(['wmctrl', '-l'], timeout=5)
    records = list(DISCOVERY.glob('*.json'))
    if 'Warp' in wm.stdout or records:
        warp_ready = True
        break
    if warp.poll() is not None:
        break
    time.sleep(1)

safe_run(['wmctrl', '-r', 'Warp', '-e', '0,1240,90,1120,1000'], timeout=5)
safe_run(['xdotool', 'search', '--name', 'Warp', 'windowsize', '1120', '1000'], timeout=5)
safe_run(['xdotool', 'search', '--name', 'Warp', 'windowmove', '1240', '90'], timeout=5)
time.sleep(2)
wm_after = safe_run(['wmctrl', '-l'], timeout=5)
(LOGS / 'wmctrl_after_launch.txt').write_text(wm_after.stdout + wm_after.stderr)
for rec in DISCOVERY.glob('*.json'):
    shutil.copy(rec, LOGS / f'discovery_{rec.name}')

def shot(path):
    proc = safe_run(['import', '-window', 'root', str(path)], timeout=20)
    if proc.returncode != 0:
        (LOGS / (path.stem + '_screenshot_error.txt')).write_text(proc.stdout + proc.stderr)
    return proc.returncode

def make_terminal_script(path, lines):
    path.write_text('\n'.join(lines) + '\n')
    os.chmod(path, 0o755)

setup_shot = SHOTS / '00__outside-staggered__setup__warp_app_visible__terminal_ui.png'
setup_script = LOGS / 'setup_terminal.sh'
setup_lines = [
    "printf 'warpctrl validation setup\\n'",
    f"printf 'HEAD: {HEAD}\\n'",
    f"printf 'WARPCTRL: {ROOT / 'target/debug/warpctrl'}\\n'",
    f"printf 'Warp app: {ROOT / 'target/debug/warp-oss'}\\n'",
    f"printf 'Display: {DISPLAY}\\n'",
    f"printf 'Discovery dir: {DISCOVERY}\\n'",
    "printf 'Permission state: outside Warp control + all granular categories preseeded true in isolated Settings > Scripting private preferences.\\n'",
    "printf '\\nWarp windows from wmctrl:\\n'",
    "wmctrl -l || true",
    "printf '\\nDiscovery records:\\n'",
    f"ls -l {DISCOVERY} || true",
    "printf '\\nKeep this terminal visible for setup evidence.\\n'",
    "sleep 300",
]
make_terminal_script(setup_script, setup_lines)
setup_term = start('setup_xterm', ['xterm', '-geometry', '150x46+10+10', '-fa', 'Monospace', '-fs', '8', '-T', 'warpctrl setup', '-e', str(setup_script)], LOGS / 'setup_xterm.log')
time.sleep(2)
shot(setup_shot)
setup_term.terminate()
try:
    setup_term.wait(timeout=3)
except subprocess.TimeoutExpired:
    setup_term.kill()

commands = [
    ('help', '$WARPCTRL --help', [str(ROOT/'target/debug/warpctrl'), '--help'], 'none', 0),
    ('completions_bash', '$WARPCTRL completions bash', [str(ROOT/'target/debug/warpctrl'), 'completions', 'bash'], 'none', 0),
    ('completions_zsh', '$WARPCTRL completions zsh', [str(ROOT/'target/debug/warpctrl'), 'completions', 'zsh'], 'none', 0),
    ('action_list_implemented', '$WARPCTRL --output-format json action list --implemented-only', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'action', 'list', '--implemented-only'], 'none', 0),
    ('action_list_stubs', '$WARPCTRL --output-format json action list --stubs-only', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'action', 'list', '--stubs-only'], 'none', 0),
    ('action_inspect_tab_create', '$WARPCTRL --output-format json action inspect tab.create', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'action', 'inspect', 'tab.create'], 'none', 0),
    ('action_inspect_input_run', '$WARPCTRL --output-format json action inspect input.run', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'action', 'inspect', 'input.run'], 'none', 0),
    ('action_inspect_drive_workflow_run', '$WARPCTRL --output-format json action inspect drive.workflow.run', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'action', 'inspect', 'drive.workflow.run'], 'none', 0),
    ('action_inspect_auth_api_key_set', '$WARPCTRL --output-format json action inspect auth.api_key.set', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'action', 'inspect', 'auth.api_key.set'], 'none', 1),
    ('capability_list_implemented', '$WARPCTRL --output-format json capability list --implemented-only', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'capability', 'list', '--implemented-only'], 'none', 0),
    ('capability_inspect_tab_create', '$WARPCTRL --output-format json capability inspect tab.create', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'capability', 'inspect', 'tab.create'], 'none', 0),
    ('capability_inspect_auth_status', '$WARPCTRL --output-format json capability inspect auth.status', [str(ROOT/'target/debug/warpctrl'), '--output-format', 'json', 'capability', 'inspect', 'auth.status'], 'none', 0),
]

manifest_entries = []
for idx, (label, display_cmd, argv, perm, expected_exit) in enumerate(commands, start=1):
    ordinal = f'{idx:02d}'
    stdout = LOGS / f'{ordinal}__{label}__stdout.txt'
    stderr = LOGS / f'{ordinal}__{label}__stderr.txt'
    termlog = LOGS / f'{ordinal}__{label}__terminal_script.log'
    script = LOGS / f'{ordinal}__{label}__terminal.sh'
    screenshot = SHOTS / f'{ordinal}__outside-staggered__catalog__{label}__terminal_ui.png'
    if label.startswith('completions'):
        answer = 'Use a wide external terminal with the built Warp app visible beside it, store the thousands-line completion output in durable logs, and show command, exit code, line/byte counts, and readable excerpts in the screenshot.'
    elif 'list_implemented' in label:
        answer = 'Use a wide external terminal with the built Warp app visible beside it, store the long JSON catalog in durable logs, and show command, exit code, JSON validity, count, and readable excerpts in the screenshot.'
    elif label == 'help':
        answer = 'Use a wide external terminal with the built Warp app visible beside it and render help output directly enough to show command groups and legibility.'
    else:
        answer = 'Use a wide external terminal with the built Warp app visible beside it, render the complete small JSON response in the terminal, and keep the app window visible as the instance under test.'
    proc = subprocess.run(argv, env=common_env, cwd=str(ROOT), text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout.write_text(proc.stdout)
    stderr.write_text(proc.stderr)
    line_count = proc.stdout.count('\n') + (1 if proc.stdout and not proc.stdout.endswith('\n') else 0)
    byte_count = len(proc.stdout.encode())
    stderr_lines = proc.stderr.count('\n') + (1 if proc.stderr and not proc.stderr.endswith('\n') else 0)
    json_valid = None
    if '--output-format' in argv and 'json' in argv:
        try:
            json.loads(proc.stdout)
            json_valid = True
        except Exception:
            json_valid = False
    full_visible = line_count <= 44 and byte_count <= 12000
    escaped_display = display_cmd.replace("'", "'\\''")
    lines = [
        f"printf 'Command: {escaped_display}\\n'",
        f"printf 'Context: outside-Warp external xterm, Warp app visible on right, exact SHA {HEAD}\\n'",
        "printf 'Permission state: parser/catalog command; Settings > Scripting private prefs preseeded true for outside-Warp categories in isolated profile.\\n'",
        f"printf 'Exit code: {proc.returncode} (expected {expected_exit})\\n'",
        f"printf 'stdout log: {stdout.relative_to(ART)}\\n'",
        f"printf 'stderr log: {stderr.relative_to(ART)}\\n'",
        f"printf 'stdout lines/bytes: {line_count}/{byte_count}; stderr lines: {stderr_lines}\\n'",
    ]
    if json_valid is not None:
        lines.append(f"printf 'JSON parse valid: {json_valid}\\n'")
    lines.append("printf '\\n--- terminal evidence ---\\n'")
    if full_visible:
        lines.append(f"cat {stdout}")
        if proc.stderr:
            lines.append("printf '\\n--- stderr ---\\n'")
            lines.append(f"cat {stderr}")
    else:
        lines.extend([
            "printf 'Output too large for one screenshot; complete stdout/stderr are in logs. Showing first and last 25 lines.\\n'",
            "printf '\\n--- stdout head ---\\n'",
            f"sed -n '1,25p' {stdout}",
            "printf '\\n--- stdout tail ---\\n'",
            f"tail -n 25 {stdout}",
            "printf '\\n--- stderr head/tail ---\\n'",
            f"sed -n '1,10p' {stderr}",
            f"tail -n 10 {stderr}",
        ])
    lines.extend(["printf '\\n[terminal held for screenshot]\\n'", "sleep 300"])
    make_terminal_script(script, lines)
    term = start(f'xterm_{label}', ['xterm', '-geometry', '150x46+10+10', '-fa', 'Monospace', '-fs', '8', '-T', f'warpctrl {label}', '-e', str(script)], termlog)
    time.sleep(2.3)
    safe_run(['wmctrl', '-r', f'warpctrl {label}', '-e', '0,10,10,1120,760'], timeout=5)
    safe_run(['wmctrl', '-r', 'Warp', '-e', '0,1240,90,1120,1000'], timeout=5)
    time.sleep(0.7)
    shot_rc = shot(screenshot)
    term.terminate()
    try:
        term.wait(timeout=3)
    except subprocess.TimeoutExpired:
        term.kill()
    unexpected = []
    visual_status = 'pass'
    if shot_rc != 0 or not screenshot.exists() or screenshot.stat().st_size == 0:
        visual_status = 'blocked'
        unexpected.append('Screenshot capture failed or produced an empty file.')
    if not full_visible:
        unexpected.append('Raw command output exceeded one-screen screenshot capacity; full output is preserved in stdout/stderr logs, and screenshot shows command, exit code, counts, and excerpts only.')
    status = 'pass' if proc.returncode == expected_exit and (json_valid is not False) else 'fail'
    if visual_status == 'blocked':
        status = 'blocked'
    entry = {
        'ordinal': ordinal,
        'command': display_cmd,
        'argv': argv,
        'context': 'outside_warp_external_xterm_with_graphical_warp_app_visible',
        'permission_state': 'Settings > Scripting private preferences preseeded true for outside-Warp control and all granular categories in isolated profile; parser/catalog commands themselves do not dispatch to the app bridge.',
        'proof_setup_question': 'What is the best way to show the impact of this CLI command?',
        'proof_setup_answer': answer,
        'expected_result': 'Exit code matches expected and output is valid/readable; auth.api_key.set is expected to return not_allowlisted; auth.status is expected to return stub metadata if present.',
        'actual_result': {
            'exit_code': proc.returncode,
            'stdout_lines': line_count,
            'stdout_bytes': byte_count,
            'stderr_lines': stderr_lines,
            'json_valid': json_valid,
        },
        'expected_exit_code': expected_exit,
        'status': status,
        'stdout_log': str(stdout.relative_to(ART)),
        'stderr_log': str(stderr.relative_to(ART)),
        'screenshot_paths': [str(screenshot.relative_to(ART))],
        'visual_inspection': {
            'expected_visual_effect': 'The external terminal should visibly show the exact command, exit status, output evidence, and the target Warp app window should remain visible/running in the same staggered screenshot.',
            'observed_visual_effect': 'Screenshot captured from the X root window after command execution. Automated checks verified the screenshot file exists and is non-empty; large outputs are represented by log-backed counts and excerpts.',
            'status': visual_status,
            'unexpected_ui_changes_or_ambiguity': unexpected,
        },
    }
    if label == 'action_inspect_auth_api_key_set':
        try:
            payload = json.loads(proc.stdout)
            code = payload.get('error', {}).get('code')
            entry['actual_result']['error_code'] = code
            if code != 'not_allowlisted':
                entry['status'] = 'fail'
        except Exception as exc:
            entry['actual_result']['error_parse_error'] = str(exc)
            entry['status'] = 'fail'
    if label == 'capability_inspect_auth_status':
        try:
            payload = json.loads(proc.stdout)
            entry['actual_result']['implementation_status'] = payload.get('implementation_status')
            if payload.get('name') != 'auth.status' or payload.get('implementation_status') != 'stub':
                entry['status'] = 'fail'
        except Exception as exc:
            entry['actual_result']['error_parse_error'] = str(exc)
            entry['status'] = 'fail'
    manifest_entries.append(entry)

implemented = json.loads((LOGS / '04__action_list_implemented__stdout.txt').read_text())
implemented_names = [item['name'] for item in implemented]
reachable = {
    'instance.list': 'instance list', 'instance.inspect': 'instance inspect', 'app.ping': 'app ping', 'app.version': 'app version', 'app.active': 'app active', 'app.focus': 'app focus',
    'capability.list': 'capability list', 'capability.inspect': 'capability inspect <action>', 'action.list': 'action list', 'action.inspect': 'action inspect <action>',
    'window.list': 'window list', 'window.inspect': 'window inspect',
    'tab.list': 'tab list', 'tab.inspect': 'tab inspect', 'tab.create': 'tab create', 'tab.rename': 'tab rename <title>', 'tab.reset_name': 'tab reset-name', 'tab.color.set': 'tab color set <color>', 'tab.color.clear': 'tab color clear',
    'pane.list': 'pane list', 'pane.inspect': 'pane inspect', 'pane.rename': 'pane rename <title>', 'pane.reset_name': 'pane reset-name',
    'session.list': 'session list', 'session.inspect': 'session inspect',
    'block.list': 'block list', 'block.inspect': 'block inspect <block_id>', 'block.output': 'block output <block_id>',
    'input.get': 'input get', 'input.run': 'input run <text>', 'history.list': 'history list',
    'theme.list': 'theme list', 'theme.get': 'theme get', 'theme.set': 'theme set <name>', 'theme.system.set': 'theme system-set <enabled>', 'theme.light.set': 'theme light-set <name>', 'theme.dark.set': 'theme dark-set <name>',
    'appearance.get': 'appearance get', 'appearance.font_size.increase': 'appearance font-size-increase', 'appearance.font_size.decrease': 'appearance font-size-decrease', 'appearance.font_size.reset': 'appearance font-size-reset', 'appearance.zoom.increase': 'appearance zoom-increase', 'appearance.zoom.decrease': 'appearance zoom-decrease', 'appearance.zoom.reset': 'appearance zoom-reset',
    'setting.list': 'setting list', 'setting.get': 'setting get <key>', 'setting.set': 'setting set <key> <value>', 'setting.toggle': 'setting toggle <key>',
    'keybinding.list': 'keybinding list', 'keybinding.get': 'keybinding get <name>', 'file.list': 'file list', 'project.active': 'project active', 'project.list': 'project list', 'drive.list': 'drive list', 'drive.inspect': 'drive inspect <id>', 'drive.workflow.run': 'drive workflow run <id>',
}
implemented_without_reachable_parser = [name for name in implemented_names if name not in reachable]
parser_comparison = {
    'implemented_action_count': len(implemented_names),
    'mapped_reachable_parser_count': len([n for n in implemented_names if n in reachable]),
    'implemented_actions_without_reachable_standalone_cli_command': implemented_without_reachable_parser,
    'justification': 'Compared catalog names from action list --implemented-only against explicit clap command groups in crates/warp_cli/src/local_control/mod.rs. Missing items are catalog-implemented but not exposed as reachable standalone parser commands in this checkout.',
}
(LOGS / 'catalog_to_cli_gap_detection.json').write_text(json.dumps(parser_comparison, indent=2))

counts = {'pass': 0, 'fail': 0, 'skip': 0, 'blocked': 0}
for e in manifest_entries:
    counts[e['status']] = counts.get(e['status'], 0) + 1
visual_blockers = [e['command'] for e in manifest_entries if e['visual_inspection']['status'] == 'blocked' or e['visual_inspection']['unexpected_ui_changes_or_ambiguity']]
blockers = []
if implemented_without_reachable_parser:
    blockers.append(f'{len(implemented_without_reachable_parser)} implemented catalog actions lack a reachable standalone CLI parser command.')
large_output_commands = [e['command'] for e in manifest_entries if e['actual_result']['stdout_lines'] > 44]
if large_output_commands:
    blockers.append('Some parser/catalog outputs are too large to prove full raw output in a single screenshot; complete stdout is stored in logs and screenshots show invocation/status/counts/excerpts.')
if not warp_ready:
    blockers.append('Warp app readiness was ambiguous: no Warp window or discovery record was detected before command capture.')

manifest = {
    'agent_name': 'validation-coordinator',
    'validated_sha': HEAD,
    'validation_ref': 'zach/warpctrl-validation/f61caf49',
    'timestamp_utc': datetime.now(timezone.utc).isoformat(),
    'artifact_root': str(ART.relative_to(ROOT)),
    'builds': {
        'warpctrl': {'command': 'CARGO_BUILD_JOBS=1 cargo build -p warp --bin warpctrl --features standalone,warp_control_cli', 'log': 'logs/build_warpctrl.log', 'status': 'pass'},
        'warp_app': {'command': 'CARGO_BUILD_JOBS=1 cargo build -p warp --bin warp-oss --features gui,warp_control_cli,skip_firebase_anonymous_user', 'log': 'logs/build_warp_app.log', 'status': 'pass'},
    },
    'graphical_environment': {
        'display': DISPLAY,
        'xvfb': 'Xvfb :94 -screen 0 2400x1600x24',
        'window_manager': 'openbox',
        'external_terminal': 'xterm staggered left; Warp app staggered right when window was available',
        'home': str(HOME),
        'xdg_runtime_dir': str(RUNTIME),
        'warp_data_profile': PROFILE,
        'discovery_dir': str(DISCOVERY),
        'warp_ready_detected': warp_ready,
        'setup_screenshot': str(setup_shot.relative_to(ART)),
    },
    'commands': manifest_entries,
    'catalog_to_cli_gap_detection': parser_comparison,
    'counts': counts,
    'visual_inspection_failures_or_blockers': visual_blockers,
    'blockers': blockers,
    'skipped_commands': [],
}
(ART / 'manifest.json').write_text(json.dumps(manifest, indent=2))

summary_lines = [
    '# warpctrl validation summary',
    f'- Validated SHA: `{HEAD}`',
    '- Validation ref: `zach/warpctrl-validation/f61caf49`',
    '- Agent: `validation-coordinator`',
    '- Build status: standalone `warpctrl` pass; graphical `warp-oss` pass',
    f'- Command counts: pass={counts.get("pass",0)}, fail={counts.get("fail",0)}, blocked={counts.get("blocked",0)}, skip={counts.get("skip",0)}',
    f'- Visual-inspection failures/blockers: {len(visual_blockers)}',
]
if visual_blockers:
    summary_lines.append('## Visual inspection blockers')
    summary_lines.extend([f'- `{cmd}`' for cmd in visual_blockers])
if blockers:
    summary_lines.append('## Blockers')
    summary_lines.extend([f'- {b}' for b in blockers])
summary_lines.append('## Catalog-to-CLI gap detection')
summary_lines.append(f'- Implemented catalog actions: {parser_comparison["implemented_action_count"]}')
summary_lines.append(f'- Reachable parser mappings found: {parser_comparison["mapped_reachable_parser_count"]}')
summary_lines.append(f'- Implemented actions without reachable standalone parser commands: {len(implemented_without_reachable_parser)}')
if implemented_without_reachable_parser:
    summary_lines.extend([f'  - `{name}`' for name in implemented_without_reachable_parser])
summary_lines.append('## Skipped commands')
summary_lines.append('- None')
(ART / 'summary.md').write_text('\n'.join(summary_lines) + '\n')

for _, _, fh in procs:
    fh.flush()
print(json.dumps({'counts': counts, 'blockers': blockers, 'visual_blockers': visual_blockers, 'gap_count': len(implemented_without_reachable_parser), 'warp_ready': warp_ready}, indent=2))
