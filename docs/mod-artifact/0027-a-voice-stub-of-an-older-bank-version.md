# MOD-ARTIFACT-0027: A voice stub of an older bank version

- **Source:** authors, and the tools that build a voice package; which is not established
- **Affects:** `<name>_vo_audio.bnk` beside `<name>_vo_audio.wpk`; the version in `BKHD`
- **Related:** #GAME-BEHAVIOR-0015, #ADR-0028

## What is found

A mod's voice package (`.wpk`) beside a bank with a header alone, at the same stem, whose
version is older than the client's: 134 where the client loads 145. The stub is the file the
author took from the game when the mod was made, or one a tool wrote.

## Why it is there

A mod keeps the files of the build it was made at. The author replaced lines in the package
and left the stub as it was.

## What reads it, and how

The client does not load the package behind a stub of an older version: none of its lines
plays, the ones the game's current events would play included (#GAME-BEHAVIOR-0015). The mod
is silent where the champion speaks.

## The repair

A stub of the client's version beside the package. A port writes every bank with a header of
the client's version, so its voice plays where the mod's does not.

## Evidence

- ltk-forge, 2026-10-01: one sample made at 16.16 carries the stub of its release made at
  14.17, version 134. 111 of its 115 own lines are played by the game's current voice events.
  The mod is silent on spells, and its port speaks.
- Checked in game on 2026-10-01 with one mod released twice (#GAME-BEHAVIOR-0015).
- ltk-forge's eighth datasets run: of the base-skin mods with own voice in the champion's WAD,
  301 have their stub at version 145.
