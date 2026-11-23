#!/usr/bin/env python3
"""Rebuild the desktop app after edits and relaunch it on a successful build."""

import json
import os
from pathlib import Path
import subprocess
import time


ROOT = Path(__file__).resolve().parent.parent
WATCH = ("Cargo.toml", "Cargo.lock", "apps/desktop", "crates/api", "games")


def snapshot():
    files = {}
    for entry in WATCH:
        root = ROOT / entry
        paths = (
            (Path(directory) / name for directory, _, names in os.walk(root) for name in names)
            if root.is_dir()
            else (root,)
        )
        for path in paths:
            if path.name == ".DS_Store":
                continue
            try:
                info = path.stat()
            except FileNotFoundError:
                continue
            files[path] = (info.st_mtime_ns, info.st_size)
    return files


def build():
    command = ["cargo", "build", "--locked", "--bin", "corner-table",
               "--message-format=json-render-diagnostics"]
    executable = None
    with subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, text=True) as cargo:
        for line in cargo.stdout:
            message = json.loads(line)
            if message.get("reason") == "compiler-artifact" and message.get("executable"):
                if message["target"]["name"] == "corner-table":
                    executable = message["executable"]
        return executable if cargo.wait() == 0 else None


def stop(client):
    if client is None or client.poll() is not None:
        return
    client.terminate()
    try:
        client.wait(timeout=2)
    except subprocess.TimeoutExpired:
        client.kill()
        client.wait()


def main():
    previous = None
    client = None
    print("Watching Corner Table. Press Ctrl+C to stop.", flush=True)
    try:
        while True:
            current = snapshot()
            if current != previous:
                time.sleep(0.35)
                if snapshot() != current:
                    continue
                previous = current
                if executable := build():
                    stop(client)
                    client = subprocess.Popen([executable], cwd=ROOT)
            time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    finally:
        stop(client)


if __name__ == "__main__":
    main()
