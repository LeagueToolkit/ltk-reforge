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

## The repair, of what is wired up (2026-10-04)

`reforge repair --no-write --again` over the 5,137 mods after #ADR-0039 and the changes to four
fixups, compared with forge's reports of the run above by `repair_compare.py --base`. forge was
not run again. reforge took 1,895 s with twelve mods in parallel. 245 mods were run again after
two changes the run led to (below).

| mods | equal | accepted deviations only | other differences |
|---:|---:|---:|---:|
| 5,137 | 1,830 | 3,307 | 0 |

What each change does over the datasets:

| change | mods | what |
|---|---:|---|
| #ADR-0039, files left out | 3,297 | 146,896 files at a former path that nothing names, in 2,886 mods, and 87,466 stale copies, in 1,725, which forge writes. 279,006 files nothing loads or names and 140 named only by removed values are left out by both. 1,081,348 files are kept |
| a texture that is a stale copy in TEX's container | 101 | 626 textures: a DDS made of a TEX the game shipped at an earlier build is that TEX again once in TEX's container. `load` judges the DDS, the build judges the TEX it ships |
| `vfx-flex-shape`, #GAME-CHANGE-0016 | 1 | 2 values of `scaleEmitOffsetByBoundObjectHeight` moved, which the strip removed as lost. No bin the client loads holds one of the other three fields on an emitter |
| `material-sampler-fields`, #GAME-CHANGE-0014 | 6 | 108 sampler states' names left for the strip, which the run above moved into `TextureName`: over the slot's name 106 times, in 4 mods |
| `vfx-spawn-shape-birth-translation`, #MOD-ARTIFACT-0019 | 21 | the mods made from 13.15 to 13.24: 1,057 values moved to `EmitterPosition` and 3,049 removed where the emitter holds a position. 720 objects differ from forge's, in 20 mods. In the 206 mods made at 14.1 or later the strip removes 276,644 values, as before |
| no value into an option, #GAME-CHANGE-0021 | 1 | 1 value, `WasdMoveOrderData.0x43bf3213` in a map's bin of a skin mod, left and reported unconvertible |
| `material-default-technique` | 0 | each of the 17,842 values is `"normal"` |

The accepted deviations of the comparison, by the mods with a difference under each: only what
is wired up 3,297, stale once contained 101, the larger reduced copy 31, the birth translation
20, one chunk per WAD and path 11, the twin of a file a bin names 9, the sampler state's name
6, one chunk per path 4, `retype` enters what it retags 2, and one each for the flex-shape
fields, the option and the order of fields.

What reforge's own checks say of its repairs:

- **Round trip.** No bin written again fails to read back.
- **Repaired again.** No package changes when it is repaired again. The second run is given
  what the first established of the mod, which its package does not show again: the dating,
  and the WADs that restore a model from before its rework.
  - 2,454 repaired mods date at another build by themselves: an object a fixup brought to the
    target build is the game's own object of that build. One value, left because the mod's
    own build read it, would be removed at the later dating.
  - 43 mods hold a restoration. In 4 of them the files at paths the rework dropped are named
    by nothing and are left out, so the package alone does not show a restoration: judged
    with no fact of the first run, its copies of the game's older bytes are stale copies.
