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

670 cases in 5,121 mods, from the runs `port` and `repair` of 2026-10-09.

### run

| case | mods | example |
|---|---:|---|
| `halt archive.no-skin` | 1 | `4924914fd7a3` |
| `halt package.empty` | 15 | `c20537006f52` |
| `halt package.shader-missing` | 27 | `b96f6a6c097c` |

### archive

| case | mods | example |
|---|---:|---|
| `archive holds=animation` | 2,793 | `98106ae9af46` |
| `archive holds=bmp` | 12 | `ab45b36f1839` |
| `archive holds=desktop-ini` | 9 | `679e38c0f295` |
| `archive holds=empty` | 29 | `dfd252fe6457` |
| `archive holds=exr` | 19 | `e349f123f06f` |
| `archive holds=fbx-binary` | 50 | `0012565578f4` |
| `archive holds=gif` | 5 | `74d70c039957` |
| `archive holds=gltf-binary` | 6 | `43cd5b57d6a7` |
| `archive holds=ico` | 1 | `37796a2c7824` |
| `archive holds=ini` | 163 | `61cc49003bc6` |
| `archive holds=inibin` | 10 | `d393d47c4e1e` |
| `archive holds=item-recommendations` | 12 | `b1f7b2c54de0` |
| `archive holds=jpeg` | 20 | `836edb7ed0cc` |
| `archive holds=json` | 252 | `40b00e9c3a2d` |
| `archive holds=markdown` | 23 | `679e38c0f295` |
| `archive holds=maya-binary` | 7 | `305f42936229` |
| `archive holds=maya-image` | 398 | `7ba786206e67` |
| `archive holds=maya-swatches` | 2 | `7b779ee3c5ec` |
| `archive holds=otf` | 6 | `3d5f554cff71` |
| `archive holds=pdn` | 31 | `3d955a3880d7` |
| `archive holds=pe` | 66 | `9d15cbf8cc63` |
| `archive holds=png` | 277 | `318a7ebe0cb8` |
| `archive holds=property-bin` | 5,120 | `690378161a3c` |
| `archive holds=psd` | 16 | `def699cffb6d` |
| `archive holds=r3d2` | 1 | `6e3d4b1755d9` |
| `archive holds=rar` | 8 | `0583e723cd3d` |
| `archive holds=ritobin-json` | 631 | `8a2c7153fd2a` |
| `archive holds=ritobin-text` | 1,707 | `3b1aaabea06e` |
| `archive holds=shortcut` | 23 | `d686e249d4c1` |
| `archive holds=simple-skin` | 4,539 | `042eb2b27725` |
| `archive holds=skeleton` | 4,538 | `042eb2b27725` |
| `archive holds=static-mesh-ascii` | 1,144 | `8eee9126c83f` |
| `archive holds=static-mesh-binary` | 2,762 | `d8cdb6ba220a` |
| `archive holds=string-table` | 1 | `a26e64fdac59` |
| `archive holds=subchunk-toc` | 85 | `767f0956dd3d` |
| `archive holds=text` | 131 | `8f856aebd68d` |
| `archive holds=texture` | 4,539 | `690378161a3c` |
| `archive holds=texture-dds` | 4,165 | `c20537006f52` |
| `archive holds=tga` | 17 | `543b88a9b9d4` |
| `archive holds=unknown` | 16 | `8dc89140f5bc` |
| `archive holds=wad` | 10 | `40b00e9c3a2d` |
| `archive holds=wav` | 19 | `493bcf426273` |
| `archive holds=web-p` | 6 | `f3e9d21184f1` |
| `archive holds=wwise-bank` | 2,792 | `b96f6a6c097c` |
| `archive holds=wwise-package` | 1,855 | `eb88a666d18a` |
| `archive holds=xcf` | 71 | `43cd5b57d6a7` |
| `archive holds=xml` | 4 | `158e6d400f92` |
| `archive holds=zip` | 27 | `003216dbe678` |
| `archive holds=zstd` | 6 | `66b3a98b2c72` |
| `archive.chunk-duplicate` | 11 | `0672cfff925b` |
| `archive.info-unreadable` | 2 | `fcb644d8a00b` |
| `archive.no-skin` | 1 | `4924914fd7a3` |
| `inventory confidence=magic` | 2,100 | `3b1aaabea06e` |
| `inventory confidence=name` | 2 | `679e38c0f295` |
| `inventory finding=bin-text` | 64 | `a3fd517fd99a` |
| `inventory finding=executable` | 66 | `9d15cbf8cc63` |
| `inventory finding=nested-archive` | 40 | `003216dbe678` |
| `inventory finding=os-artifact` | 32 | `679e38c0f295` |
| `inventory finding=source` | 2,056 | `3b1aaabea06e` |
| `inventory finding=unknown` | 16 | `8dc89140f5bc` |
| `inventory item=desktop-ini` | 9 | `679e38c0f295` |
| `inventory item=exr` | 19 | `e349f123f06f` |
| `inventory item=fbx-binary` | 50 | `0012565578f4` |
| `inventory item=gltf-binary` | 6 | `43cd5b57d6a7` |
| `inventory item=maya-binary` | 7 | `305f42936229` |
| `inventory item=maya-image` | 398 | `7ba786206e67` |
| `inventory item=maya-swatches` | 2 | `7b779ee3c5ec` |
| `inventory item=pdn` | 31 | `3d955a3880d7` |
| `inventory item=pe` | 66 | `9d15cbf8cc63` |
| `inventory item=psd` | 16 | `def699cffb6d` |
| `inventory item=rar` | 8 | `0583e723cd3d` |
| `inventory item=ritobin-json` | 631 | `8a2c7153fd2a` |
| `inventory item=ritobin-text` | 1,707 | `3b1aaabea06e` |
| `inventory item=shortcut` | 23 | `d686e249d4c1` |
| `inventory item=unknown` | 16 | `8dc89140f5bc` |
| `inventory item=xcf` | 71 | `43cd5b57d6a7` |
| `inventory item=zip` | 27 | `003216dbe678` |
| `inventory item=zstd` | 6 | `66b3a98b2c72` |
| `inventory.bin-text` | 64 | `a3fd517fd99a` |
| `inventory.executable` | 66 | `9d15cbf8cc63` |
| `inventory.nested-archive` | 40 | `003216dbe678` |
| `inventory.os-artifact` | 32 | `679e38c0f295` |
| `inventory.source` | 2,056 | `3b1aaabea06e` |
| `inventory.unknown` | 16 | `8dc89140f5bc` |
| `wad a locale's, holding the whole mod` | 71 | `975ff083bcb8` |
| `wad layout=loose` | 214 | `6de493211fd6` |
| `wad layout=packed` | 4,916 | `4924914fd7a3` |
| `wad restores a model from before a rework` | 43 | `70e59f043f84` |
| `wad role=champion` | 5,120 | `690378161a3c` |
| `wad role=locale` | 481 | `eb88a666d18a` |
| `wad role=other` | 34 | `4924914fd7a3` |
| `wad.champion-guessed` | 19 | `0672cfff925b` |
| `wad.duplicate` | 3 | `e5f3a984b590` |
| `wad.duplicate-differs` | 1 | `62b4bb9d2d67` |
| `wad.locale` | 481 | `eb88a666d18a` |
| `wad.locale-packed` | 71 | `975ff083bcb8` |
| `wad.not-carried` | 30 | `4924914fd7a3` |
| `wad.several-champions` | 7 | `29399a3faed0` |

