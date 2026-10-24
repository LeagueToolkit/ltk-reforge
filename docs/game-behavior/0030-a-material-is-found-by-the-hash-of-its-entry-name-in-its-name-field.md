# GAME-BEHAVIOR-0030: A material is found by the hash of its entry name in its name field

- **Status:** Confirmed in game (2026-10-24)
- **Builds:** 16.21
- **Affects:** `StaticMaterialDef.name`
- **Related:** #GAME-CHANGE-0028, #GAME-BEHAVIOR-0022, #GAME-BEHAVIOR-0029

## Behaviour

From 16.21 a `StaticMaterialDef` holds its `name` as an 8-byte hash (#GAME-CHANGE-0028), the
XXH3-64 of the material's own entry name lowercased: the hash the shader cache keys the
material's variants by (#GAME-BEHAVIOR-0022). The client finds the material through that
value, not through the entry name alone. A material whose `name` is not the hash of its entry
name is not drawn as it should be: the skin shows red and black where the material draws, and
the client may crash at the draw.

## In a mod

A mod made at 16.21 or later holds each material's `name` as the hash of the material's entry
name, as the game does. A port that moves a material to another entry name, under a namespace
for one, has to put the hash of the new name in `name`, or the material is lost at the new
name. A mod made before 16.21 holds a string there, which a port turns into the hash of the
new name the same way.

## Evidence

- In game (2026-10-24, 16.21.8298620): a port of one mod of a champion skin, with two
  materials carried under the port's namespace and their `name` left as the mod wrote it,
  drew the skin red and black as a `.fantome` and crashed the client as a `.modpkg`. The same
  port with the two materials' `name` set to the XXH3-64 of their new entry names, and nothing
  else changed, drew and ran. The mod's own `name` values were the XXH3-64 of its original
  entry names, `Characters/Aatrox/Skins/Skin0/Materials/Sword_inst` for one.
- A port of another mod made before 16.21, whose materials' `name` the port had turned from a
  string into the hash of the original entry name, drew and ran at 16.21 the same day. What
  makes the one case fail and the other not is not established.
