# GAME-CHANGE-0023: Health bar styles renumbered

- **Build:** 13.4.4929018, then 14.7.5715678, 14.17.6116594 and 15.4.6568035
- **Affects:** `CharacterHealthBarDataRecord.unitHealthBarStyle` (`u8`), and
  `SkinCharacterDataProperties.healthBarData`, which holds it
- **Related:** none

## Before

Up to 13.3 a champion's skin entry holds no `unitHealthBarStyle`. Its `healthBarData` holds
`attachToBone` alone, or the entry has no `healthBarData` (Annie's base skin). The client takes
the field's default.

The other units of a champion WAD hold the style: `1`, `3` and `4` (Tibbers holds `4`), and the
TFT units `11`.

## After

The style of a champion's skin is written in every skin entry, and the number of each kind of
bar moves up at each of the four builds:

| builds | a champion's skin | values that moved at the first build |
|---|---|---|
| 13.4 to 14.6 | `9` | `11` to `12` |
| 14.7 to 14.16 | `10` | `9` to `10`, `12` to `13` |
| 14.17 to 15.3 | `11` | `4` to `5`, `8` to `9`, `10` to `11`, `13` to `14` |
| 15.4 on | `12` | `11` to `12`, `14` to `15` |

The field is a `u8` with the default `0` at every build the schema covers (13.15 on).

## Riot's conversion

At each build every value from some point up rose by one, and the values below it stayed:

- **13.4.** A champion's skin entry that held no style took `9`; one with no `healthBarData` got
  one that holds only the style. `11` became `12`. `1`, `3` and `4` stayed.
- **14.7.** `9` became `10` and `12` became `13`. `1`, `3`, `4` and `8` stayed.
- **14.17.** `4` became `5`, `8` became `9`, `10` became `11` and `13` became `14`. `1` and `3`
  stayed.
- **15.4.** `11` became `12` and `14` became `15`. `1`, `3`, `5` and `9` stayed.

So a style of a mod's build is at 16.19:

| the mod's build | at 16.19 |
|---|---|
| up to 13.3, a champion's skin with no style | `12` |
| 13.4 to 14.6 | `9` is `12`; `4` is `5`; `8` is `9`; `12` is `15`; `1` and `3` stay |
| 14.7 to 14.16 | `10` is `12`; `4` is `5`; `8` is `9`; `13` is `15`; `1` and `3` stay |
| 14.17 to 15.3 | `11` is `12`; `14` is `15`; `1`, `3`, `5` and `9` stay |

A value no object of the game held across a build has no conversion read here. Three earlier
renumberings, at 9.14, 9.21 and 11.22, moved only the TFT units' values, which no skin of a
champion held.

## In a mod

A mod made before 15.4 holds the number of its own build. A champion's skin made at 14.1 holds
`9`, and at 16.19 `9` is another kind of bar, which the meta wiki finds in Naafiri's files
alone. A skin made before 13.4 holds no style, and at 16.19 the default `0` is not the
champion's bar either.

What the client draws for such a value is not checked in game. The meta wiki describes the
client's handling: `12` and `22` take one way of building the bar, `14` to `21` another, `13`
removes the bar, and every other value takes a third. Three other tools, TopazModFixer,
League-Mod-Repather and Hematite, set `12` on a champion's skin to repair a missing health bar.
Hematite sets it on every skin entry of a mod, the sub-characters' included, where the game's
value is another (Tibbers holds `5`).

## Evidence

- The census history, branch `history-v2`, read 2026-10-05. For each of the four builds, every
  object file under the champions WADs that holds the field at the build before and at the
  build, paired by file:
  - 13.3.4916222 to 13.4.4929018: 180 held a style at both, 2 of them moved (`11` to `12`).
    6,149 took `9` and 9 took `4`, where they held no style before.
  - 14.6.5706276 to 14.7.5715678: 8,057 at both; 7,824 moved `9` to `10`, 2 moved `12` to
    `13`.
  - 14.16.6120449 to 14.17.6116594: 8,474 at both; 8,232 moved `10` to `11`, 162 `4` to `5`,
    20 `8` to `9`, 2 `13` to `14`.
  - 15.3.6564086 to 15.4.6568035: 8,870 at both; 8,600 moved `11` to `12`, 2 `14` to `15`.
- The skin entries of Annie, Ahri and Garen's base skins: no style up to 13.3, then `9`, `10`,
  `11` and `12` at the four builds, and `12` to 16.19.8230722. Annie's base skin has no
  `healthBarData` at 13.3 and `healthBarData { unitHealthBarStyle: 9 }` at 13.4.
- Two TFT units' objects, followed over every build: `11` at 9.13, `12` at 9.14, `13` at 9.21,
  `11` at 11.22, then `12`, `13`, `14` and `15` at the four builds, and `15` to 16.19. An object
  holding `1` and one holding `3` are unchanged from 8.20 to 16.19. A skin entry of Tibbers
  holds `4` from 11.3 and `5` from 14.17 to 16.19.
- The schema and the meta wiki, read with `rito-meta` on 2026-10-05.
- Repair runs with the conversion on champions' skin entries (2026-10-06). Of 400 mods of the
  datasets, every twelfth by SHA-256, 29 hold such an entry of an older numbering: 161 entries
  hold `11`, 4 hold `10`, and one has no `healthBarData`; none holds `9`. Of the 54 mods of the
  reduced corpus, 9: 59 entries hold `10` and 14 hold `11`. The skin entries of mods dated
  before 13.4 that were read hold `12`, and mods dated 10.11 and 13.3 hold `11`: a mod's date
  does not tell the numbering of its entry.
