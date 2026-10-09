# ADR-0084: An events bank the client no longer loads gives way to the game's at its path

- **Status:** Accepted (2026-10-09)
- **Related:** #ADR-0030, #ADR-0075, #ADR-0082, #GAME-BEHAVIOR-0015

## Context

A unit of its own carries the events that reach its own clips, cut from the game's events bank
at the unit's events path, else from the mod's own events bank of the version the client loads
(#ADR-0030, #ADR-0075). The client loads events banks of its own version only
(#GAME-BEHAVIOR-0015).

Some mods ship the game's events bank of an older build under a path of their own, beside a
media bank of their own clips: Ben 10 Ultimate Alien Udyr renames skin 3's, skin 5's and the
base's banks to `ben1_`, `ben2_` and `ben_`, and PROJECT Urgot 1.5.0 keeps skin 3's in the base's
folder. The game has no bank at those paths, and the mod's banks are of version 134 against the
client's 145, so no events bank is found and the clips are not carried. A port changes its
target slot only (#ADR-0082), so those mods' sounds reach the game through no unit at all.

Over `baseline-port-4`, 17 bank units in 10 mods, 527 own clips, end so. The provenance of each
such events bank names the game path its bytes held: Ben 10's are skin 3's, skin 5's and the
base's own banks of builds 415 to 572.

## Decision

When neither the game's bank at the unit's events path nor an events bank of the mod's of the
client's version is found, an events bank of the unit's that is the game's of an older build
gives way to the game's events bank at the target build at the path its bytes held: the first
whose events reach an own clip, else the first. Its events are cut as the game's bank at the
unit's path would be.

## Consequences

- 445 own clips in 6 mods are carried, Ben 10's 75 and PROJECT Urgot 1.5.0's 56 among them; 76
  in 4 mods are not, because no event of the game's bank at the target build plays them
  (2026-10-09).
- The events are the target build's: an event Riot dropped since the mod's build is not kept,
  and its clips are reported as played by no event.
- A bank of another champion's that a mod borrowed gives way to that champion's bank in the
  same way.
- The audio's report names the path of the bank cut from.
