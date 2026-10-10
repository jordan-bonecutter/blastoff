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
        if node.get('app_id') == 'blastoff':
            return node
        for child in node.get('nodes', []) + node.get('floating_nodes', []):
            found = find(child)
            if found:
                return found
        return None
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
    path = str(root / (name + '.png'))
    run('grim', path)
    node = windows()
    rect = node['rect']
    content = node['window_rect']
    run(os.environ['BLASTOFF_CAPTURE_CHECK'], path, str(rect['x'] + content['x']), str(rect['y'] + content['y']), str(content['width']), str(content['height']))


try:
    assert os.environ.get('SWAYSOCK', '').startswith(str(root) + '/')
    keyboard = subprocess.Popen([os.environ['BLASTOFF_INPUT']], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=open(root / 'input.log', 'w'), text=True)
    assert keyboard.stdout.readline().strip() == 'READY', 'virtual keyboard initialization failed'
    backdrop = subprocess.Popen([os.environ['BLASTOFF_BACKDROP']], stdout=open(root / 'backdrop.log', 'w'), stderr=subprocess.STDOUT)
    time.sleep(2)
    assert backdrop.poll() is None, 'headless backdrop failed to start'
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'Wayland launcher window')
    capture('01-initial')
    assert proc.poll() is None
    # Down selects Beta; launching proves navigation and initial input focus.
    key('Up', 'Down')
    capture('00-down')
    key('Return')
    wait_for(lambda: (root / 'launched-beta').exists(), 'Beta launch from Down/Enter')
    wait_for(lambda: proc.poll() is not None, 'launcher exit after launch')
    assert proc.returncode == 0

    # More entries than visible rows must remain accessible, with clamped ends.
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'scrolling window')
    key(*(['Down'] * 20))
    capture('05-scroll-bottom')
    key('Down', 'Return')
    wait_for(lambda: (root / 'launched-omega').exists(), 'last app beyond visible rows, without wrapping')
    wait_for(lambda: proc.poll() is not None, 'bottom launcher exit')

    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'scroll reset window')
    key(*(['Down'] * 20))
    key(*(['Up'] * 20))
    capture('06-scroll-top')
    key('Up', 'Return')
    wait_for(lambda: (root / 'launched-alpha').exists(), 'scroll back to first app, without wrapping')
    wait_for(lambda: proc.poll() is not None, 'top launcher exit')

    (root / 'launched-omega').unlink()
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'scrolling search window')
    type_text('app')
    key(*(['Down'] * 20))
    capture('07-search-scroll')
    key('Return')
    wait_for(lambda: (root / 'launched-epsilon').exists(), 'last search match beyond visible rows')
    wait_for(lambda: proc.poll() is not None, 'search scrolling launcher exit')

    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'query reset window')
    key(*(['Down'] * 20))
    type_text('gma')
    capture('08-query-reset')
    key('Return')
    wait_for(lambda: (root / 'launched-gamma').exists(), 'query resets scrolled selection and viewport')
    wait_for(lambda: proc.poll() is not None, 'query reset launcher exit')
    (root / 'launched-gamma').unlink()

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

    # Pointer selection and Enter must share one launch target.
    (root / 'launched-beta').unlink()
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'hover launch window')
    node = windows()
    rect, content = node['rect'], node['window_rect']
    x = rect['x'] + content['x'] + 120
    y = rect['y'] + content['y'] + 150 + 70 + 33
    key(f'pointer:{x},{y}')
    capture('09-hover-selection')
    key('Return')
    wait_for(lambda: (root / 'launched-beta').exists(), 'Enter launches hovered Beta rather than initial Alpha')
    wait_for(lambda: proc.poll() is not None, 'hover launcher exit')
    assert proc.returncode == 0

    key('pointer:0,0')
    (root / 'launched-gamma').unlink()
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start()
    wait_for(windows, 'keyboard after hover window')
    node = windows()
    rect, content = node['rect'], node['window_rect']
    x = rect['x'] + content['x'] + 120
    y = rect['y'] + content['y'] + 150 + 33
    key(f'pointer:{x},{y}')
    # Gamma is seventh: scrolling must not reselect a row beneath the stationary pointer.
    key(*(['Down'] * 6))
    capture('10-keyboard-after-hover')
    key('Return')
    wait_for(lambda: (root / 'launched-gamma').exists(), 'keyboard scrolling retains selection under stationary pointer')
    wait_for(lambda: proc.poll() is not None, 'keyboard after hover launcher exit')
    assert proc.returncode == 0
    key('pointer:0,0')

    images = [(root / (name + '.png')).read_bytes() for name in ['01-initial', '02-fuzzy', '03-no-matches', '04-launch-error']]
    assert (root / '00-down.png').read_bytes() != images[0], 'selection must visibly move'
    assert all(len(data) > 5000 for data in images), 'screenshots must contain rendered UI'
    assert len(set(images)) == 4, 'UI screenshots must change with the application state'
    (root / 'result').write_text('PASS: native Wayland rendering, borderless alpha pixels, hover/Enter selection, keyboard after hover, scrolling all apps and search matches, clamped navigation, query reset, fuzzy search, launch, empty results, Escape, launch error.\n')
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
