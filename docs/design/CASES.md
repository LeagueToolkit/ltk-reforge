# Edge cases

What an edge case is, its baseline and its test: #ADR-0045. This file is the baseline of
every edge case the corpus shows: how many mods show it, and one of them. It also lists the
edge cases the program declares that no mod of the corpus shows.

The two tables are generated. `scripts/corpus/cases.py --doc` writes both again from the
reports of the baseline runs, between the marker lines, and `scripts/README.md` says when.
The text around them is written by hand. The counts are of the runs the tables name: a count
is a measurement of those runs, with their date.

The reduced corpus, `scripts/corpus/reduced.json`, is the fewest mods that together show
every case of the first table. It lists, per mod, the cases the mod was taken for.

## How a case is named

A case is a key that `cases.py` derives from a report.

| key | the case |
|---|---|
| `archive.no-skin` | a diagnostic code: the mod raises it ([CODES.md](CODES.md)) |
| `fixup.changed by=retype` | a diagnostic raised by that fixup or that check |
| `audio.not-carried why=no-event` | a value of a closed set in a diagnostic's data |
| `file use=stale` | a value of a closed set at a place of the report: here a file's use |
| `wad restores a model from before a rework` | a field of the report that is set, or a count above zero |
| `fixup retype` | a fixup that changed a value of the mod |
| `fixup retype as=value` | a kind of change that fixup made: `value`, `moved`, `removed`, `lost`, `unconvertible` |
| `fixup retype String>File` | the kinds a value went between, by that fixup. `nothing>Bool` is a value the fixup added |
| `fixup retype List wants F32` | a value left as it is, with the kind the target build reads there |
| `halt package.empty` | the fatal diagnostic that ended the run |
| `record #GAME-CHANGE-0007` | a record a diagnostic of the mod rests on |

The table of places in `cases.py` (`RULES`) says which fields of a report are closed sets.
A value the program adds to a closed set shows up as a new case. A field the table does not
know is printed by the script.

The areas follow a run: `run` (what ended it, its other inputs), `archive` (the zip, its
WADs, what else it carries), `load` (files, characters, units, dating, whose bytes a file
holds), `fixups`, `facts` (copies, origin, rig, clips, the checks of a slot), `decision`,
`plan` (what ships, what an entry reaches, where a file is found, the renames), `audio`,
`package` (chunks, patches, what is left out, the repair repaired again), `checks`, and
`records`.

A case is counted once per mod, whichever run shows it. The same finding can be two cases: a
unit that leaves the port is `unit left=copy` in the plan and `unit.left why=copy` in the
diagnostic's data.

## Cases the corpus shows

The example is the mod showing the fewest cases among those that show the case, named by the
first twelve digits of its SHA-256. `cases.tsv`, beside the reports, holds five examples per
case, the occurrences, and the runs that show it.

<!-- cases.py: cases -->

668 cases in 4,909 mods, from the runs `port` and `repair` of 2026-10-09.

### run

| case | mods | example |
|---|---:|---|
| `halt archive.no-skin` | 1 | `4924914fd7a3` |
| `halt package.empty` | 13 | `8b880d06d01c` |
| `halt package.shader-missing` | 24 | `b96f6a6c097c` |

### archive

| case | mods | example |
|---|---:|---|
| `archive holds=animation` | 2,638 | `98106ae9af46` |
| `archive holds=bmp` | 12 | `ab45b36f1839` |
| `archive holds=desktop-ini` | 9 | `679e38c0f295` |
| `archive holds=empty` | 29 | `dfd252fe6457` |
| `archive holds=exr` | 19 | `e349f123f06f` |
| `archive holds=fbx-binary` | 50 | `0012565578f4` |
| `archive holds=gif` | 5 | `74d70c039957` |
| `archive holds=gltf-binary` | 6 | `43cd5b57d6a7` |
| `archive holds=ico` | 1 | `37796a2c7824` |
| `archive holds=ini` | 160 | `61cc49003bc6` |
| `archive holds=inibin` | 4 | `d393d47c4e1e` |
| `archive holds=item-recommendations` | 12 | `b1f7b2c54de0` |
| `archive holds=jpeg` | 20 | `836edb7ed0cc` |
| `archive holds=json` | 243 | `40b00e9c3a2d` |
| `archive holds=markdown` | 23 | `679e38c0f295` |
| `archive holds=maya-binary` | 7 | `305f42936229` |
| `archive holds=maya-image` | 394 | `7ba786206e67` |
| `archive holds=maya-swatches` | 2 | `7b779ee3c5ec` |
| `archive holds=otf` | 6 | `3d5f554cff71` |
| `archive holds=pdn` | 31 | `3d955a3880d7` |
| `archive holds=pe` | 66 | `9d15cbf8cc63` |
| `archive holds=png` | 273 | `318a7ebe0cb8` |
| `archive holds=property-bin` | 4,908 | `690378161a3c` |
| `archive holds=psd` | 16 | `def699cffb6d` |
| `archive holds=r3d2` | 1 | `6e3d4b1755d9` |
| `archive holds=rar` | 8 | `0583e723cd3d` |
| `archive holds=ritobin-json` | 629 | `8a2c7153fd2a` |
| `archive holds=ritobin-text` | 1,610 | `3b1aaabea06e` |
| `archive holds=shortcut` | 10 | `d686e249d4c1` |
| `archive holds=simple-skin` | 4,377 | `042eb2b27725` |
| `archive holds=skeleton` | 4,376 | `042eb2b27725` |
| `archive holds=static-mesh-ascii` | 1,072 | `8eee9126c83f` |
| `archive holds=static-mesh-binary` | 2,640 | `d8cdb6ba220a` |
| `archive holds=string-table` | 1 | `a26e64fdac59` |
| `archive holds=subchunk-toc` | 82 | `767f0956dd3d` |
| `archive holds=text` | 124 | `8f856aebd68d` |
| `archive holds=texture` | 4,344 | `690378161a3c` |
| `archive holds=texture-dds` | 3,997 | `8b880d06d01c` |
| `archive holds=tga` | 11 | `543b88a9b9d4` |
| `archive holds=unknown` | 16 | `8dc89140f5bc` |
| `archive holds=wad` | 10 | `40b00e9c3a2d` |
| `archive holds=wav` | 19 | `493bcf426273` |
| `archive holds=web-p` | 6 | `f3e9d21184f1` |
| `archive holds=wwise-bank` | 2,685 | `b96f6a6c097c` |
| `archive holds=wwise-package` | 1,761 | `eb88a666d18a` |
| `archive holds=xcf` | 71 | `43cd5b57d6a7` |
| `archive holds=xml` | 4 | `158e6d400f92` |
| `archive holds=zip` | 16 | `003216dbe678` |
| `archive holds=zstd` | 6 | `66b3a98b2c72` |
| `archive.chunk-duplicate` | 11 | `0672cfff925b` |
| `archive.info-unreadable` | 2 | `fcb644d8a00b` |
| `archive.no-skin` | 1 | `4924914fd7a3` |
| `inventory confidence=magic` | 2,001 | `3b1aaabea06e` |
| `inventory confidence=name` | 2 | `679e38c0f295` |
| `inventory finding=bin-text` | 64 | `a3fd517fd99a` |
| `inventory finding=executable` | 66 | `9d15cbf8cc63` |
| `inventory finding=nested-archive` | 29 | `003216dbe678` |
| `inventory finding=os-artifact` | 19 | `679e38c0f295` |
| `inventory finding=source` | 1,957 | `3b1aaabea06e` |
| `inventory finding=unknown` | 16 | `8dc89140f5bc` |
| `inventory item=desktop-ini` | 9 | `679e38c0f295` |
| `inventory item=exr` | 19 | `e349f123f06f` |
| `inventory item=fbx-binary` | 50 | `0012565578f4` |
| `inventory item=gltf-binary` | 6 | `43cd5b57d6a7` |
| `inventory item=maya-binary` | 7 | `305f42936229` |
| `inventory item=maya-image` | 394 | `7ba786206e67` |
| `inventory item=maya-swatches` | 2 | `7b779ee3c5ec` |
| `inventory item=pdn` | 31 | `3d955a3880d7` |
| `inventory item=pe` | 66 | `9d15cbf8cc63` |
| `inventory item=psd` | 16 | `def699cffb6d` |
| `inventory item=rar` | 8 | `0583e723cd3d` |
| `inventory item=ritobin-json` | 629 | `8a2c7153fd2a` |
| `inventory item=ritobin-text` | 1,610 | `3b1aaabea06e` |
| `inventory item=shortcut` | 10 | `d686e249d4c1` |
| `inventory item=unknown` | 16 | `8dc89140f5bc` |
| `inventory item=xcf` | 71 | `43cd5b57d6a7` |
| `inventory item=zip` | 16 | `003216dbe678` |
| `inventory item=zstd` | 6 | `66b3a98b2c72` |
| `inventory.bin-text` | 64 | `a3fd517fd99a` |
| `inventory.executable` | 66 | `9d15cbf8cc63` |
| `inventory.nested-archive` | 29 | `003216dbe678` |
| `inventory.os-artifact` | 19 | `679e38c0f295` |
| `inventory.source` | 1,957 | `3b1aaabea06e` |
| `inventory.unknown` | 16 | `8dc89140f5bc` |
| `wad a locale's, holding the whole mod` | 71 | `975ff083bcb8` |
| `wad layout=loose` | 175 | `6de493211fd6` |
| `wad layout=packed` | 4,743 | `4924914fd7a3` |
| `wad restores a model from before a rework` | 16 | `70e59f043f84` |
| `wad role=champion` | 4,908 | `690378161a3c` |
| `wad role=locale` | 459 | `eb88a666d18a` |
| `wad role=other` | 24 | `4924914fd7a3` |
| `wad.champion-guessed` | 8 | `0672cfff925b` |
| `wad.duplicate` | 3 | `e5f3a984b590` |
| `wad.duplicate-differs` | 1 | `62b4bb9d2d67` |
| `wad.locale` | 459 | `eb88a666d18a` |
| `wad.locale-packed` | 71 | `975ff083bcb8` |
| `wad.not-carried` | 20 | `4924914fd7a3` |
| `wad.several-champions` | 7 | `29399a3faed0` |

