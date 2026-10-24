# GAME-CHANGE-0028: A material's name as an 8-byte hash

- **Build:** 16.21.8298620
- **Affects:** `StaticMaterialDef.name` (`0x8d39bde6` on `0xff9d3409`)
- **Related:** #GAME-CHANGE-0013, #GAME-BEHAVIOR-0002, #GAME-BEHAVIOR-0022, #MOD-ARTIFACT-0022

## Before

Up to 16.20 a material holds its `name` as a `string`. The schema's default is the empty
string.

## After

From 16.21 `name` is a `hash` stored in 8 bytes: the first `hash` property the game stores so.
A bin file does not say a `hash` value's width; a reader takes it from the sizes the file
declares, and the client from the property's helper. Every other `hash` property is stored in
4 bytes, and at 16.21 every 8-byte `hash` value in the game's bins is this property. The
schema's default is `0x0`.

The census history spells the type `hash[u64]` and the value as `0x` and 16 lowercase hex
digits. Plain `hash` stays 8 digits.

## Riot's conversion

Each string became the XXH3-64 (seed 0) of the string lowercased. Every one of 33,375 materials
in both builds is so. No game value shows what an empty string became.

## In a mod

A mod made before 16.21 holds `name: string` on its materials. What the client does with the
string at 16.21 is not established; a value of another type is dropped (#GAME-BEHAVIOR-0002),
which leaves the name at its default. What the client uses the name for is not established.
The shader cache keys a material's variants by the XXH3-64 of the material's entry name
(#GAME-BEHAVIOR-0022), which is the same hash function and width.

## Evidence

- The meta database (generation 2026-10-05, read 2026-10-24): `StaticMaterialDef.name` is a
  `string` with default `""` from 13.15 to 16.20 and a `hash` with default `0x0` from 16.21.
  The change is at build 8298620. The database carries a `hash` property's kind and not its
  width.
- The same database, every type change between 16.20 and 16.21: seven properties. This one;
  the keys of `MapMaterialSwap.Swaps` and `MeshHelperDef.MaterialMapping` from `string` to
  `hash`; two properties of unnamed classes from a link to a `hash`; one integer narrowed; one
  pointer's class changed.
- Counted in the census history's entry texts by census-sync (2026-10-24): 33,736 values of
  this property as `hash[u64]` in 33,613 files at 16.21.8298620, and no other property stored
  in 8 bytes. For 33,375 materials in both 16.20.8248524 and 16.21.8298620, the 16.21 value is
  the XXH3-64 of the 16.20 string lowercased. No file written before 16.21 changed.