### load

| case | mods | example |
|---|---:|---|
| `bin.entry-duplicate` | 128 | `05ad37f99624` |
| `bin.link-missing` | 2,741 | `b96f6a6c097c` |
| `bin.link-missing former=false` | 37 | `6bae337c0ee7` |
| `bin.link-missing former=true` | 2,720 | `b96f6a6c097c` |
| `bin.taken-from-game` | 1,273 | `690378161a3c` |
| `bin.taken-from-game from a former bin` | 63 | `c20537006f52` |
| `bin.taken-from-game history=true` | 1,273 | `690378161a3c` |
| `bin.taken-from-game with files no bin names` | 440 | `cb2c00df0e69` |
| `bin.taken-from-game with files the game never held` | 403 | `2a321a390f6f` |
| `bin.taken-from-game with stale files` | 196 | `c92cdc9e684c` |
| `bin.unreadable` | 58 | `a3fd517fd99a` |
| `character equal to the game's` | 216 | `c20537006f52` |
| `character kind=champion` | 5,120 | `690378161a3c` |
| `character kind=sub` | 681 | `c20537006f52` |
| `character with a root bin` | 259 | `a2c700b70d1d` |
| `character.equal-to-game` | 216 | `c20537006f52` |
| `dating evidence=file` | 2,994 | `8b880d06d01c` |
| `dating evidence=object` | 1,190 | `b96f6a6c097c` |
| `dating with no bound` | 936 | `690378161a3c` |
| `file kind=anm` | 2,793 | `98106ae9af46` |
| `file kind=bin` | 5,120 | `690378161a3c` |
| `file kind=bnk` | 2,792 | `b96f6a6c097c` |
| `file kind=dds` | 4,166 | `4924914fd7a3` |
| `file kind=other` | 2,353 | `8f856aebd68d` |
| `file kind=scb` | 2,875 | `d8cdb6ba220a` |
| `file kind=sco` | 1,144 | `8eee9126c83f` |
| `file kind=skl` | 4,538 | `042eb2b27725` |
| `file kind=skn` | 4,539 | `042eb2b27725` |
| `file kind=tex` | 4,540 | `4924914fd7a3` |
| `file kind=wpk` | 1,855 | `eb88a666d18a` |
| `file made by=bank-id` | 1,453 | `7bc34b21a01b` |
| `file made by=legacy-format` | 1,969 | `b770e36141eb` |
| `file made by=moved-asset` | 1,889 | `088d0a057ec0` |
| `file source=archive` | 5,121 | `4924914fd7a3` |
| `file source=game` | 1,273 | `690378161a3c` |
| `file source=made` | 2,824 | `088d0a057ec0` |
| `file source=rewritten` | 1,453 | `7bc34b21a01b` |
| `file use=former-path` | 2,870 | `c20537006f52` |
| `file use=loaded` | 5,120 | `690378161a3c` |
| `file use=override` | 3,104 | `b96f6a6c097c` |
| `file use=referenced` | 5,112 | `690378161a3c` |
| `file use=stale` | 1,670 | `10ab724f95a9` |
| `file use=unused` | 3,219 | `4924914fd7a3` |
| `file with no path spelled` | 3,116 | `2a321a390f6f` |
| `file.format-converted` | 1,969 | `b770e36141eb` |
| `file.format-converted by=legacy-formats` | 1,969 | `b770e36141eb` |
| `file.format-converted how=as-is` | 2 | `6e3d4b1755d9` |
| `file.format-converted how=dds-as-tex` | 1,877 | `b770e36141eb` |
| `file.format-converted how=sco-as-scb` | 346 | `032db643a88a` |
| `file.former-path` | 2,870 | `c20537006f52` |
| `file.moved` | 1,889 | `088d0a057ec0` |
| `file.moved by=moved-assets` | 1,889 | `088d0a057ec0` |
| `file.moved how=as-is` | 1,818 | `088d0a057ec0` |
| `file.moved how=dds-as-tex` | 471 | `7ceb25cc7fc6` |
| `file.moved how=sco-as-scb` | 61 | `2ad573a25725` |
| `file.moved passes a copy over` | 783 | `088d0a057ec0` |
| `file.moved rename=base-suffix` | 34 | `088d0a057ec0` |
| `file.moved rename=resolution-prefix-2x` | 98 | `1ab1481ba822` |
| `file.moved rename=resolution-prefix-4x` | 2 | `e47acc8b0cd9` |
| `file.moved rename=shared-bin` | 1,463 | `40e2361d74da` |
| `file.moved rename=variant-tag` | 786 | `012afca8b8bb` |
| `file.moved rename=variant-tag-and-prefix-2x` | 11 | `3c7cd0bdba70` |
| `file.older-game` | 3,089 | `8b880d06d01c` |
| `file.older-game of bytes the game still ships` | 154 | `801766ddc63e` |
| `file.stale` | 1,670 | `10ab724f95a9` |
| `file.unused` | 3,218 | `2a321a390f6f` |
| `provenance at path=not-game-path` | 4,575 | `c20537006f52` |
| `provenance at path=other-bytes` | 5,120 | `690378161a3c` |
| `provenance at path=same-bytes` | 2,120 | `690378161a3c` |
| `provenance in a skin's folder` | 4,340 | `690378161a3c` |
| `provenance origin=history` | 3,089 | `8b880d06d01c` |
| `provenance origin=install` | 4,094 | `690378161a3c` |
| `provenance origin=own` | 5,119 | `690378161a3c` |
| `provenance still shipped=false` | 3,059 | `8b880d06d01c` |
| `provenance still shipped=true` | 154 | `801766ddc63e` |
| `rework.restoration` | 43 | `70e59f043f84` |
| `unit whose bin does not read` | 2 | `e6e43d1fb3db` |
| `unit whose entry name spells no slot` | 6 | `aacf8b151707` |
| `unit with no skin entry` | 3 | `e6e43d1fb3db` |

