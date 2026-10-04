# GAME-BEHAVIOR-0007: A file is there for a champion when a WAD mounted with it holds the path

- **Status:** Inferred
- **Builds:** 16.19
- **Affects:** every file an object names; the WADs of the install
- **Related:** #GAME-BEHAVIOR-0006, #MOD-ARTIFACT-0003

## Behaviour

The client mounts a champion's own WAD, with its locale WADs, when the champion plays. The WADs
named `Global`, `Data`, `UI`, `Shaders` and `Common` are mounted in every game. Another
champion's WAD is mounted only when that champion plays too.

A file an object names is found when a mounted WAD holds its path. Riot ships one asset in
several champions' WADs, so a path is there for a champion when any WAD mounted with it holds
it, whichever WAD a reader of the install finds first.

Not established: the WADs a map mounts, and whether a WAD of a champion in the same game serves
another champion's objects.

## In a mod

- A mod that uses another champion's texture ships the bytes in its own champion's WAD. A
  reference to the other champion's path alone shows nothing unless that champion plays.
- A file whose bytes the game holds in a mounted WAD needs no copy: the reference names the
  game's path.

## Evidence

- Mods that borrow another champion's files ship the bytes (#MOD-ARTIFACT-0003).
- ltk-forge, 2026-10-01, over the 2,912 mods of its eighth datasets run that had a file found
  nowhere or only in an unmounted WAD: asking every WAD that holds a path, where it had asked
  the first, turned 12,342 references found nowhere and 17,242 found only in an unmounted WAD
  into the game's own. `yunara_base_fire_2x2.tex` is in Shyvana's WAD and in Yunara's.
- The list of WADs mounted in every game is read off the install's file names and was not
  checked in game. What would confirm it: a port whose object names a file only another
  champion's WAD holds, seen in a game without that champion.
