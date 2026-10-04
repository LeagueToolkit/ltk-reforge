#!/usr/bin/env bash
# The command, the app and its installer for x86_64-pc-windows-msvc, built on Linux (#ADR-0053).
#
# Needs the packages of setup-ubuntu.sh and the tools of mise.toml: `mise run build-windows`.
# Writes under target/x86_64-pc-windows-msvc/release/.
set -euo pipefail

target=x86_64-pc-windows-msvc
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"

# Onto the toolchain in use, whichever of mise.toml and rust-toolchain.toml chose it.
rustup target add "$target"

# Microsoft's CRT and SDK, fetched on the first run, and the variables that point clang-cl and
# lld-link at them. `cargo xwin build` sets the same ones and would overwrite the two below.
eval "$(cargo xwin env --target "$target")"
export CC_x86_64_pc_windows_msvc="$root/scripts/cross/clang-cl"
# LNK4099: the static CRT the app links names PDBs Microsoft does not ship. In RUSTFLAGS, which
# cargo-xwin exports and cargo reads in place of the target's own.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=/ignore:4099"

# First, and `--locked`: a Cargo.lock that does not match Cargo.toml stops the build here.
cargo build --release --locked -p reforge-cli --target "$target"

cd app
pnpm install --frozen-lockfile
pnpm tauri build --ci --target "$target"
