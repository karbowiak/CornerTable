#!/usr/bin/env python3
"""Collect source and license notices for a native Corner Table archive."""

import argparse
import json
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
ASSET_NOTICES = (
    "apps/desktop/assets/fonts/DejaVu-LICENSE.txt",
    "apps/desktop/assets/fonts/OpenSans-LICENSE.txt",
    "apps/desktop/local-audio/README.md",
    "games/chess/COZY-CHESS-LICENSE.txt",
    "games/word-games/DICTIONARY-LICENSE.txt",
)
NOTICE_PREFIXES = ("license", "copying", "notice", "copyright", "unlicense")


def runtime_packages(metadata):
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    desktop = next(package for package in packages.values() if package["name"] == "corner-table")
    pending = [desktop["id"]]
    visited = set()
    while pending:
        package_id = pending.pop()
        if package_id in visited:
            continue
        visited.add(package_id)
        pending.extend(
            dependency["pkg"]
            for dependency in nodes[package_id]["deps"]
            if any(kind["kind"] is None for kind in dependency["dep_kinds"])
        )
    return sorted(
        (packages[package_id] for package_id in visited if packages[package_id]["source"]),
        key=lambda package: (package["name"], package["version"]),
    )


def package_notices(package):
    root = Path(package["manifest_path"]).parent
    notices = {
        path for path in root.iterdir()
        if path.is_file() and path.name.lower().startswith(NOTICE_PREFIXES)
    }
    if package["license_file"]:
        notices.add(root / package["license_file"])
    return sorted(notices)


def collect(target, destination):
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target],
        cwd=ROOT,
        text=True,
    ))
    destination.mkdir(parents=True, exist_ok=True)
    asset_dir = destination / "assets-and-data"
    asset_dir.mkdir(exist_ok=True)
    for notice in ASSET_NOTICES:
        source = ROOT / notice
        name = "Audio sources.md" if source.name == "README.md" else source.name
        shutil.copyfile(source, asset_dir / name)

    entries = ["# Corner Table third-party notices", "", f"Cargo target `{target}`", ""]
    for package in runtime_packages(metadata):
        name = f"{package['name']}-{package['version']}"
        license_text = package["license"] or "See license file"
        source = package["repository"] or package["source"]
        entries.append(f"- `{name}` · {license_text} · {source}")
        notices = package_notices(package)
        if notices:
            package_dir = destination / "crates" / name
            package_dir.mkdir(parents=True, exist_ok=True)
            for notice in notices:
                shutil.copyfile(notice, package_dir / notice.name)
    (destination / "README.md").write_text("\n".join(entries) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target")
    parser.add_argument("destination", type=Path)
    options = parser.parse_args()
    collect(options.target, options.destination)
