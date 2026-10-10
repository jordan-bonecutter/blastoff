# Blastoff

A Spotlight-inspired application launcher for Linux: a rounded dark panel, real app icons, fuzzy search, and keyboard navigation. Built in Rust with Iced and the wgpu Vulkan renderer. No GTK, Qt, or webview. The translucent panel shows your desktop through it. A working Vulkan driver is required; there is no tiny-skia fallback.

## Run

```sh
cargo run --release --locked
```

Type an app name or abbreviation (`vsc` matches Visual Studio Code). Use **↑/↓** to select and scroll through all matches; navigation stops at the first and last result. Press **Enter** to open the highlighted row, and **Escape** to dismiss. Moving the pointer over a result selects it, so Enter launches that same highlighted app. Arrow keys take over selection when used; scrolling does not reselect a row under a stationary pointer. Click a result to launch it. The window also closes when it loses focus by default. A failed launch shows an error and keeps the launcher open.

Install with `cargo install --path . --locked`. Add these rules to your Sway config, using the full installed binary path in the binding if needed:

```sway
for_window [app_id="blastoff"] floating enable, border none, move position center
bindsym $mod+space exec ~/.cargo/bin/blastoff
```

On Wayland, your compositor controls positioning; the floating rule is required on tiling compositors. Blastoff currently uses a regular window, not the layer-shell protocol. The window supports true alpha transparency and rounded corners. Background blur needs compositor support and is not implemented here. It starts a fresh process for each invocation; it does not register a global shortcut itself.

## Configure

Copy [config/config.toml](config/config.toml) to `~/.config/blastoff/config.toml` (or `$XDG_CONFIG_HOME/blastoff/config.toml`). All fields are optional. Restart the launcher to apply changes.

```toml
width = 680.0
max_results = 7
font_size = 17.0
radius = 20.0
opacity = 0.92
background = "#181b24"
foreground = "#eef0f7"
accent = "#a5b4fc"
icon_theme = "Adwaita"
terminal = ["foot", "-e"]
close_on_unfocus = true
```

Colors use `#RRGGBB`. Width is 400–1400 logical pixels; results 1–12; font size 12–28; corner radius 0–40. `opacity` ranges from 0 to 1 (default 0.92); 1 makes the panel opaque. Width describes the complete window, without an outer margin. `max_results` controls the visible row count, not the total number of matches. `terminal` is an executable and argument array, used for desktop entries with `Terminal=true`.

Use `blastoff --config PATH` to select another config, `blastoff --list` to inspect discovered applications without opening a window, or `blastoff --help` for usage. Invalid configuration reports an error rather than silently discarding your settings.

Applications come from XDG `applications` directories. User entries override system entries, including hidden entries. Desktop visibility, `TryExec`, localized names, terminal apps, and working directories are supported. Commands are launched directly, without a shell. Entries that require D-Bus activation but provide no `Exec` fallback are not supported yet. Icons resolve through the configured theme and its inheritance chain, then hicolor and pixmaps; missing icons get a letter tile. PNG, SVG, JPEG, and ICO artwork are supported. Initial results are alphabetical; search includes names, desktop IDs, keywords, and descriptions. All matches are available in a scrollable list; mouse wheel and keyboard navigation both scroll. Changing the query resets the selection and scroll position.

## Rust dependencies and the OS boundary

Application code, UI, text layout, GPU rendering, SVG rendering, and enabled bitmap decoders use Rust crates. Iced defaults are disabled; the `wgpu-bare` feature enables GPU rendering with only the Vulkan backend, Linux theme detection is disabled, and image codecs are explicitly selected. Iced's Linux window/input/clipboard backends still call system Wayland/X11, `libxkbcommon`, and Vulkan driver libraries. “Rust UI, no GTK” does **not** mean “no native system libraries”: if every dependency must avoid FFI entirely, this implementation does not meet that stricter constraint. The graphics driver is supplied by the OS, not bundled or compiled as an application dependency.

Implementation references: [Iced feature flags](https://docs.rs/crate/iced/0.14.0/features), [Iced window settings](https://docs.rs/iced/0.14.0/iced/window/struct.Settings.html), and the [desktop Exec specification](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).

## Testing — headless only

Run all UI testing in an isolated headless compositor. Never launch test windows in the user's desktop session or inject input into it.

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
./tests/headless-ui.sh
```

The UI harness requires a working Vulkan adapter in addition to `sway` built with the headless wlroots backend, `swaymsg`, `grim`, Python 3, `timeout`, and the system XKB keymap files (usually `xkeyboard-config`). Sway must support the virtual-keyboard and virtual-pointer protocols. These are **test tools**, not application dependencies. It does not require Xvfb, Xwayland, or a physical display. On machines without a GPU, install Mesa Lavapipe (Arch: `vulkan-swrast`), then run:

```sh
VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json ./tests/headless-ui.sh
```

Use the ICD path for your distro if it differs. Lavapipe emulates the Vulkan adapter for tests; the application still exercises wgpu, never tiny-skia. On a desktop with a supported GPU, wgpu uses the normal Vulkan driver.

The script builds Blastoff and test-only Rust virtual-keyboard/pointer, backdrop, and screenshot-check helpers, creates an isolated `/tmp/blastoff-ui.*` directory, replaces the XDG data/config/runtime directories, unsets the existing display/socket variables, and starts Sway with `WLR_BACKENDS=headless` and `WLR_RENDERER=pixman`. The compositor uses pixman while the launcher and backdrop use wgpu/Vulkan. It supplies deterministic desktop entries whose launch commands only create marker files. The input helper refuses to run unless `SWAYSOCK` belongs to that test directory.

Checks cover native Wayland window rendering, automatic search focus, Up/Down navigation with clamped ends, scrolling beyond the visible rows (both apps and search results), query scroll reset, and Enter launch, fuzzy search, Enter with no matches, Escape dismissal, and launch errors. A blue backdrop and pixel checks verify actual alpha blending, transparent rounded corners, and absence of black margins or a panel outline. Tests also verify Enter launches the hovered row and keyboard scrolling retains selection with a stationary pointer. The script exits nonzero on failure, limits the compositor to 90 seconds, and prints the artifact directory. Inspect `result`, `sway.log`, `launcher.log`, `input.log`, `backdrop.log`, and these screenshots:

- `01-initial.png`: icon rows and selection.
- `00-down.png`: selection after keyboard navigation.
- `02-fuzzy.png`: the narrowed Gamma result.
- `03-no-matches.png`: the empty state.
- `04-launch-error.png`: the inline launch error.
- `05-scroll-bottom.png`: the last app beyond the initial visible rows.
- `06-scroll-top.png`: navigation back to the first app.
- `07-search-scroll.png`: the last match in a long search result list.
- `08-query-reset.png`: a new query resets the scrolled viewport.
- `09-hover-selection.png`: hovering Beta selects the target launched by Enter.
- `10-keyboard-after-hover.png`: keyboard navigation scrolls to Gamma with the pointer stationary.

Open screenshots with a file/image viewer; do not rerun the application on a live desktop for visual review. Artifacts are kept for inspection. The harness also requires distinct screenshots across states to catch stale rendering. Functional checks assert behavior; screenshot appearance needs visual inspection. This harness covers Wayland; the X11 backend is compiled but is not exercised by it. See [AGENTS.md](AGENTS.md) for persistent agent instructions.
