# Repository instructions

Blastoff is a Rust Linux application launcher. Preserve the no-GTK/no-Qt/no-webview design and the tiny-skia software renderer. Keep image decoders in Rust; do not enable Iced defaults or wgpu without discussing the dependency change. See README.md for the current system-library boundary.

## UI testing must be headless

The user explicitly requires all UI testing to run headlessly. Never run the launcher, a GUI test, or synthetic keyboard/mouse input against the user's current Wayland/X11 session. Do not assume the ambient DISPLAY, WAYLAND_DISPLAY, or SWAYSOCK is safe.

Run `./tests/headless-ui.sh`. It starts a dedicated Sway headless compositor with pixman, an isolated runtime/socket and XDG fixtures, and a guarded Rust virtual keyboard. Read the printed `/tmp/blastoff-ui.*` artifact path and inspect its screenshots with an image-viewing tool. Logs and the `result` file are kept there. See README.md's testing section for prerequisites and exact commands. Do not replace this with a live-session smoke test if a prerequisite is missing; install/use a headless test prerequisite or report the blocker.

For behavior changes, run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and the headless UI harness. Add meaningful fixture/assertion coverage when changing navigation, launch behavior, desktop parsing, or error handling. Update the harness and README together if the test procedure changes.

Application commands must be passed as arguments to `std::process::Command`, never to a shell. Preserve XDG root priority and hidden desktop-entry masking. Keep test apps harmless and deterministic; they may create marker files only within the isolated artifact directory.