### load

| case | mods | example |
|---|---:|---|
| `bin.entry-duplicate` | 99 | `05ad37f99624` |
| `bin.link-missing` | 2,627 | `b96f6a6c097c` |
| `bin.link-missing former=false` | 31 | `6bae337c0ee7` |
| `bin.link-missing former=true` | 2,606 | `b96f6a6c097c` |
| `bin.taken-from-game` | 1,218 | `690378161a3c` |
| `bin.taken-from-game from a former bin` | 57 | `8b880d06d01c` |
| `bin.taken-from-game history=true` | 1,218 | `690378161a3c` |
| `bin.taken-from-game with files no bin names` | 407 | `cb2c00df0e69` |
| `bin.taken-from-game with files the game never held` | 380 | `1310624b2b01` |
| `bin.taken-from-game with stale files` | 184 | `c92cdc9e684c` |
| `bin.unreadable` | 58 | `a3fd517fd99a` |
| `character equal to the game's` | 206 | `8b880d06d01c` |
| `character kind=champion` | 4,908 | `690378161a3c` |
| `character kind=sub` | 660 | `8b880d06d01c` |
| `character with a root bin` | 237 | `a2c700b70d1d` |
| `character.equal-to-game` | 206 | `8b880d06d01c` |
| `dating evidence=file` | 2,878 | `8b880d06d01c` |
| `dating evidence=object` | 1,137 | `b96f6a6c097c` |
| `dating with no bound` | 893 | `690378161a3c` |
| `file kind=anm` | 2,638 | `98106ae9af46` |
| `file kind=bin` | 4,908 | `690378161a3c` |
| `file kind=bnk` | 2,685 | `b96f6a6c097c` |
| `file kind=dds` | 3,998 | `4924914fd7a3` |
| `file kind=other` | 2,245 | `8f856aebd68d` |
| `file kind=scb` | 2,752 | `d8cdb6ba220a` |
| `file kind=sco` | 1,072 | `8eee9126c83f` |
| `file kind=skl` | 4,376 | `042eb2b27725` |
| `file kind=skn` | 4,377 | `042eb2b27725` |
| `file kind=tex` | 4,345 | `4924914fd7a3` |
| `file kind=wpk` | 1,761 | `eb88a666d18a` |
| `file made by=bank-id` | 1,368 | `7bc34b21a01b` |
| `file made by=legacy-format` | 1,895 | `b770e36141eb` |
| `file made by=moved-asset` | 1,835 | `088d0a057ec0` |
| `file source=archive` | 4,909 | `4924914fd7a3` |
| `file source=game` | 1,218 | `690378161a3c` |
| `file source=made` | 2,720 | `088d0a057ec0` |
| `file source=rewritten` | 1,368 | `7bc34b21a01b` |
| `file use=former-path` | 2,749 | `8b880d06d01c` |
| `file use=loaded` | 4,908 | `690378161a3c` |
| `file use=override` | 2,988 | `b96f6a6c097c` |
| `file use=referenced` | 4,900 | `690378161a3c` |
| `file use=stale` | 1,600 | `10ab724f95a9` |
| `file use=unused` | 3,075 | `4924914fd7a3` |
| `file with no path spelled` | 2,995 | `1310624b2b01` |
| `file.format-converted` | 1,895 | `b770e36141eb` |
| `file.format-converted by=legacy-formats` | 1,895 | `b770e36141eb` |
| `file.format-converted how=as-is` | 2 | `6e3d4b1755d9` |
| `file.format-converted how=dds-as-tex` | 1,804 | `b770e36141eb` |
| `file.format-converted how=sco-as-scb` | 334 | `032db643a88a` |
| `file.former-path` | 2,749 | `8b880d06d01c` |
| `file.moved` | 1,835 | `088d0a057ec0` |
| `file.moved by=moved-assets` | 1,835 | `088d0a057ec0` |
| `file.moved how=as-is` | 1,777 | `088d0a057ec0` |
| `file.moved how=dds-as-tex` | 453 | `7ceb25cc7fc6` |
| `file.moved how=sco-as-scb` | 60 | `2ad573a25725` |
| `file.moved passes a copy over` | 760 | `088d0a057ec0` |
| `file.moved rename=base-suffix` | 34 | `088d0a057ec0` |
| `file.moved rename=resolution-prefix-2x` | 98 | `1ab1481ba822` |
| `file.moved rename=resolution-prefix-4x` | 1 | `0bdfa15cb5b7` |
| `file.moved rename=shared-bin` | 1,433 | `40e2361d74da` |
| `file.moved rename=variant-tag` | 758 | `012afca8b8bb` |
| `file.moved rename=variant-tag-and-prefix-2x` | 11 | `3c7cd0bdba70` |
| `file.older-game` | 2,955 | `8b880d06d01c` |
| `file.older-game of bytes the game still ships` | 147 | `801766ddc63e` |
| `file.stale` | 1,600 | `10ab724f95a9` |
| `file.unused` | 3,074 | `1310624b2b01` |
| `provenance at path=not-game-path` | 4,376 | `8b880d06d01c` |
| `provenance at path=other-bytes` | 4,908 | `690378161a3c` |
| `provenance at path=same-bytes` | 2,027 | `690378161a3c` |
| `provenance in a skin's folder` | 4,138 | `690378161a3c` |
| `provenance origin=history` | 2,955 | `8b880d06d01c` |
| `provenance origin=install` | 3,910 | `690378161a3c` |
| `provenance origin=own` | 4,907 | `690378161a3c` |
| `provenance still shipped=false` | 2,925 | `8b880d06d01c` |
| `provenance still shipped=true` | 147 | `801766ddc63e` |
| `rework.restoration` | 16 | `70e59f043f84` |
| `unit whose bin does not read` | 2 | `e6e43d1fb3db` |
| `unit whose entry name spells no slot` | 6 | `aacf8b151707` |
| `unit with no skin entry` | 3 | `e6e43d1fb3db` |

