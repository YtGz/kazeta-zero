# Repository Guidelines

## Project Structure & Module Organization
- `bios/`: Macroquad UI; config in `src/config.rs`, RA launch flow in `src/utils.rs`.
- `overlay/`: Overlay daemon; IPC/rendering/hotkeys in `src/ipc.rs`, `rendering.rs`, `hotkeys.rs`; themes/assets in `assets/`.
- `input-daemon/`: Linux-only evdev hotkey watcher (inotify-driven).
- `ra/`: RetroAchievements library + CLI (`kazeta-ra`) for hashing/API/cache.
- `rootfs/`: Systemd units, polkit rules, udev/session files; helpers: `dev-run.sh`, `build-image.sh`, `upgrade-to-zero.sh`, `Dockerfile*`, `run-bios-docker.sh`.

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

## Upstream Sync

Kazeta Zero is a fork of a fork of a fork. The chain is:

```
kazetaos/kazeta  →  the-outcaster/kazeta-plus  →  goldsziggy/kazeta-plus-plus  →  YtGz/kazeta-zero
```

GitHub forks don't auto-sync from their parents, so each link diverges. To pull in upstream changes from all three repos:

### One-time setup

```bash
git remote add kazeta https://github.com/kazetaos/kazeta.git
git remote add kazeta-plus https://github.com/the-outcaster/kazeta-plus.git
# upstream (kazeta-plus-plus) is already configured
```

### Sync procedure (run periodically)

Merge in **chain order** (original → middle fork → direct parent) so dependencies cascade correctly. `kazeta-plus` changes may build on `kazeta` changes, and `upstream` changes may build on `kazeta-plus` changes:

```bash
git fetch --all

# 1. Original Kazeta (optical media, boot fixes, Docker changes)
git merge kazeta/main --allow-unrelated-histories

# 2. Kazeta+ (Dreamcast runtime, controller configs, upgrade script fixes)
git merge kazeta-plus/main --allow-unrelated-histories

# 3. Kazeta++ (fullscreen, compositor, service cleanup)
git merge upstream/main --allow-unrelated-histories
```

Resolve conflicts keeping Kazeta Zero versions (rebrand changes take priority). Push to origin when done:

```bash
git push origin main
```

### Package decisions (do not revert during sync)

Kazeta Zero diverges from upstream on several architectural choices. These are
deliberate decisions — do not revert them when merging upstream changes:

**Adopted from original kazeta (kazetaos/kazeta):**
- **greetd** instead of lightdm (Wayland-only, no X11 display manager)
- **systemd-networkd + systemd-resolved** instead of NetworkManager + iwd
- **gamescope as compositor** (no picom, no xorg-server, no kazeta-compositor)
- **Xbox Series controller emulation** (not Xbox 360)
- **Optical media automount** support (udev rules for sr0 devices)

**Kept from kazeta-plus (the-outcaster/kazeta-plus) — fork features:**
- **bluez / bluez-utils** — Bluetooth controller support (BIOS has Bluetooth UI)
- **keyd** — Steam Deck volume/brightness button mapping
- **gamemode** — CPU governor optimization for games
- **steam** — Steam game support as cartridges

**Removed (not needed):**
- ~~mangohud / lib32-mangohud~~ — overlay daemon does its own performance monitoring
- ~~xorg-server / picom / kazeta-compositor~~ — gamescope handles compositing
- ~~lightdm / accountsservice~~ — replaced by greetd
- ~~networkmanager / iwd / fuse-overlayfs~~ — replaced by systemd-networkd
- ~~clang~~ — build-time only, not needed at runtime

When merging upstream, if a conflict touches any of these packages or
architectural choices, keep the Kazeta Zero version.
