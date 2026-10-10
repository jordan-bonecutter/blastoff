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


def start(*args, config="config.toml"):
    env = os.environ.copy()
    env.pop('DISPLAY', None)
    return subprocess.Popen([binary, '--config', str(root / config), *args], env=env,
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
    subprocess.run(['wl-copy', 'gma'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
    # Repeat startup with a populated clipboard to expose connection-lifetime races.
    for attempt in range(12):
        initially_inside = attempt % 2 == 1
        key('pointer:420,320' if initially_inside else 'pointer:0,0')
        wait_for(lambda: not windows(), 'clipboard stress previous window removal')
        proc = start('--button-launch')
        wait_for(lambda: ((node := windows()) and node['window_rect']['width'] == 1280) or proc.poll() is not None,
                 'clipboard stress overlay')
        assert proc.poll() is None, 'button-launch failed with clipboard populated'
        if attempt == 0:
            time.sleep(0.4)
            run('grim', str(root / '12-clipboard-startup.png'))
            run(os.environ['BLASTOFF_CAPTURE_CHECK'], str(root / '12-clipboard-startup.png'), '300', '290', '680', '380')
        time.sleep(0.2)
        assert proc.poll() is None, 'startup with pointer already inside must stay open'
        if initially_inside:
            key('pointer:20,20')
        else:
            key('Escape')
        wait_for(lambda: proc.poll() is not None, 'clipboard stress dismissal')
        assert proc.returncode == 0
        log = (root / 'launcher.log').read_text()
        assert 'not a valid new object id' not in log and 'Invalid argument' not in log, 'Wayland clipboard protocol error'

    wait_for(lambda: not windows(), 'previous window removal')
    key('pointer:0,0')
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

    subprocess.run(['wl-copy', 'gma'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)

    # Button launches use a transparent screen-sized input surface. Moving
    # outside before first entry (and the opening click's release) cannot close it.
    (root / 'dismiss.toml').write_text('close_on_unfocus = true\nmax_results = 3\nicon_theme = "hicolor"\n')
    for action in ['outside-click', 'enter-exit', 'escape', 'inside-click', 'search-launch', 'app-click', 'focus-switch']:
        wait_for(lambda: not windows(), 'previous window removal')
        key('pointer:0,0', 'pointer-press')
        proc = start('--button-launch', config='dismiss.toml')
        wait_for(lambda: (node := windows()) and node['window_rect']['width'] == 1280,
                 'button-launch overlay')
        time.sleep(0.5)
        key('pointer-release', 'pointer:20,20')
        assert proc.poll() is None, 'initial outside pointer motion must not dismiss'
        if action == 'outside-click':
            # Capture and check the centered panel against the unchanged backdrop.
            run('grim', str(root / '11-button-launch.png'))
            run(os.environ['BLASTOFF_CAPTURE_CHECK'], str(root / '11-button-launch.png'),
                '300', '290', '680', '380')
            key('click')
        elif action == 'enter-exit':
            key('pointer:420,320')
            assert proc.poll() is None, 'pointer entry must keep launcher open'
            key('pointer:20,20')
        elif action == 'escape':
            key('Escape')
        elif action == 'search-launch':
            (root / 'launched-gamma').unlink()
            key('Paste', 'Return')
            wait_for(lambda: (root / 'launched-gamma').exists(), 'button-launch clipboard paste focus and Enter')
        elif action == 'focus-switch':
            key('pointer:420,320')
            run('swaymsg', '[app_id="blastoff-test-background"] focus')
            time.sleep(0.3)
            assert proc.poll() is None, 'focus switching with stationary pointer must keep launcher open'
            run('swaymsg', f'[pid="{proc.pid}"] focus')
            key('Escape')
        elif action == 'app-click':
            (root / 'launched-alpha').unlink()
            key('pointer:420,472', 'click')
            wait_for(lambda: (root / 'launched-alpha').exists(), 'button-launch click app')
        else:
            key('pointer:420,320', 'click')
            assert proc.poll() is None, 'clicking search must keep launcher open'
            key('Escape')
        wait_for(lambda: proc.poll() is not None, 'button-launch ' + action)
        assert proc.returncode == 0

    assert 'not a valid new object id' not in (root / 'launcher.log').read_text(), 'Wayland clipboard protocol error'
    assert 'Invalid argument' not in (root / 'launcher.log').read_text(), 'Wayland connection failure'

    # Default mode still respects close_on_unfocus, including pointer focus changes.
    wait_for(lambda: not windows(), 'previous window removal')
    proc = start(config='dismiss.toml')
    wait_for(windows, 'default dismissal window')
    key('pointer:420,320')
    assert proc.poll() is None
    key('pointer:20,20')
    wait_for(lambda: proc.poll() is not None, 'default focus-loss dismissal')
    assert proc.returncode == 0

    images = [(root / (name + '.png')).read_bytes() for name in ['01-initial', '02-fuzzy', '03-no-matches', '04-launch-error']]
    assert (root / '00-down.png').read_bytes() != images[0], 'selection must visibly move'
    assert all(len(data) > 5000 for data in images), 'screenshots must contain rendered UI'
    assert len(set(images)) == 4, 'UI screenshots must change with the application state'
    (root / 'result').write_text('PASS: clipboard startup stress inside/outside, paste, focus switch, button-launch outside click, enter/exit, Escape, inside click, search/Enter and app click, default focus dismissal, native Wayland rendering, borderless alpha pixels, hover/Enter selection, keyboard after hover, scrolling all apps and search matches, clamped navigation, query reset, fuzzy search, launch, empty results, Escape, launch error.\n')
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