### fixups

| case | mods | example |
|---|---:|---|
| `change in an object no skin entry reaches` | 1,286 | `2ad546b10048` |
| `fixup health-bar-style` | 392 | `771fb4f81695` |
| `fixup health-bar-style U8>U8` | 390 | `771fb4f81695` |
| `fixup health-bar-style as=value` | 392 | `771fb4f81695` |
| `fixup health-bar-style nothing>Embed` | 2 | `45a9a2f7f9db` |
| `fixup known-artifacts-static-material` | 1 | `a18972e7c489` |
| `fixup known-artifacts-static-material as=removed` | 1 | `a18972e7c489` |
| `fixup known-artifacts-value-color` | 238 | `e1a072126e73` |
| `fixup known-artifacts-value-color as=removed` | 238 | `e1a072126e73` |
| `fixup known-artifacts-vfx-emitter` | 36 | `a109a8e59dbe` |
| `fixup known-artifacts-vfx-emitter as=removed` | 36 | `a109a8e59dbe` |
| `fixup known-artifacts-vfx-system` | 3 | `42c4119154ef` |
| `fixup known-artifacts-vfx-system as=removed` | 3 | `42c4119154ef` |
| `fixup material-default-technique` | 136 | `eff9a3902ba0` |
| `fixup material-default-technique as=removed` | 136 | `eff9a3902ba0` |
| `fixup material-sampler-fields` | 363 | `65205508102d` |
| `fixup material-sampler-fields as=moved` | 363 | `65205508102d` |
| `fixup material-shader-relink` | 88 | `04a8fa2bd3a9` |
| `fixup material-shader-relink Link>Link` | 88 | `04a8fa2bd3a9` |
| `fixup material-shader-relink as=value` | 88 | `04a8fa2bd3a9` |
| `fixup reshape-hoist` | 4 | `9f9a7268dd0d` |
| `fixup reshape-hoist Pointer>String` | 4 | `9f9a7268dd0d` |
| `fixup reshape-hoist as=moved` | 4 | `9f9a7268dd0d` |
| `fixup reshape-hoist as=value` | 4 | `9f9a7268dd0d` |
| `fixup retype` | 3,788 | `b96f6a6c097c` |
| `fixup retype Bool>Flag` | 1 | `2749b36689c6` |
| `fixup retype Embed>Pointer` | 15 | `a18972e7c489` |
| `fixup retype F32 wants Option` | 1 | `21c1e6eb6091` |
| `fixup retype Hash wants File` | 1 | `711bb07212b5` |
| `fixup retype Hash>File` | 349 | `14b001e04a69` |
| `fixup retype List2>List2` | 1 | `21c1e6eb6091` |
| `fixup retype List>List` | 122 | `ab780f5711fe` |
| `fixup retype List>List2` | 2 | `0b9a40624870` |
| `fixup retype Map>Map` | 66 | `646effb09ec6` |
| `fixup retype Option>Option` | 3,702 | `b96f6a6c097c` |
| `fixup retype String>File` | 3,774 | `b96f6a6c097c` |
| `fixup retype U16>U8` | 4 | `9924a6475bcc` |
| `fixup retype U32>U8` | 131 | `1dbb0c66a5df` |
| `fixup retype U8>U16` | 47 | `1b25d069efac` |
| `fixup retype as=removed` | 35 | `9167626e7edd` |
| `fixup retype as=unconvertible` | 2 | `711bb07212b5` |
| `fixup retype as=value` | 3,788 | `b96f6a6c097c` |
| `fixup strip-undeclared` | 950 | `7206b6fe8985` |
| `fixup strip-undeclared as=lost` | 247 | `a2c700b70d1d` |
| `fixup strip-undeclared as=removed` | 759 | `7206b6fe8985` |
| `fixup vfx-emission-surface` | 230 | `55269231c645` |
| `fixup vfx-emission-surface as=moved` | 163 | `d2abd82ef49c` |
| `fixup vfx-emission-surface as=removed` | 227 | `55269231c645` |
| `fixup vfx-filtering` | 9 | `9924a6475bcc` |
| `fixup vfx-filtering as=moved` | 9 | `9924a6475bcc` |
| `fixup vfx-flex-shape` | 14 | `45a9a2f7f9db` |
| `fixup vfx-flex-shape as=moved` | 14 | `45a9a2f7f9db` |
| `fixup vfx-legacy-simple` | 27 | `19cb1efaaf7b` |
| `fixup vfx-legacy-simple as=moved` | 6 | `45a9a2f7f9db` |
| `fixup vfx-legacy-simple as=removed` | 23 | `19cb1efaaf7b` |
| `fixup vfx-linger` | 4 | `ab7f591efedc` |
| `fixup vfx-linger as=moved` | 4 | `ab7f591efedc` |
| `fixup vfx-mesh-align` | 3 | `0b9a40624870` |
| `fixup vfx-mesh-align as=moved` | 3 | `0b9a40624870` |
| `fixup vfx-mesh-align as=value` | 1 | `4122f27aac0a` |
| `fixup vfx-mesh-align nothing>Bool` | 1 | `4122f27aac0a` |
| `fixup vfx-spawn-shape` | 78 | `5068f7822ad6` |
| `fixup vfx-spawn-shape as=moved` | 78 | `5068f7822ad6` |
| `fixup vfx-spawn-shape-birth-translation` | 254 | `07bc0e8e2a0d` |
| `fixup vfx-spawn-shape-birth-translation as=moved` | 194 | `07bc0e8e2a0d` |
| `fixup vfx-spawn-shape-birth-translation as=removed` | 195 | `07bc0e8e2a0d` |
| `fixup vfx-texture-mult` | 29 | `93fc5770bc4d` |
| `fixup vfx-texture-mult as=moved` | 29 | `93fc5770bc4d` |
| `fixup vfx-texture-mult-strays` | 3 | `19cb1efaaf7b` |
| `fixup vfx-texture-mult-strays as=moved` | 3 | `19cb1efaaf7b` |
| `fixup vfx-tool-shape` | 15 | `1fbe7c86d035` |
| `fixup vfx-tool-shape as=moved` | 15 | `1fbe7c86d035` |
| `fixup vfx-tool-shape as=removed` | 1 | `ea60862faee2` |
| `fixup.changed` | 3,788 | `b96f6a6c097c` |
| `fixup.changed by=health-bar-style` | 392 | `771fb4f81695` |
| `fixup.changed by=material-sampler-fields` | 363 | `65205508102d` |
| `fixup.changed by=material-shader-relink` | 88 | `04a8fa2bd3a9` |
| `fixup.changed by=reshape-hoist` | 4 | `9f9a7268dd0d` |
| `fixup.changed by=retype` | 3,788 | `b96f6a6c097c` |
| `fixup.changed by=vfx-emission-surface` | 163 | `d2abd82ef49c` |
| `fixup.changed by=vfx-filtering` | 9 | `9924a6475bcc` |
| `fixup.changed by=vfx-flex-shape` | 14 | `45a9a2f7f9db` |
| `fixup.changed by=vfx-legacy-simple` | 6 | `45a9a2f7f9db` |
| `fixup.changed by=vfx-linger` | 4 | `ab7f591efedc` |
| `fixup.changed by=vfx-mesh-align` | 3 | `0b9a40624870` |
| `fixup.changed by=vfx-spawn-shape` | 78 | `5068f7822ad6` |
| `fixup.changed by=vfx-spawn-shape-birth-translation` | 194 | `07bc0e8e2a0d` |
| `fixup.changed by=vfx-texture-mult` | 29 | `93fc5770bc4d` |
| `fixup.changed by=vfx-texture-mult-strays` | 3 | `19cb1efaaf7b` |
| `fixup.changed by=vfx-tool-shape` | 15 | `1fbe7c86d035` |
| `fixup.lost` | 247 | `a2c700b70d1d` |
| `fixup.lost by=strip-undeclared` | 247 | `a2c700b70d1d` |
| `fixup.removed` | 1,081 | `7206b6fe8985` |
| `fixup.removed by=known-artifacts-static-material` | 1 | `a18972e7c489` |
| `fixup.removed by=known-artifacts-value-color` | 238 | `e1a072126e73` |
| `fixup.removed by=known-artifacts-vfx-emitter` | 36 | `a109a8e59dbe` |
| `fixup.removed by=known-artifacts-vfx-system` | 3 | `42c4119154ef` |
| `fixup.removed by=material-default-technique` | 136 | `eff9a3902ba0` |
| `fixup.removed by=retype` | 35 | `9167626e7edd` |
| `fixup.removed by=strip-undeclared` | 759 | `7206b6fe8985` |
| `fixup.removed by=vfx-emission-surface` | 227 | `55269231c645` |
| `fixup.removed by=vfx-legacy-simple` | 23 | `19cb1efaaf7b` |
| `fixup.removed by=vfx-spawn-shape-birth-translation` | 195 | `07bc0e8e2a0d` |
| `fixup.removed by=vfx-tool-shape` | 1 | `ea60862faee2` |
| `fixup.unconvertible` | 2 | `711bb07212b5` |
| `fixup.unconvertible by=retype` | 2 | `711bb07212b5` |

