# ADR-0024: No archive is extracted to disk

- **Status:** Accepted (2026-10-01)
- **Related:** `docs/design/ARCHITECTURE.md`, "Untrusted input"

## Context

Mods come from third-party sites. Over ltk-forge's datasets, 66 mods carry executables and 45
carry archives inside them. An entry name is chosen by whoever packed the mod. Extracting an
archive puts those files and those names on a disk, where a file browser, a thumbnailer or a
later tool acts on them.

## Decision

No archive is extracted to disk, by reforge or by a script, not to a scratch folder either (the
user's call, 2026-10-01, during ltk-forge).

- Entries are read in memory, under caps.
- Only derived facts are written: counts, names, hashes.
- An archive found inside a mod is identified by its first bytes and never opened.

## Consequences

- To look at one file of a mod, its bytes are read in memory and what is needed is printed.
- The only bytes of a mod that reach a disk are the chunks of a package a run writes, under
  checked paths.
