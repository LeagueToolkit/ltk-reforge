# GAME-CHANGE-0009: Variant tags dropped from asset names

- **Build:** one build per file, at 67 patches from 8.22 to 16.15; from 16.15.7983109 the game
  holds no tagged path under `assets/`
- **Affects:** paths under `assets/`: `.dds`, `.tex`, `.scb`, `.sco`, `.anm`, `.skn`, `.skl`,
  and every bin value that names one
- **Related:** #GAME-CHANGE-0007, #GAME-CHANGE-0008, #GAME-CHANGE-0010, #GAME-CHANGE-0011

## Before

A file's name holds a tag between its stem and its extension, `<stem>.<tag>.<ext>`:

```
assets/characters/yunara/skins/base/yunara_base.yunara.skl
assets/characters/gwen/skins/skin38/gwen_skin38_weapons_tx_cm.chroma_gwen_battlequeens.tex
assets/characters/jinx/skins/skin62/particles/jinx_skin62_r_tar_star005.skins_jinx_skin62.dds
```

A tag is a name such as `skins_jinx_skin62`, `tft_set7`, `pie_c_12_2` or `leblanc_rework`. The
history holds 3,509 tags. What Riot's tools use a tag for is not established. The bin values
that name the file hold the tagged path.

The game holds 6,895 tagged paths at 8.20.2483196 and 218,449 at 16.8.7632820, the most of any
build.

## After

The file is at the same folder and stem with no tag, `<stem>.<ext>`, and the bin values name
that path.

Riot drops tags a batch at a time, at 67 patches from 8.22 to 16.15. A tagged path lives 90
builds at the median before its tag is dropped, and 256 at the most. The largest batches:

| build | tags dropped |
|---|---:|
| 13.1.4875491 | 41,056 |
| 13.22.5399807 | 21,280 |
| 13.23.5434798 | 22,882 |
| 16.9.7690805 | 163,447 |
| 16.14.7937003 | 23,556 |
| 16.15.7983109 | 53,658 |

The last tagged path begins at 16.14.7937003. The game holds 55,135 tagged paths at
16.14.7949266 and none at 16.15.7983109: 53,658 lose their tag there and 1,477 end another way.
From that build to 16.19.8207193 the game holds no tagged path under `assets/`.

## Riot's conversion

- **The path.** Everything from the first dot of the file's name to its last is dropped. 633,243
  tagged paths end at one build with the untagged path beginning at the next, 530,561 of them
  under `assets/characters/`.
- **The file.** The same bytes. Of 10,000 sampled moves, 9,999 hold one content under both
  paths.
- **The names in bins.** The game objects that name the tagged path name the untagged path from
  that build. No game object names the tagged path after it.

The other tagged paths, of 754,708:

| end | paths |
|---|---:|
| the tagged `.dds` ends and the tagged `.tex` begins (#GAME-CHANGE-0007) | 50,392 |
| the game never held the untagged path | 36,740 |
| the untagged path begins at a later build | 19,112 |
| the game held the untagged path beside the tagged one | 10,967 |
| the untagged path ended before the tagged one | 4,227 |
| the untagged `.tex` begins at the next build | 27 |

## In a mod

A mod made before a file's tag was dropped ships its file at the tagged path, and its bins name
the tagged path.

- **A file at a tagged path the game dropped.** No game object names it at a later build. A mod
  with no skin bin that ships only such files replaces nothing: the game reads the asset from
  the untagged path.
- **A mod's bin that names a tagged file the mod ships.** The client reads it as before.
- **A mod's bin that names a tagged file of the game's, which the mod does not ship.** The path
  is one the game does not hold.

Riot moved the same bytes, so the mod's file goes to the untagged path as it is. Where the
untagged path has since become a `.tex` or a `.scb`, that change applies as well
(#GAME-CHANGE-0007, #GAME-CHANGE-0008).

A dot in a name is no tag by itself. An author's `foo.backup.dds` has the shape of a tagged
name, and the game never held that path. The history tells the two apart.

## Evidence

- The history index of 755 builds, 8.20.2483196 to 16.19.8207193 (read 2026-10-03), with path
  names from the name tables. A tagged path is a path under `assets/` whose file name holds more
  than one dot. Shader files under `assets/shaders/` are named that way for another reason and
  are left out.
- 754,708 tagged paths: 404,491 `.dds`, 204,733 `.tex`, 65,842 `.scb`, 50,572 `.anm`, 12,795
  `.skn`, 12,788 `.skl`, 3,172 `.sco` and 315 others. None is held at 16.19.8207193.
- A drop is counted where the tagged path's last build is followed by the first build of the
  untagged path. The 633,243 are 313,130 `.dds`, 189,932 `.tex`, 60,443 `.scb`, 44,025 `.anm`,
  11,774 `.skn`, 11,770 `.skl`, 1,958 `.sco` and 211 others.
- The same bytes: the content keys under the tagged path at its last build and under the
  untagged path at its first, over two samples of 5,000 drops. 5,000 of 5,000 over every folder
  and 4,999 of 5,000 under `assets/characters/` share a key.
- The names in bins: of 2,000 sampled drops, 1,998 tagged paths were named by a game object up
  to their last build, and their untagged paths from the next. No object names one of the
  tagged paths at a later build. The other 2 are named by no game object.
- The baseline run (2026-10-03): 2,327 of the 5,137 mods ship a file under a tagged name, 179,570
  files. ltk-forge copied 46,737 of them, in 786 mods, to the untagged path, a `.dds` to the
  untagged `.tex` where the game holds that one: files at a path the game held at earlier
  builds, where the game holds the untagged path and the mod ships no file there. 24 of the
  46,737 also lost a resolution prefix (#GAME-CHANGE-0010).
