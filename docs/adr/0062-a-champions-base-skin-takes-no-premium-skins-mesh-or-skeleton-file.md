# ADR-0062: A champion's base skin takes no premium skin's mesh or skeleton file

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0013, #ADR-0015, #ADR-0016, #ADR-0021, #MOD-ARTIFACT-0002;
  `docs/design/ARCHITECTURE.md`, "Decide" and "Package"

## Context

A check of the base skin looks for a premium skin put in the base's place. It runs once per
champion WAD, on the skin0 entry of the champion the WAD is named after, and reads six values
of that entry for a skin other than 0 of that one champion:

| value of the skin0 entry | names another skin when it holds |
|---|---|
| `championSkinName` | `<Champion>Skin<N>` |
| a tag of `skinAudioProperties.tagEventList` | `<Champion>Skin<N>` |
| the `name` of a unit of `skinAudioProperties.bankUnits` | `<Champion>_Skin<N>` with `_SFX` after it |
| `skinMeshProperties.skeleton` | `<champion>/skins/skin<N>` |
| `skinMeshProperties.simpleSkin` | `<champion>/skins/skin<N>` |
| `skinMeshProperties.texture` | `<champion>/skins/skin<N>` |

It reads a value only where the entry holds it as a string. `texture` is a file hash from
16.17 on, so a bin typed for the build spells no path there. It reads nothing else: an effect,
a material, the animation graph, the resolver, a voice unit and every object the entry links
are outside it, as are another champion's skins and a sub-character's skin0 entry. The game's
own base skins name premium skins' files in their effects (the user, 2026-10-05).

A port to skin0 carries no `championSkinName`, keeps the game's audio block with units and tags
named after the base's tag, and ships its own files under its skin0 directory. A file the game
holds is named at the game's path. A mesh that is a premium skin's file does not fit the base
(#ADR-0013). A skeleton that is a premium skin's file with the base's joints did fit it, and a
port to skin0 then named that skin's path in `skeleton`.

## Decision

The user, 2026-10-05:

- **A champion's unit on a premium skin's mesh or skeleton file, as the game ships it, does
  not go to skin0.** It goes to that premium skin. A skeleton file of a premium skin does not
  fit the base though it has the base's joints.
- **A premium skin on the base's own mesh or skeleton file is the base's.** Such a file fits
  the base.
- **A sub-character's unit may take another skin's file.** Its checks are as they were.
- **A game file in a premium skin's folder that is no skin's mesh or skeleton is not judged by
  this rule.** The check of the base skin is adjusted for it.

A unit kept off the base by a premium skin's skeleton file is weighed at its premium origin,
then at the skin whose clips it mostly plays (#ADR-0015), then at each skin whose skeleton the
file is, and goes to the first it fits. Where it fits none it stays on skin0 with an error
(#ADR-0016).

The check `base-skin` reads the patched skin0 entry of each champion a port patches for the
six values, and raises `check.other-skin`.

## Consequences

- Over the 5,137 mods of the datasets (2026-10-05, the history index of 757 builds): 3,907
  ports patch a champion's base skin. Before the rule 5 are flagged. One is a unit on skin 7's
  skeleton file with the base's joints under an own mesh, decided for the base: it goes to
  skin 7 by its skeleton, and no other decision changes. Four name the old base model the
  game keeps in skin 5's folder, by `skeleton` and `simpleSkin`, and stay flagged.
- Players of the base do not see a mod that goes to a premium skin by this rule.
- A unit that fits no slot (#ADR-0016), and a target forced to the base, can still name a
  premium skin's skeleton in a skin0 entry. `check.other-skin` reports it.
- The base candidate's skeleton check reads `premium-skeleton` where it read `slot-joints`,
  for units in premium slots too: 86 mods' facts differ, with no change of target.