### fixups

| case | mods | example |
|---|---:|---|
| `change in an object no skin entry reaches` | 1,356 | `2ad546b10048` |
| `fixup health-bar-style` | 408 | `771fb4f81695` |
| `fixup health-bar-style U8>U8` | 406 | `771fb4f81695` |
| `fixup health-bar-style as=value` | 408 | `771fb4f81695` |
| `fixup health-bar-style nothing>Embed` | 2 | `45a9a2f7f9db` |
| `fixup known-artifacts-static-material` | 1 | `a18972e7c489` |
| `fixup known-artifacts-static-material as=removed` | 1 | `a18972e7c489` |
| `fixup known-artifacts-value-color` | 240 | `e1a072126e73` |
| `fixup known-artifacts-value-color as=removed` | 240 | `e1a072126e73` |
| `fixup known-artifacts-vfx-emitter` | 40 | `a109a8e59dbe` |
| `fixup known-artifacts-vfx-emitter as=removed` | 40 | `a109a8e59dbe` |
| `fixup known-artifacts-vfx-system` | 3 | `42c4119154ef` |
| `fixup known-artifacts-vfx-system as=removed` | 3 | `42c4119154ef` |
| `fixup material-default-technique` | 142 | `eff9a3902ba0` |
| `fixup material-default-technique as=removed` | 142 | `eff9a3902ba0` |
| `fixup material-sampler-fields` | 374 | `65205508102d` |
| `fixup material-sampler-fields as=moved` | 374 | `65205508102d` |
| `fixup material-shader-relink` | 89 | `04a8fa2bd3a9` |
| `fixup material-shader-relink Link>Link` | 89 | `04a8fa2bd3a9` |
| `fixup material-shader-relink as=value` | 89 | `04a8fa2bd3a9` |
| `fixup reshape-hoist` | 6 | `9f9a7268dd0d` |
| `fixup reshape-hoist Pointer>String` | 6 | `9f9a7268dd0d` |
| `fixup reshape-hoist as=moved` | 6 | `9f9a7268dd0d` |
| `fixup reshape-hoist as=value` | 6 | `9f9a7268dd0d` |
| `fixup retype` | 3,957 | `b96f6a6c097c` |
| `fixup retype Bool>Flag` | 14 | `2749b36689c6` |
| `fixup retype Embed>Pointer` | 19 | `a18972e7c489` |
| `fixup retype F32 wants Option` | 1 | `21c1e6eb6091` |
| `fixup retype Hash wants File` | 1 | `711bb07212b5` |
| `fixup retype Hash>File` | 357 | `14b001e04a69` |
| `fixup retype List wants F32` | 29 | `93859ba95948` |
| `fixup retype List wants Vec3` | 26 | `8e02074e29ea` |
| `fixup retype List2>List2` | 1 | `21c1e6eb6091` |
| `fixup retype List>List` | 130 | `ab780f5711fe` |
| `fixup retype List>List2` | 2 | `0b9a40624870` |
| `fixup retype Map>Map` | 71 | `646effb09ec6` |
| `fixup retype Option>Option` | 3,859 | `b96f6a6c097c` |
| `fixup retype String>File` | 3,943 | `b96f6a6c097c` |
| `fixup retype U16>U8` | 4 | `9924a6475bcc` |
| `fixup retype U32>U8` | 134 | `1dbb0c66a5df` |
| `fixup retype U8>U16` | 81 | `1b25d069efac` |
| `fixup retype as=removed` | 73 | `9db170f43edc` |
| `fixup retype as=unconvertible` | 38 | `93859ba95948` |
| `fixup retype as=value` | 3,957 | `b96f6a6c097c` |
| `fixup strip-undeclared` | 1,022 | `7206b6fe8985` |
| `fixup strip-undeclared as=lost` | 281 | `a2c700b70d1d` |
| `fixup strip-undeclared as=removed` | 821 | `7206b6fe8985` |
| `fixup vfx-emission-surface` | 230 | `55269231c645` |
| `fixup vfx-emission-surface as=moved` | 163 | `d2abd82ef49c` |
| `fixup vfx-emission-surface as=removed` | 227 | `55269231c645` |
| `fixup vfx-filtering` | 14 | `a62c018e8eb8` |
| `fixup vfx-filtering as=moved` | 14 | `a62c018e8eb8` |
| `fixup vfx-flex-shape` | 14 | `45a9a2f7f9db` |
| `fixup vfx-flex-shape as=moved` | 14 | `45a9a2f7f9db` |
| `fixup vfx-legacy-simple` | 76 | `a62c018e8eb8` |
| `fixup vfx-legacy-simple as=moved` | 6 | `45a9a2f7f9db` |
| `fixup vfx-legacy-simple as=removed` | 72 | `a62c018e8eb8` |
| `fixup vfx-linger` | 9 | `a62c018e8eb8` |
| `fixup vfx-linger as=moved` | 9 | `a62c018e8eb8` |
| `fixup vfx-mesh-align` | 6 | `a62c018e8eb8` |
| `fixup vfx-mesh-align as=moved` | 6 | `a62c018e8eb8` |
| `fixup vfx-mesh-align as=value` | 4 | `a62c018e8eb8` |
| `fixup vfx-mesh-align nothing>Bool` | 4 | `a62c018e8eb8` |
| `fixup vfx-spawn-shape` | 78 | `5068f7822ad6` |
| `fixup vfx-spawn-shape as=moved` | 78 | `5068f7822ad6` |
| `fixup vfx-spawn-shape-birth-translation` | 272 | `07bc0e8e2a0d` |
| `fixup vfx-spawn-shape-birth-translation as=moved` | 198 | `07bc0e8e2a0d` |
| `fixup vfx-spawn-shape-birth-translation as=removed` | 213 | `07bc0e8e2a0d` |
| `fixup vfx-texture-mult` | 31 | `93fc5770bc4d` |
| `fixup vfx-texture-mult as=moved` | 31 | `93fc5770bc4d` |
| `fixup vfx-texture-mult-strays` | 9 | `a62c018e8eb8` |
| `fixup vfx-texture-mult-strays as=moved` | 9 | `a62c018e8eb8` |
| `fixup vfx-tool-shape` | 15 | `1fbe7c86d035` |
| `fixup vfx-tool-shape as=moved` | 15 | `1fbe7c86d035` |
| `fixup vfx-tool-shape as=removed` | 1 | `ea60862faee2` |
| `fixup.changed` | 3,957 | `b96f6a6c097c` |
| `fixup.changed by=health-bar-style` | 408 | `771fb4f81695` |
| `fixup.changed by=material-sampler-fields` | 374 | `65205508102d` |
| `fixup.changed by=material-shader-relink` | 89 | `04a8fa2bd3a9` |
| `fixup.changed by=reshape-hoist` | 6 | `9f9a7268dd0d` |
| `fixup.changed by=retype` | 3,957 | `b96f6a6c097c` |
| `fixup.changed by=vfx-emission-surface` | 163 | `d2abd82ef49c` |
| `fixup.changed by=vfx-filtering` | 14 | `a62c018e8eb8` |
| `fixup.changed by=vfx-flex-shape` | 14 | `45a9a2f7f9db` |
| `fixup.changed by=vfx-legacy-simple` | 6 | `45a9a2f7f9db` |
| `fixup.changed by=vfx-linger` | 9 | `a62c018e8eb8` |
| `fixup.changed by=vfx-mesh-align` | 6 | `a62c018e8eb8` |
| `fixup.changed by=vfx-spawn-shape` | 78 | `5068f7822ad6` |
| `fixup.changed by=vfx-spawn-shape-birth-translation` | 198 | `07bc0e8e2a0d` |
| `fixup.changed by=vfx-texture-mult` | 31 | `93fc5770bc4d` |
| `fixup.changed by=vfx-texture-mult-strays` | 9 | `a62c018e8eb8` |
| `fixup.changed by=vfx-tool-shape` | 15 | `1fbe7c86d035` |
| `fixup.lost` | 281 | `a2c700b70d1d` |
| `fixup.lost by=strip-undeclared` | 281 | `a2c700b70d1d` |
| `fixup.removed` | 1,152 | `7206b6fe8985` |
| `fixup.removed by=known-artifacts-static-material` | 1 | `a18972e7c489` |
| `fixup.removed by=known-artifacts-value-color` | 240 | `e1a072126e73` |
| `fixup.removed by=known-artifacts-vfx-emitter` | 40 | `a109a8e59dbe` |
| `fixup.removed by=known-artifacts-vfx-system` | 3 | `42c4119154ef` |
| `fixup.removed by=material-default-technique` | 142 | `eff9a3902ba0` |
| `fixup.removed by=retype` | 73 | `9db170f43edc` |
| `fixup.removed by=strip-undeclared` | 821 | `7206b6fe8985` |
| `fixup.removed by=vfx-emission-surface` | 227 | `55269231c645` |
| `fixup.removed by=vfx-legacy-simple` | 72 | `a62c018e8eb8` |
| `fixup.removed by=vfx-spawn-shape-birth-translation` | 213 | `07bc0e8e2a0d` |
| `fixup.removed by=vfx-tool-shape` | 1 | `ea60862faee2` |
| `fixup.unconvertible` | 38 | `93859ba95948` |
| `fixup.unconvertible by=retype` | 38 | `93859ba95948` |

