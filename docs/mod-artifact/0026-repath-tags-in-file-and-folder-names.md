# MOD-ARTIFACT-0026: Repath tags in file and folder names

- **Source:** the game's own variant tags (#GAME-CHANGE-0009), kept in a file's name by tools
  that move a skin's files to paths of the mod's own, so that the mod does not override the
  game's files
- **Affects:** the names of the files a mod's objects name, and of the folders they sit in
- **Related:** #ADR-0027, #GAME-CHANGE-0009

## What is found

A file whose name holds, between its stem and its extension, one of the game's variant tags in
one of four shapes:

- `<stem>.skins_<character>_skin<N>.<ext>` or `.skins_<character>_base`: a skin folder;
- `<stem>.<character>_skin<N>.<ext>`: a character and a skin;
- `<stem>.pie_c_<major>_<minor>.<ext>`: a release of the game;
- `<stem>.tft_set<N>.<ext>`: a set of another game mode.

The file usually sits in a folder of the mod's own, under the name it had in the game. A stem
itself may start with `<character>_skin<N>_` or `<character>_base_`, as the game names many of
its own files.

It is told from an author's own name by the shape of the part: a part between two dots that
has one of the four shapes above. A part of any other shape is the author's (`body.v2.tex`), or
one of the game's tags of another shape (`yunara_base.yunara.skl`).

## Why it is there

The game named its files with these tags up to 16.14 (#GAME-CHANGE-0009). A repath tool copies
each file a skin names to a folder of the mod's own and rewrites the references, so that the
mod's files sit beside the game's and replace none. It keeps the file's name, tag included.

## What reads it, and how

The client finds a file by the hash of its whole path and reads no meaning into a name. A tag
changes nothing in game.

## The repair

None is needed for the file to load. A port that places a file under a directory of its own
(#ADR-0027) drops a tag of the four shapes and the leading `<character>_skin<N>_` or
`<character>_base_`, which the directory says, and keeps the rest of the author's name. A step
is taken only when something of the name is left.

## Evidence

- Counted in the file names ltk-forge's reports list, one count per name listed, a name listed
  once per place it appears in a report (2026-10-04). Over the reports of 900 mods of the
  datasets: `.skins_<character>_skin<N>` is the most frequent tag, with 4,600 names for one
  champion's `.tex` files alone, and `.pie_c_<major>_<minor>.dds` has 3,481. Over the reports
  of 2,400 mods the set's tag is written `.tft_set<N>` (572 names with `.tft_set15.tex`, 366
  with `.tft_set13.tex`) and never `.tftset<N>`.
- The file names of the 5,137 mods of the datasets, against the CommunityDragon hash tables and
  the history index of 757 builds (2026-10-06): `.tft_set<N>` in 1,921 names of 838 mods, each a
  file name of the game's own paths; `.pie_c_<major>_<minor>` in 6,967 names of 115 mods and
  `.skins_<character>_skin<N>` in 107,354 names of 1,422 mods, 99.9% of each the game's file
  names. 237 of the `.tft_set<N>` names are paths the game held; the others sit in folders of
  the mods' own. The history holds 92,772 game paths with `.tft_set<N>`, from 9.22 to 16.14,
  and none at 16.19. No game path and no mod's file holds `.tftset<N>`.
- Of the 625 files with `.tft_set<N>` the port's baseline ships, 512 are bytes the game
  shipped: 279 at earlier builds, 233 in a WAD not mounted with the champion.
