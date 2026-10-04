# GAME-BEHAVIOR-0021: A field Riot's bins hold that no class declares is not read

- **Status:** Inferred
- **Builds:** 16.19
- **Affects:** every bin class
- **Related:** #GAME-BEHAVIOR-0002, #MOD-ARTIFACT-0020

## Behaviour

Riot's own bins hold fields that no class of the client declares. `mRuleName` is one. They are
data of Riot's tools that the build of the game's files leaves in. The client registers no
member for such a field, so it drops the property as it drops any field its class does not
declare (#GAME-BEHAVIOR-0002). The meta schema lists what the client registers, so it does not
list these fields.

A field on a game object therefore does not show that a build reads it. A field the schema
declares is read. A field only the game's bins hold is carried and unused.

## In a mod

A mod's copy of a game object carries the field with the rest of the object. Kept or removed,
it changes nothing the client does.

It is told from a field a tool wrote (#MOD-ARTIFACT-0020) by where it is found: this one is on
the game's own objects, that one is on none.

## Evidence

- Stated by a maintainer of the LeagueToolkit tools (2026-10-04): Riot's bins hold fields that
  are internal to Riot, some of which a leaked internal build of 13.9 names, and the client
  does not use them. `mRuleName` is the example given.
- The user states the same (2026-10-04): the fields are written into the bins, the game does
  not use them, and that is why the meta schema lacks them.
- Not checked here: which classes hold `mRuleName`, and how many such fields the install's bins
  hold. A count of the fields on game objects that the schema declares on no class of the
  object's chain would give both.
- No check in game is recorded. One game object repacked without the field, with nothing else
  changed and no difference seen, would confirm it.