### facts

| case | mods | example |
|---|---:|---|
| `check aspect=clips` | 4,509 | `690378161a3c` |
| `check aspect=mesh` | 5,120 | `690378161a3c` |
| `check aspect=rig` | 3 | `e6e43d1fb3db` |
| `check aspect=skeleton` | 5,120 | `690378161a3c` |
| `check aspect=slot` | 5 | `77058d9704d3` |
| `check aspect=submeshes` | 4,166 | `042eb2b27725` |
| `check fit=does-not-fit` | 1,412 | `d905b9c41357` |
| `check fit=fits` | 5,118 | `690378161a3c` |
| `check fit=fits-with-fix` | 22 | `13137f23efc8` |
| `check fit=unknown` | 122 | `e2e6ce24228c` |
| `check why=clips-blocked` | 423 | `91c798fedc8b` |
| `check why=clips-fit` | 4,505 | `690378161a3c` |
| `check why=copied-hidden` | 22 | `13137f23efc8` |
| `check why=copied-shown` | 189 | `12520a916f1d` |
| `check why=copied-visible` | 265 | `90fb9073d4e0` |
| `check why=foreign-mesh` | 100 | `e2e6ce24228c` |
| `check why=foreign-skeleton` | 99 | `e2e6ce24228c` |
| `check why=no-clips` | 20 | `46e2a68b6b93` |
| `check why=no-copied-submesh` | 4,166 | `042eb2b27725` |
| `check why=no-entry` | 3 | `e6e43d1fb3db` |
| `check why=no-mesh` | 1 | `fc86c0d71d96` |
| `check why=no-skeleton` | 1 | `fc86c0d71d96` |
| `check why=no-such-skin` | 5 | `77058d9704d3` |
| `check why=other-skin-mesh` | 778 | `d905b9c41357` |
| `check why=other-skin-skeleton` | 723 | `d905b9c41357` |
| `check why=own-mesh` | 4,166 | `042eb2b27725` |
| `check why=own-skeleton` | 2,874 | `548613f5aa7c` |
| `check why=premium-skeleton` | 82 | `ab3df3f38fa0` |
| `check why=slot-joints` | 1,848 | `042eb2b27725` |
| `check why=slot-mesh` | 1,177 | `690378161a3c` |
| `check why=slot-skeleton` | 1,191 | `690378161a3c` |
| `clips blocked=needs-own-rig` | 419 | `91c798fedc8b` |
| `clips blocked=own-off` | 2 | `8d8774c67b13` |
| `clips blocked=refused` | 421 | `91c798fedc8b` |
| `clips graph found=false` | 2 | `137304ee1cad` |
| `clips graph found=true` | 5,120 | `690378161a3c` |
| `clips on the unit's own rig=false` | 1,289 | `690378161a3c` |
| `clips on the unit's own rig=true` | 4,149 | `042eb2b27725` |
| `clips own, off the slot's joints` | 2 | `8d8774c67b13` |
| `clips reused from another skin` | 1,777 | `0db3470a3dd1` |
| `clips reused on the same joints=false` | 1,740 | `0db3470a3dd1` |
| `clips reused on the same joints=true` | 59 | `a069969dabbc` |
| `clips unknown` | 238 | `a8cbdd4e3846` |
| `copies spread over other skins` | 1,007 | `2a4bdfa3de06` |
| `copies.distinct-units` | 491 | `ab3df3f38fa0` |
| `copies.found` | 1,029 | `2a4bdfa3de06` |
| `copies.near` | 614 | `29172c455892` |
| `copy kind=near` | 614 | `29172c455892` |
| `copy kind=renamed` | 214 | `605c42917603` |
| `copy kind=same-rig` | 513 | `2a4bdfa3de06` |
| `copy role=copy` | 1,029 | `2a4bdfa3de06` |
| `copy role=representative` | 5,120 | `690378161a3c` |
| `game object via=history` | 3,455 | `b96f6a6c097c` |
| `game object via=install` | 3,771 | `690378161a3c` |
| `game object=game-exact` | 4,867 | `690378161a3c` |
| `game object=game-renamed` | 644 | `16cdc7e5ea65` |
| `origin evidence weight=strong` | 4,535 | `690378161a3c` |
| `origin evidence weight=weak` | 4,530 | `690378161a3c` |
| `origin evidence=audio-tag` | 1,096 | `c30b3459834f` |
| `origin evidence=files` | 2,646 | `690378161a3c` |
| `origin evidence=graph` | 1,023 | `c30b3459834f` |
| `origin evidence=graph-closeness` | 1,352 | `04ac5afc04f1` |
| `origin evidence=mesh` | 1,824 | `6de493211fd6` |
| `origin evidence=objects` | 4,195 | `690378161a3c` |
| `origin evidence=resolver` | 286 | `37b0f73a5134` |
| `origin evidence=resolver-closeness` | 4,070 | `690378161a3c` |
| `origin evidence=skeleton` | 1,825 | `6de493211fd6` |
| `origin evidence=skin-name` | 839 | `ab3df3f38fa0` |
| `origin evidence=texture` | 1,572 | `1a276d766e03` |
| `origin own mesh=false` | 1,274 | `690378161a3c` |
| `origin own mesh=true` | 4,166 | `042eb2b27725` |
| `origin strength=strong` | 4,427 | `690378161a3c` |
| `origin strength=weak` | 601 | `042eb2b27725` |
| `origin verdict=base` | 3,313 | `690378161a3c` |
| `origin verdict=custom` | 130 | `2c79b080d055` |
| `origin verdict=premium` | 1,936 | `d905b9c41357` |
| `origin verdict=unknown` | 157 | `2dc58d50ccd1` |
| `origin with no evidence that decides` | 287 | `2c79b080d055` |
| `origin.ambiguous` | 27 | `894d1ee26bd9` |
| `origin.conflicting` | 291 | `ab3df3f38fa0` |
| `origin.none` | 157 | `2dc58d50ccd1` |
| `rework made=after` | 457 | `df91618c3af9` |
| `rework made=before` | 15 | `c20537006f52` |
| `rework made=unknown` | 2 | `90fb9073d4e0` |
| `rework=10.11` | 14 | `6a28ca75546c` |
| `rework=10.7` | 5 | `b013038f2030` |
| `rework=11.12` | 21 | `07a9cab5109b` |
| `rework=11.23` | 51 | `90fb9073d4e0` |
| `rework=12.16` | 30 | `9d9402ecf4d5` |
| `rework=13.20` | 23 | `df91618c3af9` |
| `rework=13.3` | 76 | `4c5ec791f9fa` |
| `rework=14.20` | 19 | `8854260c90a3` |
| `rework=14.24` | 18 | `5ed8007fd17b` |
| `rework=14.7` | 13 | `bd60e259470b` |
| `rework=14.9` | 65 | `455499bf8e8c` |
| `rework=15.17` | 16 | `1d3c6e306523` |
| `rework=15.7` | 25 | `c81fc359a259` |
| `rework=16.6` | 29 | `c20537006f52` |
| `rework=9.12` | 48 | `64ff2706f13a` |
| `rework=9.16` | 21 | `37304c11dd60` |
| `rig clip at a moved path` | 66 | `ba6e499a11c5` |
| `rig clip own=false` | 4,710 | `690378161a3c` |
| `rig clip own=true` | 1,560 | `98106ae9af46` |
| `rig clip source=game` | 3,609 | `690378161a3c` |
| `rig clip source=missing` | 7 | `5db3d0c678cf` |
| `rig clip source=mod` | 2,436 | `98106ae9af46` |
| `rig graph from=archive` | 2,395 | `98106ae9af46` |
| `rig graph from=game` | 2,846 | `690378161a3c` |
| `rig mesh at a moved path` | 102 | `298c1ffbf113` |
| `rig mesh own=false` | 1,271 | `690378161a3c` |
| `rig mesh own=true` | 4,166 | `042eb2b27725` |
| `rig mesh source=game` | 877 | `690378161a3c` |
| `rig mesh source=missing` | 1 | `fc86c0d71d96` |
| `rig mesh source=mod` | 4,442 | `042eb2b27725` |
| `rig part unreadable=skeleton` | 1 | `0ab4237c5cdf` |
| `rig skeleton at a moved path` | 102 | `298c1ffbf113` |
| `rig skeleton own=false` | 1,285 | `690378161a3c` |
| `rig skeleton own=true` | 4,149 | `042eb2b27725` |
| `rig skeleton source=game` | 875 | `690378161a3c` |
| `rig skeleton source=missing` | 1 | `fc86c0d71d96` |
| `rig skeleton source=mod` | 4,441 | `042eb2b27725` |
| `rig submesh=hidden-never` | 792 | `52aa42a711bd` |
| `rig submesh=hidden-shown` | 1,551 | `34a1e0d78ac0` |
| `rig submesh=visible` | 4,166 | `042eb2b27725` |
| `rig.submesh-copied` | 475 | `90fb9073d4e0` |
| `rig.submesh-copied visibility=hidden-never` | 147 | `45c9eb0eb82d` |
| `rig.submesh-copied visibility=hidden-shown` | 348 | `957d17b60f1e` |
| `rig.submesh-copied visibility=visible` | 265 | `90fb9073d4e0` |
| `rig.unreadable` | 1 | `0ab4237c5cdf` |
| `rig.unreadable parts=skeleton` | 1 | `0ab4237c5cdf` |
| `slot the game has a bin at=true` | 5,120 | `690378161a3c` |

