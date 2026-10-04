# MOD-ARTIFACT-0022: Values converted by the manager at install

- **Source:** LTK Manager's repair rule for property types, applied when it installs a mod
- **Affects:** every bin property whose type in the mod is one the rule converts to the type
  the running build reads
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0013, #GAME-CHANGE-0019, #GAME-CHANGE-0020,
  #GAME-CHANGE-0021, #MOD-ARTIFACT-0021

## What is found

A bin value in a type the mod's own build did not read, in a mod whose author saw the value
work. A mod made at 16.17 or later that names a texture by a path string is the case: 16.17
reads a `file` there (#GAME-CHANGE-0013).

One value does not show it. A string where the build reads a `file` is also what a mod made
before 16.17 holds. It is told by the mod's date: the value is of a type no build since the
mod's own read, and of a type the manager's rule converts.

## Why it is there

LTK Manager repairs property types as it installs a mod. It writes the installed copy with each
value in the type the running build reads, where its rule has a conversion. The author's
archive keeps the old type, and the author sees the mod work.

The conversions of the rule, as ltk-forge's docs list them:

| from | to | how |
|---|---|---|
| `string` | `file` | the xxHash of the lowercased path, in a leaf, a list, an option or a map's value |
| `hash` of a path | `file` | through the path, which a name table has to spell |
| a map keyed by `hash` | a map keyed by `file` | the same, for every key |
| `embed` | `pointer`, and back | the same struct under the other tag |
| `list` | `list2`, and back | the same items under the other tag |
| `bool` | `flag`, and back | the same value under the other tag |
| an integer | a wider integer | the same number |
| none | a null `pointer` | the zero class hash |
| an empty option | an option of another item type | the item type alone changes |

The rule does not narrow an integer, does not convert between `link` and `hash`, does not put a
value into an option, and does not move a value into a struct.

## What reads it, and how

The client drops a value of another type than the build reads (#GAME-BEHAVIOR-0002). It reads
the installed copy the manager wrote, where the value has the build's type.

A mod installed without the rule is read as its archive holds it, and the value is dropped.
Which other managers convert types is not established.

## The repair

The value in the type the build reads, by the conversion of the table. A value of a type the
rule does not convert was dropped for the author too, whatever installed the mod
(#MOD-ARTIFACT-0021).

## Evidence

- ltk-forge's docs describe the rule, name it `bin/property-type`, and state that the manager
  applies it as it installs a mod. ltk-reforge has not read the manager's source or run it.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired, by the two kinds of
  each conversion the rule also makes:

  | from | to | mods | values |
  |---|---|---:|---:|
  | `string` | `file` | 3,918 | 276,351 |
  | `hash` | `file` | 340 | 1,704 |
  | `u8` | `u16` | 78 | 2,221 |
  | none | a `pointer` | 38 | 185 |
  | `embed` | `pointer` | 18 | 90 |
  | `bool` | `flag` | 14 | 52 |
  | `pointer` | `embed` | 3 | 5 |
  | `list` | `list2` | 2 | 2 |

- The same run dates 4,185 of the 5,137 mods, and none at 16.17 or later. The newest are 10
  mods dated at 16.16, 9 of which name files by strings. So no mod of the run is shown to hold
  a string that its own build did not read. ltk-forge's docs state that 79 mods dated at 16.19
  name textures by string, with no date for the count.
- `VfxSystemDefinitionData.flags` is a `u8` in the game's bins from 9.22.2961221 to
  12.23.4875509 and a `u16` from 13.1.4875491 (counted in the census history's entry texts by
  the field's hash, 2026-10-04). The reports name the two kinds of a conversion and not the
  field.
- No check in game compared a mod installed by the manager with the same mod installed without
  the rule. That check would confirm what the author saw.
