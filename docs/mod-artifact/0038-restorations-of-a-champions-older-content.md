# MOD-ARTIFACT-0038: Restorations of a champion's older content

- **Source:** authors' practice
- **Affects:** a champion's and its skins' models, textures, animations, effects and voice
- **Related:** #MOD-ARTIFACT-0004, #MOD-ARTIFACT-0036, #ADR-0019, #ADR-0023, #ADR-0082

## What is found

A mod that brings back what the game held at older builds: a champion's or a skin's model,
textures, animations or effects from before a rework or a visual update, its voice, or a skin
Riot cancelled for the old model. Many ship several skins of the champion, each with its old
look.

Its name or description says so: "Old Morgana", "Oldest Leblanc", "Old Spirit Guard Udyr 2013",
"pre-rework", "legacy", "restored". A restored voice shows as own clips in the champion's VO
banks: lines the game no longer plays.

The bytes alone do not tell a restoration from a custom skin. Its files are the game's at older
builds, or conversions of them that match no build, and custom skins carry the game's older files
too (#MOD-ARTIFACT-0004). Four shapes say old and are not restorations: one champion's old model
put on another champion, a mod's own old textures moved to a newer container, a new visual in an
old style, and old in another sense than the game's.

## Why it is there

Players want a champion's look or voice back after Riot changed it.

## What reads it, and how

The client reads its files as any mod's, at the slots and paths it ships them.

## The repair

None. The mod is what the author meant: its old files are its work (#ADR-0023), and it ships
each skin on purpose (#ADR-0082).

## Evidence

- Over `baseline-port-4` (2026-10-09): 173 mods say old in their name or description. 157 are
  restorations: 99 of the look, 56 of the look and the voice, 2 of the voice alone. Of the 16
  others, 8 put another champion's old model on theirs, 4 move their own old textures to a newer
  container, 3 say old in another sense, and one is a new visual in an old style.
- 93 of the 157 ship units beside a main unit (`docs/design/RUNS.md`, "Extra units beside a
  champion's main unit").
- The restorations of a voice hold 14 to 505 own VO clips. Old Rengar Rebirth holds 4 and Old
  Xerath 2, beside a restored look.
- A custom skin of the same run carries up to 3,916 game files older than its own build.