### facts

| case | mods | example |
|---|---:|---|
| `check aspect=clips` | 4,371 | `690378161a3c` |
| `check aspect=mesh` | 4,908 | `690378161a3c` |
| `check aspect=rig` | 3 | `e6e43d1fb3db` |
| `check aspect=skeleton` | 4,908 | `690378161a3c` |
| `check aspect=slot` | 5 | `77058d9704d3` |
| `check aspect=submeshes` | 4,073 | `042eb2b27725` |
| `check fit=does-not-fit` | 1,311 | `d905b9c41357` |
| `check fit=fits` | 4,906 | `690378161a3c` |
| `check fit=fits-with-fix` | 22 | `13137f23efc8` |
| `check fit=unknown` | 107 | `e2e6ce24228c` |
| `check why=clips-blocked` | 423 | `91c798fedc8b` |
| `check why=clips-fit` | 4,367 | `690378161a3c` |
| `check why=copied-hidden` | 22 | `13137f23efc8` |
| `check why=copied-shown` | 184 | `12520a916f1d` |
| `check why=copied-visible` | 259 | `90fb9073d4e0` |
| `check why=foreign-mesh` | 85 | `e2e6ce24228c` |
| `check why=foreign-skeleton` | 84 | `e2e6ce24228c` |
| `check why=no-clips` | 20 | `46e2a68b6b93` |
| `check why=no-copied-submesh` | 4,073 | `042eb2b27725` |
| `check why=no-entry` | 3 | `e6e43d1fb3db` |
| `check why=no-mesh` | 1 | `fc86c0d71d96` |
| `check why=no-skeleton` | 1 | `fc86c0d71d96` |
| `check why=no-such-skin` | 5 | `77058d9704d3` |
| `check why=other-skin-mesh` | 688 | `d905b9c41357` |
| `check why=other-skin-skeleton` | 637 | `d905b9c41357` |
| `check why=own-mesh` | 4,073 | `042eb2b27725` |
| `check why=own-skeleton` | 2,786 | `548613f5aa7c` |
| `check why=premium-skeleton` | 54 | `30d4458b5b09` |
| `check why=slot-joints` | 1,835 | `042eb2b27725` |
| `check why=slot-mesh` | 1,061 | `690378161a3c` |
| `check why=slot-skeleton` | 1,075 | `690378161a3c` |
| `clips blocked=needs-own-rig` | 419 | `91c798fedc8b` |
| `clips blocked=own-off` | 2 | `8d8774c67b13` |
| `clips blocked=refused` | 421 | `91c798fedc8b` |
| `clips graph found=false` | 2 | `0ba7ef809ade` |
| `clips graph found=true` | 4,908 | `690378161a3c` |
| `clips on the unit's own rig=false` | 1,158 | `690378161a3c` |
| `clips on the unit's own rig=true` | 4,056 | `042eb2b27725` |
| `clips own, off the slot's joints` | 2 | `8d8774c67b13` |
| `clips reused from another skin` | 1,648 | `e8fd071c5e28` |
| `clips reused on the same joints=false` | 1,615 | `e8fd071c5e28` |
| `clips reused on the same joints=true` | 53 | `a069969dabbc` |
| `clips unknown` | 197 | `a8cbdd4e3846` |
| `copies spread over other skins` | 1,004 | `2a4bdfa3de06` |
| `copies.distinct-units` | 340 | `61cf9f5b17b9` |
| `copies.found` | 1,023 | `2a4bdfa3de06` |
| `copies.near` | 614 | `29172c455892` |
| `copy kind=near` | 614 | `29172c455892` |
| `copy kind=renamed` | 214 | `605c42917603` |
| `copy kind=same-rig` | 507 | `2a4bdfa3de06` |
| `copy role=copy` | 1,023 | `2a4bdfa3de06` |
| `copy role=representative` | 4,908 | `690378161a3c` |
| `game object via=history` | 3,317 | `b96f6a6c097c` |
| `game object via=install` | 3,600 | `690378161a3c` |
| `game object=game-exact` | 4,663 | `690378161a3c` |
| `game object=game-renamed` | 612 | `16cdc7e5ea65` |
| `origin evidence weight=strong` | 4,325 | `690378161a3c` |
| `origin evidence weight=weak` | 4,321 | `690378161a3c` |
| `origin evidence=audio-tag` | 1,004 | `c30b3459834f` |
| `origin evidence=files` | 2,467 | `690378161a3c` |
| `origin evidence=graph` | 942 | `c30b3459834f` |
| `origin evidence=graph-closeness` | 1,248 | `04ac5afc04f1` |
| `origin evidence=mesh` | 1,641 | `6de493211fd6` |
| `origin evidence=objects` | 4,012 | `690378161a3c` |
| `origin evidence=resolver` | 285 | `37b0f73a5134` |
| `origin evidence=resolver-closeness` | 3,895 | `690378161a3c` |
| `origin evidence=skeleton` | 1,642 | `6de493211fd6` |
| `origin evidence=skin-name` | 737 | `30d4458b5b09` |
| `origin evidence=texture` | 1,438 | `866d5a5bb663` |
| `origin own mesh=false` | 1,143 | `690378161a3c` |
| `origin own mesh=true` | 4,073 | `042eb2b27725` |
| `origin strength=strong` | 4,217 | `690378161a3c` |
| `origin strength=weak` | 573 | `042eb2b27725` |
| `origin verdict=base` | 3,198 | `690378161a3c` |
| `origin verdict=custom` | 129 | `2c79b080d055` |
| `origin verdict=premium` | 1,765 | `d905b9c41357` |
| `origin verdict=unknown` | 148 | `2dc58d50ccd1` |
| `origin with no evidence that decides` | 277 | `2c79b080d055` |
| `origin.ambiguous` | 24 | `894d1ee26bd9` |
| `origin.conflicting` | 276 | `30d4458b5b09` |
| `origin.none` | 148 | `2dc58d50ccd1` |
| `rework made=after` | 414 | `df91618c3af9` |
| `rework made=before` | 13 | `8b880d06d01c` |
| `rework made=unknown` | 2 | `90fb9073d4e0` |
| `rework=10.11` | 14 | `6a28ca75546c` |
| `rework=10.7` | 5 | `b013038f2030` |
| `rework=11.12` | 21 | `07a9cab5109b` |
| `rework=11.23` | 50 | `90fb9073d4e0` |
| `rework=12.16` | 18 | `9d9402ecf4d5` |
| `rework=13.20` | 20 | `df91618c3af9` |
| `rework=13.3` | 75 | `4c5ec791f9fa` |
| `rework=14.20` | 19 | `8854260c90a3` |
| `rework=14.24` | 16 | `5ed8007fd17b` |
| `rework=14.7` | 13 | `bd60e259470b` |
| `rework=14.9` | 47 | `455499bf8e8c` |
| `rework=15.17` | 16 | `1d3c6e306523` |
| `rework=15.7` | 21 | `c81fc359a259` |
| `rework=16.6` | 27 | `8b880d06d01c` |
| `rework=9.12` | 46 | `64ff2706f13a` |
| `rework=9.16` | 21 | `37304c11dd60` |
| `rig clip at a moved path` | 63 | `ba6e499a11c5` |
| `rig clip own=false` | 4,531 | `690378161a3c` |
| `rig clip own=true` | 1,483 | `98106ae9af46` |
| `rig clip source=game` | 3,493 | `690378161a3c` |
| `rig clip source=missing` | 4 | `5db3d0c678cf` |
| `rig clip source=mod` | 2,307 | `98106ae9af46` |
| `rig graph from=archive` | 2,259 | `98106ae9af46` |
| `rig graph from=game` | 2,767 | `690378161a3c` |
| `rig mesh at a moved path` | 100 | `298c1ffbf113` |
| `rig mesh own=false` | 1,140 | `690378161a3c` |
| `rig mesh own=true` | 4,073 | `042eb2b27725` |
| `rig mesh source=game` | 812 | `690378161a3c` |
| `rig mesh source=missing` | 1 | `fc86c0d71d96` |
| `rig mesh source=mod` | 4,293 | `042eb2b27725` |
| `rig part unreadable=skeleton` | 1 | `0ab4237c5cdf` |
| `rig skeleton at a moved path` | 100 | `298c1ffbf113` |
| `rig skeleton own=false` | 1,154 | `690378161a3c` |
| `rig skeleton own=true` | 4,056 | `042eb2b27725` |
| `rig skeleton source=game` | 810 | `690378161a3c` |
| `rig skeleton source=missing` | 1 | `fc86c0d71d96` |
| `rig skeleton source=mod` | 4,292 | `042eb2b27725` |
| `rig submesh=hidden-never` | 776 | `52aa42a711bd` |
| `rig submesh=hidden-shown` | 1,539 | `34a1e0d78ac0` |
| `rig submesh=visible` | 4,073 | `042eb2b27725` |
| `rig.submesh-copied` | 464 | `90fb9073d4e0` |
| `rig.submesh-copied visibility=hidden-never` | 143 | `45c9eb0eb82d` |
| `rig.submesh-copied visibility=hidden-shown` | 343 | `957d17b60f1e` |
| `rig.submesh-copied visibility=visible` | 259 | `90fb9073d4e0` |
| `rig.unreadable` | 1 | `0ab4237c5cdf` |
| `rig.unreadable parts=skeleton` | 1 | `0ab4237c5cdf` |
| `slot the game has a bin at=true` | 4,908 | `690378161a3c` |

