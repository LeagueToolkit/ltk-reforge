# MOD-ARTIFACT-0002: A premium skin pasted into the base slot

- **Source:** authors
- **Affects:** `skin0.bin`, the skin entry, its resolver and animation graph, the files they name
- **Related:** #MOD-ARTIFACT-0001

## What is found

A `skin0.bin` whose content derives from a premium skin N of the game. The signs:

- the resolver or the graph link is skin N's, or the entry's `championSkinName` is;
- the mesh, skeleton or texture is skin N's file, by its path or by the bytes of a shipped copy;
- files sit under `characters/<c>/skins/skin<N>/`;
- an audio tag is `<Champion>Skin<NN>`;
- the unit's graph or resolver has fewer edits against skin N's than against any other skin's.

A file of the base that premium skins also use says nothing. The mod's own bytes at the base's
path say nothing either: that is where any base-skin mod puts its files.

## Why it is there

An author starts from the premium skin they like, changes what they want, and saves it as the
base skin so that every player sees it, owner of skin N or not.

## What reads it, and how

The client loads it as the base skin. What plays depends on what was pasted: skin N's clips on
skin N's skeleton play as on skin N. Names of skin N's objects that the client loads only for
skin N resolve only where the mod ships them.

## The repair

On the base, where the work fits the base's rig. Otherwise a mod for skin N.

## Evidence

- ltk-forge's first corpus run (2026-09-29, 1,029 mods): 154 units pasted and fitting the base,
  676 fitting only their premium origin, 30 derived from the base.
- ltk-forge's first full datasets run (2026-09-30, 5,544 mods): 300 pasted and fitting the base,
  607 fitting only their premium origin, 2,209 on the base.
