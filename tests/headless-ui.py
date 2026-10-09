#!/usr/bin/env python3
"""Run only inside the isolated compositor started by headless-ui.sh."""
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(os.environ['BLASTOFF_TEST_DIR'])
binary = os.environ['BLASTOFF_BINARY']


def run(*args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        raise RuntimeError(f'{args}: {result.stderr}')
    return result.stdout.strip()


def wait_for(predicate, label, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise AssertionError('Timed out: ' + label)


def windows():
    tree = json.loads(run('swaymsg', '-t', 'get_tree', '-r'))
    def find(node):
        return node.get('app_id') == 'blastoff' or any(find(n) for n in node.get('nodes', []) + node.get('floating_nodes', []))
    return find(tree)


def start():
    env = os.environ.copy()
    env.pop('DISPLAY', None)
    return subprocess.Popen([binary, '--config', str(root / 'config.toml')], env=env,
                            stdout=open(root / 'launcher.log', 'a'), stderr=subprocess.STDOUT)


def key(*keys):
    keyboard.stdin.write(' '.join(keys) + '\n')
    keyboard.stdin.flush()
    assert keyboard.stdout.readline().strip() == 'DONE', 'virtual keyboard command failed'
    time.sleep(0.2)


def type_text(value):
    key('text:' + value)
    time.sleep(0.3)


def capture(name):
    time.sleep(0.4)
    run('grim', str(root / (name + '.png')))


try:
    assert os.environ.get('SWAYSOCK', '').startswith(str(root) + '/')
    keyboard = subprocess.Popen([os.environ['BLASTOFF_INPUT']], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=open(root / 'input.log', 'w'), text=True)
    assert keyboard.stdout.readline().strip() == 'READY', 'virtual keyboard initialization failed'
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'Wayland launcher window')
    capture('01-initial')
    assert proc.poll() is None
    # Down selects Beta; launching proves navigation and initial input focus.
    key('Up', 'Down', 'Down')
    capture('00-down')
    key('Return')
    wait_for(lambda: (root / 'launched-beta').exists(), 'Beta launch from Down/Enter')
    wait_for(lambda: proc.poll() is not None, 'launcher exit after launch')
    assert proc.returncode == 0

    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(lambda: windows(), 'search window')
    type_text('gma')
    capture('02-fuzzy')
    key('Return')
    wait_for(lambda: (root / 'launched-gamma').exists(), 'fuzzy Gamma launch')
    wait_for(lambda: proc.poll() is not None, 'search launcher exit')
    assert proc.returncode == 0

    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(lambda: windows(), 'empty state window')
    type_text('zzzzzz')
    capture('03-no-matches')
    key('Return')
    assert proc.poll() is None, 'Enter on empty results must keep launcher open'
    key('Escape')
    wait_for(lambda: proc.poll() is not None, 'Escape dismissal')
    assert proc.returncode == 0

    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(lambda: windows(), 'error state window')
    type_text('broken')
    key('Return')
    capture('04-launch-error')
    assert proc.poll() is None, 'failed launch must keep launcher open'
    key('Escape')
    wait_for(lambda: proc.poll() is not None, 'error window dismissal')

    images = [(root / (name + '.png')).read_bytes() for name in ['01-initial', '02-fuzzy', '03-no-matches', '04-launch-error']]
    assert (root / '00-down.png').read_bytes() != images[0], 'selection must visibly move'
    assert all(len(data) > 5000 for data in images), 'screenshots must contain rendered UI'
    assert len(set(images)) == 4, 'UI screenshots must change with the application state'
    (root / 'result').write_text('PASS: native Wayland rendering, navigation, fuzzy search, launch, empty results, Escape, launch error.\n')
except Exception:
    import traceback
    (root / 'result').write_text(traceback.format_exc())
finally:
    try:
        if 'keyboard' in globals():
            keyboard.stdin.close()
            keyboard.wait(timeout=5)
    finally:
        subprocess.run(['swaymsg', 'exit'], capture_output=True)
