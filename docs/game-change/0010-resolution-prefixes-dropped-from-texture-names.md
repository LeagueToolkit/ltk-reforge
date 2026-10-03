# GAME-CHANGE-0010: Resolution prefixes dropped from texture names

- **Build:** the build a folder's textures move to `.tex` (#GAME-CHANGE-0007), one build from
  15.3 to 15.15 for a champion; from 16.5.7492173 the game holds no prefixed path
- **Affects:** `2x_<name>.dds` and `4x_<name>.dds` beside `<name>.dds`
- **Related:** #GAME-CHANGE-0007, #GAME-CHANGE-0009

## Before

Beside a texture `<name>.dds` the game holds two reduced copies in the same folder:
`2x_<name>.dds` at half the width and height, and `4x_<name>.dds` at a quarter. A copy holds
one mip, and so do most of the full textures.

The copies exist for a skin's own textures: 119,996 of the 139,740 prefixed paths sit directly
in a skin's folder, 10,966 are under `assets/maps/`, and 42 are in a skin's `particles/`. A
prefixed file is a `.dds` in every case, and 69,849 of the 69,891 textures that have one have
both.

Game objects name the full texture and not its copies: 8 of the 139,142 prefixed paths under
`assets/` were ever named by one. When the client reads a reduced copy in place of the full
texture is not established.

The game holds 8,492 prefixed paths at 8.20.2483196 and 68,646 at 14.21.6271015, the most of
any build.

## After

The game holds the texture once, as `<name>.tex` with its mips in the one file, and no prefixed
file beside it. The last prefixed path begins at 16.1.7342383. The game holds 32 prefixed paths
at 16.4.7480682 and none from 16.5.7492173.

## Riot's conversion

The reduced copies are dropped at the build the full texture moves to `.tex`. 71,220 prefixed
paths end at the build their texture's `.tex` path begins:

| build | prefixed paths ended |
|---|---:|
| 13.8.5036621 | 1,900 |
| 14.22.6311160 | 9,374 |
| 15.3.6535410 | 9,072 |
| 15.5.6607315 | 9,368 |
| 15.7.6686132 | 11,464 |
| 15.9.6763588 | 1,950 |
| 15.10.6796849 | 8,248 |
| 15.14.6950510 | 10,010 |
| 15.15.6988035 | 9,368 |
| 15.16.7022986 | 134 |
| 16.5.7492173 | 32 |
| other builds | 300 |

The `.tex` that takes their place holds a mip chain where the `.dds` held one mip
(#GAME-CHANGE-0007). Whether the chain's lower mips hold the pixels of the reduced copies is
not established: the history records a texture's top mip only.

Of the other prefixed paths, 362 end at a build where the `.tex` stood already, and 57,818 are
a tagged name whose tag was dropped with the prefix kept (#GAME-CHANGE-0009). 6,186 end with
their full `.dds`, and 4,154 end another way.

## In a mod

A mod made before its champion's build ships `2x_` and `4x_` files beside its texture, when its
author kept the copies of the game's or made new ones.

- **At a later build** the game holds no file at a prefixed path, and the texture is at
  `<name>.tex`. Whether the client of a later build still asks for a prefixed file is not
  established.
- **The full texture is the mod's `<name>.dds`**, and that file follows #GAME-CHANGE-0007.
- **A mod that ships a reduced copy alone** holds its texture at a half or a quarter of the
  size. The `4x_` file is the smaller of the two.

## Evidence

- The history index of 755 builds, 8.20.2483196 to 16.19.8207193 (read 2026-10-03), with path
  names from the name tables. A prefixed path is one whose file name begins `2x_` or `4x_`:
  69,857 and 69,883 paths. A file's tagged and untagged names count as two paths.
- The sizes: the index's texture records of a prefixed file and of its full texture at the
  prefixed path's last build, over a sample of 6,000. The full texture is twice the `2x_` file
  on each side in 2,920 of 2,955 pairs with a record on both sides, and four times the `4x_`
  file in 3,007 of 3,045. The prefixed file holds one mip in all but 2 of the 6,000, and the
  full `.dds` holds one mip in 5,600.
- The names in bins are counted over every prefixed path under `assets/` and every build.
- The baseline run (2026-10-03): 1,445 of the 5,137 mods ship a file under a prefixed name,
  15,296 files. ltk-forge copied 947 of them, in 106 mods, to the unprefixed path: files at a
  path the game held at earlier builds, where the mod ships no file for the unprefixed name.
  It passed over 11,255 files in 783 mods, under this rule and the tag rule together
  (#GAME-CHANGE-0009), because the mod ships a file for the asset already.
- ltk-forge takes the `4x_` file before the `2x_` file when a mod ships both and no full
  texture. By the sizes above that is the smaller file.