### decision

| case | mods | example |
|---|---:|---|
| `candidate feasible=false` | 616 | `bec3c44096ea` |
| `candidate feasible=true` | 4,366 | `690378161a3c` |
| `candidate=skin0 first` | 4,342 | `690378161a3c` |
| `candidate=the champion's target` | 453 | `46aa40949486` |
| `candidate=the premium origin` | 537 | `ed7617f29465` |
| `candidate=the skin whose clips it plays` | 86 | `91c798fedc8b` |
| `candidate=the skin whose skeleton it takes` | 1 | `bec3c44096ea` |
| `decision by the clips played` | 86 | `91c798fedc8b` |
| `decision made before a rework` | 13 | `8b880d06d01c` |
| `decision makes a new bin` | 5 | `77058d9704d3` |
| `decision origin on the base's rig` | 18 | `32968482d989` |
| `decision reason=base` | 3,311 | `690378161a3c` |
| `decision reason=clips-origin` | 84 | `91c798fedc8b` |
| `decision reason=conflict` | 1 | `552d4d5e68c1` |
| `decision reason=fallback` | 497 | `ed7617f29465` |
| `decision reason=infeasible` | 3 | `8d8774c67b13` |
| `decision reason=paste-cleaned` | 446 | `560ccdbe142b` |
| `decision reason=pre-rework` | 13 | `8b880d06d01c` |
| `decision reason=skeleton-origin` | 1 | `bec3c44096ea` |
| `decision reason=slot` | 803 | `d905b9c41357` |
| `decision reason=sub-follows` | 428 | `46aa40949486` |
| `decision reason=sub-left-to-base` | 28 | `03a8a0aa8c28` |
| `decision reason=superseded` | 5 | `0f805b36e089` |
| `decision reason=unjudged` | 2 | `e6e43d1fb3db` |
| `decision taken by another unit` | 5 | `0f805b36e089` |
| `decision unfit` | 23 | `77058d9704d3` |
| `decision with chroma followers` | 1,155 | `33a76ab3965e` |
| `decision with no target` | 47 | `8b880d06d01c` |
| `target.chosen` | 4,892 | `690378161a3c` |
| `target.chosen reason=base` | 3,311 | `690378161a3c` |
| `target.chosen reason=clips-origin` | 86 | `91c798fedc8b` |
| `target.chosen reason=fallback` | 497 | `ed7617f29465` |
| `target.chosen reason=paste-cleaned` | 446 | `560ccdbe142b` |
| `target.chosen reason=skeleton-origin` | 1 | `bec3c44096ea` |
| `target.chosen reason=slot` | 803 | `d905b9c41357` |
| `target.chosen reason=sub-follows` | 413 | `46aa40949486` |
| `target.conflict` | 1 | `552d4d5e68c1` |
| `target.forced-unfit` | 23 | `77058d9704d3` |
| `target.forced-unfit follows=true` | 23 | `77058d9704d3` |
| `target.infeasible` | 3 | `8d8774c67b13` |
| `target.pre-rework` | 13 | `8b880d06d01c` |
| `target.sub-left` | 28 | `03a8a0aa8c28` |
| `target.superseded` | 5 | `0f805b36e089` |
| `target.unjudged` | 3 | `e6e43d1fb3db` |
| `targets conflict` | 1 | `552d4d5e68c1` |

