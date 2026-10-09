# GAME-CHANGE-0027: A gear condition as ContextualConditionGearIdEquip

- **Build:** 15.18.7095052
- **Affects:** the conditions of a contextual rule, `ContextualRule.mConditions`, of the class
  `ContextualConditionGearId` (`0x6ecc3b19`)
- **Related:** #GAME-CHANGE-0026, #GAME-BEHAVIOR-0028

## Before

From 14.21 to 15.17 a contextual rule of a skin with gear (Sett's and Jinx's forms, Kai'Sa's,
Morgana's, Sivir's) holds a `ContextualConditionGearId` among its conditions. The class declares
a bool `0x58ecfcaa`, `CheckPrevGearId: bool`, `NewGearId: u32` and `PrevGearId: u32`. No name
table spells `0x58ecfcaa`.

## After

From 15.18 the same condition is a `ContextualConditionGearIdEquip` (`0x061b427f`), which
declares `CheckNewGearId: bool` (`0xaa8fd0c5`), `CheckPrevGearId`, `NewGearId` and
`PrevGearId`. No build declares `ContextualConditionGearId`.

## Riot's conversion

The class hash, and the bool `0x58ecfcaa` renamed `CheckNewGearId` with its value. Every other
field keeps its name and value, and each condition keeps its place in the rule's list.

| up to 15.17 | from 15.18 |
|---|---|
| `ContextualConditionGearId` | `ContextualConditionGearIdEquip` |
| `0x58ecfcaa: bool` | `CheckNewGearId: bool` |
| `CheckPrevGearId`, `NewGearId`, `PrevGearId` | the same |

## In a mod

A mod made from 14.21 to 15.17 that carries a gear skin's voice rules holds the old class. The
client from 15.18 does not declare it. A clip event of an undeclared class crashes the client at
load (#GAME-BEHAVIOR-0028); whether a contextual condition of one does is not established.

## Evidence

- The census history's entry texts at 15.17.7085788 and 15.18.7095052 (read 2026-10-09). At
  15.17.7085788, 6 files hold 22 structs of `0x6ecc3b19`: the skin bins of Jinx, Kai'Sa,
  Morgana, Ruby Jinx, Sett and Sivir. At 15.18.7095052, none.
- The same structs paired across the two builds, by file and by order in the file: each of the
  22, with `0x6ecc3b19` written `0x061b427f` and `0x58ecfcaa` written `0xaa8fd0c5`, is the
  struct at 15.18.7095052, and nothing else differs.
- The meta database (generation 2026-10-05, read 2026-10-09) declares
  `ContextualConditionGearId` from 14.21 to 15.17 and `ContextualConditionGearIdEquip` from
  15.18, with the fields above.
- The mods of the datasets in scope (4,909, read 2026-10-09): 6 hold the old class, 213 structs
  in `ContextualRule.mConditions`, 5 of them Sett's skins and 1 Jinx's.
