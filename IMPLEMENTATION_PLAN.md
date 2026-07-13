# Kazeta++ Offline GameCube Achievements — Implementation Plan

## Project Goal

Transform this fork of kazeta-plus-plus into a system that provides **fully
offline, no-account, no-tracking RetroAchievements for GameCube games** running
through a standalone Dolphin runtime.

The target machine will have **no internet connection and no RetroAchievements
account**. Achievement definitions are fetched once on a separate
internet-connected machine, baked into each cartridge as a local data file, and
evaluated entirely on-device using the rcheevos engine. Unlocks are stored in a
local SQLite database and never transmitted anywhere.

## Background & Constraints

### Why standard RetroAchievements doesn't fit

RetroAchievements.org requires an account, at least one online session to
authenticate + identify the game + download the achievement set, and syncs
unlocks upstream when connectivity returns. This is incompatible with the
"no internet, no tracking" requirement.

### What makes this possible

1. **rcheevos** (the C library by RetroAchievements) evaluates achievement
   conditions client-side by reading emulator memory each frame. The evaluation
   engine works fully offline — it just needs the condition definitions and
   memory access.

2. The RetroAchievements API response (`API_GetGameInfoExtended.php` /
   `API_GetGameInfoAndUserProgress.php`) contains a `MemAddr` field for each
   achievement. Despite the misleading name, this is the **full achievement
   logic** — the condition string that rcheevos parses via `rc_parse_trigger()`
   (e.g. `0xH00801234=1.0.5.0=d0xH00801234_P:0xH00805678=2`). It includes
   memory addresses, hit counts, pause conditions, delta comparisons, etc.

3. This data can be fetched **once** on an internet-connected machine with a
   (throwaway) RA account, exported to a local JSON file, and baked into each
   cartridge's SD card. The playing machine loads it from the file instead of
   the API.

4. RetroAchievements.org **does** support GameCube (console ID 16) and Wii
   (console ID 19). Achievement sets exist for many GameCube titles, e.g.
   Mario Kart: Double Dash is game ID 7693.

5. Standalone Dolphin (>= version 2407-68) has built-in RetroAchievements
   support and can hash/identify RVZ and ISO files natively using rcheevos'
   `rc_hash_gamecube()` function.

### What this fork does NOT do

- Does not contact retroachievements.org from the playing machine
- Does not require an RA account on the playing machine
- Does not upload, sync, or report unlocks anywhere
- Does not use leaderboards (those are inherently online)

## Target Games (Cartridge Set)

Seven GameCube games, all using the `dolphin` runtime:

| Cartridge ID                  | Game                          | RA Game ID |
|-------------------------------|-------------------------------|------------|
| `mario-kart-double-dash`      | Mario Kart: Double Dash       | 7693       |
| `mario-party-4`               | Mario Party 4                 | (lookup)   |
| `chronicles-of-narnia`        | The Chronicles of Narnia      | (lookup)   |
| `nhl-2004`                    | NHL 2004                      | (lookup)   |
| `evolution-skateboarding`     | Evolution Skateboarding       | (lookup)   |
| `dbz-budokai-2`               | Dragon Ball Z Budokai 2       | (lookup)   |
| `ssx-on-tour`                 | SSX on Tour                   | (lookup)   |

Cartridge definitions (cart.kzi, icon.png, flash scripts) already exist in a
separate repo: `https://github.com/YtGz/kazeta-gamecube` (private), cloned at
`~/code/kazeta/gamecube`.

## Architecture: Two-Machine Model

```
┌─────────────────────────┐         ┌──────────────────────────────────┐
│  PREP MACHINE           │         │  PLAYING MACHINE (Kazeta++ fork)  │
│  (one-time, internet)   │         │  (never online, no account)       │
│                         │         │                                   │
│  1. RA account (throw-  │         │  ┌─────────────────────────────┐  │
│     away ok)            │         │  │ Cartridge SD card           │  │
│  2. Fetch achievement   │  ────┐  │  │  ├── cart.kzi              │  │
│     definitions via API │      │  │  │  ├── icon.png              │  │
│  3. Export to local     │      ├──>  │  ├── game.rvz               │  │
│     .ra-definitions     │      │  │  │  ├── achievements.json     │  │
│     JSON file           │      │  │  │  │   (definitions)        │  │
│  4. Copy to SD card     │      │  │  │  └── badges/               │  │
│                         │      │  │  │      (icon images)        │  │
└─────────────────────────┘      │  │  └─────────────────────────────┘  │
                                  │  │                                   │
                                  │  │  kazeta-ra loads definitions     │
                                  │  │  from achievements.json           │
                                  │  │  instead of API                   │
                                  │  │                                   │
                                  │  │  rcheevos evaluates conditions   │
                                  │  │  against Dolphin memory each frame│
                                  │  │                                   │
                                  │  │  Unlocks stored in local SQLite  │
                                  │  │  Never synced anywhere           │
                                  │  └──────────────────────────────────┘
```

