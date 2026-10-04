# Measured runs

Runs over the corpus, oldest first, each with its date and what it measured. A run about one
record goes in that record's evidence. What forge measured before the rewrite is in forge's own
runs log.

## The baseline (2026-10-03)

forge at `d6cd66f` over the corpus, with forge's tracked runner (`scripts/corpus/port_run.py`
there), against the 16.19.8207193 install copy, the history index of 755 builds and the meta
schema from LTK Manager's cache. Each step of the rewrite is compared with it (#ADR-0004).

- **Scope.** 8,604 mods classed, 5,164 in scope by the class table (#ADR-0033), 5,137 of them in
  the datasets. None failed.
- **Time.** 1,633 s at ten in parallel. 16,306 s summed over the mods, median 2.4 s, at most
  43.5 s.
- **Outcome.** 5,136 port. One halts: no WAD ships a champion's skin.

Champion units' targets under the default policy:

| target | reason | units |
|---|---|---:|
| skin0 | base | 3,455 |
| skin0 | paste cleaned | 464 |
| skin0 | fits no slot | 3 |
| skinN | slot | 5,748 |
| skinN | fallback to the premium origin | 497 |
| skinN | the skin whose clips it plays | 84 |
| skinN | unjudged | 2 |
| none | made before a rework | 17 |
| none | superseded | 4 |

Errors, by mods: a value of another type than the build declares in 41, made before a rework in
15, a name unresolved in 3, fits no slot in 3. Fields lost to a layout change in 30 mods, a value
with no conversion in 1.

Leaving the port: 1,960 chroma units folded, 31 mods re-shipping the game's own skins. 5 new
sub-character bins. Audio: 5,017 units over the game's, 568 of their own, 1,859 voice units in
the voice layer and 1,102 in a locale layer.

**The reduced corpus.** 27 mods show the 149 features of the run, in 65 s summed, 9 s at eight
in parallel. Two runs of it show no change in unit fates, targets, reasons or diagnostics.

**forge's reports are not byte for byte the same across two runs.** 9 of the 27 differ, in one
place: the order of a clip's joint hashes in the rig facts. A comparison reads that list as a
set.

## The history index (2026-10-03)

`reforge history index` over the census history at commit `39fa90b1c644`: 755 builds, from
8.20.2483196 to 16.19.8207193.

- **Time.** 1,369 s for a whole build on 32 threads: the walk 1,341 s, the write 28 s.
- **Size.** 1.61 GB.
- **Against forge's index** of the same commit (its format 8): every row of every table is
  equal, lives and head counts included, with names compared as text and joints and submeshes
  by their skeleton's and mesh's path and record. The lists of builds are equal.
- **Append.** An index of 29 builds with the 30th appended equals the whole build of 30 in every
  table.
- **Unparsed entry texts.** None.

| table | rows |
|---|---:|
| `files` | 1,893,046 |
| `paths` | 2,147,213 |
| `contents` | 3,374,160 |
| `objects` | 3,276,316 |
| `refs` | 8,375,230 |
| `tags` | 39,475 |
| `units` | 33,455 |
| `unit_banks` | 88,088 |
| `unit_events` | 1,303,662 |
| `clips` | 4,694,013 |
| `events` | 878,090 |
| `event_wems` | 6,477,024 |
| `skeletons`, `joints` | 30,519 and 1,322,078 |
| `meshes`, `submeshes` | 30,591 and 83,982 |
| `textures` | 1,625,296 |
| `names` | 88,173 |

Two builds of the history carry a version with the mark of a second manifest,
`14.8.5756478__2` and `14.8.5759209__2`.

`reforge history probe` against the 16.19.8207193 install copy:

| file | in the history | keys found |
|---|---|---|
| Ahri's `skin0.bin` | at 16.19.8207193 | 4 of 4 objects, 4 of 4 by norm key |
| Lux's `skin7.bin` | at 16.19.8207193 | 469 of 469 objects, 469 of 469 by norm key |
| Ahri's base effects bank | from 16.4.7461248 | 83 of 83 clips |
| Ahri's base voice package, `en_us` | from 15.11.6834878 | 157 of 157 clips |

The readers of the inputs, against this machine's (2026-10-03):

- **Install.** An install at 16.19.8230722: 393 WADs and 808,646 chunks open in 231 ms. None is
  unreadable.
- **Names.** The name-hash index over the hash cache's paths builds in 1.33 s, once per version
  of the cache. With it there, the names open in 0.9 ms.
- **Schema.** The meta database's 5,547 classes parse in 26 ms.
- **`reforge-magic`** gives the kind forge's gives on 4,000,000 inputs, and `reforge-wwise`
  reads and writes back 10,326 banks of the game byte for byte.

## The load stage (2026-10-04)

`reforge load` over the 5,137 mods of the datasets, each compared with forge's baseline report
of the same mod by `scripts/corpus/load_compare.py` in forge: WADs, files with their use and
source, characters, units, dating, inventory, file provenance, and the diagnostics of the
archive and of the load stage by code and subject.

| run | mods | equal | accepted deviations only | other differences | time |
|---|---:|---:|---:|---:|---:|
| reforge with forge's behaviour | 5,137 | 5,127 | 10 | 0 | 1,022 s |
| reforge with the three decisions of 2026-10-04 | 5,137 | 5,020 | 117 | 0 | 1,012 s |

Eight mods in parallel, about 1.3 s a mod. No mod failed. The reduced corpus of 27 mods takes
6 s.

The accepted deviations, each mod under one:

| deviation | mods | reason |
|---|---:|---|
| the larger reduced copy | 100 | #ADR-0038: 941 textures made from the `2x_` file, 914 with other bytes |
| a duplicate chunk in a packed WAD | 6 | reforge notes it (`archive.chunk-duplicate`); forge skips it without a note |
| a path spelled by a duplicate | 4 | an archive lists a chunk by its hash and again by its path; reforge keeps the first entry's bytes with the path the second spells |
| one chunk per path | 4 | a mod with no champion skin bin that ships a sub-character's keeps its own bin; forge adds the game's beside it |
| the earlier reworks | 3 | Mordekaiser, Pantheon and Fiddlesticks are in the rework table (#GAME-CHANGE-0001): 383 files of three restorations are not stale |
| a date at a marked build | 0 | no mod of the datasets is dated at a build with a `__N` mark |

Two faults of reforge found by the comparison and fixed:

- A file's skin folder and its effects flag were read off a path the name tables spelled. Forge
  reads them off the archive's own spelling. 17 of the 27 reduced mods differed.
- A path longer than 512 bytes was refused. The hash cache spells 1,264 game paths longer than
  that, the longest 2,454 bytes. The cap is 4,096.

Two rules checked against the same datasets afterwards (2026-10-04):

- **The `_base` rename for the mesh and the skeleton only** (#GAME-CHANGE-0011). It changes no
  mod: the 74 files the rename makes over the datasets are 37 meshes and 37 skeletons. The full
  comparison after it gives the same 5,020 equal mods and the same differences.
- **A legacy format's twin only at a path the game once held.** Not made. It would stop 30
  twins in 11 mods, and every one is an asset the mod means to replace (#GAME-CHANGE-0007,
  evidence). A `.dds` gets its `.tex` twin wherever the game ships the `.tex` and the mod ships
  no file there, as forge has it.

## The repair (2026-10-04)

`reforge repair --no-write --again` over the 5,137 mods of the datasets, each compared with
forge's `repair` pipeline run with no output directory, by `scripts/corpus/repair_compare.py` in
forge. Neither program writes a file of a mod: each report holds what the repair would write,
every chunk with the digest of its bytes and, for a bin, the digest of each object as `ltk_meta`
writes it. Compared: the halt, the chunks shipped per WAD and path hash, the bytes of each chunk
that is no bin, each bin's entries and each object's digest, and the codes of the package and
its checks.

| run | mods | equal | accepted deviations only | other differences |
|---|---:|---:|---:|---:|
| the full datasets | 5,137 | 5,028 | 109 | 0 |
| the same, with the 15 mods below run again after the fix | 5,137 | 5,021 | 116 | 0 |

forge took 3,663 s with eight mods in parallel, beside reforge's run. No mod failed in either
program. The reduced corpus of 27 mods takes 11 s in reforge: 23 equal, 4 under an accepted
deviation.

The accepted deviations, each mod under one:

| deviation | mods | reason |
|---|---:|---|
| the larger reduced copy | 90 | #ADR-0038: the texture made of the `2x_` file is shipped by both, in other bytes |
| one chunk per WAD and path | 11 | forge keeps what a repair writes by path alone, so of two WADs that hold a file at one path it writes one; reforge writes both |
| the twin of a file a bin names | 7 | in a packed WAD a file at a path the game dropped is named by the mod's bins alone. reforge gives the file fixups that path and makes the file the game reads; forge makes it when the repaired mod is repaired again |
| one chunk per path | 4 | the load stage's deviation: where forge holds the mod's own bin and the game's bin it took at one path, it writes the game's; reforge writes the mod's own, repaired |
| `retype` enters what it retags | 2 | forge retags a struct and leaves the values under it for a second run. reforge's object is forge's after two runs, checked on one of the two mods |
| the order of fields | 2 | the fixups run by phase, then by build (#ADR-0007), and forge's passes in another order: an emitter gets `Linger` and `FlexShapeDefinition` in the other order. The values are the same |

The last row needs each object's digest with its fields in one order, which forge's report of
the full run does not hold: forge's report of those two mods was made again.

What reforge's own checks say of its repairs, over the 5,137 mods:

- **Round trip.** No bin written again fails to read back.
- **Repaired again.** In 7 mods the package, repaired again, gained files: the twins above,
  which the first repair did not make. `load` gives the file fixups the paths the mod's bins
  spell. The 7 mods run again change nothing.
- **Types.** 144 mods hold a value of another type than the build reads, and 271 a field its
  class does not declare, after the repair. forge's type check finds the same in the same mods,
  but the two mods where `retype` enters what it retags. Three kinds:
  - *A bin at a former path, which the client does not load.* It is shipped as it is, and no
    fixup reads it. 98 of the 144 mods hold their values of another type in such bins alone,
    and 50 of the 271 hold undeclared fields there: `VfxAssetRemap.oldAsset` a hash where the
    build reads a file (41 mods), `VfxMaterialOverrideDefinitionData.baseTexture` and
    `StaticMaterialShaderSamplerDef.texturePath` strings where it reads files (34 and 28),
    `StaticMaterialShaderSamplerDef.samplerName` (15), an emitter's fields of an older layout.
  - *A class the target build does not have.* `ItemRecommendationContextList`, in a champion's
    root bin, left the game at 16.6: 227 mods hold one, with its three fields. `strip-undeclared`
    leaves an object of a class the build lacks.
  - *A value with no conversion*, in a bin the client loads (46 mods). A float animation where
    the build reads a vector's: `VfxAnimatedVector3fVariableData.values` a list of floats (26
    mods, which `retype` reports as unconvertible), `ValueVector3.dynamics` a pointer to
    `VfxAnimatedFloatVariableData` (23), an emitter's `scale0` a `ValueFloat` where the build
    reads a `ValueVector3` (10).

One mod of the reduced corpus was written: a project directory of 407 chunks and a `.modpkg`
of 9.6 MB. The `.modpkg` read back through `ltk_modpkg` holds 410 chunks, the 407 and three of
its own, and every chunk inflates, to 57.9 MB.

`ltk_modpkg` 0.12.5 writes its chunks in the order of a hashed map: two packages of one project
hold the same chunks in another order, so a `.modpkg` is not the same bytes on two runs.

The 15 mods of forge's in-game sample set (14 skin mods and a fix), repaired in memory with
`--again` (2026-10-04): each repairs. 39 bins are written again and every one reads back. No
value is of another type than the build reads. A second repair changes nothing in any of them.
One finding in the fix: two `ValueColor` fields under a hash no class declares, in a bin that is
not written again.
