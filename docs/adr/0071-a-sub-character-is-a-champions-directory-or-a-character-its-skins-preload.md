# ADR-0071: A sub-character is a champion's directory or a character its skins preload

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-BEHAVIOR-0001, #GAME-BEHAVIOR-0007, #ADR-0013

## Context

A champion's sub-characters were the directories whose names start with the champion's and that
have a skin bin in the mod's WAD of the champion. Counted over the 174 champion WADs of 16.19
([RUNS.md](../design/RUNS.md), "The behaviour carried over from ltk-forge"): the check for a skin
bin removes every name the prefix takes wrongly, and the rule misses Heimerdinger's two turrets
(`heimertblue`, `heimertyellow`, 27 skins each), two helpers at skin0 alone (`testcuberender` in
Skarner's WAD, `trundlewall` in Zoe's), and 93 `jade_*` characters, each a champion's variant of a
game mode. The champion's skins name the turrets in `extraCharacterPreloads`. That list leaves out
67 of the 102 sub-characters that are not `jade_*`, so it does not replace the prefix.

The options: keep the prefix; every character with a skin bin in the champion's WAD, the mode
variants with them; the prefix and the characters the game's skins of the champion preload.

## Decision

A champion's sub-characters are the directories whose names start with its name and the
characters its game skins preload, each with a skin bin in the mod's WAD of the champion (the
user, 2026-10-06).

## Consequences

- Heimerdinger's turrets port with him. 2 mods of the datasets ship their bins.
- The helpers and the mode variants stay outside the group.
- A run asks the game for the champion's skins while it sorts the mod's characters.