- **Types.** 47 mods hold a value of another type than the build reads and 227 a field its
  class does not declare, each in a bin the client loads. The findings in bins nothing loads
  are gone: 98 of the 144 mods and 44 of the 271 of the run above hold none. What is left is
  a value with no conversion (a float animation where the build reads a vector's, 26 mods) and
  the class the game dropped at 16.6, `ItemRecommendationContextList` (227 mods).

Two faults the second repair found, fixed in `load`:

- Of several files of a mod that move to one asset, the first was taken. Where it holds what
  the game shipped at its path, the file made of it is a stale copy and is left out, and the
  second repair made the asset's file of the next. A file of the mod's own bytes is taken
  first (#ADR-0023). 2 mods changed when repaired again. The load stage differs from forge in 6
  of the 245 mods run again.
- A texture judged in the container it ships in (above). 3 of the 233 mods the fixup changes
  affect changed when repaired again.

The load stage was compared again on the reduced corpus and on the 245 mods, with two accepted
deviations added to `load_compare.py`. It was not compared again over the datasets.

## Sound units of a mod's own (2026-10-04)

Measured over forge's baseline reports and the mods' own banks, read in memory: where the events
bank of a sound unit of the mod's own can be cut from.

| the unit's events can be cut from | units | mods |
|---|---:|---:|
| the mod's own events bank, of the client's version | 322 | 199 |
| the game's events bank only | 246 | 217 |
| neither, and forge carried none of them | 145 | 84 |

The first two rows are the 568 units forge carried, in 383 mods. Of the 246, 163 ship no events
bank, 57 ship one whose events play none of the unit's clips, and 26 ship one of version 134
that does.

Of the 145: 64 units in 18 mods have a second events bank of the client's version that plays
their clips, and forge reads the first only; 75 have a bank of version 134 that plays them; 5
ship none.

Every bank a port writes takes its header from a game events bank, the 5,017 units that play
under the game's own events included. That is read from forge's code, not measured.

Not measured: whether a unit cut from the mod's own bank plays in game.

## Facts and the decision (2026-10-04)

`reforge inspect` over the 5,137 mods of the datasets, each compared with forge's baseline
report of the same mod by `scripts/corpus/decide_compare.py` in forge. forge was not run again.
A unit is named by its character and its source slot. Compared: each unit's role among the
copies, the objects that are the game's, each origin with its evidence, each rig, the clips at
each candidate slot, each candidate's checks with their text, each decision with its candidates
under `auto/own-rig` and its target and reason under the three other levels of animation reuse,
the reworks, and the diagnostics of the facts and the decision by code and subject.

| run | mods | equal | accepted deviations only | other differences | time |
|---|---:|---:|---:|---:|---:|
| the reduced corpus | 27 | 25 | 2 | 0 | 7 s |
| the datasets | 5,137 | 4,456 | 681 | 0 | 1,353 s |

Twelve mods in parallel, about 3.2 s a mod. No mod failed. Three mods were run again after the
full run, with the text of one piece of evidence corrected (below).

Of the 11,256 units with a decision in either program, 11,254 have forge's origin, target and
reason under every level of animation reuse. The other two are in mods under an accepted
deviation: a sub-character whose origin is unknown where forge says the base, by one file made
otherwise (#ADR-0038), with the same target; and the second representative forge keeps where
reforge has one cluster (below), which forge supersedes.

The accepted deviations:

| deviation | mods | differences | reason |
|---|---:|---:|---|
| every object fixed | 604 | 76,797 | #ADR-0002. forge repairs only the objects of the bins a port writes, and says how many it left (`scope.outside`). Two copies whose objects a fixup changed alike share more objects in reforge: a copy is renamed where forge says near or same rig, or near with more objects shared. 55,783 kinds of copies in 603 mods, and the 21,014 notes of near copies that follow in 271. The clusters, representatives and spreads are forge's |
| copies joined once fixed | 1 | 62 | #ADR-0002. Two bins share 55 of 59 objects once both are fixed, and under 80% in forge. reforge has one cluster of 23 units, forge two. forge's second representative ends superseded, so both port the same one unit |
| the earlier reworks judged | 74 | 74 | Mordekaiser, Pantheon and Fiddlesticks are in the rework table (#GAME-CHANGE-0001): 74 mods of theirs are judged against the rework, and none is made before it without a model of its own. No decision differs |
| a load deviation carried | 6 | 29 | the mod's load deviates ("The load stage"), and a tally of its game files or objects follows: the larger reduced copy (1 mod), own bytes before the game's (1), one chunk per path (4) |

One fault of reforge found by the full run and fixed: the text of a file's evidence "a path
under skin N's folder" named the skin the folder counts as, where forge names the folder's own
slot (a chroma's). Three mods differed.

What the decisions are, under `auto/own-rig`, over 11,255 representative units of 115,127
units:

| reason | units |
|---|---:|
| a mod for its own slot | 5,748 |
| the base | 3,455 |
| follows its champion | 865 |
| fallback to the premium origin | 497 |
| a paste cleaned onto the base | 464 |
| a sub-character left to the game's base | 90 |
| the skin whose clips it plays | 84 |
| superseded | 29 |
| made before a rework | 18 |
| fits no slot | 3 |
| unjudged | 2 |

Origins: 7,284 premium, 3,667 base, 164 unknown, 140 custom. 4,227 submeshes in 475 mods are
copied from a premium skin (#MOD-ARTIFACT-0024). 42,693 units in 614 mods are near copies.

What forge does that the port keeps, found while porting (none changes a decision over the
datasets):

- The joints of a game skin's skeleton are known only for the slots a unit of the character
  was compared with: its candidates and the skins its skeleton or clips match. At another slot
  (a forced target) the check "the slot's joints" cannot hold.
- Two units of one character on one slot are ranked by a key that ends with the source slot,
  which differs between them. The ranks are never equal, so `target.conflict` is never raised.

## Clip joints and events banks in the history index (2026-10-04)

The census history's `history-v2` branch at commit `5b121f51ad75`: 755 builds, from
8.20.2483196 to 16.19.8207193. The index was written to scratch directories. The index a run
reads was not rebuilt.

**The keys in the history.** The first build and the head both carry `clip.joints` on every
animation clip's entry (94 of 94 in one champion's WAD at the first build) and `bank.header` and
`bank.objects` on every bank with a `HIRC` section. At the head:

| | count |
|---|---:|
| events banks | 18,689 |
| their objects | 4,862,673 |
| distinct objects (type, id, body) | 2,360,592 |
| bytes of the distinct objects' bodies | 115,072,742 |
| objects with no id | 0 |
| animation clips | 64,903 |
| joints they list | 6,949,858 |
| distinct joint lists | 15,563 |

**The census form rebuilds the bank.** A file written as `BKHD` with the entry's header, then
`HIRC` with the entry's objects, has the entry's SHA-256 for 18,635 of the 18,689 events banks at
the head, and for 5,864 of 5,869 at the first build (version 125). Of the 54 at the head that do
not, 23 hold clips too. The other 31 hold a section the census does not record.

**Eight builds**, 8.20.2483196 to 8.24.2558524, every family: written in 13 s, 251 MB.

- Every events bank entry at that head is in the index with its header and its objects in
  order: 6,056 banks, 1,354,041 objects. Every animation clip entry is there by path and by
  file key with its joints: 15,614 clips, 1,250,869 joints, 2,188 lists.
- The index of the first six builds with two appended equals the index of eight in
  `joint_lists`, `list_joints`, `animations`, `bank_objects` and `event_banks`, row for row.

**The whole history**, the clips and rigs families only: 390 s on 32 threads, 1.64 GB.

| table | rows |
|---|---:|
| `bank_objects` | 7,830,002 |
| `event_banks` | 162,930, at 16,291 paths |
| `joint_lists`, `list_joints` | 16,773 and 2,079,289 |
| `animations` | 125,169, at 122,348 paths, of 51,190 contents |

- The same check against the head's entries: 18,689 banks and 4,862,673 objects, 64,903 clips
  and 6,949,858 joints, none missing or different.
- `event_banks` by version: 5,775 of 125, 43,471 of 132, 79,844 of 134, 33,840 of 145. They
  list 49,000,478 objects.
- **Size.** The bodies in `bank_objects` are 369 MB and the lists in `event_banks` 392 MB. The
  file without the two tables is 554 MB, so they take 1.08 GB. Without the three tables of clip
  joints it is 502 MB, so those take 52 MB. The index of every family was 1.61 GB before them
  (2026-10-03).
- `reforge history probe` against the 16.19.8207193 install copy: the base effects events bank
  of one champion (256 objects) and its `en_us` voice events bank (320 objects) are in the index
  with the header and objects `reforge-wwise` reads from the game's file. One animation clip of
  the install is found by its file key with 211 joints.

**The whole history, every family**: written in 2,263 s, 2.75 GB, 755 builds up to
16.19.8207193. The tables the index held before have the row counts of the index built from the
`history` branch (2026-10-03): `files` 1,893,046, `contents` 3,374,160, `clips` 4,694,013,
`objects` 3,276,316. The five tables added have the row counts of the clips and rigs build
above.

Not measured: the memory the walk takes, and a clip's joints against the clip's own bytes.

## The port, without audio (2026-10-04)

`reforge port --no-write` over the reduced corpus of 27 mods, against the 16.19.8207193 install
copy and the history index, each report compared with forge's port report of the same mod by
`scripts/corpus/port_compare.py` in forge. forge was not run by the comparison: its reports,
with what its port writes and each patch applied, were made beforehand. The audio of a port is
not built in reforge, and every difference that follows from it is filed under one deviation.

Compared: per unit whether it ships and why it left the port otherwise; per layer, WAD and
path hash each chunk, a bin by the digest of each object as `ltk_meta` writes it and any other
chunk by the digest of its bytes; per layer and patched game bin the links, the edits the
overlay skips, and the digest of each object of the bin after the patch; the diagnostics of
the plan, the build and the checks by code.

| | |
|---|---:|
| mods run | 27, in 17 s at four in parallel |
| faults | 0 |
| runs ended by a fatal diagnostic | 2: one archive with no skin, one port that holds nothing (its one unit was made before a rework) |
| mods compared | 27 |
| equal | 8 |
| under an accepted deviation only | 19 |
| with a difference not accepted | 0 |
| units | 321, none whose state differs |
| chunks reforge ships | 2,050 |
| patches reforge writes | 69 |

Accepted deviations:

| deviation | differences | mods | what differs |
|---|---:|---:|---|
| the audio is not built | 205 | 18 | 134 chunks of banks forge ships (80 in the base layer, 54 in voice layers), 16 voice-layer patches, 33 skin entries whose audio block forge's base patch sets, 2 objects of one graph whose literal sound names forge rewrote, 20 `audio` diagnostics |
| an empty package halts | 3 | 1 | a port that holds nothing ends reforge's run at the build (`package.empty`); forge says so only where it writes |

Outside the audio, every chunk reforge ships has forge's path and digest, every patched game
bin has forge's objects after the patch, and every unit has forge's state.

**The checks.** Over the 27 mods the checks raise two errors, and forge's checks raise the same
two on the same mods:

- `check.name-unresolved`, one mod: a sub-character follows its champion to a premium slot,
  and 52 names the patched entry reaches there resolve in nothing its group loads. They are
  particles of another skin of the champion.
- `check.type-mismatch`, one mod: one carried value is a hash where the build reads a file
  reference (`VfxAssetRemap.oldAsset`). No fixup converts it.

**Two runs.** A second run of the 27 mods gives the same 27 reports, byte for byte: the digest
of every chunk and of every patch among them.

**Written.** One mod was written as a project directory and a `.modpkg`: one patch, its
manifest, one bin and four files.

Not measured: the port over the datasets, the time of the overlay and the checks apart, and
any port in game.

## The port (2026-10-04)

`reforge port --no-write` over the 5,137 mods of the datasets, with the audio, against the
16.19.8207193 install copy and the history index of 755 builds (the `history-v2` branch at
`5b121f51ad75`), each report compared with forge's port report of the same mod by
`scripts/corpus/port_compare.py` in forge. forge was not run by the comparison: its reports,
with what its port writes and each patch applied, were made beforehand (2,222 s, no mod
failed). What is compared is what "The port, without audio" lists. forge raises
`audio.carried` and `audio.not-carried` once for each bank, layer and reason of a unit, and
reforge once per unit, so those two codes are compared by how many units have one.

| run | mods | equal | accepted deviations only | other differences | time |
|---|---:|---:|---:|---:|---:|
| the reduced corpus | 27 | 26 | 1 | 0 | 8 s at eight in parallel |
| the datasets, first | 5,137 | 4,966 | 15 | 156 | 1,459 s at twelve in parallel |
| the datasets, with 156 mods run again | 5,137 | 5,089 | 48 | 0 | 84 s for the 156 |

No mod failed. The datasets were run once. The 156 mods with a difference were run again after
the two faults below were fixed, and the 33 of them that still differed a second time, so that
their reports hold the new name of each carried object. The other 4,981 reports are of the
first run.

**Nothing of the audio differs.** Of the 156 mods of the first run, none differs in a bank, a
voice layer, the audio block of a patched entry, a rewritten sound name or an `audio`
diagnostic. Two faults of reforge outside the audio, which the reduced corpus does not show,
account for 125 of them:

- **The first user of a file.** A file ships under the folder of the first object that names
  it, in the order the entry reaches the objects (#ADR-0027). `links` and `file_refs` gave an
  object's names struct by struct, an outer struct's before those of the structs it holds.
  forge gives them in the pre-order of the values. 65 mods shipped 2,523 files at another
  place than forge's. Both functions give the pre-order.
- **A group's loose ends.** The `Apply` check took a name or a file as unresolved in the mod
  already only when the plan had found it so for the unit walked. A sub-character's walk
  enters the objects its champion ships, whose loose ends are the champion's.
  `check.file-missing` was raised in 59 mods and `check.name-unresolved` in 2 where forge
  raises none. The check takes the loose ends of every unit of the group, and follows the
  patched entries and resolvers of all of them, as forge does.

The load, repair and decide comparisons over the reduced corpus give what they gave before the
change of order: 26, 4 and 25 mods equal of 27, none with a difference not accepted.

The accepted deviations, each mod under one. All but the first are deviations of an earlier
stage, in what a port carries of them:

| deviation | mods | differences | what differs |
|---|---:|---:|---|
| an empty package halts | 15 | 45 | a port that holds nothing ends reforge's run at the build (`package.empty`); forge says so only where it writes |
| the birth translation | 20 | 113 | carried emitters whose misplaced birth translation reforge moved or removed (#MOD-ARTIFACT-0019) |
| the sampler state's name | 4 | 22 | carried materials whose sampler state's name the strip removes, where forge moved it (#GAME-CHANGE-0014) |
| the larger reduced copy | 3 | 3 | a texture made of the `2x_` file, shipped by both at one place in other bytes (#ADR-0038) |
| `retype` enters what it retags | 2 | 4 | a carried object, and the value of another type forge's check finds in it |
| the earlier reworks | 2 | 9 | 2 animation files of a restoration that are the mod's own and ship, and the graphs that name them (#GAME-CHANGE-0001) |
| the order of fields | 1 | 3 | carried emitters with `Linger` and `FlexShapeDefinition` in the other order (#ADR-0007). The reports hold one digest of an object, in the order of its fields: that the values are the same is the repair comparison's finding for this mod |
| copies joined once fixed | 1 | 1 | a unit that is a copy in reforge and superseded in forge; neither ships it (#ADR-0002) |

A difference in a carried object is filed under a deviation only when the report shows that
the fixup of that deviation changed that object: the plan lists the name each carried object
takes.

**What the ports hold**, over the 5,137 mods. Each count is forge's own of its run but for the
two files of the earlier reworks:

| | |
|---|---:|
| runs ended by a fatal diagnostic | 16: one archive with no skin, 15 ports that hold nothing |
| chunks | 430,840 |
| files of the mod kept as they are | 184,759 |
| textures put in TEX's container | 221,329 |
| bins made | 10,911 |
| banks made | 13,841 |
| patches | 12,637: 8,583 in the base layer, 1,644 in `voice`, 970 in a locale's voice layer, 1,440 in `hud` |
| mods with a `voice` layer, with a locale's | 1,079 and 541 |

**The audio**, in 2,241 mods:

| | |
|---|---:|
| bank units over the game's, whose clips the game's events play | 5,017, of 393,515 own clips |
| bank units with events of their own | 568, of 20,285 own clips and 8,381 events kept |
| of those, cut from the mod's own events bank (#ADR-0030) | 215 |
| kept events that share an id with the game's bank at the path | 0 |
| kept events no unit's list names, which keep their ids | 645 |
| sounds under a kept event that play nothing, their clip being the game's | 390 |
| units in the base layer, in `voice`, in a locale's voice layer | 2,624, 1,859 and 1,102 |
| units that stand in for a game unit (#MOD-ARTIFACT-0028) | 40 |
| locale copies of the champion WAD's voice carried once | 1 |
| units whose literal sound names were rewritten | 153: 448 names to the mod's tag, 422 to the base's |
| banks of the mod whose own clips ship in a bank made | 3,413 |

forge's message names the mod's own events bank for 105 of the 215 units, the ones beside a
game events bank none of whose events plays their clips. For the other 110 the game holds no
events bank at the unit's events path.

Not carried, in 159 units of 89 mods (`audio.not-carried`), as in forge:

| why | banks | own clips |
|---|---:|---:|
| no event of its events bank plays the clips | 128 | 1,091 |
| the game's skin at the target has no audio block with an events bank | 29 units | 383 |
| a bank at a path the target's units do not load, with no events bank of the client's version | 17 | 527 |

One mod holds a bank that does not read (`audio.bank-unreadable`), which forge does not report.

**The checks.** Over the datasets the checks raise errors of two codes, in the mods forge's
checks raise them in:

- `check.type-mismatch`, 39 mods: a carried value of another type than the build reads. forge
  finds 41: the two more are the mods where `retype` enters what it retags.
- `check.name-unresolved`, 3 mods, 8 units.

No patch fails to apply, no record is skipped, no link the port adds leads to nothing, no file
a patched entry names is missing, and every bin and every bank the run wrote reads back, the
banks at the client's version.

**Two runs.** A second run of the 27 reduced mods gives the same 27 reports, byte for byte.

**The in-game samples.** The 14 sample mods and the fix port with nothing written
(`--no-write`): no check raises an error or a warning in any of them. 8 carry sounds, in 24
banks. 4 have a voice layer, one of them a locale's. One was written as a project directory and
a `.modpkg` of 21.8 MB: 214 chunks, the layers `base` and `voice`, the voice layer's manifest,
its patch and its three banks under its own directory.

**The datasets again, on one program.** After the two fixes every one of the 5,137 mods was
run again with one binary, against the same forge reports: 5,089 equal, 48 under the accepted
deviations above, none with another difference, no mod failed, 1,314 s at fourteen in
parallel. 115,127 units, 430,840 chunks and 12,637 patches compared.

Not measured: any port in game, whether a unit cut from the mod's own events bank plays, and
the time of the overlay and the checks apart.

## The commands on a run (2026-10-04)

The four commands were rewritten on `Porter`, `Repairer` and `Run`. Two debug builds of the
command, one from before the change and one from after it, were run on the same mods against
the 16.19.8207193 install copy and the history index, and what each gave was compared byte
for byte: the report, the standard output and the exit code.

Eight mods of the reduced corpus, the one that halts and the seven forge ports quickest, each
under nine invocations:

| invocation | |
|---|---|
| `load --dump` | |
| `load --fix --dump --every-file` | |
| `inspect --dump` | |
| `inspect --target base --anim-reuse allow` | |
| `port --no-write --dump` | |
| `port --no-write --block check,severity=error --no-check types --keep-stale --game-events-only` | |
| `repair --no-write --again --dump` | |
| `port --out <dir> --modpkg` | 7 mods written, one halted |
| `repair --out <dir> --modpkg --block severity=error` | 5 mods written, 2 refused, one halted |

- The 72 reports, the 72 outputs and the 72 exit codes (62 runs ended with 0, 8 halted with 2,
  2 refused the write with 3) are equal.
- The 12 project directories written are equal file for file, 489 files, the report beside
  each package among them.
- The 12 `.modpkg` files differ. They also differ between two runs of one build: the build
  from before the change, run twice on one mod, wrote two files of the same length
  (14,244,618 bytes) that differ in most of their bytes, into one directory as into two. The
  project directories of the two builds, written from the same packages, are equal. What in
  the packing differs was not looked into.

The other 19 mods of the reduced corpus, each under `load --fix`, `inspect`,
`port --no-write` and `repair --no-write --again`: the 76 reports, outputs and exit codes are
equal.

Not measured: the release build, the datasets, and a run that ends at a fault, whose error
text on standard error names the stage and not the step inside it.

## Ports in game (2026-10-04)

23 sample mods ported and repaired by reforge against a copy of the game at 16.19, each
written as a `.modpkg`: the 15 of the in-game set and 8 that a change was tried on. Every port
and every repair ended with exit 0.

The user installed these in that copy of the game, and each works:

| mod (SHA-256) | port | repair | the port's target | what it holds |
|---|---|---|---|---|
| `000d766b35b6…` | works | works | skin0 | a mod with no skin bin |
| `00d1e642623e…` | works | works | skin0 | a premium paste that fits the base; a sound unit of its own |
| `21708c2c69f7…` | works | works | skin 16 by fallback, a sub-character following though its model does not fit | a submesh copied from a premium skin |
| `4c064748283f…` | works | works | skin0 | a skin0 unit on the base |
| `5c1f2ee92c54…` | works | works | skin0, a sub-character following | a premium paste that fits the base |
| `784c22089dfa…` | works | works | skin0, a sub-character following | a skin0 unit on the base |
| `96e867967927…` | works | works | skin0, two sub-characters following | a premium paste that fits the base |
| `d99c901022fa…` | works | works | skin0 | a skin0 unit on the base |
| `b8a96bd3adfa…` | works | not tried | skin0 | one value lost by a fixup (`fixup.lost`) |
| `ed8259ce1cd1…` | works | not tried | skin0, a sub-character following | effects built from other skins' files |

"Works" is the user's judgement of the mod in game. What was looked at in each (the model,
the effects, the sounds) is not recorded, but for the sounds of `00d1e642623e…`. Its port
carries 10 own clips in a bank unit of its own, whose 9 events are cut from the game's events
bank of the skin the mod pasted: the casts of three spells, a dagger picked up, the recall.
The user heard the mod's own sounds in game, and the sounds of a dagger bouncing and landing,
which the mod does not replace, as the base skin's. Whether the base skin's sound is heard
beside the mod's on the same hook (#GAME-BEHAVIOR-0016) is not recorded.

Not tried: the other 13 ports and 15 repairs, a voice layer and a `hud` layer.

### Test mods (2026-10-06)

Each test mod is the game's `skin0.bin` of one champion with one change, built from the
16.19.8207193 copy by an example of `reforge` and seen by the user in a replay of the champion at
skin 0 in that copy.

| test mod | the change | seen | record |
|---|---|---|---|
| `legacy-simple-test-vi-kept-control` | none | Vi as without the mod | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-vi-kept-scaled` | the `scale` curve of the `LegacySimple` of `glow` in `Vi_Base_Passive_BackPack` four times the game's | the ring on her back very big | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-vi-kept-removed` | that `LegacySimple` removed | no ring on her back | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-zac-control` | none | Zac as without the mod | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-zac-kept-scaled` | the `scale` curve of the `LegacySimple` of `Shadow` in `Zac_Base_W_cas` four times the game's | the dark swirl of W big | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-zac-kept-removed` | that `LegacySimple` removed | no swirl seen | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-zac-simple-scaled` | `Splat` in `Zac_Base_W_cas`, a complex emitter, given a `LegacySimple` of `birthScale` 200 and `scale` 4 | no difference from the control noticed | #GAME-BEHAVIOR-0024 |
| `legacy-simple-test-zac-simple-zeroed` | `Splat` given a `LegacySimple` of `birthScale` 0 and `scale` 0 | no difference from the control seen | #GAME-BEHAVIOR-0024 |
| `birth-translation-test-zac-control` | `Splat` in `Zac_Base_W_cas` given an empty `VfxShapeLegacy` and `EmitterPosition` 300 up | the goo above Zac | #MOD-ARTIFACT-0019 |
| `birth-translation-test-zac-misplaced` | the same vector as a `birthTranslation` in that shape, no `EmitterPosition` | the goo not above him, near his back; its place not sure | #MOD-ARTIFACT-0019 |
| `birth-translation-test-zac-both` | `EmitterPosition` 300 up and a `birthTranslation` of 150 up | the goo above him; its height against the control's not judged | #MOD-ARTIFACT-0019 |
| `entry-twice-test-azir-control` | Azir's skin entry drawing `Azir_base_BA_Beam_1x3.tex` on the whole model | Azir nearly black with orange streaks | #GAME-BEHAVIOR-0020 |
| `entry-twice-test-azir-changed-first` | that entry, then the game's under the same hash | Azir nearly black with orange streaks | #GAME-BEHAVIOR-0020 |
| `entry-twice-test-azir-changed-last` | the game's entry, then the changed one | Azir not drawn dark | #GAME-BEHAVIOR-0020 |

### Ports of the roadmap's open cases (2026-10-06)

Three mods of the datasets ported by `reforge port` at 0.1.2 against the 16.19.8207193 copy,
each to a `.modpkg` targeting skin 0, each ended with exit 0. The user installed each in that copy
and watched a replay of the champion at skin 0.

| mod (SHA-256) | what it holds | seen |
|---|---|---|
| `06069f24f123…` | a `hud` layer: 6 spell icons and HUD textures, 10 records on the root bin | with `hud` off, only the champion's portrait the mod's; with it on, the spell icons the mod's too |
| `1670c64ba7a5…` | a `voice` layer: 396 voice clips the game's events play; 4 effects clips in the base layer | with `voice` off, the game's voice; with it on, the mod's |
| `40606c273797…` | 31 own clips in a bank unit of its own, 12 events cut from the mod's own events bank (#ADR-0030), posted by the clips of the mod's own graph and by two emitters | none of the mod's sounds heard |

For `40606c273797…` the user watched a recall, a death and the respawn, the R and the emotes, with
Kennen drawn as the mod's model and animations: Kennen's own sounds, none of the mod's. The
original mod, installed as it is, plays the mod's sounds on the emotes.

Compared in memory (2026-10-06):

- The port carries the mod's graph and emitters, whose clips and effects name the 12 events
  literally, and lists the 12 in its bank unit.
- The port's events bank holds the original's sounds, container and attenuations byte for byte.
  Its play actions name its own header id, as the original's name the original's. It leaves out
  two events and the actions of the music events that set the volume of buses.
- The port's media bank holds the original's 31 clips byte for byte, and none of its five bus
  objects. Three of them are buses the game's init bank defines (393239870 among them, the bus
  the eleven sounds route to), and two are not (3276768291, 3605772915).
- The original's skin entry has one bank unit, `Kennen_Base_SFX`, loading the game's base events
  bank beside the mod's banks, and no voice unit. The port keeps the game's two units and adds
  one of its own.

Two variants of the port, each with one of those differences undone, seen in game by the user:

| variant | the change from the port | seen | record |
|---|---|---|---|
| `PikachuKennen-Nyht-bus` | the media bank given the original's five bus objects | the mod's sounds play; Kennen's own voice heard beside them, which the original leaves out | #GAME-BEHAVIOR-0025 |
| `PikachuKennen-Nyht-unit` | the mod's banks in the game's `Kennen_Base_SFX`, no unit of the port's own | Kennen's own sounds only | #GAME-BEHAVIOR-0025 |
| `PikachuKennen-Nyht-game-buses` | the media bank given the three bus objects whose ids the game's init bank defines | Kennen's own sounds only | #GAME-BEHAVIOR-0025 |
| `PikachuKennen-Nyht-own-buses` | the media bank given the two bus objects the init bank lacks | the mod's sounds play; Kennen's own voice heard beside them | #GAME-BEHAVIOR-0025 |

The port writes each media bank with the clips alone, so the bus objects a mod's media bank holds
are not carried.

The port of `40606c273797…` by the build with the rules of #ADR-0064 and #ADR-0063, its media
bank holding the mod's two buses and its block no voice unit, seen in game by the user
(2026-10-06): the mod's sounds play and Kennen's voice is not heard, as with the original.

## The baselines and the edge cases (2026-10-04)

The two baselines reforge is compared with (#ADR-0044), over the 5,137 mods of the datasets,
against the 16.19.8207193 install copy and the history index of 755 builds. The tooling is
`scripts/corpus/` of this repository.

| baseline | command | mods | ended by a fatal diagnostic | failed | time |
|---|---|---:|---|---:|---|
| the port | `port --no-write` | 5,137 | 16: one `archive.no-skin`, 15 `package.empty` | 0 | 1,314 s at fourteen in parallel |
| the repair | `repair --no-write --again` | 5,137 | 1: `archive.no-skin` | 0 | 1,808 s at fourteen in parallel, beside other work on the machine |

**The port's baseline is not a new run.** Its reports are those of "The port", "The datasets
again, on one program", made before the commands were rewritten ("The commands on a run").
The release build of the rewritten commands gives the same reports: 30 mods spread evenly
over the 5,137 and the 54 mods of the reduced corpus were run again, and each report is the
baseline's byte for byte but for one line, the place of the install, which is the `--game`
argument written back.

**The repair's baseline** is one run of that build. Every repair that was built was built
again from its own package (5,136), and no second repair changed a value or a chunk. 4,771
repaired mods date at a build by themselves. The checks raise `check.field-undeclared` in 227
mods and `check.type-mismatch` in 47, the port's 39 among them.

**The edge cases** (#ADR-0045), derived from the reports of both baselines by `cases.py`:
629 cases seen, 90 declared by the program and not seen. 232 show in both runs, 383 in the
port only, 14 in the repair only. [CASES.md](CASES.md) lists them with their counts.

| area | seen | not seen |
|---|---:|---:|
| run | 2 | 10 |
| archive | 98 | 23 |
| load | 79 | 9 |
| fixups | 92 | 1 |
| facts | 130 | 5 |
| decision | 39 | 5 |
| plan | 49 | 6 |
| audio | 38 | 5 |
| package | 44 | 16 |
| checks | 24 | 10 |
| records | 34 | 0 |

22 cases are shown by one mod and 13 by two. No report holds a text field or a flag that the
table of `cases.py` does not know. Deriving the cases from the 10,274 reports takes 11 s at
sixteen in parallel.

**The reduced corpus**: 54 mods show the 629 cases, 1.47 GB of archives. 11 of them are the
only mod to show a case.

| run of the reduced corpus | mods | ended by a fatal diagnostic | time at fourteen in parallel |
|---|---:|---:|---|
| `port --no-write`, twice | 54 | 3 | 22 s and 25 s |
| `repair --no-write --again`, twice | 54 | 1 | 35 s and 34 s |

- **Two runs.** The two runs of each command give the same 54 reports byte for byte, and
  `compare.py` reports no difference.
- **Against the baselines.** `compare.py` reports no difference between the baseline's
  reports of the 54 mods and the reduced run's, for the port and for the repair.
- The reports of the reduced runs show the same 629 cases.

Not measured: `inspect` and `load` over the datasets, a run without the history index or the
schema, and a run that writes.

## The evidence embedded, against the baselines (2026-10-04)

The first change measured against the baselines: every run embeds its evidence in the package
(#ADR-0041), and the report holds it as a part of its own. The 54 mods of the reduced corpus,
run with the change and compared with the baselines by `compare.py`:

| command | mods | equal | with a difference | the differences |
|---|---:|---:|---:|---|
| `port --no-write` | 54 | 3 | 51 | the part `evidence`, in the 51 runs that build a package |
| `repair --no-write --again` | 54 | 1 | 53 | the part `evidence`, in the 53 runs that build a package |

No other section differs: no unit, decision, chunk, patch, fixup or diagnostic. The baselines
of the datasets were not made again, so they hold no `evidence` part.

## Packing the census history (2026-10-05)

The 10,986,063 objects of a mirror clone of the census history (`history-v2` at
16.19.8230722, `main`, and the tag `history-v2-index-16.9`), packed again by git 2.52 into one
pack: every delta found anew (`--no-reuse-object`), a depth of 50, objects stored without zlib
(`pack.compression=0`), 1 GiB of window memory per thread, 32 threads. As GitHub served it, the
clone held one pack of 10.06 GiB, compressed with zlib.

| order of the delta search | window | `.pack` bytes | time |
|---|---:|---:|---|
| git's name hash | 250 | 5,749,231,628 | 16 min 16 s, with a bitmap |
| git's name hash, version 2 | 250 | 5,887,049,441 | 12 min 16 s |
| `--path-walk` | 125 | 5,598,482,071 | 9 min 41 s |
| `--path-walk` | 250 | 5,413,698,374 | 10 min 9 s |
| `--path-walk` | 500 | 5,210,341,216 | 13 min 12 s |
| `--path-walk` | 1000 | 5,010,708,741 | 15 min 36 s |
| names of class and kind, keys from the tip | 1000 | 4,718,214,551 | 26 min 44 s |
| ranks, class first, keys from the tip | 1000 | 4,704,005,667 | 27 min |
| ranks, entry first, keys from the tip | 1000 | 5,158,264,169 | 23 min 27 s |
| ranks, class first, keys from every version | 1000 | 4,565,296,836 | 37 min 41 s |

- **The orders.** Git sorts the objects for the search by a hash of their path's last 16
  characters, then by size. An entry's text and a WAD entry's file are named for a hash, so
  that hash scatters similar files. The last four rows give each object a name of their own on
  `pack-objects`'s input: a bin entry's text named for its class, a WAD entry's file for its
  kind; or a name whose 16 characters spell a rank under git's hash, the ranks sorting a bin
  entry's text by class, entry, bin and WAD, or by entry, WAD and class. The class is read from
  the bin's WAD entry file, the kind from the file itself: at the tip, 88% of the entry texts'
  versions and 58% of the entry files' have one; in every version, all of the texts' and 92%
  of the files'.
- **The window.** Under `--path-walk`, each doubling saved 185 to 203 MB.
- **The chains.** The longest is 44 to 46 in every pack, the mean of the deltas 4.6 to 5.7.
- **The bytes, by file**, in the `--path-walk` pack of window 500:

| files | objects | in the pack | unpacked |
|---|---:|---:|---:|
| bin entries' texts (`.rito`) | 3,165,814 | 2,532,068,173 (48.6%) | 100,964,906,588 |
| WAD entries' files | 2,143,251 | 2,000,922,815 (38.4%) | 11,739,758,900 |
| trees | 5,530,566 | 666,593,355 (12.8%) | 2,947,027,864 |
| WADs' files (`_wad.yaml`) | 144,901 | 10,361,542 (0.2%) | 8,607,630 |
| commits and the rest | 1,531 | 395,299 | 456,436 |

- **After zstd.** The bare repository in a tar, then `zstd --ultra -22 --long=31 -T1`:

| pack in the tar | `.tar` bytes | `.tar.zst` bytes |
|---|---:|---:|
| git's name hash, window 250, with its bitmap | 6,186,762,240 | 1,778,546,057 |
| `--path-walk`, window 1000 | 5,390,540,800 | 1,771,541,306 |
| ranks, class first, keys from the tip | 5,082,685,440 | 1,749,813,288 |

The release `history-v2-snapshot-0` holds the `--path-walk` pack of window 1000, its frame
taking a `.keep` for the pack afterwards: 1,771,553,727 bytes.

## The buses of a mod's media bank (2026-10-06)

The rule of #ADR-0064 measured: `reforge port --no-write` at 0.1.2 without the rule and with
it, against the 16.19.8207193 copy and the history index of 757 builds, over the 54 mods of the
reduced corpus and the 3 mods whose media bank holds buses (#MOD-ARTIFACT-0035). Every run
ended with exit 0.

`compare.py` finds no difference in the 54 mods of the reduced corpus. Each of the 3 differs in
one chunk, the media bank of its unit cut from the mod's own events bank: `sfx_audio.bnk` in two,
`sfx2_audio.bnk` in one. The port of `40606c273797…` writes that bank byte for byte as the
variant `PikachuKennen-Nyht-own-buses` holds it, whose sounds play in game ("Ports in game").

## The game's units a mod leaves out (2026-10-06)

The rule of #ADR-0063 measured the same way: `reforge port --no-write` at the head with the
buses of #ADR-0064 and with the rule, over the 54 mods of the reduced corpus and the 142 mods of
the port's baseline whose block keeps an effects unit of the base's name and no voice unit.
Every run ended as at the head: 3 mods of the reduced corpus by a fatal diagnostic
(`archive.no-skin` in one, `package.empty` in two), the others with exit 0.

`compare.py`: 127 of the 194 mods differ, 1 of them of the reduced corpus. 124 leave out one
game unit each, the base skin's voice unit, and their patch holds the block without it. 21 of
the 124 carry no own clip, and their patch gains an audio record. In 3 the facts find a unit
left out and the port targets another slot, so the block stays as the game's.

A first run without the bound on a unit's kind left out the base skin's effects unit in 3 mods,
whose block holds another skin's effects units in its place (`13137f23efc8…`, `278079f09fc4…`,
`7e478f0479c9…`). With the bound none is left out.

## The rules of the fixer batch (2026-10-06)

Each rule measured over the 54 mods of the reduced corpus, `repair --no-write --again` and
`port --no-write`, against the 16.19.8207193 copy and the history index of 757 builds, with the
rule and without it (the build before). Every run ended as at the head: in the repair, 1 mod by
`archive.no-skin`; in the port, 3 (`archive.no-skin` in one, `package.empty` in two).

**A complex emitter's simple-particle fields removed** (#ADR-0068). 6 mods differ, in the repair
and in the port, every difference on an emitter of a system's `complexEmitterDefinitionData`.
Without the rule, 731 values of 4 mods moved into a `LegacySimple` of a complex emitter and the
strip removed the old fields of 2 more. With it, `vfx-legacy-simple` removes 1,164 values from
complex emitters in the 6 mods, and still moves 24 on simple emitters in 1. Two float animations
`retype` left unconvertible in one mod were in those fields, and the type check counts fewer
fields in 4. No emitter outside the two lists holds the old fields.

**A birth translation inside a `VfxShapeLegacy` mended whatever the date** (#MOD-ARTIFACT-0019).
3 mods of the reduced corpus differ, in the repair and in the port, every difference at a
`SpawnShape.birthTranslation`. The 206 mods of the repair's baseline whose strip removed such a
value, repaired without the rule and with it: every one differs, at those values alone. Without
it the strip removes 276,638 values. With it 106,828 become an `EmitterPosition` in 142 mods and
169,810 are removed beside the position the emitter holds. What dates the 206 is in the record.

**The first object of a hash listed twice kept** (#GAME-BEHAVIOR-0020). Counted first: of the
128 mods of the datasets whose loaded bin lists an entry hash more than once, 93 hold objects
under one hash that differ and 35 hold equal ones only. The record has the classes and the
fields. 3 mods of the reduced corpus differ, in the repair and in the port, each one of the 93.
The 128 repaired without the rule and with it: the 93 differ and the 35 are equal. Ported: 91 of
the 93 differ, and the 37 others are equal. The differences follow the object kept: its values,
the fixups' changes to them, the files it names, and with them the files a package carries or
leaves behind. No run of the 128 ended otherwise than with exit 0.

**A float where the build reads a vector: no rule.** The census history holds no build at which
an emitter's `scale0`, `birthScale0`, `birthRotation0` or `birthRotationalVelocity0` is a float
(#MOD-ARTIFACT-0036), so no conversion of Riot's is read, and nothing was changed or run.
