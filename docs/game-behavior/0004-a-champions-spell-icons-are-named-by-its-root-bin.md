# GAME-BEHAVIOR-0004: A champion's spell icons are named by its root bin

- **Status:** Confirmed in game (2026-10-01)
- **Builds:** 16.19
- **Affects:** `data/characters/<champion>/<champion>.bin`, `SpellObject`, `mSpell.mImgIconName`,
  the files under `hud/icons2d/` and `hud/textures/`
- **Related:** none

## Behaviour

A champion's spell icons and the textures of its targeters are named by the champion's root bin:
each spell's `SpellObject` holds its icon in `mSpell.mImgIconName`. No skin's bin names them.
A champion whose passive has an icon per skin holds those icons in the root bin's spell too.

So a spell icon shows for the champion under every skin.

A skin's portraits (`iconCircle`, `iconSquare`) are the skin entry's own fields and belong to
the skin.

## In a mod

- A mod that overrides a spell icon's file changes the icon under every skin of the champion.
- No change to a skin's bin can give one skin spell icons of its own.

## Evidence

- The census history shows no field of a skin's entry that names a spell icon.
- Checked in game on 2026-10-01: a port's seven ability icons, shipped under the port's own
  paths and named by records on the root bin, show under the champion's skins with the layer
  holding them on.
