# MOD-ARTIFACT-0033: Effects moved into a champion's vfx bin

- **Source:** League-Mod-Repather, the repath tool that preceded Hematite
- **Affects:** `data/<champion>_vfx.bin`, the `VfxSystemDefinitionData` objects of a skin's bins,
  and the links of those bins
- **Related:** #MOD-ARTIFACT-0029, #MOD-ARTIFACT-0034, #GAME-CHANGE-0012

## What is found

A bin at `data/<champion>_vfx.bin`, the champion's name in lower case, that links nothing and
holds every `VfxSystemDefinitionData` of the skin: one object per name, the first one found
kept. The bins of `data/characters/` that held effects hold none, and each links
`data/<champion>_vfx.bin` as its last link.

The path holds the champion's name alone, not the skin. The game has held no bin directly under
`data/` since 16.10 (#GAME-CHANGE-0012), and never one of this name.

The mods that carry it carry the tool's other marks: its repath prefix (#MOD-ARTIFACT-0029),
often its transparent placeholders (#MOD-ARTIFACT-0034), and in `info.json` a name ending
` (Repathed)` and a description ending `Repathed with League Mod Repather.`

## Why it is there

The tool moves the effects out of the skin's bins after it has merged the bins they link. Its
source gives no reason.

## What reads it, and how

The client loads the bin through the links of the skin's bins: the effects load and resolve as
they did. Two mods of one champion made by the tool ship a bin at the same path. Installed
together, one of the two bins is loaded and both skins' links reach it, so one mod's skin loads
the other mod's effects.

## The repair

None is needed for the effects to load. The objects are the skin's own, in a bin of another
name.

## Evidence

- The baseline repair run (2026-10-04), over the 5,137 mods of the datasets: 265 mods ship 268
  bins at `data/<champion>_vfx.bin`, each loaded. 92 of the mods also hold files under the
  tool's word prefix (#MOD-ARTIFACT-0029), 55 its placeholders, 35 its text in `info.json`. Of
  the 173 with no word prefix, 162 hold files under a second folder of another name followed by
  a game category, as the tool writes a prefix its user gives.
- The bin's layout and the links are read from the source of League-Mod-Repather (2026-10-05).
  Nothing was run.