## Implementation Tasks

### Task 1: Add GameCube/Wii to the ConsoleId enum

**Files:** `ra/src/types.rs`

**What to do:**

1. Add two variants to the `ConsoleId` enum:
   ```rust
   GameCube = 16,
   Wii = 19,
   ```

2. Add string mappings to `from_str()`:
   ```rust
   "gamecube" | "gc" | "ngc" => Some(Self::GameCube),
   "wii" => Some(Self::Wii),
   ```

3. Add string mappings to `to_string()`:
   ```rust
   Self::GameCube => "gamecube".to_string(),
   Self::Wii => "wii".to_string(),
   ```

**Difficulty:** Trivial (~15 lines)

**Test:** `cargo test -p kazeta-ra` should still pass. Add a test verifying
`ConsoleId::from_str("gamecube") == Some(ConsoleId::GameCube)` and that
`ConsoleId::GameCube.as_u32() == 16`.

---

### Task 2: Add GameCube disc hashing via rcheevos FFI

**Files:** `ra/src/hash.rs`, `ra/Cargo.toml`, new file `ra/src/rcheevos_ffi.rs`

**What to do:**

The current `hash.rs` uses pure-Rust MD5 for all consoles. GameCube requires
partition-aware hashing (not full-file MD5), and must handle compressed RVZ
files. The official rcheevos C library has `rc_hash_gamecube()` in
`hash_disc.c` which does this correctly, including RVZ decompression.

