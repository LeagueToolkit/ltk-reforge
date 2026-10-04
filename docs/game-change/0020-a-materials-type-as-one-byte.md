# GAME-CHANGE-0020: A material's type as one byte

- **Build:** 16.18.8159717
- **Affects:** `StaticMaterialDef.type`
- **Related:** #GAME-BEHAVIOR-0002, #MOD-ARTIFACT-0021, #MOD-ARTIFACT-0022

## Before

Up to 16.17 a material holds its `type` as a `u32`. The schema's default is 1, and a material
that holds the field holds 0, 2, 3, 4 or 5.

## After

From 16.18 `type` is a `u8`, with the same default and the same values.

## Riot's conversion

Each `u32` became a `u8` of the same number.

## In a mod

A mod made before 16.18 holds `type: u32` on its materials. A later build drops it
(#GAME-BEHAVIOR-0002), and the material's type is the default, 1, where the author's was
another. What the client draws differently for each type is not established.

LTK Manager's repair rule widens an integer and does not narrow one (#MOD-ARTIFACT-0022), so it
does not convert this value.

## Evidence

- The meta database (generation 2026-09-28, read 2026-10-04): `StaticMaterialDef.type` is a
  `u32` with default 1 from 13.15 to 16.17 and a `u8` with default 1 from 16.18. The change is
  at build 8159717.
- Counted in the census history's entry texts by the class's and the field's hashes
  (2026-10-04):

  | build | materials | hold `type` | 0 | 2 | 3 | 4 | 5 |
  |---|---:|---:|---:|---:|---:|---:|---:|
  | 13.15.5229820 | 16,365 | 5,802 as `u32` | 5,710 | 0 | 32 | 60 | 0 |
  | 16.17.8104348 | 33,453 | 11,555 as `u32` | 9,392 | 179 | 1,761 | 197 | 26 |
  | 16.18.8159717 | 32,929 | 10,925 as `u8` | 8,760 | 171 | 1,763 | 205 | 26 |
  | 16.19.8207193 | 33,165 | 10,927 as `u8` | 8,760 | 171 | 1,762 | 208 | 26 |

- The same materials paired across 16.17.8104348 and 16.18.8159717: 10,779 that hold a `type`
  are in both builds, and each holds the same number as a `u8`.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 125 of the 5,137
  mods hold a `u32` where 16.19 reads a `u8` and the value fits, 157 values. The reports name
  the two kinds and not the field, so the count can hold other fields than this one.
