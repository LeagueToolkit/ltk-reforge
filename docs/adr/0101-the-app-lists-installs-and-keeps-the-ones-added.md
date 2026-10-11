# ADR-0101: The app lists installs and keeps the ones added

- **Status:** Accepted (2026-10-11)
- **Related:** #ADR-0046; `docs/design/ROADMAP-UX.md`, "The inputs" and "The inputs page"

## Context

A run is judged against one game install. The app holds one path for it, `inputs.game`, found
once at the first start with `ltk_mod_core`, as LTK Manager finds it, or picked as a folder.
A person with the live game and the PBE, or with a copy of an older build kept for testing,
switches between them by picking folders again.

`ltk_mod_core` gives one path: the first it finds in the Riot client's list of installs
(`RiotClientInstalls.json`), a running client, the usual folders on each drive
(`C:\Riot Games\League of Legends` and the same under `Program Files`), and the registry. It
skips the PBE's folder, `League of Legends (PBE)`, on purpose. LTK Manager manages installs: it
lists the ones it detects and the ones a person adds, and one of them is used.

Two options were open: one path, as the app has it, or a list of installs with one chosen.

## Decision

The app lists the game installs it detects on the machine and the ones a person added, and
one of them is chosen (the user's call, 2026-10-11).

- Detection covers the live install and the PBE.
- An added install is any folder that opens as an install: the install's folder, its `Game`
  folder or its `DATA/FINAL`.
- The chosen install is the path a run reads. It stays in `inputs.game`, so a settings file an
  older app wrote still reads, and it is always one of the listed installs.

## Consequences

- The settings gain a key for the installs added by hand.
- The app reads the Riot client's list of installs itself, since `ltk_mod_core` skips the PBE.
- No install is opened to list it, since opening one reads every WAD's table of contents and
  takes seconds. Only its build is read, from its `content-metadata.json`. The chosen install
  is opened once, as before, when the inputs open.
- A detected install cannot be removed from the list: it is detected again at the next look.
