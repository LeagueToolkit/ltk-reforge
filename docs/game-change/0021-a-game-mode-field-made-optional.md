# GAME-CHANGE-0021: A game-mode field made optional

- **Build:** 15.20; the game's objects lose the value at 15.20.7164396, and 15.20.7170001 is
  the first build the schema has the new type at
- **Affects:** `WasdMoveOrderData.0x43bf3213`
- **Related:** #GAME-BEHAVIOR-0002, #MOD-ARTIFACT-0022

## Before

`WasdMoveOrderData` is a game-mode config: its base is `IGameModeConfig`. From 15.14 to 15.19
it declares the field `0x43bf3213` as an `f32` with default 0. No name table spells the
field's name.

The game's objects that hold the field are in the bins of five maps. They hold 20 at 15.14 and
50 at 15.19.

## After

From 15.20 the field is an option of `f32`, empty by default. No game object holds it.

## Riot's conversion

Each of the five game objects lost the value at 15.20.7164396, and nothing else of the object
changed at that build. No game object holds the field as an option at a later build, so no game
value shows an `f32` carried into the option.

That a present option of the same number means what the `f32` meant is inferred from the type
alone. It is not established.

## In a mod

A mod that holds the field as an `f32` loses it at a build from 15.20, which reads an option
there (#GAME-BEHAVIOR-0002). The field is of a map's game mode, not of a skin. No mod of the
baseline run holds it.

LTK Manager's repair rule does not put a value into an option (#MOD-ARTIFACT-0022).

## Evidence

- The meta database (generation 2026-09-28, read 2026-10-04): `WasdMoveOrderData`, hash
  `0x92309121`, derives from `IGameModeConfig`. Its `0x43bf3213` is an `f32` with default 0
  from 15.14 to 15.19, builds 6950646 to 7151836, and an option of `f32` with default none from
  15.20, build 7170001. The database holds no earlier build of 15.20. `IGameModeConfig`
  declares no such field itself. It is the one property whose type changed between 15.19 and
  15.20.
- Counted in the census history's entry texts by the field's hash (2026-10-04): no line at
  15.13.6934876, 4 at 15.14.6950510, each `f32 = 20`, and 5 at 15.19.7151836, each `f32 = 50`.
  No line at 15.20.7164396, 15.20.7190545, 15.22.7229166 or 16.19.8207193.
- The five entries compared across 15.19.7151836 and 15.20.7164396: each is the same entry of
  a map's bin, and each loses the one line and gains none.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: no value was put
  into an option.