### plan

| case | mods | example |
|---|---:|---|
| `file verdict=dangling` | 1,220 | `b96f6a6c097c` |
| `file verdict=game` | 4,705 | `690378161a3c` |
| `file verdict=own` | 4,878 | `690378161a3c` |
| `file verdict=unmounted` | 323 | `e2e6ce24228c` |
| `hud.carried` | 1,333 | `cb2c00df0e69` |
| `object reached as=carried` | 4,356 | `8023f23a12d3` |
| `object reached as=entry` | 4,895 | `690378161a3c` |
| `object reached as=game` | 1,694 | `2ec7826df1e5` |
| `object reached as=graph` | 2,258 | `98106ae9af46` |
| `object reached as=resolver` | 4,886 | `690378161a3c` |
| `object.carried` | 4,908 | `690378161a3c` |
| `object.copied` | 161 | `4d21b6c01aa2` |
| `object.game-carried` | 740 | `8023f23a12d3` |
| `object.link-dangling` | 1,612 | `3457bcac6ffc` |
| `object.unreached` | 1,444 | `551e18abc8c4` |
| `reach carries game objects for overrides` | 763 | `8023f23a12d3` |
| `reach enters the champion's objects` | 268 | `21891a4c64e2` |
| `reach keeps links to the game's objects` | 4,828 | `690378161a3c` |
| `ref.dangling` | 1,175 | `b96f6a6c097c` |
| `ref.dangling at a former path` | 214 | `b96f6a6c097c` |
| `ref.unmounted` | 300 | `e2e6ce24228c` |
| `references pointed at the game's files` | 3,577 | `b96f6a6c097c` |
| `renames keep links to the source slot` | 6 | `bec3c44096ea` |
| `renames retarget a unit` | 596 | `bec3c44096ea` |
| `renames swap game names` | 418 | `bec3c44096ea` |
| `renames to the target's names` | 596 | `bec3c44096ea` |
| `unit left=chroma-folded` | 80 | `03ee59f50b34` |
| `unit left=conflict` | 1 | `552d4d5e68c1` |
| `unit left=copy` | 1,023 | `2a4bdfa3de06` |
| `unit left=equal-to-game` | 206 | `8b880d06d01c` |
| `unit left=game-skin` | 32 | `9c97ead533ea` |
| `unit left=off-target` | 245 | `4bdf6ae01b20` |
| `unit left=pre-rework` | 13 | `8b880d06d01c` |
| `unit left=sub-left` | 28 | `03a8a0aa8c28` |
| `unit left=superseded` | 5 | `0f805b36e089` |
| `unit ships its graph` | 2,126 | `a8cbdd4e3846` |
| `unit without a resolver` | 53 | `012afca8b8bb` |
| `unit.left` | 1,468 | `8b880d06d01c` |
| `unit.left why=chroma-folded` | 80 | `03ee59f50b34` |
| `unit.left why=conflict` | 1 | `552d4d5e68c1` |
| `unit.left why=copy` | 1,023 | `2a4bdfa3de06` |
| `unit.left why=equal-to-game` | 206 | `8b880d06d01c` |
| `unit.left why=game-skin` | 32 | `9c97ead533ea` |
| `unit.left why=off-target` | 245 | `4bdf6ae01b20` |
| `unit.left why=pre-rework` | 13 | `8b880d06d01c` |
| `unit.left why=sub-left` | 28 | `03a8a0aa8c28` |
| `unit.left why=superseded` | 5 | `0f805b36e089` |
| `unit.left-with-work` | 209 | `4bdf6ae01b20` |
| `unit.left-with-work kind=slot-work` | 183 | `4bdf6ae01b20` |
| `unit.left-with-work kind=unsupported` | 38 | `712c14e409f4` |
| `unit.new-bin` | 5 | `77058d9704d3` |
| `unit.ported` | 4,879 | `690378161a3c` |
| `unit.ported graph=false` | 2,830 | `690378161a3c` |
| `unit.ported graph=true` | 2,122 | `a8cbdd4e3846` |
| `unit.ported new_bin=false` | 4,879 | `690378161a3c` |
| `unit.ported new_bin=true` | 5 | `77058d9704d3` |
| `unit.retargeted` | 596 | `bec3c44096ea` |

### audio

| case | mods | example |
|---|---:|---|
| `audio.bank-id-set` | 1,368 | `7bc34b21a01b` |
| `audio.bank-id-set by=bank-id` | 1,368 | `7bc34b21a01b` |
| `audio.bank-unreadable` | 1 | `b02bf9ee3b33` |
| `audio.carried` | 2,129 | `b96f6a6c097c` |
| `audio.carried a locale copy once` | 1 | `9b85da734738` |
| `audio.carried events from the game's bank at an older one's path` | 6 | `64698c1614d3` |
| `audio.carried events no unit's list names` | 100 | `eb1e1ad93ca9` |
| `audio.carried from_mod=false` | 294 | `b0ff2afbf036` |
| `audio.carried from_mod=true` | 123 | `a2065296f9af` |
| `audio.carried layer=base` | 1,255 | `b96f6a6c097c` |
| `audio.carried layer=voice` | 1,022 | `ed6637fa8f40` |
| `audio.carried layer=voice-locale` | 519 | `eb88a666d18a` |
| `audio.carried shape=overlay` | 1,819 | `b96f6a6c097c` |
| `audio.carried shape=own` | 384 | `b0ff2afbf036` |
| `audio.carried sounds that play nothing` | 33 | `a109a8e59dbe` |
| `audio.carried stands in for a game unit` | 29 | `5c2e33d0b72d` |
| `audio.literals-renamed` | 159 | `c60ba3ab5389` |
| `audio.literals-renamed to the base's` | 86 | `c60ba3ab5389` |
| `audio.literals-renamed to the mod's events` | 96 | `cdadd71210b5` |
| `audio.not-carried` | 65 | `4a19b5b08ab6` |
| `audio.not-carried why=no-event` | 60 | `4a19b5b08ab6` |
| `audio.not-carried why=no-game-audio` | 5 | `3052a5d3f558` |
| `audio.units-left-out` | 124 | `999455df6a97` |
| `bank in a locale's WAD` | 526 | `eb88a666d18a` |
| `bank role=events` | 2,083 | `10ab724f95a9` |
| `bank role=media` | 2,166 | `b96f6a6c097c` |
| `bank role=package` | 1,717 | `eb88a666d18a` |
| `bank role=stub` | 1,072 | `4d772c5713b1` |
| `bank unreadable` | 1 | `b02bf9ee3b33` |
| `bank with clips of the mod's own` | 2,160 | `b96f6a6c097c` |
| `carried unit layer=base` | 1,255 | `b96f6a6c097c` |
| `carried unit layer=voice` | 1,022 | `ed6637fa8f40` |
| `carried unit layer=voice-locale` | 519 | `eb88a666d18a` |
| `carried unit over the game's events` | 1,819 | `b96f6a6c097c` |
| `carried unit stands in for a game unit` | 29 | `5c2e33d0b72d` |
| `carried unit with events of its own` | 384 | `b0ff2afbf036` |
| `sound literals renamed` | 170 | `b0ff2afbf036` |
| `sound unit voice=false` | 4,905 | `690378161a3c` |
| `sound unit voice=true` | 4,762 | `690378161a3c` |
| `unit leaves out a game unit` | 124 | `999455df6a97` |

