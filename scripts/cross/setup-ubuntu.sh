#!/usr/bin/env bash
# The system packages of the checks and of the Windows build on Ubuntu (#ADR-0053). Run as root.
# The rest of the tools come from mise.toml.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

packages=(
  # What mise and cargo fetch and unpack with, and the host compiler of build scripts.
  ca-certificates curl git xz-utils unzip file build-essential pkg-config
  # The Windows build: clang-cl, lld-link, llvm-lib and llvm-rc, and makensis for the installer.
  clang lld llvm nsis
  # The checks build the app for Linux: Tauri's libraries there.
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
)

apt-get update -q
apt-get install -y -q --no-install-recommends "${packages[@]}"
