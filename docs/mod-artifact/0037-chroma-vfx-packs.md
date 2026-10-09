# MOD-ARTIFACT-0037: Chroma VFX packs

- **Source:** authors' practice
- **Affects:** the skin bins at a skin's chroma slots, their particle objects and textures
- **Related:** #MOD-ARTIFACT-0014, #ADR-0018, #ADR-0082

## What is found

A mod that gives each chroma of one skin ability effects in that chroma's colors. It ships a
skin bin at each chroma's slot, and at times one at the skin's own slot. Each bin holds the
effects' particle objects, and most hold textures recolored for that chroma. No bin holds an own
model, skeleton, animation or clip, and none sits at skin0.

Its name or description says so: "chroma VFX", "Abilities match chromas", "All chromas". Read
against the game, two or more champion units ship, all at slots other than skin0, and each
derives from the one premium skin.

The work at each slot is deliberate. It is not a bin carried along by the sharing between skins
(#ADR-0082), and not the game's own skin shipped again (#MOD-ARTIFACT-0014). Two other shapes say
chroma and are not packs: a custom chroma, which recolors the skin's own textures, and a skin put
over a premium skin and each of its chromas, which ships own models or covers the skin itself.

## Why it is there

A chroma is a slot of its own with a skin bin of its own (#ADR-0018), and an effect's colors live
in that bin's objects and the textures they name. An author who wants each chroma's abilities to
match its colors writes every chroma's bin. The descriptions say "Adds matching VFX to every
chroma" and "1 file for all chromas", and leave the skin itself as it is.

## What reads it, and how

The client reads the bin at the slot of the skin a player picks, so each chroma shows its own
effects.

## The repair

None. A pack made today has the same form.

## Evidence

- Over `baseline-port-4` (2026-10-09): 55 mods, 68 archives with their byte-identical copies.
  54 say chroma in their name or description. One says only "Imported from wad", and its units
  hold as many own textures and objects, at the same chromas, as a pack that does.
- None of the 55 has a skin0 unit, and all of them ship units beside a main unit
  (`docs/design/RUNS.md`, "Extra units beside a champion's main unit").
- 307 mods say chroma in their name or description; 220 of them ship one champion unit.
