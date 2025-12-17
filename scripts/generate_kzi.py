#!/usr/bin/env python3
"""
Batch-generate Kazeta .kzi metadata folders for a directory of games.

Features:
- Detect runtime based on file extension (override with --runtime).
- Derive game name/ID from filename.
- Optionally fetch an icon from SteamGridDB if an API key is provided.
- Create per-game subdirectories with the .kzi and icon.
"""
import argparse
import json
import os
import pathlib
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Dict, Optional, Tuple


DEFAULT_RUNTIME_MAP: Dict[str, str] = {
    ".iso": "ps2-1.0.kzr",
    ".bin": "ps2-1.0.kzr",
    ".chd": "ps2-1.0.kzr",
    ".exe": "windows-1.2-experimental.kzr",
    ".gba": "vba-m",
    ".gb": "vba-m",
    ".gbc": "vba-m",
    ".nds": "linux-1.1.kzr",
    ".n64": "linux-1.1.kzr",
}


def slugify(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r"[^a-z0-9]+", "-", text)
    return text.strip("-") or "game"


def title_from_filename(stem: str) -> str:
    parts = re.split(r"[_\-\.]+", stem)
    return " ".join(p.capitalize() for p in parts if p) or stem


def detect_runtime(path: pathlib.Path, default_runtime: str) -> str:
    return DEFAULT_RUNTIME_MAP.get(path.suffix.lower(), default_runtime)


def fetch_steamgriddb_icon(name: str, api_key: str, dest: pathlib.Path) -> Optional[pathlib.Path]:
    headers = {"Authorization": f"Bearer {api_key}"}

    def _get(url: str) -> Optional[dict]:
        req = urllib.request.Request(url, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                return json.load(resp)
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, json.JSONDecodeError):
            return None

    search_url = "https://www.steamgriddb.com/api/v2/search/autocomplete?" + urllib.parse.urlencode(
        {"term": name}
    )
    search = _get(search_url)
    if not search or not search.get("success") or not search.get("data"):
        return None

    game_id = search["data"][0].get("id")
    if not game_id:
        return None

    icon_url = f"https://www.steamgriddb.com/api/v2/icons/game/{game_id}"
    icons = _get(icon_url)
    if not icons or not icons.get("success") or not icons.get("data"):
        return None

    first = icons["data"][0]
    download_url = first.get("url")
    if not download_url:
        return None

    dest.parent.mkdir(parents=True, exist_ok=True)
    try:
        req = urllib.request.Request(download_url, headers=headers)
        with urllib.request.urlopen(req, timeout=10) as resp, open(dest, "wb") as fh:
            fh.write(resp.read())
        return dest
    except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, OSError):
        return None


def write_kzi(path: pathlib.Path, name: str, game_id: str, exec_path: str, runtime: str, icon: str) -> None:
    lines = [
        f"Name={name}",
        f"Id={game_id}",
        f"Exec={exec_path}",
        f"Icon={icon}",
        f"Runtime={runtime}",
    ]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def process_game(
    file_path: pathlib.Path,
    output_root: pathlib.Path,
    default_runtime: str,
    override_runtime: Optional[str],
    api_key: Optional[str],
    dry_run: bool,
) -> Tuple[pathlib.Path, pathlib.Path]:
    name = title_from_filename(file_path.stem)
    game_id = slugify(file_path.stem)
    runtime = override_runtime or detect_runtime(file_path, default_runtime)

    game_dir = output_root / game_id
    kzi_path = game_dir / f"{game_id}.kzi"
    icon_path = game_dir / "icon.png"

    if dry_run:
        return kzi_path, icon_path

    game_dir.mkdir(parents=True, exist_ok=True)

    icon_name = "icon.png"
    if api_key:
        fetched = fetch_steamgriddb_icon(name, api_key, icon_path)
        if not fetched:
            icon_name = "icon.png"
        else:
            icon_name = fetched.name

    # If icon missing and not fetched, keep placeholder name; user can drop their own
    write_kzi(
        kzi_path,
        name=name,
        game_id=game_id,
        exec_path=file_path.name,
        runtime=runtime,
        icon=icon_name,
    )

    return kzi_path, icon_path


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate .kzi metadata for a directory of games.")
    parser.add_argument("input_dir", type=pathlib.Path, help="Directory containing game files (ISO/CHD/EXE/etc)")
    parser.add_argument("--output-dir", type=pathlib.Path, help="Where to write per-game folders (default: input dir)")
    parser.add_argument("--runtime", help="Override runtime for all games (e.g., ps2-1.0.kzr)")
    parser.add_argument("--default-runtime", default="linux-1.1.kzr", help="Fallback runtime when extension is unknown")
    parser.add_argument("--apikey", help="SteamGridDB API key (optional)")
    parser.add_argument("--dry-run", action="store_true", help="Only print actions, do not write files")
    args = parser.parse_args()

    input_dir = args.input_dir
    output_root = args.output_dir or input_dir

    if not input_dir.is_dir():
        print(f"Input directory not found: {input_dir}", file=sys.stderr)
        return 1

    games = [p for p in input_dir.iterdir() if p.is_file()]
    if not games:
        print("No files found to process.", file=sys.stderr)
        return 1

    for g in games:
        kzi_path, icon_path = process_game(
            g,
            output_root=output_root,
            default_runtime=args.default_runtime,
            override_runtime=args.runtime,
            api_key=args.apikey,
            dry_run=args.dry_run,
        )
        if args.dry_run:
            print(f"[DRY RUN] Would create: {kzi_path} (runtime auto={detect_runtime(g, args.default_runtime)})")
        else:
            print(f"Created {kzi_path}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