### decision

| case | mods | example |
|---|---:|---|
| `candidate feasible=false` | 616 | `bec3c44096ea` |
| `candidate feasible=true` | 4,504 | `690378161a3c` |
| `candidate=skin0 first` | 4,474 | `690378161a3c` |
| `candidate=the champion's target` | 464 | `46aa40949486` |
| `candidate=the premium origin` | 537 | `ed7617f29465` |
| `candidate=the skin whose clips it plays` | 86 | `91c798fedc8b` |
| `candidate=the skin whose skeleton it takes` | 1 | `bec3c44096ea` |
| `decision by the clips played` | 86 | `91c798fedc8b` |
| `decision made before a rework` | 15 | `c20537006f52` |
| `decision makes a new bin` | 5 | `77058d9704d3` |
| `decision origin on the base's rig` | 23 | `de5d7c1e5189` |
| `decision reason=base` | 3,426 | `690378161a3c` |
| `decision reason=clips-origin` | 84 | `91c798fedc8b` |
| `decision reason=conflict` | 1 | `552d4d5e68c1` |
| `decision reason=fallback` | 497 | `ed7617f29465` |
| `decision reason=infeasible` | 3 | `8d8774c67b13` |
| `decision reason=paste-cleaned` | 463 | `560ccdbe142b` |
| `decision reason=pre-rework` | 15 | `c20537006f52` |
| `decision reason=skeleton-origin` | 1 | `bec3c44096ea` |
| `decision reason=slot` | 971 | `d905b9c41357` |
| `decision reason=sub-follows` | 439 | `46aa40949486` |
| `decision reason=sub-left-to-base` | 28 | `03a8a0aa8c28` |
| `decision reason=superseded` | 5 | `0f805b36e089` |
| `decision reason=unjudged` | 2 | `e6e43d1fb3db` |
| `decision taken by another unit` | 5 | `0f805b36e089` |
| `decision unfit` | 23 | `77058d9704d3` |
| `decision with chroma followers` | 1,229 | `33a76ab3965e` |
| `decision with no target` | 49 | `c20537006f52` |
| `target.chosen` | 5,102 | `690378161a3c` |
| `target.chosen reason=base` | 3,426 | `690378161a3c` |
| `target.chosen reason=clips-origin` | 86 | `91c798fedc8b` |
| `target.chosen reason=fallback` | 497 | `ed7617f29465` |
| `target.chosen reason=paste-cleaned` | 463 | `560ccdbe142b` |
| `target.chosen reason=skeleton-origin` | 1 | `bec3c44096ea` |
| `target.chosen reason=slot` | 971 | `d905b9c41357` |
| `target.chosen reason=sub-follows` | 424 | `46aa40949486` |
| `target.conflict` | 1 | `552d4d5e68c1` |
| `target.forced-unfit` | 23 | `77058d9704d3` |
| `target.forced-unfit follows=true` | 23 | `77058d9704d3` |
| `target.infeasible` | 3 | `8d8774c67b13` |
| `target.pre-rework` | 15 | `c20537006f52` |
| `target.sub-left` | 28 | `03a8a0aa8c28` |
| `target.superseded` | 5 | `0f805b36e089` |
| `target.unjudged` | 3 | `e6e43d1fb3db` |
| `targets conflict` | 1 | `552d4d5e68c1` |