### package

| case | mods | example |
|---|---:|---|
| `again built=true` | 4,884 | `690378161a3c` |
| `again redated` | 4,533 | `690378161a3c` |
| `chunk origin=bank` | 2,115 | `b467633d7198` |
| `chunk origin=contained` | 2,907 | `40300b55696d` |
| `chunk origin=delta` | 3,769 | `40300b55696d` |
| `chunk origin=kept` | 4,892 | `690378161a3c` |
| `chunk origin=made` | 4,346 | `8023f23a12d3` |
| `layer=base` | 4,892 | `690378161a3c` |
| `layer=hud` | 1,329 | `cb2c00df0e69` |
| `layer=voice` | 1,019 | `ed6637fa8f40` |
| `layer=voice-locale` | 517 | `eb88a666d18a` |
| `package.empty` | 13 | `8b880d06d01c` |
| `package.field-kept` | 385 | `0247e6935e9d` |
| `package.in-place` | 4,884 | `690378161a3c` |
| `package.in-place left=former-path` | 2,731 | `8b880d06d01c` |
| `package.in-place left=orphaned` | 10 | `3e6149fca10c` |
| `package.in-place left=stale` | 1,636 | `c92cdc9e684c` |
| `package.in-place left=unused` | 3,058 | `1310624b2b01` |
| `package.in-place ships files a fixup made` | 2,651 | `088d0a057ec0` |
| `package.in-place writes bins again` | 3,769 | `40300b55696d` |
| `package.ported` | 4,879 | `690378161a3c` |
| `package.ported left=former-path` | 2,725 | `8481ee36daca` |
| `package.ported left=game` | 189 | `9ee78322b1d9` |
| `package.ported left=not-carried` | 4,879 | `690378161a3c` |
| `package.ported left=orphaned` | 2,840 | `eb5aba5314eb` |
| `package.ported left=recut` | 2,115 | `b467633d7198` |
| `package.ported left=stale` | 1,586 | `c92cdc9e684c` |
| `package.ported left=unused` | 3,059 | `1310624b2b01` |
| `package.resolver-frozen` | 4,204 | `8023f23a12d3` |
| `package.shader-missing` | 24 | `b96f6a6c097c` |
| `package.texture-contained` | 2,944 | `40300b55696d` |
| `package.texture-contained contained` | 2,907 | `40300b55696d` |
| `package.texture-contained kept=format` | 274 | `b540716595d2` |
| `package.texture-contained kept=not-one-surface` | 22 | `0707d08b753c` |
| `package.texture-contained kept=partial-blocks` | 653 | `50b8b16da548` |
| `package.texture-contained kept=partial-mips` | 120 | `b540716595d2` |
| `package.texture-contained kept=truncated` | 2 | `8c8bf89b8b5a` |
| `package.thumbnail-dropped` | 16 | `9cd0944e4517` |
| `package.thumbnail-dropped why=entry-too-large` | 16 | `9cd0944e4517` |
| `patch adds links` | 4,349 | `8023f23a12d3` |
| `patch layer=base` | 4,879 | `690378161a3c` |
| `patch layer=hud` | 1,329 | `cb2c00df0e69` |
| `patch layer=voice` | 1,019 | `ed6637fa8f40` |
| `patch layer=voice-locale` | 517 | `eb88a666d18a` |
| `patch of=root` | 1,329 | `cb2c00df0e69` |
| `patch of=skin` | 4,879 | `690378161a3c` |
| `patch outcome=applied` | 4,879 | `690378161a3c` |

### checks

| case | mods | example |
|---|---:|---|
| `check.done` | 4,892 | `690378161a3c` |
| `check.done by=apply` | 4,879 | `690378161a3c` |
| `check.done by=base-skin` | 4,879 | `690378161a3c` |
| `check.done by=round-trip` | 4,892 | `690378161a3c` |
| `check.done by=types` | 4,892 | `690378161a3c` |
| `check.done check=apply` | 4,879 | `690378161a3c` |
| `check.done check=base-skin` | 4,879 | `690378161a3c` |
| `check.done check=round-trip` | 4,892 | `690378161a3c` |
| `check.done check=types` | 4,892 | `690378161a3c` |
| `check.done source=schema` | 4,892 | `690378161a3c` |
| `check.field-undeclared` | 198 | `a2c700b70d1d` |
| `check.field-undeclared by=types` | 198 | `a2c700b70d1d` |
| `check.name-unresolved` | 2 | `25c6531f00ba` |
| `check.name-unresolved by=apply` | 2 | `25c6531f00ba` |
| `check.type-mismatch` | 8 | `1378ad3d3f90` |
| `check.type-mismatch by=types` | 8 | `1378ad3d3f90` |
| `check.type-mismatch found=Embed` | 1 | `4a554cb95fa0` |
| `check.type-mismatch found=F32` | 1 | `21c1e6eb6091` |
| `check.type-mismatch found=Hash` | 1 | `711bb07212b5` |
| `check.type-mismatch found=List` | 5 | `1378ad3d3f90` |
| `check.type-mismatch wants=Embed` | 1 | `4a554cb95fa0` |
| `check.type-mismatch wants=File` | 1 | `711bb07212b5` |
| `check.type-mismatch wants=List` | 5 | `1378ad3d3f90` |
| `check.type-mismatch wants=Option` | 1 | `21c1e6eb6091` |

### records

