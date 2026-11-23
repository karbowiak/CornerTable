#!/usr/bin/env bash
set -euo pipefail

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential clang pkg-config libfontconfig1-dev libfreetype-dev \
    libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-dev \
    libx11-xcb-dev libxcb1-dev libvulkan-dev