### plan

| case | mods | example |
|---|---:|---|
| `file verdict=dangling` | 1,274 | `b96f6a6c097c` |
| `file verdict=game` | 4,908 | `690378161a3c` |
| `file verdict=own` | 5,082 | `690378161a3c` |
| `file verdict=unmounted` | 324 | `e2e6ce24228c` |
| `hud.carried` | 1,439 | `cb2c00df0e69` |
| `object reached as=carried` | 4,522 | `8023f23a12d3` |
| `object reached as=entry` | 5,105 | `690378161a3c` |
| `object reached as=game` | 1,762 | `2ec7826df1e5` |
| `object reached as=graph` | 2,394 | `98106ae9af46` |
| `object reached as=resolver` | 5,096 | `690378161a3c` |
| `object.carried` | 5,120 | `690378161a3c` |
| `object.game-carried` | 778 | `8023f23a12d3` |
| `object.link-dangling` | 1,976 | `0db3470a3dd1` |
| `object.unreached` | 1,590 | `551e18abc8c4` |
| `reach carries game objects for overrides` | 778 | `8023f23a12d3` |
| `reach enters the champion's objects` | 279 | `21891a4c64e2` |
| `reach keeps links to the game's objects` | 5,015 | `690378161a3c` |
| `ref.dangling` | 1,270 | `b96f6a6c097c` |
| `ref.dangling at a former path` | 262 | `b96f6a6c097c` |
| `ref.unmounted` | 321 | `e2e6ce24228c` |
| `references pointed at the game's files` | 3,743 | `b96f6a6c097c` |
| `renames keep links to the source slot` | 6 | `bec3c44096ea` |
| `renames retarget a unit` | 596 | `bec3c44096ea` |
| `renames swap game names` | 418 | `bec3c44096ea` |
| `renames to the target's names` | 596 | `bec3c44096ea` |
| `unit left=chroma-folded` | 113 | `03ee59f50b34` |
| `unit left=conflict` | 1 | `552d4d5e68c1` |
| `unit left=copy` | 1,029 | `2a4bdfa3de06` |
| `unit left=equal-to-game` | 216 | `c20537006f52` |
| `unit left=game-skin` | 33 | `9c97ead533ea` |
| `unit left=pre-rework` | 15 | `c20537006f52` |
| `unit left=sub-left` | 28 | `03a8a0aa8c28` |
| `unit left=superseded` | 5 | `0f805b36e089` |
| `unit ships its graph` | 2,263 | `a8cbdd4e3846` |
| `unit without a resolver` | 57 | `012afca8b8bb` |
| `unit.left` | 1,380 | `c20537006f52` |
| `unit.left why=chroma-folded` | 113 | `03ee59f50b34` |
| `unit.left why=conflict` | 1 | `552d4d5e68c1` |
| `unit.left why=copy` | 1,029 | `2a4bdfa3de06` |
| `unit.left why=equal-to-game` | 216 | `c20537006f52` |
| `unit.left why=game-skin` | 33 | `9c97ead533ea` |
| `unit.left why=pre-rework` | 15 | `c20537006f52` |
| `unit.left why=sub-left` | 28 | `03a8a0aa8c28` |
| `unit.left why=superseded` | 5 | `0f805b36e089` |
| `unit.new-bin` | 5 | `77058d9704d3` |
| `unit.ported` | 5,086 | `690378161a3c` |
| `unit.ported graph=false` | 2,934 | `690378161a3c` |
| `unit.ported graph=true` | 2,257 | `a8cbdd4e3846` |
| `unit.ported new_bin=false` | 5,086 | `690378161a3c` |
| `unit.ported new_bin=true` | 5 | `77058d9704d3` |
| `unit.retargeted` | 596 | `bec3c44096ea` |

### audio