| case | mods | example |
|---|---:|---|
| `record #GAME-BEHAVIOR-0001` | 5 | `77058d9704d3` |
| `record #GAME-BEHAVIOR-0002` | 4,898 | `690378161a3c` |
| `record #GAME-BEHAVIOR-0004` | 1,333 | `cb2c00df0e69` |
| `record #GAME-BEHAVIOR-0007` | 300 | `e2e6ce24228c` |
| `record #GAME-BEHAVIOR-0023` | 24 | `b96f6a6c097c` |
| `record #GAME-CHANGE-0001` | 29 | `8b880d06d01c` |
| `record #GAME-CHANGE-0002` | 9 | `9924a6475bcc` |
| `record #GAME-CHANGE-0003` | 4 | `ab7f591efedc` |
| `record #GAME-CHANGE-0004` | 27 | `19cb1efaaf7b` |
| `record #GAME-CHANGE-0005` | 29 | `93fc5770bc4d` |
| `record #GAME-CHANGE-0006` | 78 | `5068f7822ad6` |
| `record #GAME-CHANGE-0007` | 2,982 | `40300b55696d` |
| `record #GAME-CHANGE-0008` | 359 | `032db643a88a` |
| `record #GAME-CHANGE-0009` | 758 | `012afca8b8bb` |
| `record #GAME-CHANGE-0010` | 107 | `1ab1481ba822` |
| `record #GAME-CHANGE-0011` | 34 | `088d0a057ec0` |
| `record #GAME-CHANGE-0012` | 1,433 | `40e2361d74da` |
| `record #GAME-CHANGE-0014` | 363 | `65205508102d` |
| `record #GAME-CHANGE-0015` | 136 | `eff9a3902ba0` |
| `record #GAME-CHANGE-0016` | 14 | `45a9a2f7f9db` |
| `record #GAME-CHANGE-0017` | 3 | `0b9a40624870` |
| `record #GAME-CHANGE-0018` | 230 | `55269231c645` |
| `record #GAME-CHANGE-0023` | 392 | `771fb4f81695` |
| `record #GAME-CHANGE-0024` | 88 | `04a8fa2bd3a9` |
| `record #MOD-ARTIFACT-0001` | 1,023 | `2a4bdfa3de06` |
| `record #MOD-ARTIFACT-0004` | 1,600 | `10ab724f95a9` |
| `record #MOD-ARTIFACT-0010` | 71 | `975ff083bcb8` |
| `record #MOD-ARTIFACT-0011` | 8 | `0672cfff925b` |
| `record #MOD-ARTIFACT-0012` | 4 | `e5f3a984b590` |
| `record #MOD-ARTIFACT-0013` | 1,218 | `690378161a3c` |
| `record #MOD-ARTIFACT-0016` | 1,999 | `3b1aaabea06e` |
| `record #MOD-ARTIFACT-0017` | 3 | `19cb1efaaf7b` |
| `record #MOD-ARTIFACT-0018` | 4 | `9f9a7268dd0d` |
| `record #MOD-ARTIFACT-0019` | 254 | `07bc0e8e2a0d` |
| `record #MOD-ARTIFACT-0020` | 269 | `e1a072126e73` |
| `record #MOD-ARTIFACT-0023` | 1,368 | `7bc34b21a01b` |
| `record #MOD-ARTIFACT-0024` | 464 | `90fb9073d4e0` |
| `record #MOD-ARTIFACT-0031` | 15 | `1fbe7c86d035` |

<!-- cases.py: end -->

## Cases no mod shows

A diagnostic code of [CODES.md](CODES.md) that no mod raises, and a value the program
declares for a closed set that no report holds. Each is tested like any other case and is
marked as not seen (#ADR-0045).

Some cannot show in these runs. `write.written` and `write.blocked` are raised by a run that
writes. The codes of the `run` area speak of the run's other inputs, and the runs had all of
them.

<!-- cases.py: unseen -->

104 cases, against the runs `port` and `repair` of 2026-10-09.

### run, not seen

- `history.absent`
- `history.index-missing`
- `history.index-stale`
- `history.install-newer`
- `install.version-unknown`
- `install.wad-unreadable`
- `names.mimir-absent`
- `schema.absent`
- `schema.install-newer`
- `schema.unreadable`

### archive, not seen

- `archive.budget-exceeded`
- `archive.budget-exceeded limit=archive-file`
- `archive.budget-exceeded limit=inflated`
- `archive.codec-unsupported`
- `archive.entry-corrupt`
- `archive.entry-too-large`
- `archive.entry-unreadable`
- `archive.name-unsafe`
- `archive.not-zip`
- `archive.repack-duplicate`
- `archive.stored-crc-wrong`
- `archive.too-many-entries`
- `archive.wad-unreadable`
- `archive.wad-unreadable why=chunk-past-end`
- `archive.wad-unreadable why=chunk-too-large`
- `archive.wad-unreadable why=chunk-unreadable`
- `archive.wad-unreadable why=mount`
- `archive.wad-unreadable why=panicked`
- `archive.wad-unreadable why=table-larger-than-data`
- `archive.wad-unreadable why=too-many-chunks`
- `archive.wad-unreadable why=version`
- `inventory confidence=heuristic`
- `wad.champion-ambiguous`

### load, not seen

- `bin.load-capped`
- `bin.taken-from-game history=false`
- `file kind=ptch`
- `file.format-unreadable`
- `file.format-unreadable why=count-larger-than-data`
- `file.format-unreadable why=panicked`
- `file.format-unreadable why=read`
- `file.format-unreadable why=write`
- `file.move-failed`
- `file.moved rename=variant-tag-and-prefix-4x`
- `file.same-size-capped`

### fixups, not seen

- `catalog.capped`
- `fixup reshape-wrap`

### facts, not seen

- `check why=unbound`
- `copy kind=byte-identical`
- `rig part unreadable=mesh`
- `rig.unreadable parts=mesh`
- `slot the game has a bin at=false`

### decision, not seen

- `candidate=the policy`
- `decision reason=forced`
- `target.forced-unfit follows=false`

### plan, not seen

- `hud entry with an unspelled field`
- `hud.carried with an unspelled field`
- `unit left=no-target`
- `unit.left why=no-target`
- `unit.new-bin-failed`
- `unit.not-written`

### audio, not seen

- `audio.bank-unnamed`
- `audio.carried events sharing an id with the game's`
- `audio.not-carried why=locale-differs`
- `audio.not-carried why=new-bin`
- `audio.not-carried why=no-events-bank`
- `audio.not-carried why=no-unit-to-model`

### package, not seen

- `again built=false`
- `again changes chunks`
- `again changes values`
- `chunk origin=whole`
- `package.entry-whole`
- `package.in-place keeps files with no path spelled`
- `package.in-place writes a bin whole`
- `package.texture-contained kept=not-read-back`
- `package.texture-contained kept=too-large`
- `package.texture-contained kept=unreadable`
- `package.thumbnail-dropped why=entry-unreadable`
- `package.thumbnail-dropped why=not-an-image`
- `package.thumbnail-dropped why=not-encoded`
- `package.thumbnail-dropped why=too-large`
- `patch not applied`
- `patch outcome=failed`
- `patch outcome=no-game-bin`
- `patch with skipped edits`
- `write.blocked`
- `write.written`

### checks, not seen

- `check.apply-failed`
- `check.done source=install`
- `check.file-missing`
- `check.link-unresolved`
- `check.load-capped`
- `check.manifest-unreadable`
- `check.other-skin`
- `check.other-skin value=audio-tag`
- `check.other-skin value=bank-unit`
- `check.other-skin value=simple-skin`
- `check.other-skin value=skeleton`
- `check.other-skin value=skin-name`
- `check.other-skin value=texture`
- `check.record-skipped`
- `check.round-trip-failed`
- `check.round-trip-failed problem=entries`
- `check.round-trip-failed problem=object`
- `check.round-trip-failed problem=unreadable`

<!-- cases.py: end -->