1. Add rcheevos as a C dependency. Options:
   - **Option A (recommended):** Add rcheevos as a git submodule under
     `vendor/rcheevos/` and build it via a `build.rs` script using the `cc`
     crate. Only compile the `rhash/` subset (md5.c, hash.c, hash_disc.c,
     cdreader.c) — not the full library.
   - **Option B:** Vendor the relevant C files directly into `vendor/rcheevos/`
     (copy from https://github.com/RetroAchievements/rcheevos). Simpler but
     harder to update.

2. Create `ra/src/rcheevos_ffi.rs` with FFI bindings:
   ```rust
   use std::ffi::CString;
   use std::os::raw::c_char;

   #[link(name = "rcheevos", kind = "static")]
   extern "C" {
       fn rc_hash_gamecube(hash: *mut c_char, iterator: *const ...) -> i32;
   }
   ```
   The rcheevos hash iterator API (`rc_hash_iterator_t`) needs a filereader
   callback. You'll need to implement a Rust filereader that bridges to
   rcheevos' `rc_hash_filereader_t` interface. See rcheevos' `rc_hash.h` for
   the callback signatures.

3. Add a `hash_gamecube_rom()` function in `hash.rs` that:
   - Calls the FFI `rc_hash_gamecube()` (which handles RVZ/ISO/GCM and the
     partition-aware DOL+header hash)
   - Returns the MD5 hex string

4. Update the `hash_rom()` match to route `ConsoleId::GameCube` and
   `ConsoleId::Wii` to the new function.

5. Update `detect_console()` to recognize `.iso`, `.gcm`, `.rvz`, `.nkit.iso`
   extensions as potential GameCube/Wii discs (check magic bytes at offset
   0x1c for GameCube: `0xC2 0x33 0x9F 0x3D`).

**Reference — how rcheevos hashes GameCube (from hash_disc.c):**
- Check magic word `0xC2339F3D` at offset 0x1c
- Parse apploader header/body/trailer sizes
- MD5 the partition header block (up to 1MB)
- Read boot DOL offset, parse 18 DOL segment offsets+sizes (7 code + 11 data)
- MD5 each of the 18 segments in sequence
- Finalize → 32-char hex hash

**Difficulty:** Medium. The FFI + build.rs + filereader bridge is the most
complex part. RVZ support comes free from rcheevos if you use its filereader
(it decompresses transparently).

**Test:** Hash a known GameCube ISO and compare against the hash returned by
standalone Dolphin's RA integration for the same file. They must match.

---

### Task 3: Build a standalone-Dolphin runtime (.kzr)

**What to do:**

The current `dolphin-1.0.kzr` runtime uses the RetroArch libretro Dolphin core
(`dolphin_libretro.so`), which is NOT supported by RetroAchievements for
GameCube. Standalone Dolphin (>= 2407-68) has built-in RA support and is the
only RA-supported path for GameCube.

A `.kzr` file is an EROFS filesystem image containing:
- An AppImage or binary
- A `.kazeta/share/run` bash script that launches the emulator
- Configuration files

1. Download a standalone Dolphin AppImage (>= 2407-68) from
   https://dolphin-emu.org/download/ or the GitHub releases.

2. Create the runtime directory structure:
   ```
   dolphin-standalone-1.0/
   ├── dolphin-emu.AppImage       (standalone Dolphin, made executable)
   └── .kazeta/share/
       ├── run                     (launch script)
       └── licenses/
   ```

3. Write the `run` script:
   ```bash
   #!/bin/bash
   # Launch standalone Dolphin with the ROM path from the .kzi
   exec ./dolphin-emu.AppImage --exec "$(cat $1)" -b
   ```
   The `-b` flag launches the game directly (batch mode, no GUI).
   `$1` is a file containing the ROM path, passed by the Kazeta launcher.

4. Pre-configure Dolphin's RetroAchievements settings by including a
   `Config/RetroAchievements.ini` in the runtime:
   ```ini
   [Achievements]
   Enabled = True
   HardcoreEnabled = False
   ; Dual core MUST be disabled for RA to work
   ; (handled in Dolphin settings, not here)
   ```
   And `Config/Dolphin.ini` with:
   ```ini
   [Core]
   ; Dual core must be OFF for RetroAchievements
   bCPUThread = False
   ```

5. Build the EROFS image:
   ```bash
   mkfs.erofs -L dolphin-standalone-1.0 dolphin-standalone-1.0.kzr dolphin-standalone-1.0/
   ```

**Credential injection problem:** Standalone Dolphin needs RA credentials
(username + API token) to activate achievements. Since the playing machine has
no account, you need to either:
- **Option A:** Patch Dolphin to skip auth when loading from a local
  definitions file (requires modifying Dolphin source — complex).
- **Option B (recommended):** Create a dummy/local RA profile in Dolphin's
  config that bypasses the server. Since we're replacing the entire RA data
  flow with local files (Task 4), Dolphin's built-in RA may not be needed at
  all — instead, kazeta-ra reads memory from Dolphin and evaluates
  achievements itself. This is the cleaner path.

**If Option B (kazeta-ra evaluates, not Dolphin):** The runtime just needs to
run Dolphin. Memory access for achievement evaluation is handled by kazeta-ra
reading Dolphin's process memory or via a Dolphin memory-hook plugin. See
Task 5 for details.

**Difficulty:** Easy-medium (building the .kzr is straightforward; the memory
access strategy is the design decision).

---

### Task 4: Replace server-based RA with local-only RA

This is the core change. It has several sub-parts:

#### 4a: Create a local achievement definitions file format

**New file:** `ra/src/local_definitions.rs`

Define a JSON format for baked-in achievement sets:

```json
{
  "game_id": 7693,
  "game_title": "Mario Kart: Double Dash",
  "console_id": 16,
  "console_name": "Nintendo GameCube",
  "icon_url": null,
  "rich_presence_patch": "",
  "achievements": [
    {
      "id": 55001,
      "title": "First Grand Prix",
      "description": "Complete your first Grand Prix",
      "points": 10,
      "badge_name": "55001",
      "mem_addr": "0xH00801234=1.0.5.0=d0xH00801234",
      "type": "standard",
      "display_order": 1
    }
  ],
  "leaderboards": []
}
```

The `mem_addr` field is the critical one — it contains the rcheevos condition
string that the evaluation engine parses. This is exactly what the RA API
returns in its `MemAddr` field.

The file is placed on the SD card as `achievements.json` alongside `cart.kzi`.

**Difficulty:** Easy (just a serde struct + file loader)

#### 4b: Create a fetch-and-export tool (runs on the prep machine)

**New file:** `ra/src/export.rs` or a separate binary `kazeta-ra-export`

This tool runs on the internet-connected prep machine:
1. Takes RA credentials (username + API key) and a game ID
2. Calls `API_GetGameInfoExtended.php` to fetch the full achievement set
3. Exports the response to `achievements.json` in the local format above
4. Also downloads badge images to a `badges/` directory

```
kazeta-ra-export --username USER --api-key KEY --game-id 7693 --output-dir ./mario-kart-double-dash/
```

This is the **only** component that touches the internet or an RA account.

**Difficulty:** Easy (it's essentially a thin wrapper around the existing
`RAClient::get_game_info_and_progress()` that writes JSON to a file instead
of passing it to the overlay)

#### 4c: Add a local definitions loader to kazeta-ra

**Files:** `ra/src/main.rs`, `ra/src/local_definitions.rs`

Add a new code path in `kazeta-ra game-start` that:
1. Checks for `achievements.json` on the cartridge (same directory as the ROM
   or the .kzi)
2. If found, loads definitions from the file (no API call, no auth needed)
3. If NOT found, falls back to the existing online flow (for backward compat
   with other consoles that still use online RA)

The loader populates the same `GameInfoAndProgress` struct that the existing
code uses, so the overlay and cache integration work unchanged.

**Difficulty:** Medium (wiring into the existing game-start flow without
breaking the online path)

#### 4d: Replace server award/sync with local SQLite unlocks

**Files:** `ra/src/api.rs`, `ra/src/cache.rs`, `ra/src/main.rs`

Currently, `award_achievement()` in `api.rs` sends an HTTP POST to
retroachievements.org. Replace this with a local-only path:

1. Add a `local_unlocks` table to the SQLite cache:
   ```sql
   CREATE TABLE IF NOT EXISTS local_unlocks (
       achievement_id INTEGER PRIMARY KEY,
       game_hash TEXT NOT NULL,
       date_earned TEXT NOT NULL,
       is_hardcore BOOLEAN DEFAULT FALSE
   );
   ```

2. Add a `LocalUnlockManager` (or extend `RACache`) that:
   - Records unlocks in `local_unlocks` table
   - Never contacts any server
   - On game start, reads `local_unlocks` to determine which achievements are
     already earned (replaces the server's "user progress" response)

3. Modify the `award-achievement` CLI subcommand to write to local SQLite
   instead of calling `award_achievement()` on `RAClient`.

4. Remove or disable the `login` and `sync` flows entirely. The `kazeta-ra
   login` command should either be removed or print a message saying this fork
   uses local-only achievements.

**Difficulty:** Medium (mostly wiring + ensuring the overlay gets notified the
same way)

#### 4e: Remove authentication requirement

**Files:** `ra/src/auth.rs`, `ra/src/main.rs`, `bios/src/ui/retroachievements.rs`

1. The `kazeta-ra status` command should report `enabled: true` when
   `achievements.json` is found on the cartridge, regardless of whether
   credentials exist.

2. The `setup_retroachievements()` function in `bios/src/utils.rs` (line 527)
   currently checks `kazeta-ra status` for `"enabled":true`. Ensure the local
   path sets this correctly.

3. The RA settings UI in `bios/src/ui/retroachievements.rs` should show
   "Local mode (offline)" instead of login fields when no credentials are
   present but local definitions are available.

**Difficulty:** Easy-medium

---

### Task 5: Achievement evaluation (the engine)

This is the piece that actually detects when you've earned an achievement by
reading game memory. kazeta-ra uses the rcheevos C library (already added as a
dependency in Task 2) to evaluate achievements by reading Dolphin's emulator
memory.

#### Memory access: Dolphin MemoryWatcher (confirmed, built-in)

Research against Dolphin's source code confirms that **MemoryWatcher** is the
viable memory access path. It is built into Dolphin, enabled by default on all
Linux/UNIX builds, and not gated by RetroAchievements hardcore mode.

**How it works (from `Source/Core/Core/MemoryWatcher.cpp`):**
- Dolphin reads a `Locations.txt` file from its user directory
  (`~/.local/share/dolphin-emu/MemoryWatcher/Locations.txt` by default)
- The file is a newline-separated list of hex addresses (without `0x`), e.g.:
  ```
  00801234
  00805678
  ```
  Pointer chains are supported: `ABCD EF` watches `(*0xABCD) + 0xEF`
- At every frame end, Dolphin reads each address (4 bytes, u32) via
  `PowerPC::MMU::HostRead<u32>()` and sends **only changed values** to a Unix
  domain socket (`SOCK_DGRAM`) at `~/.local/share/dolphin-emu/MemoryWatcher/MemoryWatcher`
- Output format: two lines per changed address — the address string from
  Locations.txt, then the new value in hex

**Other approaches considered and ruled out:**
- **GDB stub:** Dolphin has one (`GDBStub.cpp`, supports `m` read-memory packet
  via TCP or Unix socket), but it's disabled in hardcore mode and requires one
  ASCII-hex request-response round trip per address read — far slower than
  MemoryWatcher's batch UDP send for 100+ addresses per frame
- **ptrace / `/proc/pid/mem`:** Works but fragile — requires scanning
  `/proc/<pid>/maps` to find Dolphin's emulated RAM region, which shifts between
  versions. High maintenance, breaks across Dolphin updates
- **Lua/scripting:** Dolphin has no scripting interface. The `Expression` class
  is an internal breakpoint condition evaluator, not a general scripting API

#### Implementation steps

1. **Address extraction:** Parse all memory addresses from the achievement
   `mem_addr` condition strings (the rcheevos trigger format, e.g.
   `0xH00801234=1`). Collect unique 4-byte-aligned addresses across all
   achievements for the current game.

2. **Configure MemoryWatcher:** Before launching Dolphin, write the extracted
   addresses to `Locations.txt` in Dolphin's user directory. This tells
   MemoryWatcher which addresses to watch and push.

3. **Socket listener:** kazeta-ra opens a Unix domain socket at the
   `MemoryWatcher` path and listens for UDP datagrams. Each datagram contains
   address+value pairs for changed memory locations. kazeta-ra maintains a
   cache of the latest value for each address.

4. **Evaluation loop:** kazeta-ra runs a background thread with a fixed-rate
   timer (e.g. 60 Hz) that:
   - Reads the cached memory values (no I/O — just reads the in-memory cache)
   - Calls `rc_runtime_tick()` (rcheevos) with the memory buffer. The tick rate
     is decoupled from MemoryWatcher's push rate so hit counters advance even on
     frames where no values changed
   - When rcheevos reports an achievement trigger, records the unlock in the
     local SQLite `local_unlocks` table and sends a `RaAchievementUnlocked` IPC
     message to the overlay

5. **Byte extraction:** MemoryWatcher reads u32 (4 bytes) per address. rcheevos
   conditions reference specific sizes (`0xH` = 8-bit, `0x` = 16-bit, `0xX` =
   32-bit, `0xL` = lower 16-bit, `0xU` = upper 16-bit). kazeta-ra extracts the
   right bytes from each u32 before feeding them to the evaluation engine.

6. **rcheevos integration:** Use rcheevos' `rc_runtime_t` API (lower-level,
   avoids the server-coupled `rc_client_t`):
   - `rc_runtime_init()` — initialize the runtime
   - `rc_parse_trigger()` — parse each achievement's `mem_addr` condition string
   - `rc_runtime_activate_achievement()` — register each trigger
   - Implement a `rc_memref_t` reader callback that reads from kazeta-ra's
     cached memory values (not from Dolphin directly)
   - `rc_runtime_tick()` — advance all triggers each frame

**Difficulty:** Medium. The memory access strategy is confirmed — MemoryWatcher
is built into every Linux Dolphin build, uses a standard Unix socket, and
requires no Dolphin modification. The remaining work is: address extraction
(string parsing), Locations.txt generation (file write), a UDP socket listener
(standard Rust networking), a fixed-rate timer calling rcheevos (FFI + timer),
and byte-size extraction from u32 values. The rcheevos `rc_runtime_t` API is
well-documented and doesn't require server interaction.

---

### Task 6: Update cartridge tooling

**Repo:** `~/code/kazeta/gamecube` (the separate cartridge repo)

1. Update all 7 `cart.kzi` files: change `Runtime=dolphin` to
   `Runtime=dolphin-standalone` (matching the new .kzr name from Task 3).

2. Update `flash-sd-card.sh` to also copy `achievements.json` and `badges/`
   to the SD card alongside `cart.kzi`, `icon.png`, and the ROM.

3. Create an `achievements.json` placeholder in each cartridge directory (to
   be filled with real data by the export tool from Task 4b).

4. Update `README.md` with the new workflow (prep machine → export → SD card).

**Difficulty:** Easy

---

## Build & Test Commands

```bash
# Build the RA library + CLI
cd ra && cargo build --release

# Run RA tests
cargo test -p kazeta-ra

# Build the overlay daemon
cd overlay && cargo build --features daemon --release

# Build the BIOS
cd bios && cargo build --features dev

# Build the input daemon
cd input-daemon && cargo build --release

# Lint everything
cargo fmt --all
cargo clippy --all-targets --all-features

# Dev loop (builds + runs BIOS + overlay + input)
./dev-run.sh

# Full production build
./build-all.sh --release
```

## Key Files Reference

| File | Purpose |
|------|---------|
| `ra/src/types.rs` | `ConsoleId` enum — add GameCube=16, Wii=19 |
| `ra/src/hash.rs` | ROM hashing — add GameCube partition-aware hash via FFI |
| `ra/src/api.rs` | RA API client — `award_achievement()` to be replaced with local |
| `ra/src/auth.rs` | Credentials — to be made optional for local mode |
| `ra/src/cache.rs` | SQLite cache — add `local_unlocks` table |
| `ra/src/main.rs` | CLI tool — add local definitions loading path |
| `ra/src/lib.rs` | Library exports — add new modules |
| `ra/Cargo.toml` | Add `cc` crate for FFI build, rcheevos dependency |
| `bios/src/utils.rs:120` | `trigger_game_launch()` — launches game + sets up RA |
| `bios/src/utils.rs:527` | `setup_retroachievements()` — calls `kazeta-ra game-start` |
| `bios/src/ui/retroachievements.rs` | RA settings UI — show "local mode" |
| `overlay/src/state.rs` | Achievement tracker — receives IPC, shows toasts |
| `overlay/src/ipc.rs` | IPC protocol — `RaAchievementUnlocked`, `RaGameStart` etc. |
| `overlay/src/rendering.rs` | Overlay rendering — achievement lists, progress bars |
| `overlay/src/themes.rs` | 5 themes (Dark, Light, RetroGreen, PlayStation, Xbox) |
| `rootfs/usr/bin/kazeta-mount` | Mounts .kzr as overlayfs lower layer |
| `rootfs/usr/bin/kazeta-runtime-helper` | Installs .kzr to /usr/share/kazeta/runtimes/ |

## Implementation Order

1. **Task 1** (ConsoleId enum) — trivial, do first, unblocks everything
2. **Task 4a** (local definitions file format) — defines the data contract
3. **Task 4b** (export tool) — lets you fetch real achievement data to test with
4. **Task 2** (GameCube hashing via FFI) — needed to identify discs
5. **Task 4c** (local definitions loader) — wire the offline path
6. **Task 4d** (local SQLite unlocks) — replace server award
7. **Task 4e** (remove auth requirement) — make it work without an account
8. **Task 3** (standalone Dolphin .kzr) — the runtime
9. **Task 5** (evaluation engine) — memory access confirmed via Dolphin MemoryWatcher
10. **Task 6** (cartridge tooling) — update the cartridge repo

Tasks 1-4 are achievable without a running Dolphin instance. Task 5 requires
Dolphin running with a game for testing. Task 3 produces the .kzr that ties it
all together.

## External Dependencies to Add

| Dependency | Purpose | How |
|------------|---------|-----|
| `rcheevos` (C library) | GameCube hashing + achievement evaluation | git submodule in `vendor/rcheevos/` or vendored source |
| `cc` crate | Compile C code from build.rs | Add to `ra/Cargo.toml` `[build-dependencies]` |
| `dolphin-emu` AppImage | Standalone Dolphin runtime | Download from dolphin-emu.org, bundle in .kzr |

## Related Repositories

- **This fork:** `https://github.com/YtGz/kazeta-plus-plus` (private)
- **Upstream:** `https://github.com/goldsziggy/kazeta-plus-plus`
- **Cartridge definitions:** `https://github.com/YtGz/kazeta-gamecube` (private)
- **rcheevos (C library):** `https://github.com/RetroAchievements/rcheevos`
- **Original Kazeta+:** `https://github.com/the-outcaster/kazeta-plus`
- **Original Kazeta OS:** `https://github.com/kazetaos/kazeta`
- **RetroAchievements API docs:** `https://api-docs.retroachievements.org/`
- **RetroAchievements emulator support:** `https://docs.retroachievements.org/general/emulator-support-and-issues.html`

## Ethical Note

Achievement definitions are community-created content on retroachievements.org.
This fork fetches them once (with an RA account) and uses them locally without
ongoing server contact. The playing machine never connects to RA or reports
data. Whether this use of community-authored definitions is acceptable is a
personal decision. The alternative (writing achievement conditions from
scratch) would require reverse-engineering each game's memory layout, which is
impractical for 7 games.
