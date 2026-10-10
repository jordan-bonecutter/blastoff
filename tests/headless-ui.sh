#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for tool in sway swaymsg grim wl-copy python3 timeout; do
    command -v "$tool" >/dev/null || { echo "Missing test dependency: $tool" >&2; exit 1; }
done
cargo build --locked --bin blastoff --example headless-input --example check-capture --example headless-backdrop
export BLASTOFF_BACKDROP="$(pwd)/target/debug/examples/headless-backdrop"
export BLASTOFF_CAPTURE_CHECK="$(pwd)/target/debug/examples/check-capture"
export BLASTOFF_INPUT="$(pwd)/target/debug/examples/headless-input"
export BLASTOFF_BINARY="$(pwd)/target/debug/blastoff"
export BLASTOFF_TEST_DIR="$(mktemp -d /tmp/blastoff-ui.XXXXXX)"
export XDG_RUNTIME_DIR="$BLASTOFF_TEST_DIR/runtime"
mkdir -m 700 "$XDG_RUNTIME_DIR"
export XDG_DATA_HOME="$BLASTOFF_TEST_DIR/data"
export XDG_DATA_DIRS="$BLASTOFF_TEST_DIR/empty"
export XDG_CONFIG_HOME="$BLASTOFF_TEST_DIR/config"
mkdir -p "$XDG_DATA_HOME/applications" "$XDG_DATA_HOME/icons/hicolor/scalable/apps" "$XDG_DATA_DIRS" "$XDG_CONFIG_HOME"
cat > "$XDG_DATA_HOME/icons/hicolor/scalable/apps/fixture.svg" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><rect width="48" height="48" rx="12" fill="#a5b4fc"/><path d="M14 32L24 12L34 32H14Z" fill="#181b24"/></svg>
SVG
for name in Alpha Beta Gamma Delta Epsilon Eta Iota Kappa Lambda Omega; do
    lower="${name,,}"
    cat > "$XDG_DATA_HOME/applications/$lower.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$name
Comment=Headless test application
Icon=fixture
Exec=/usr/bin/touch $BLASTOFF_TEST_DIR/launched-$lower
DESKTOP
done
cat > "$XDG_DATA_HOME/applications/broken.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Broken
Comment=Exercises launch errors
Exec=/nonexistent/blastoff-test
DESKTOP
cat > "$BLASTOFF_TEST_DIR/config.toml" <<'CONFIG'
close_on_unfocus = false
icon_theme = "hicolor"
max_results = 3
CONFIG
cat > "$BLASTOFF_TEST_DIR/sway.conf" <<CONFIG
xwayland disable
output * resolution 1280x960
seat seat0 fallback true
for_window [app_id="blastoff-test-background"] floating enable, border none, resize set 1280 960, move position 0 0
for_window [app_id="blastoff"] floating enable, border none, move position center
for_window [class="blastoff"] floating enable, border none, move position center
for_window [title="Blastoff"] floating enable, border none, move position center
exec python3 "$(pwd)/tests/headless-ui.py"
CONFIG
unset DISPLAY WAYLAND_DISPLAY WAYLAND_SOCKET SWAYSOCK
export ICED_BACKEND=wgpu WGPU_BACKEND=vulkan
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1
export SWAYSOCK="$BLASTOFF_TEST_DIR/sway.sock"
printf 'Headless UI artifacts: %s\n' "$BLASTOFF_TEST_DIR"
# timeout also stops a compositor if the driver fails to start.
timeout 120s sway -c "$BLASTOFF_TEST_DIR/sway.conf" > "$BLASTOFF_TEST_DIR/sway.log" 2>&1 || {
    cat "$BLASTOFF_TEST_DIR/sway.log" >&2
    exit 1
}
cat "$BLASTOFF_TEST_DIR/result"
[[ "$(head -c 5 "$BLASTOFF_TEST_DIR/result")" == 'PASS:' ]]
