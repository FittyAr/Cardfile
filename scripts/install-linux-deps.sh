#!/usr/bin/env bash
# System packages needed to build GPUI on Debian/Ubuntu.
set -euo pipefail
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config libssl-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libx11-dev libx11-xcb-dev libxcb1-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libvulkan-dev libfontconfig-dev libfreetype-dev libasound2-dev \
  libgtk-3-dev
