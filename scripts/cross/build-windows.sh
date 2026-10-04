#!/usr/bin/env bash
# The command, the app and its installer for x86_64-pc-windows-msvc, built on Linux (#ADR-0053),
# for a person: `mise run build-windows`. The `windows` job of .github/workflows/build.yml runs
# the same commands as its own steps.
#
# Writes under target/x86_64-pc-windows-msvc/release/.
set -euo pipefail

cd "$(dirname "$0")/../.."
. scripts/cross/env.sh

# First, and `--locked`: a Cargo.lock that does not match Cargo.toml stops the build here.
cargo build --release --locked -p reforge-cli --target x86_64-pc-windows-msvc

pnpm --dir app install --frozen-lockfile
pnpm --dir app tauri build --ci --target x86_64-pc-windows-msvc