| case | mods | example |
|---|---:|---|
| `audio.bank-id-set` | 1,453 | `7bc34b21a01b` |
| `audio.bank-id-set by=bank-id` | 1,453 | `7bc34b21a01b` |
| `audio.bank-unreadable` | 1 | `b02bf9ee3b33` |
| `audio.carried` | 2,241 | `b96f6a6c097c` |
| `audio.carried a locale copy once` | 1 | `9b85da734738` |
| `audio.carried events no unit's list names` | 119 | `eb1e1ad93ca9` |
| `audio.carried from_mod=false` | 295 | `b0ff2afbf036` |
| `audio.carried from_mod=true` | 142 | `a2065296f9af` |
| `audio.carried layer=base` | 1,347 | `b96f6a6c097c` |
| `audio.carried layer=voice` | 1,079 | `ed6637fa8f40` |
| `audio.carried layer=voice-locale` | 541 | `eb88a666d18a` |
| `audio.carried shape=overlay` | 1,935 | `b96f6a6c097c` |
| `audio.carried shape=own` | 400 | `b0ff2afbf036` |
| `audio.carried sounds that play nothing` | 32 | `a109a8e59dbe` |
| `audio.carried stands in for a game unit` | 27 | `5c2e33d0b72d` |
| `audio.literals-renamed` | 165 | `c60ba3ab5389` |
| `audio.literals-renamed to the base's` | 95 | `c60ba3ab5389` |
| `audio.literals-renamed to the mod's events` | 93 | `cdadd71210b5` |
| `audio.not-carried` | 71 | `4a19b5b08ab6` |
| `audio.not-carried why=no-event` | 56 | `4a19b5b08ab6` |
| `audio.not-carried why=no-events-bank` | 10 | `b1ea4758a917` |
| `audio.not-carried why=no-game-audio` | 5 | `3052a5d3f558` |
| `audio.units-left-out` | 124 | `999455df6a97` |
| `bank in a locale's WAD` | 548 | `eb88a666d18a` |
| `bank role=events` | 2,181 | `10ab724f95a9` |
| `bank role=media` | 2,253 | `b96f6a6c097c` |
| `bank role=package` | 1,811 | `eb88a666d18a` |
| `bank role=stub` | 1,159 | `4d772c5713b1` |
| `bank unreadable` | 1 | `b02bf9ee3b33` |
| `bank with clips of the mod's own` | 2,264 | `b96f6a6c097c` |
| `carried unit layer=base` | 1,347 | `b96f6a6c097c` |
| `carried unit layer=voice` | 1,079 | `ed6637fa8f40` |
| `carried unit layer=voice-locale` | 541 | `eb88a666d18a` |
| `carried unit over the game's events` | 1,935 | `b96f6a6c097c` |
| `carried unit stands in for a game unit` | 27 | `5c2e33d0b72d` |
| `carried unit with events of its own` | 400 | `b0ff2afbf036` |
| `sound literals renamed` | 168 | `b0ff2afbf036` |
| `sound unit voice=false` | 5,116 | `690378161a3c` |
| `sound unit voice=true` | 4,974 | `690378161a3c` |
| `unit leaves out a game unit` | 124 | `999455df6a97` |

### package

| case | mods | example |
|---|---:|---|
| `again built=true` | 5,093 | `690378161a3c` |
| `again redated` | 4,733 | `690378161a3c` |
| `chunk origin=bank` | 2,225 | `b467633d7198` |
| `chunk origin=contained` | 3,002 | `40300b55696d` |
| `chunk origin=delta` | 3,935 | `40300b55696d` |
| `chunk origin=kept` | 5,101 | `690378161a3c` |
| `chunk origin=made` | 4,543 | `8023f23a12d3` |
| `layer=base` | 5,101 | `690378161a3c` |
| `layer=hud` | 1,433 | `cb2c00df0e69` |
| `layer=voice` | 1,074 | `ed6637fa8f40` |
| `layer=voice-locale` | 539 | `eb88a666d18a` |
| `package.empty` | 15 | `c20537006f52` |
| `package.field-kept` | 395 | `0247e6935e9d` |
| `package.in-place` | 5,093 | `690378161a3c` |
| `package.in-place left=former-path` | 2,849 | `c20537006f52` |
| `package.in-place left=orphaned` | 32 | `9db170f43edc` |
| `package.in-place left=stale` | 1,705 | `c92cdc9e684c` |
| `package.in-place left=unused` | 3,199 | `2a321a390f6f` |
| `package.in-place ships files a fixup made` | 2,748 | `088d0a057ec0` |
| `package.in-place writes bins again` | 3,935 | `40300b55696d` |
| `package.ported` | 5,086 | `690378161a3c` |
| `package.ported left=former-path` | 2,841 | `8481ee36daca` |
| `package.ported left=game` | 218 | `9ee78322b1d9` |
| `package.ported left=not-carried` | 5,086 | `690378161a3c` |
| `package.ported left=orphaned` | 2,880 | `eb5aba5314eb` |
| `package.ported left=recut` | 2,225 | `b467633d7198` |
| `package.ported left=stale` | 1,655 | `c92cdc9e684c` |
| `package.ported left=unused` | 3,200 | `2a321a390f6f` |
| `package.resolver-frozen` | 4,393 | `8023f23a12d3` |
| `package.shader-missing` | 27 | `b96f6a6c097c` |
| `package.texture-contained` | 3,039 | `40300b55696d` |
| `package.texture-contained contained` | 3,002 | `40300b55696d` |
| `package.texture-contained kept=format` | 304 | `b540716595d2` |
| `package.texture-contained kept=not-one-surface` | 24 | `0707d08b753c` |
| `package.texture-contained kept=partial-blocks` | 685 | `50b8b16da548` |
| `package.texture-contained kept=partial-mips` | 121 | `b540716595d2` |
| `package.texture-contained kept=truncated` | 2 | `8c8bf89b8b5a` |
| `package.thumbnail-dropped` | 18 | `9cd0944e4517` |
| `package.thumbnail-dropped why=entry-too-large` | 18 | `9cd0944e4517` |
| `patch adds links` | 4,548 | `8023f23a12d3` |
| `patch layer=base` | 5,086 | `690378161a3c` |
| `patch layer=hud` | 1,433 | `cb2c00df0e69` |
| `patch layer=voice` | 1,074 | `ed6637fa8f40` |
| `patch layer=voice-locale` | 539 | `eb88a666d18a` |
| `patch of=root` | 1,433 | `cb2c00df0e69` |
| `patch of=skin` | 5,086 | `690378161a3c` |
| `patch outcome=applied` | 5,086 | `690378161a3c` |

### checks

