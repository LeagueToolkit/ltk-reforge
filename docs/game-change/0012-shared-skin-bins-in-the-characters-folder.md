# GAME-CHANGE-0012: Shared skin bins in the character's folder

- **Build:** 16.10.7742490, for every character at once
- **Affects:** `data/<c>_skins_<skins>.bin`, `data/characters/<c>/<c>_multi_skins_<skins>.bin`,
  and the links of every skin bin
- **Related:** #MOD-ARTIFACT-0013, #MOD-ARTIFACT-0015

## Before

Objects that several skins of a character share are in a bin of their own, directly under
`data/`. Its name is the character's, then the skins that share it:

```
data/darius_skins_skin0_skins_skin5_skins_skin6_skins_skin7_skins_skin8.bin
data/darius_skins_root_skins_skin0_skins_skin1_skins_skin14_ ... .bin
```

A skin's bin links the shared bins it has a part in. At 16.9.7728292 Darius's `skin0.bin` links
19 of them, each with `skin0` in its name.

Almost every object in a shared bin is an effect: 595,056 of the 604,313 objects first seen in
one are a `VfxSystemDefinitionData`. The others are mostly materials and contextual actions.

The name holds the set of skins, so a bin for another set is at another path.

At 16.9.7728292 the game holds 4,866 bins directly under `data/`.

## After

A shared bin is in its character's folder, with `_multi` between the character and the skins:

```
data/characters/darius/darius_multi_skins_skin0_skins_skin5_skins_skin6_skins_skin7_skins_skin8.bin
```

The skins' bins link that path. From 16.10.7742490 the game holds no bin directly under
`data/`. It holds 4,721 `_multi_` bins at 16.19.8207193.

## Riot's conversion

- **The path.** `data/<c>_<rest>.bin` becomes `data/characters/<c>/<c>_multi_<rest>.bin`, the
  rest of the name unchanged. 3,750 of the 4,866 bins move so at 16.10.7742490, for 383
  characters.
- **The file.** The same bytes: 3,692 of the 3,750 hold one content under both paths. The other
  58 changed at that build as well.
- **The links.** Each link to a shared bin is rewritten to the new path, and nothing else of the
  linking bin changes. In the `skin0.bin` of 167 champions every link to a bin directly under
  `data/` at 16.9.7728292 is the `_multi_` path at 16.10.7742490, 1,970 links, and the bin holds
  no other new link. In 24 more the set of links changes at that build too.
- **Not moved.** 1,116 bins directly under `data/` end at 16.10.7742490 with no `_multi_` path
  of their name. 1,075 of them belong to characters whose names begin `pet`. Where their
  objects went is not followed here.

## In a mod

A skin bin made from the game's before 16.10 keeps the game's links of its build: the shared
bins at their paths directly under `data/`.

- **The mod does not ship the shared bin.** From 16.10 the link names a path neither the mod nor
  the game holds (`bin.link-missing`). ltk-forge's docs state that the client skips such a
  link. The objects of the game's shared bin are not loaded through it.
- **The mod ships its own copy at the old path and links it.** The client loads the mod's copy
  as before.
- **A mod with no skin bin ships a copy at the old path.** From 16.10 no game bin links that
  path, and the copy is not loaded.
- **A file at the old path that is text.** #MOD-ARTIFACT-0015.

An old path has a `_multi_` path of the same name only when the set of skins in the name is the
set of a build from 16.10 on. 3,776 of the 8,642 paths the game ever held directly under `data/`
ended before 16.10.

## Evidence

- The history index of 755 builds, 8.20.2483196 to 16.19.8207193 (read 2026-10-03), with path
  names from the name tables. A move is counted where the path under `data/` ends at
  16.9.7728292 and the `_multi_` path of the same character and the same rest of the name
  begins at 16.10.7742490. The first `_multi_` path of the history begins at that build.
- The same bytes: a content key held under the old path at its last build and under the new
  path at its first.
- The classes: the class of every object the index first saw in a bin of either form, spelled
  by the name tables.
- The links: the link lists of `skin0.bin` in each of the 202 WADs of the game's `champions`
  folder, read from the census history at 16.9.7728292 and at 16.10.7742490 (2026-10-03). 191
  link a bin directly under `data/` at the earlier build. In 144 of the 167 the list is the
  same but for the moved paths, and in 23 the moved links are in another order.
- The baseline run (2026-10-03): 2,706 of the 5,137 mods hold a bin that links a shared bin at
  its old path, which neither the mod nor the game holds: 300,105 links.
- The same run: 1,489 mods ship a file at an old path, 6,248 files. ltk-forge copied 5,434 of
  them, in 1,463 mods, to the `_multi_` path of the same name: files at a path the game held at
  earlier builds, where the game holds the new path and the mod ships no file there.
- ltk-forge's docs name no dated check in game for the skipped link.
- Other tools (source read 2026-10-05): TopazModFixer looks for a linked shared bin at its old
  path and at its `_multi_` path, either way. League-Mod-Repather adds the game skin bin's links
  to a mod's skin bin, stating that a mod without the game's newer linked bins crashes the
  client when certain abilities are used (Yone's E). It names no build and no check.
