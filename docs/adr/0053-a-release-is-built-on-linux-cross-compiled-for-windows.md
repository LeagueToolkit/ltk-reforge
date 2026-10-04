# ADR-0053: A release is built on Linux, cross-compiled for Windows

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0046; `docs/design/ROADMAP-UX.md`, "Releases"; `scripts/README.md`, "cross"

## Context

A release holds the command and the app's installer, both for Windows and the MSVC ABI. The
release workflow built them on a Windows runner, and nothing ran the checks.

Nothing in the workspace needs a Windows host to build:

- The Rust code compiles for `x86_64-pc-windows-msvc` on any host that has the target's
  standard library.
- The C code of the dependencies (SQLite, liblzma, zstd, libwebp) compiles with `clang-cl`, and
  `lld-link` links against Microsoft's CRT and SDK, which `cargo-xwin` fetches.
- Tauri builds the app for another target and makes the NSIS installer with `makensis`. It
  calls this experimental.
- The tests use no Windows API and pass on Linux.

Tried on 2026-10-04 on Ubuntu 26.04 with clang 21: the command, the app and the installer
built from an empty home directory, and the command and the app ran on Windows 11. The
workspace's tests and the front end's checks passed on the same machine.

The options were to keep the Windows runner and add the checks there, or to build and check
on Linux.

## Decision

The checks run on Linux and a release is cross-compiled there, the user's choice of
2026-10-04.

- One sourced file sets the build's environment, for a person and for the workflows alike.
  The workflows run the build's commands as steps of their own.
- The tools that are not system packages are pinned in `mise.toml`, and its tasks run the
  checks and the build.
- The workflows run in the Ubuntu release the build was tried on.
- A release is published only when the checks pass.
- libwebp's SSE4.1 sources get `-msse4.1` and no other source does, so a release needs no
  more of a CPU than one built with MSVC.

A build on Windows with MSVC stays possible and is what a developer on Windows runs.

## Consequences

- A release is compiled by clang and linked by lld, not by MSVC. A fault that only one of
  them produces shows in one build and not the other.
- The tests run on Linux only. Behaviour that differs on Windows (paths, the game's install,
  the hash cache's place) is not tested by the workflows.
- Tauri does not sign an installer on a host that is not Windows. The installer is not signed.
- The build downloads Microsoft's CRT and SDK, under Microsoft's licence for them.
- Two files pin Rust's version (`rust-toolchain.toml`, `mise.toml`) and two pin pnpm's
  (`app/package.json`, `mise.toml`). They are raised together.
