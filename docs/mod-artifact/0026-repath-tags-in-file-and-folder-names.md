# MOD-ARTIFACT-0026: Repath tags in file and folder names

- **Source:** tools that move a skin's files to paths of the mod's own, so that the mod does
  not override the game's files
- **Affects:** the names of the files a mod's objects name, and of the folders they sit in
- **Related:** #ADR-0027, #GAME-CHANGE-0009

## What is found

A file whose name holds, between its stem and its extension, a tag that says where the tool
took the file from:

- `<stem>.skins_<character>_skin<N>.<ext>` or `.skins_<character>_base`: the skin folder;
- `<stem>.<character>_skin<N>.<ext>`: the character and the skin;
- `<stem>.pie_c_<major>_<minor>.<ext>`: the game's release;
- `<stem>.tft_set<N>.<ext>`: a set of another game mode.

A stem itself may start with `<character>_skin<N>_` or `<character>_base_`, as the game names
many of its own files.

It is told from an author's own name by the shape of the part: a part between two dots that
has one of the four shapes above. A part of any other shape is the author's (`body.v2.tex`).

The game's own variant tag (#GAME-CHANGE-0009) has the same place in a name and another shape:
`yunara_base.yunara.skl`.

## Why it is there

A repath tool copies each file a skin names to a new path and rewrites the references, so that
the mod's files sit beside the game's and replace none. It builds the new name from the old
path's folders, which keeps two files of one stem from different folders apart.

## What reads it, and how

The client finds a file by the hash of its whole path and reads no meaning into a name. A tag
changes nothing in game.

## The repair

None is needed for the file to load. A port that places a file under a directory of its own
(#ADR-0027) drops the tag and the leading `<character>_skin<N>_` or `<character>_base_`, which
the directory says, and keeps the rest of the author's name. A step is taken only when
something of the name is left.

## Evidence

- Counted in the file names ltk-forge's reports list, one count per name listed, a name listed
  once per place it appears in a report (2026-10-04). Over the reports of 900 mods of the
  datasets: `.skins_<character>_skin<N>` is the most frequent tag, with 4,600 names for one
  champion's `.tex` files alone, and `.pie_c_<major>_<minor>.dds` has 3,481. Over the reports
  of 2,400 mods the set's tag is written `.tft_set<N>` (572 names with `.tft_set15.tex`, 366
  with `.tft_set13.tex`) and never `.tftset<N>`.
- How many mods carry a tag was not counted.
