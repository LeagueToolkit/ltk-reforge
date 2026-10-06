# MOD-ARTIFACT-0035: Buses in a mod's media bank

- **Source:** authors' own Wwise projects, inferred from the banks; two of the three mods share
  a bus id of their own
- **Affects:** `.bnk`: the bus objects of a media bank's `HIRC`
- **Related:** #GAME-BEHAVIOR-0025, #ADR-0030, #ADR-0064

## What is found

A mod's media bank, of version 145, that holds bus objects in a `HIRC` beside its clips. Some
are copies of the game's buses under the game's ids (`Master Audio Bus`, `SFX`, `mus`), with
other parameters than the game's init bank gives them. The others are buses whose ids the game's
init bank does not define, children of those copies. The mod's own events bank routes objects to
both.

A game media bank holds no `HIRC`: it is told from one by the bus objects alone.

## Why it is there

The author built the banks in a Wwise project of their own, whose bus hierarchy repeats the
game's bus names and adds buses for the mod, and the bank export wrote the buses with the
media. That is inferred from the banks: no tool source is read.

## What reads it, and how

The client loads the bus objects with the bank (#GAME-BEHAVIOR-0025). Without the buses the game
lacks, none of the sounds of the mod's events bank are heard; the copies of the game's buses
change nothing heard.

## The repair

The bank's buses whose ids the game's init bank does not define, carried with its clips; the
copies of the game's buses left out (#ADR-0064).

## Evidence

- Read in memory from the 2,792 mods of the datasets whose baseline report holds a bank
  (2026-10-06), against the init bank of the 16.19.8207193 install copy: 3 mods hold bus objects,
  each in one media bank of version 145 with its clips (14, 31 and 13), none in an events bank.

  | mod (SHA-256) | copies of the game's buses | buses the game lacks |
  |---|---:|---:|
  | `f52c4205486e…` | 2 | 2 |
  | `40606c273797…` | 3 | 2 |
  | `0adb65ca701a…` | 3 | 1 |

  Bus 3276768291 is in the first two. In the baseline each of the three is a unit whose events
  are cut from the mod's own events bank (#ADR-0030).
- Checked in game on `40606c273797…` (#GAME-BEHAVIOR-0025).
