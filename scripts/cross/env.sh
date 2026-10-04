# The environment of a build for x86_64-pc-windows-msvc on Linux (#ADR-0053). Sourced from the
# repository's root, before cargo or Tauri is given `--target x86_64-pc-windows-msvc`:
#
#   . scripts/cross/env.sh
#
# Needs the packages of setup-ubuntu.sh and the tools of mise.toml.

# Microsoft's CRT and SDK, fetched on the first run, and the variables that point clang-cl and
# lld-link at them. `cargo xwin build` sets the same ones and would overwrite the two below.
xwin_env=$(cargo xwin env --target x86_64-pc-windows-msvc) || return 1
eval "$xwin_env"
unset xwin_env

# After the lines above, which put rustup's own cargo first on PATH: the target goes onto the
# toolchain that cargo runs, not the one a shim of mise chose.
rustup target add x86_64-pc-windows-msvc || return 1

export CC_x86_64_pc_windows_msvc="$PWD/scripts/cross/clang-cl"
# LNK4099: the static CRT the app links names PDBs Microsoft does not ship. In RUSTFLAGS, which
# cargo-xwin exports and cargo reads in place of the target's own.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=/ignore:4099"
