# Repository Guidelines

## Project Structure & Module Organization
- `bios/`: Macroquad UI; config in `src/config.rs`, RA launch flow in `src/utils.rs`.
- `overlay/`: Overlay daemon; IPC/rendering/hotkeys in `src/ipc.rs`, `rendering.rs`, `hotkeys.rs`; themes/assets in `assets/`.
- `input-daemon/`: Linux-only evdev hotkey watcher (inotify-driven).
- `ra/`: RetroAchievements library + CLI (`kazeta-ra`) for hashing/API/cache.
- `rootfs/`: Systemd units, polkit rules, udev/session files; helpers: `dev-run.sh`, `build-image.sh`, `upgrade-to-plus.sh`, `Dockerfile*`, `run-bios-docker.sh`.

## Architecture Overview (from `ARCHITECTURE_OVERLAY.md`)
- Multi-process: BIOS and overlay never run together; `kazeta-session` launches BIOS, then game → overlay + input daemon → back to BIOS on exit.
- IPC: JSON over `/tmp/kazeta-overlay.sock`; key messages `show_overlay`, `hide_overlay`, `show_toast`, `game_started`, `ra_unlock`.
- Optimizations: Overlay idles ~20 FPS when hidden; input daemon is event-driven; RA hashing streams ROMs (1MB buffer, 8KB chunks).

## Build, Test, and Development Commands
- Fast loop: `./dev-run.sh` builds debug overlay/input/bios and starts them; cleans `/tmp/kazeta-overlay.sock`.
- Builds: `cargo build --features dev` (bios), `cargo build --features daemon` (overlay), `cargo build` (input), `cargo build --release` (ra/cli); add `--release` for production.
- Quality: `cargo fmt --all` then `cargo clippy --all-targets --all-features`.
- Packaging: `./build-image.sh` (container tools) or use `Dockerfile*` for containerized runs.

## Coding Style & Naming Conventions
- Rust defaults: 4-space indent, snake_case functions/modules, CamelCase types, SCREAMING_SNAKE_CASE consts.
- Keep IPC schemas consistent between `overlay/src/ipc.rs` and callers; document protocol tweaks inline.
- Prefer non-blocking/async paths in daemons; avoid long blocking calls on render/input threads.
- Run `cargo fmt` after edits; scope `#[allow]` narrowly when silencing clippy.

## Testing Guidelines
- `cargo test` per crate (`bios/`, `overlay/`, `input-daemon/`, `ra/`).
- Overlay manual: see `overlay/TESTING.md`; `cargo run --features daemon`, toggle via Guide/F12/Ctrl+O, send JSON via `nc -U /tmp/kazeta-overlay.sock`.
- Input checks: `overlay/test_controller_input.sh`; multi-device via `test-multiplayer.sh`.
- RA flows: `kazeta-ra status`, `hash-rom --path ROM --console <id>`, `send-achievements-to-overlay` for IPC validation.

## Commit & Pull Request Guidelines
- Commit style matches history: prefixes like `feat:`, `refactor:`, `docs:`; imperative subjects ~72 chars.
- PRs: summary, key changes, tests run (`cargo test`, `cargo clippy`), screenshots/gifs for UI, linked issues/wiki for IPC/runtime/config changes.
- Keep formatting changes with related code; avoid mixed-format-only commits.

## Security & Configuration Tips
- Treat `rootfs/` as production config; keep permissions and unit names intact unless intentionally changed.
- No embedded secrets; RA API keys must come from user/env.
- IPC socket path stays `/tmp/kazeta-overlay.sock`; clean stale sockets instead of moving it.
- When altering build scripts, confirm hashes against `sha256sum.txt` before publishing artifacts.
