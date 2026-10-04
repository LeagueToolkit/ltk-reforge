# GAME-CHANGE-0007: Textures in the TEX container

- **Build:** 12.1 for the first `.tex` path; a champion's skin folders at one build each, from
  15.3 to 15.15 by the champion's name (below)
- **Affects:** `.dds`, `.tex`, and every bin value that names a texture by its path
- **Related:** #GAME-BEHAVIOR-0003, #GAME-CHANGE-0009, #GAME-CHANGE-0010

## Before

The game holds a texture as a DDS file at a path ending `.dds`, and a bin value names it by
that path. Most hold one mip. A skin's main textures have reduced copies in files of their own
(#GAME-CHANGE-0010).

At 14.18.6182357 the game holds 377,854 `.dds` paths and 22,173 `.tex` paths.

## After

The game holds the texture in the TEX container at the same stem with the extension `.tex`, and
the bin values name that path. The first `.tex` path is at 12.1.4143965, under `assets/ux/`.
Files new to the game begin as `.tex` from then on in some folders: 32,321 `.tex` paths begin
before 14.19, 8,830 of them under `assets/characters/`.

The files the game already held move in waves, a folder at a time:

| build | paths | what moved |
|---|---:|---|
| 12.2 to 12.5 | 693 | `assets/ux/` |
| 13.8.5036621 | 1,273 | `assets/maps/` |
| 14.19.6195285, 14.20.6232808 | 6,023 | characters' `hud/` folders |
| 14.22.6311160 | 56,235 | the folders of 673 characters, 575 whose names begin `tft` and 88 `pet`; `assets/maps/`; `assets/shared/` |
| 15.1.6450085 | 3,070 | textures directly in a skin's folder, 224 characters |
| 15.3.6535410 | 36,762 | champions Aatrox to Dr. Mundo |
| 15.5.6607315 | 37,411 | champions Ekko to Jinx |
| 15.7.6686132 | 44,671 | champions Kai'Sa to Morgana |
| 15.9.6763588 | 9,581 | turrets, the nexus and 132 other characters; `assets/maps/` |
| 15.10.6796849 | 34,324 | champions Naafiri to Ryze |
| 15.14.6950510 | 41,774 | champions Samira to Twitch |
| 15.15.6988035 | 38,440 | champions Udyr to Zyra, and LeBlanc |
| 15.16.7022986 | 19,598 | `assets/loadouts/`, `assets/ux/`, `assets/shared/`, `assets/items/` |

A champion's wave moves its `skins/` folders: the skins' own textures and their `particles/`.
A sub-character moves with its champion.

At 16.19.8207193 the game holds 530,942 `.tex` paths and 5,906 `.dds` paths. The `.dds` left
are 2,459 under `data/`, 1,893 spell icons under `hud/icons2d/`, 513 under `assets/items/`, 429
with `cube` in their name, 412 under `assets/ux/`, 129 under `assets/spells/` and 71 others.

## Riot's conversion

- **The path.** The stem stays and the extension becomes `.tex`. 331,770 `.dds` paths end at
  one build with a `.tex` path of the same stem beginning at the next, 286,631 of them under
  `assets/characters/`. A variant tag in the name stays through the move (#GAME-CHANGE-0009).
- **The names in bins.** The game objects that name the `.dds` path name the `.tex` path from
  that build. No game object names the `.dds` path after it.
- **The pixels.** Riot encoded the textures again. In a sample of 20,000 moved textures the size
  is the same in 19,756, and the top mip's bytes are the same in 497.
- **The mips.** 17,263 of the 20,000 go from one mip to a chain. 1,402 hold a chain before and
  after, 1,331 hold one mip before and after, and 4 go from a chain to one mip.
- **The format**, as the census spells it for each container:

  | DDS | TEX | of 20,000 |
  |---|---|---:|
  | `bc3_unorm_srgb` | `bc3` | 12,724 |
  | `bc1_unorm_srgb` | `bc1` | 5,965 |
  | `bc3_unorm_srgb` | `bc1` | 751 |
  | `bc1_unorm_srgb` | `bc3` | 412 |
  | `bc2_unorm_srgb` | `bc3` | 116 |
  | `bc2_unorm_srgb` | `bc1` | 20 |
  | `bc3_unorm_srgb` | `bc7` | 12 |

- **The reduced copies** are dropped with the move (#GAME-CHANGE-0010).

Not established: the bytes of a TEX file's header, and what Riot's encoder did with a texture
beyond the format, the size, the mip count and the top mip. The history holds no file bytes.

## In a mod

A mod made before its champion's build ships `.dds` files at the game's `.dds` paths, and its
bins name `.dds` paths.

- **A file at a path the game dropped.** No game object names it at a later build. A mod with no
  skin bin that ships only such files replaces nothing: the game reads the texture from the
  `.tex` path.
- **A mod's bin that names a `.dds` the mod ships.** The client reads the file as before, by its
  magic (#GAME-BEHAVIOR-0003).
- **A mod's bin that names a `.dds` of the game's, which the mod does not ship.** The path is
  one the game does not hold.
- **The mod's DDS at the `.tex` path.** It loads there as it is (#GAME-BEHAVIOR-0003). The game
  itself never held a DDS file at a `.tex` path, so no game file shows this.

## Evidence

- The history index of 755 builds, 8.20.2483196 to 16.19.8207193 (read 2026-10-03). Path names
  come from the name tables, which spell 2,146,367 of its 2,147,213 paths. The paths no table
  spells are not counted.
- A move is counted where a `.dds` path's last build is followed by the first build of a `.tex`
  path of the same stem. The table counts moves by the build the `.tex` path begins at. A
  champion is listed at the build most of its paths moved at, for characters with 150 moves or
  more.
- The pixels, the mips and the formats are read off the index's texture records at the `.dds`
  path's last build and the `.tex` path's first, over a sample of 20,000 of the 331,770 moves.
- The names in bins: of 1,000 sampled moves, 997 `.dds` paths were named by a game object up to
  their last build, and their `.tex` paths from the next. No object names one of the `.dds`
  paths at a later build. The other 3 are named by no game object.
- By its magic, every file the game held at a `.tex` path is a TEX file or of no known kind
  (801,843 and 450 contents), and every file at a `.dds` path is a DDS file or of no known kind
  (830,776 and 364).
- The meta database (generation 2026-09-28, read 2026-10-03): `VfxEmitterDefinitionData.texture`
  is a string from 13.15 to 16.19, and `SkinMeshDataProperties.texture` is a string from 13.15
  to 16.16 and a file hash from 16.17. The value is the path with its extension under either
  type.
- The baseline run (2026-10-03): 4,078 of the 5,137 mods ship a file under a `.dds` name,
  378,714 files. ltk-forge copied 112,550 of them, in 1,870 mods, to the `.tex` path of the same
  stem: files at a path the game dropped, where the game holds the `.tex` and the mod ships no
  file there.
- Of the `.tex` files made for those mods, 30 in 11 mods are made from a `.dds` at a path the
  game never held (the history index, 2026-10-04). All 30 are one case: a champion's HUD icons
  and loading-screen picture under a name with its variant tag dropped (#GAME-CHANGE-0009) and
  the old extension, where the game held the tagged `.dds` and holds the untagged `.tex`. No mod
  of the datasets ships a `.dds` of its own making beside a game `.tex` of the same stem.