| case | mods | example |
|---|---:|---|
| `check.done` | 5,101 | `690378161a3c` |
| `check.done by=apply` | 5,086 | `690378161a3c` |
| `check.done by=base-skin` | 5,086 | `690378161a3c` |
| `check.done by=round-trip` | 5,101 | `690378161a3c` |
| `check.done by=types` | 5,101 | `690378161a3c` |
| `check.done check=apply` | 5,086 | `690378161a3c` |
| `check.done check=base-skin` | 5,086 | `690378161a3c` |
| `check.done check=round-trip` | 5,101 | `690378161a3c` |
| `check.done check=types` | 5,101 | `690378161a3c` |
| `check.done source=schema` | 5,101 | `690378161a3c` |
| `check.field-undeclared` | 218 | `a2c700b70d1d` |
| `check.field-undeclared by=types` | 218 | `a2c700b70d1d` |
| `check.name-unresolved` | 3 | `25c6531f00ba` |
| `check.name-unresolved by=apply` | 3 | `25c6531f00ba` |
| `check.other-skin` | 4 | `9b4674f14653` |
| `check.other-skin by=base-skin` | 4 | `9b4674f14653` |
| `check.other-skin value=simple-skin` | 4 | `9b4674f14653` |
| `check.other-skin value=skeleton` | 4 | `9b4674f14653` |
| `check.type-mismatch` | 47 | `93859ba95948` |
| `check.type-mismatch by=types` | 47 | `93859ba95948` |
| `check.type-mismatch found=Embed` | 14 | `b733c0aa0293` |
| `check.type-mismatch found=F32` | 1 | `21c1e6eb6091` |
| `check.type-mismatch found=Hash` | 1 | `711bb07212b5` |
| `check.type-mismatch found=List` | 31 | `1378ad3d3f90` |
| `check.type-mismatch found=Pointer` | 23 | `93859ba95948` |
| `check.type-mismatch wants=Embed` | 14 | `b733c0aa0293` |
| `check.type-mismatch wants=File` | 1 | `711bb07212b5` |
| `check.type-mismatch wants=List` | 31 | `1378ad3d3f90` |
| `check.type-mismatch wants=Option` | 1 | `21c1e6eb6091` |
| `check.type-mismatch wants=Pointer` | 23 | `93859ba95948` |

### records

| case | mods | example |
|---|---:|---|
| `record #GAME-BEHAVIOR-0001` | 5 | `77058d9704d3` |
| `record #GAME-BEHAVIOR-0002` | 5,109 | `690378161a3c` |
| `record #GAME-BEHAVIOR-0004` | 1,439 | `cb2c00df0e69` |
| `record #GAME-BEHAVIOR-0007` | 321 | `e2e6ce24228c` |
| `record #GAME-BEHAVIOR-0023` | 27 | `b96f6a6c097c` |
| `record #GAME-CHANGE-0001` | 58 | `c20537006f52` |
| `record #GAME-CHANGE-0002` | 14 | `a62c018e8eb8` |
| `record #GAME-CHANGE-0003` | 9 | `a62c018e8eb8` |
| `record #GAME-CHANGE-0004` | 76 | `a62c018e8eb8` |
| `record #GAME-CHANGE-0005` | 31 | `93fc5770bc4d` |
| `record #GAME-CHANGE-0006` | 78 | `5068f7822ad6` |
| `record #GAME-CHANGE-0007` | 3,083 | `40300b55696d` |
| `record #GAME-CHANGE-0008` | 371 | `032db643a88a` |
| `record #GAME-CHANGE-0009` | 786 | `012afca8b8bb` |
| `record #GAME-CHANGE-0010` | 108 | `1ab1481ba822` |
| `record #GAME-CHANGE-0011` | 34 | `088d0a057ec0` |
| `record #GAME-CHANGE-0012` | 1,463 | `40e2361d74da` |
| `record #GAME-CHANGE-0014` | 374 | `65205508102d` |
| `record #GAME-CHANGE-0015` | 142 | `eff9a3902ba0` |
| `record #GAME-CHANGE-0016` | 14 | `45a9a2f7f9db` |
| `record #GAME-CHANGE-0017` | 6 | `a62c018e8eb8` |
| `record #GAME-CHANGE-0018` | 230 | `55269231c645` |
| `record #GAME-CHANGE-0023` | 408 | `771fb4f81695` |
| `record #GAME-CHANGE-0024` | 89 | `04a8fa2bd3a9` |
| `record #MOD-ARTIFACT-0001` | 1,029 | `2a4bdfa3de06` |
| `record #MOD-ARTIFACT-0004` | 1,670 | `10ab724f95a9` |
| `record #MOD-ARTIFACT-0010` | 71 | `975ff083bcb8` |
| `record #MOD-ARTIFACT-0011` | 19 | `0672cfff925b` |
| `record #MOD-ARTIFACT-0012` | 4 | `e5f3a984b590` |
| `record #MOD-ARTIFACT-0013` | 1,273 | `690378161a3c` |
| `record #MOD-ARTIFACT-0016` | 2,098 | `3b1aaabea06e` |
| `record #MOD-ARTIFACT-0017` | 9 | `a62c018e8eb8` |
| `record #MOD-ARTIFACT-0018` | 6 | `9f9a7268dd0d` |
| `record #MOD-ARTIFACT-0019` | 272 | `07bc0e8e2a0d` |
| `record #MOD-ARTIFACT-0020` | 275 | `e1a072126e73` |
| `record #MOD-ARTIFACT-0023` | 1,453 | `7bc34b21a01b` |
| `record #MOD-ARTIFACT-0024` | 475 | `90fb9073d4e0` |
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

105 cases, against the runs `port` and `repair` of 2026-10-09.

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

- `fixup reshape-wrap`

### facts, not seen

- `catalog.capped`
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
- `unit left=off-target`
- `unit.left why=no-target`
- `unit.left why=off-target`
- `unit.left-with-work`
- `unit.left-with-work kind=slot-work`
- `unit.left-with-work kind=unsupported`
- `unit.new-bin-failed`
- `unit.not-written`

### audio, not seen

- `audio.bank-unnamed`
- `audio.carried events sharing an id with the game's`
- `audio.not-carried why=locale-differs`
- `audio.not-carried why=new-bin`
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
- `check.other-skin value=audio-tag`
- `check.other-skin value=bank-unit`
- `check.other-skin value=skin-name`
- `check.other-skin value=texture`
- `check.record-skipped`
- `check.round-trip-failed`
- `check.round-trip-failed problem=entries`
- `check.round-trip-failed problem=object`
- `check.round-trip-failed problem=unreadable`

<!-- cases.py: end -->
