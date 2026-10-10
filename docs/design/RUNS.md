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

### The baselines of 2026-10-06

The baselines made again after the fixer batch ("The rules of the fixer batch"), over the 5,137
mods of the datasets, against the 16.19.8207193 copy and the history index of 757 builds, at
24 in parallel with one thread each. They cover the port's two rules of #ADR-0063 and #ADR-0064,
which had no run of the datasets before.

| baseline | command | done | ended by a fatal diagnostic | failed | time |
|---|---|---:|---|---:|---|
| `baseline-repair-2` | `repair --no-write --again` | 5,109 | 28: one `archive.no-skin`, 27 `package.shader-missing` | 0 | 1,213 s |
| `baseline-port-2` | `port --no-write` | 5,102 | 35: one `archive.no-skin`, 15 `package.empty`, 19 `package.shader-missing` | 0 | 1,002 s |

- **The repair.** Every one of the 5,109 packages built again from its own package: no second
  repair changed a value or a chunk. The checks raise `check.field-undeclared` in 218 mods and
  `check.type-mismatch` in 47. The batch's rules in it: `vfx-spawn-shape-birth-translation`
  moves 110,401 values and removes 187,122 in 272 mods; `vfx-legacy-simple` removes 13,106
  values of complex emitters and moves 129 in 76 mods; `material-shader-relink` relinks 547
  passes in 89 mods.
- **Against the baselines of 2026-10-04.** Every mod differs: each report holds the evidence
  part the earlier baselines lack (#ADR-0041), and the history's head is another. The other
  differences are those of the rules and fixes committed since, the batch's among them. This
  comparison does not attribute each row to one of them.
- **The edge cases**, derived by `cases.py` from the reports of both: 665 cases seen and 98
  declared and not seen. [CASES.md](CASES.md) lists them. The table of `cases.py` gained the
  places the reports hold since the earlier baselines: the evidence part, which says again
  what other parts say, the units a port leaves out of the game's block, the reasons a thumbnail
  is dropped, and the materials and shaders of `package.shader-missing`.
- **The reduced corpus**: 63 mods show the 665 cases, 1.53 GB of archives. 14 of them are the
  only mod to show a case. Run twice per command at 24 in parallel, the repair in 20 s and 17 s
  and the port in 16 s and 17 s, as the baselines ended them: in the repair 1 `archive.no-skin`
  and 2 `package.shader-missing`, in the port 1 `archive.no-skin`, 2 `package.empty` and 1
  `package.shader-missing`. `compare.py` reports no difference between the two runs, nor
  between a run and the baseline's reports of the 63 mods.

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

**The shaders mods link and define**, counted by `scripts/corpus/shaders.py` over the 5,137 mods
of the datasets, against the shader bin of the 16.19.8207193 copy, the bins the client loads read
off the repair's baseline: 104 mods hold a material linking a shader the bin lacks, 15 shaders in
all, each one a build removed (#GAME-CHANGE-0024 has them by shader). No mod defines a
`CustomShaderDef`, in a loaded bin or in any other.

**A material's removed shader relinked, or no package** (#ADR-0069). The rule of a mod's own
`CustomShaderDef` was not built: no mod of the datasets has the case. Reduced corpus: 2 repairs
and 1 port end with `package.shader-missing`, 6 mods have materials relinked, and every package
embeds the new fixup's id in its evidence. The 112 mods whose bin links a missing shader,
without the rule and with it:

| command | mods | ended by `package.shader-missing` | relinked, of those with a package | other differences |
|---|---:|---:|---:|---|
| `repair --no-write --again` | 112 | 27 | 79 of 85 | 3 mods write a bin again that they kept, for a relinked material |
| `port --no-write` | 112 | 19 | 85 of 93 | links that dangled resolve to a game shader: units of 59 mods lose a dangling link |

A run ended keeps no package, check or summary of a package. Every other difference is the
relinked links, the objects that hold them and the fixup's count.

## The behaviour carried over from ltk-forge (2026-10-06)

The thirteen items of [ROADMAP.md](ROADMAP.md), "Carried over from ltk-forge", measured before
any of them changes. The counts are over the reports of `baseline-port-2`, the 5,137 mods of the
datasets, against the 16.19.8207193 copy, the history index of 757 builds (head
16.19.8230722) and the CommunityDragon hash tables. Archives and WADs were read in memory.

**A `.tft_set<N>` tag in a ported file's name.**

- The port ships 625 names with `.tft_set<N>`, in 221 mods: `tft_set13` 302, `tft_set12` 146,
  `tft_set16` 60, eleven other sets 117. No shipped name and no file name of a mod holds
  `.tftset<N>`.
- The mods' file names hold the tag 1,921 times, in 838 mods. Each is a file name the game's own
  paths had: the history holds 92,772 game paths with the tag, from 9.22 to 16.14, none at the
  head. 237 of the 1,921 are game paths, the others sit in folders of the mod's own. The other
  two shapes of #MOD-ARTIFACT-0026 are the game's file names as well: `.pie_c_<major>_<minor>`
  6,962 of 6,967 names, `.skins_<character>_skin<N>` 107,257 of 107,354.
- With the tag dropped, 6 shipped names in 6 mods meet a name the port ships already, an
  effect's `color-hold.tex` beside its `color-hold.tft_set12.tex`, and would take `-2`. No two
  tagged names drop to one.
- The port ships 23,753 names, in 959 mods, with another tag the game's paths held (`domina`,
  `mel`, `aurora`, `leblanc_rework` the most), and 1,399, in 105 mods, with a dotted part no
  game path had.

**Two units of one character on one slot.** 6 slots in 6 mods are decided by two units or more.
The rank decides by the unit written for the slot in 3, by a readable entry in 1, by skin0 in 1,
and by the source slot in 1: `552d4d5e68c1`, where 18 units of `teemomushroom` (slots 14, 27 to
36, 47 to 53, none at the champion's slot) follow Teemo's skin0 unit to skin25, none fitting
there. The unit of skin14 keeps the slot, and 17 are superseded.

**The joints of a game skeleton at a target no unit was compared with.** 15 decisions in 14 mods,
each a sub-character following its champion. At 4 targets the game has no skin of the
sub-character, and the slot check blocks. At 8 the skeleton is the mod's own and fits whatever
the joints. At 3 (`8336be0190b4` twice, `efddf09d6993`) the skeleton is a game file of another
skin, and the check says "other joints than skin 16's" with skin 16's joints never read. The
index's skeletons of 16.19 differ in all three (6 joints against 1, 16 against 8, 79 against
115), and the mesh and the clips block there too. No decision would change.

**Retarget renames in every object.** 596 mods retarget a unit, and its renames change 16,831
objects. 15,223, in 139 mods, are outside the unit's group: 15,167 in bins only units that leave
the port load (97 mods), 50 in bins no unit loads (43 mods), and 6 in bins that a shipping unit
of another group loads (`0ba7ef809ade`, `137304ee1cad`). They are 9,934 graphs, 4,884 skin
entries and 405 resolvers. The renames scoped to the group, run over the 597 mods: 595 equal.
In the 2 others one unit goes from skin0 to a premium skin and another from skin1 to skin0, and
skin1's bin holds objects under skin0's entry and resolver names. Scoped, those keep the names
the second unit's entry takes, and that unit loses its patch.

**What a unit reaches, reported for units that leave.** 3,529 findings in 118 mods are raised on
units that leave the port: `object.unreached` 1,739, `object.link-dangling` 1,236, `ref.dangling`
412, `ref.unmounted` 118, `object.game-carried` 24. Each such unit is a chroma folded into its
parent or the game's own skin shipped again. Each of the 118 mods has findings on units that
ship too. Units that ship have 9,968.

**A locale bank of a unit that is not voice.** 2,665 copies of banks carry a locale, and one is
of a unit that is not voice: `b7dcacfd0617`, `Yone_Base_SFX`, a media bank of version 134 at the
base's effects path, shipped in the en_US WAD alone, with 57 own clips of 78. The port carries
it in the layer `voice-en-us`.

**A locale copy whose own clips are not the champion WAD's.** No report holds
`audio.not-carried why=locale-differs`. Of 1,471 locale copies with own clips, one has a copy in
the champion WAD with own clips at the same path (`9b85da734738`), and the two are byte for byte
the same.

**The first own events bank.** The 64 units in 18 mods of "Sound units of a mod's own", 630 own
clips: in each the first events bank of the client's version, in the order the bin lists its
banks, plays none of the unit's own clips, and one later bank plays all of them. 18 Rengar mods
ship one of two copies of one added bank. Of the 8 with its larger copy, 3 list it first and are
carried, and 5 list the base's first and are not. No event a later bank keeps is in another bank
of the unit. 26 of the events `959d7baf9bb5` would carry hold the ids of the game's Volibear
base events bank.

**Sub-characters by the champion's name.** Over the 174 champion WADs of 16.19, 195 pairs of a
champion and a character whose skin bins its WAD holds.

- The names starting with a champion's, from the tables and the WAD: 138. The check for a skin
  bin in the WAD keeps 98, each a sub-character. Of the 40 it drops, 5 are Vi's (Viego, Viktor,
  their sub-characters, `visionward`) and 35 are characters of earlier builds.
- Characters with skin bins in a champion's WAD that do not start with its name: `heimertblue`
  and `heimertyellow` (Heimerdinger's turrets, 27 slots each), `testcuberender` (Skarner,
  skin0), `trundlewall` (Zoe, skin0; Trundle's too), and 93 `jade_*` characters in 72 WADs from
  16.15.7983109, each a champion's variant of a game mode with its own character record.
- The skins' `extraCharacterPreloads` names 52 characters. It leaves out 67 of the 102
  sub-characters that are not `jade_*`, and 17 of its names are in no WAD of the champion.
- The mods with bins of those characters: 2 Heimerdinger, 1 Skarner, 4 Zoe and 2 with `jade_*`.

**Four outcomes no run reaches.** Every character is added with a group, and `Catalogs::find`
asks the game for each, so every representative unit gets a decision: `Left::NoTarget` follows
from no input. Of the game's skin entries at the head, every champion's has a bank unit but
TFT's, and 5,319 of 9,826 sub-characters' have none: a port of a sub-character's own audio to
one of those reaches `no-unit-to-model`.

**`unit.new-bin`.** 5 in 5 mods, each on a unit that ships, and `unit.ported` says in each that
the bin is written whole.

**The bounds.**

| bound | value | largest in the datasets | largest in the game | passing it |
|---|---:|---|---|---|
| archive file | 4 GiB | 439,570,870 bytes | | fatal |
| entries of an archive | 250,000 | 2,033 | | fatal |
| one entry or chunk, inflated | 512 MiB | an entry of 439,570,394 bytes, a stored packed WAD; a chunk of 304,352,832 | a chunk of 220,975,794 | the entry is skipped, a packed WAD dropped |
| all entries and chunks, inflated | 4 GiB | 1,056,182,694 bytes | | fatal |
| chunks of a packed WAD | 250,000 | 5,261 | 90,012, `Companions.wad.client`; the most at 26 builds from 8.20 | the WAD is dropped |
| bins followed from one root | 64 | 89, in one mod (`2c2bd16ccd4a`), whose 100 units load 64 | | nothing raised: the rest is not loaded |
| bins read of one game skin | 16 | 1,807 mods have a character with a skin cut | 41 (Evelynn skin0); 533 of 19,609 skins pass 16 | nothing raised: the catalog holds the first 16 |
| chunks of a file's size read | 64 | 1,167 candidates of one size; 258,056 of 1,315,880 files judged meet more than 64 | | nothing raised: 28,970 files in 1,554 mods would take another first path, no verdict changes |

The game holds no WAD near 200,000 chunks. A slot is looked for up to 120: champions reach 116,
and 46 `jade_*` characters have skin bins at 301 to 305.

### The changes, one at a time

Each change measured over the 63 mods of the reduced corpus and over the mods the change is
about, against the build before it, the reduced corpus against `baseline-port-2` and
`baseline-repair-2` as well. Every run ended as the baselines did unless the row says
otherwise.

**The text of a check's reason.** Six texts say what their check tests, and the block of clips
that needs an own rig holds the clips borrowed. 27 ports differ, in the reasons' texts and that
count alone. No repair differs.

**`unit.new-bin` raised by the build.** The 5 mods that raise it: the finding comes with the
package, names the bin written in place of the slot, and the ids of the diagnostics after it
move by one. The reduced corpus holds one of them, and no other port differs.

**The archive's counts of entries and chunks at 262,144.** No run differs.

**One entry or chunk inflated, up to 1 GiB.** No run differs.

**Bins followed from one root, up to 128, and `bin.load-capped` past it.** The reduced corpus does
not differ. `2c2bd16ccd4a`, whose 100 units loaded 64 bins each: they load all of theirs, up to 89.
Its shipped kled unit carries 85 objects where it carried 60: 25 names that resolved to the
game's objects find the mod's past the old cap, and 11 files left behind as orphaned ship. No
run raises the note.

**Game chunks of a file's size read up to 2,048, and `file.same-size-capped` past it.** The 1,801
mods with a file whose game copy sat past the 64th chunk of its size, and the reduced corpus:
no repair differs, and 1,785 ports do (15 of the reduced corpus). More copies are found: the
first game path and the skin folder of files change in 1,785 mods, and the origin evidence counts
more game files of a skin in 134. 352 files in 71 mods held a copy in the champion's WAD past
the cap and one only outside the WADs mounted with the champion; their references now name the
copy in the champion's WAD, and the port no longer ships them. The origin of 5 decisions moves
(in 2 a premium origin goes, in 1 skin 12 becomes skin 9), and no target changes: one reason goes
from `paste-cleaned` to `base`. A run takes as long: 7.2 s a mod on average, with and without.
No run raises the note.

**Bins read of one game skin up to 64, and `catalog.capped` past it; the apply check loads up to
128 bins, and `check.load-capped` past it.** The 1,807 mods with a character whose game skin links
more than 16 bins, and the reduced corpus: no repair differs, and 733 ports do (12 of the reduced
corpus). The objects of the bins past the 16th are known to the facts and the plan: in 529 mods
names that dangled resolve to the game's objects, 216 count more game objects among the objects
loaded, and in 189 the game objects carried because they name files the mod overrides change. In
2 Zed mods (`7c3a7383be57`, `dd17fbb652af`) every object the skin0 unit reaches is the game's byte
for byte, and the unit leaves as the game's own skin. No run raises either note.

**A sub-character by the champion's name or the game's preloads** (#ADR-0071). The reduced corpus
does not differ. The 2 Heimerdinger mods that ship the turrets' bins port them as sub-characters,
each with a patch at skin0; in `c3e402036ead` the turrets' 146 skin bins are loaded, and the
copies among them leave. The Xayah mod `8afd81d86710`, which ships `taliyahwallchunk` in
Xayah's WAD, ports it as Xayah's, patched at skin4.

**What a unit reaches, reported for the units that ship** (#ADR-0072). The 118 mods with findings
on units that leave, and the reduced corpus: no repair differs, and the ports differ in those
findings alone, 3,493 fewer in 118 mods (`object.unreached` 1,739, `object.link-dangling` 1,200,
`ref.dangling` 412, `ref.unmounted` 118, `object.game-carried` 24). 36 of the 1,236 counted
before resolve to the game's objects since the cap on a game skin's bins.

**A locale WAD's bank of a unit that is not voice, in the base layer** (#ADR-0073). The reduced
corpus does not differ. In `b7dcacfd0617` the unit of `Yone_Base_SFX` moves from the layer
`voice-en-us` and the en_US WAD to the base layer and the champion WAD: the base patch gains its
audio record, and the layer's patch sets the block with the unit among the base's.

**A locale copy with other own clips, not carried** (#ADR-0074). Kept as it was: no code changes
but a comment, and no run is made.

**The first own events bank that plays the clips** (#ADR-0075). The reduced corpus does not differ.
The 18 mods with a bank no first events bank played: the 64 entries of `audio.not-carried` with no
event are gone, and the banks carried go from 140 to 204, each with its events cut from the later
bank. The patches of 17 of them gain the units' records.

**The retarget's renames in every object.** Kept, with the measurement above: scoped to the
group, the renames drop a patch in the two mods where they reach another group's objects. No code
changes and no run is made.

**The game's skeleton of every skin read.** The reduced corpus and the 14 mods whose targets lay
outside the slots compared do not differ: at those targets the skeleton is the mod's own, the game
has no skin, or the joints differ and the mesh and the clips block too. Five of the 14, timed
alone with the build before and this one, take 48.8 s and 49.0 s.

**Units of one rank on a slot, left to the game** (#ADR-0076). The 6 mods where two units decide
one slot, and the reduced corpus: one mod differs, `552d4d5e68c1`, of the reduced corpus. Its 18
units of `teemomushroom` leave with `target.conflict` in place of 17 `target.superseded`, the unit
of skin14 among them, whose patch goes; Teemo's unit ports to skin25 as before.

**The outcomes no run reached.** `target.conflict` is raised by one mod since the tie leaves its
units. The two reasons the build has for not writing a unit's audio follow from inputs the
datasets do not hold: a sub-character's own audio at a skin whose game entry has no bank unit,
and a layer of a unit whose bin is written whole. `Left::NoTarget` follows from no input, as
measured above, and stays as the reason of a unit with no decision. No code changes.

**A `.tft_set<N>` tag dropped from a ported file's name** (#MOD-ARTIFACT-0026, corrected: the four
shapes are the game's own variant tags). The 221 mods that ship such a name, and the reduced
corpus: no repair differs, and 221 ports do (2 of the reduced corpus), in names alone. 625 files
take their name without the tag, and the bins and 3 patches that name them follow. In the 6 mods
where the name met a shipped one, the tagged file, placed first, takes the bare name and the
other takes `-2`. No diagnostic differs.

### The baselines after the changes

Made again over the 5,137 mods of the datasets once the changes were in, against the
16.19.8207193 copy and the history index of 757 builds, at 24 in parallel with one thread each,
and compared with the baselines they replace.

| baseline | command | done | ended by a fatal diagnostic | failed | time | differs from the one replaced |
|---|---|---:|---|---:|---|---:|
| `baseline-port-3` | `port --no-write` | 5,102 | 35: one `archive.no-skin`, 15 `package.empty`, 19 `package.shader-missing` | 0 | 969 s | 3,600 mods |
| `baseline-repair-3` | `repair --no-write --again` | 5,109 | 28: one `archive.no-skin`, 27 `package.shader-missing` | 0 | 1,273 s | 4 mods |

- **The differences** are those of the changes above, section by section: the port's in
  provenance, facts, decisions, plan, chunks, patches and diagnostics, the repair's in the 3 mods
  of the new sub-characters and the one whose units load more bins. Every repair built again
  from its own package changes nothing. No run raises one of the four notes of a passed bound,
  and one raises `target.conflict`.
- **The edge cases**, derived by `cases.py` from the reports of both: 670 cases seen and 100
  declared and not seen. The four notes of a passed bound are among the cases not seen;
  `target.conflict` is seen.
- **The reduced corpus**: 64 mods show the 670 cases, 1.60 GB of archives. 15 of them are the
  only mod to show a case. Run twice per command, the port in 15 s and 16 s and the repair in
  16 s twice, as the baselines ended them. `compare.py` reports no difference between the two
  runs, nor between a run and the baseline's reports of the 64 mods.

## The history index, format 1, on the first ten builds (2026-10-07)

`reforge history index` over the census history's `history-v2` branch up to its tenth build,
8.20.2483196 to 9.1.2577563, on 24 threads, into a scratch directory. The index a run reads was
not rebuilt (#ADR-0077).

- **Time.** The ten builds in 20.6 s. The first five in 15.7 s, and the next five appended to
  them in 4.5 s.
- **Size.** 282 MB.
- **Append.** The index of five builds with five appended equals the index of ten, every row of
  every table, names and WADs compared as text, and `builds.tsv` byte for byte.
- **Against the clone's trees** at builds 4 and 9 (`an_index_agrees_with_the_trees_of_its_builds`,
  `reforge-census`): the placements alive are the tree's entry files, 151,048 and 167,513, by
  path, WAD, file key and checksum; the entry texts alive are the tree's texts, 54,165 and 57,686,
  by bin, entry, WAD and blob; every bin is the first WAD's, with its links and its objects. Every
  97th text read by its blob is the text the search of the tree finds, and parses: 559 and 595
  texts, read in 0.11 ms and 0.13 ms by blob against 3.3 ms and 3.9 ms by search (medians).
- **Objects reached.** The 3,976 skin entries of 9.1.2577563 reach 35 objects at the median and
  260 at most.

| table | rows | bytes a row, its indexes included |
|---|---:|---:|
| `placements` | 205,068 | 88.7 |
| `wad_lives` | 7,825 | 25.1 |
| `wads` | 2,649 | 114.4 |
| `entry_texts` | 70,718 | 47.0 |
| `bins`, `bin_entries` | 11,781 and 157,272 | 82.7 and 16.6 |
| `schema` | 3,234 | 38.0 |
| `links` | 158,380 | 29.3 |
| `objects` | 68,088 | 106.2 |

The 4,895 bins the nine builds after the first changed hold 21.3 objects each, and a changed bin
is a row of `bins` with all its objects again.

## The history index, format 1, every build (2026-10-08)

`reforge history index` over the census history's `history-v2` branch at `f68de01e049c`: 757
builds, 8.20.2483196 to 16.19.8230722, every family, on 24 threads, into a scratch directory.

- **Time.** 3,231 s: the walk 2,956 s, the write 275 s. The format 0 index of every family was
  written in 2,263 s on 32 threads (2026-10-04).
- **Memory.** 19.4 GiB at the peak of the process's working set, 20.9 GiB of private bytes.
- **Size.** 3,665,747,968 bytes, against 2,748,997,632 for format 0 of the same commit. With
  `zstd --ultra -22 --long=31 -T1`, as the release's database: 1,007,913,337 bytes, 27.5%, in
  2,979 s; format 0's was 767 MB (2026-10-05).
- **Append.** The index of the first 752 builds, written in 3,276 s, with the last five
  appended: 14.1 s for the process, 12.0 s by its own count, about 7 s of it 16.19.8207193, at
  1.96 GiB of working set. It equals the whole index of 757 builds, every row of every table,
  names, WADs and objects compared by what they hold, and `builds.tsv` byte for byte.
- **Against the clone's trees** at builds 378 and 756 (`an_index_agrees_with_the_trees_of_its_builds`):
  606,247 and 948,353 placements, 260,432 and 459,675 entry texts and every bin agree. Every 97th
  text read by its blob is the one the search of the tree finds, and parses: 2,685 and 4,739
  texts, in 0.22 ms and 0.27 ms by blob against 8.5 ms and 10.2 ms by search (medians).
- **`path`**, from `placements`, timed in Python's `sqlite3` over 20,000 paths of the history
  and 20,000 hashes it never held: 13.0 µs and 7.4 µs at the median, 82.9 µs and 16.9 µs at the
  99th percentile. The `paths` read of format 0 took 5.3 µs and 4.0 µs, 154.0 µs and 16.8 µs.
  Every step is a search of a key, and the kind of the first file is ordered in a small
  temporary tree: a path has 2.19 placements on average and 2,759 at most.
- **Objects reached.** The 21,024 skin entries of 16.19.8230722 reach 61 objects at the median
  and 487 at most, under `MAX_REACHED` (4,096). The recursive query takes 0.61 ms at the median
  and 7.7 ms at most, in Python's `sqlite3`.

| table | rows | MB, its indexes included | bytes a row |
|---|---:|---:|---:|
| `placements` | 4,694,830 | 435.1 | 92.7 |
| `wad_lives` | 147,360 | 3.9 | 26.3 |
| `wads` | 5,133 | 0.6 | 111.7 |
| `entry_texts` | 3,888,062 | 191.7 | 49.3 |
| `bins` | 773,674 | 72.6 | 93.8 |
| `bin_entries` | 19,841,522 | 356.8 | 18.0 |
| `schema` | 22,065 | 0.8 | 34.5 |
| `links` | 1,831,427 | 58.3 | 31.8 |
| `objects` | 3,277,555 | 359.4 | 109.7 |

- A bin holds 25.6 objects on average. As an object key and a class, `bin_entries` would take
  about 36 bytes a row, 710 MB; as the id of its row in `objects`, it takes 18 bytes, and
  `objects` takes 72 MB more than format 0's 287 MB for the id and its index.
- 1,238 object keys are held under two classes: `objects` has 3,277,555 rows against format
  0's 3,276,317.
- The tables format 1 drops took 226 MB (`contents`) and 49 MB (`paths`) in format 0.

## Entry objects stored apart from the index (2026-10-08)

Every distinct entry text that `entry_texts` names in the format 1 index of 757 builds (head
`f68de01e049c`), read from the clone, parsed, and written as `ltk_meta` writes its object from
the entry hash on: 3,165,814 texts, 100.96 GB of text, 22.42 GB of objects, in 1,788 s on 24
threads. Every text parsed and held one object. The largest object is 545,257 bytes. 1,238
pairs of texts write the same bytes under two classes.

- **Against `objects`.** 2,955,843 of the objects' digests are object keys. The other 209,971
  are all of texts first held at builds of legacy bins: 51,282 of 64,071 in season 8, 150,587 of
  203,758 in season 9, 8,102 of 126,812 in season 10, none after.
- **Layouts.** Each read checked every 97th object against its uncompressed bytes, and none
  differed. The payload counts the dictionaries. A read is single-threaded, the file in the
  system's cache.

| layout | payload | file | read, median and p99 | frames a read |
|---|---:|---:|---:|---:|
| the objects in one file and 32-byte records in another, each compressed whole (`--ultra -22 --long=31`) | 288.2 and 72.0 MB | | | |
| one row per object, uncompressed | 22.42 GB | 25.29 GB | | 1 |
| sqlite-zstd's layout: each row alone, a dictionary for each of the six largest classes and one for the rest, a ten-thousandth of their bytes | 2,130 MB | 2,615 MB | 36 and 92 µs | 1 |
| the same, dictionaries 3e-5 and 1e-3 of their bytes | 2,364 and 2,174 MB | 2,901 and 2,676 MB | | 1 |
| each row alone, a dictionary for each of the 38 classes with 1.6 MB or more, up to 1 MiB | 2,068 MB | 2,543 MB | 34 and 60 µs | 1 |
| each version against the one before, a keyframe every 16, no dictionary | 882 MB | 1,200 MB | 144 and 526 µs | 16 |
| the same, keyframes with a dictionary per class up to 1 MiB | 545 MB | 797 MB | 125 and 438 µs | 16 |
| the same, dictionaries up to 256 KiB, up to 8 MiB | 574 and 562 MB | 831 and 815 MB | | 16 |
| the same at level 22 | 544 MB | 795 MB | | 16 |
| the same, a keyframe every 8 | 630 MB | 893 MB | 90 and 246 µs | 8 |
| the same, the first version the only keyframe | 514 MB | 762 MB | 124 and 1,069 µs | 162 |
| each version against its block's keyframe, a keyframe every 8 | 866 MB | 1,149 MB | 58 and 101 µs | 2 |
| the same, a keyframe every 16 | 946 MB | 1,236 MB | 68 and 154 µs | 2 |
| the same, the first version the only keyframe | 1,060 MB | 1,371 MB | 64 and 129 µs | 2 |

- **sqlite-zstd's maintenance** compresses rows in place a chunk at a time, and finds each chunk
  by a scan of the table from its start. On these rows, three runs read 150 GB to 1.1 TB and
  wrote about 85 GB each in 15 minutes without finishing a group, and were stopped. Its layout,
  written in one pass, read through its own view.
- **Similar objects together.** On the first 200,000 rows, 1.39 GB: a dictionary per group of
  rows ordered by a MinHash of their bytes over 8-byte windows, groups of 32 KiB to 1 MiB with
  dictionaries of 2% to 25% of a group, took 129 to 170 MB, against 128 MB with a dictionary per
  class and 35.5 MB for each version against the one before.
- **The 16-byte blob key** and its index took 153 MB of the 797 MB file of versions against the
  one before.

## The history index with every object's bytes (2026-10-08)

`reforge history index` over the census history's `history-v2` branch at `f68de01e049c`: 757
builds, 8.20.2483196 to 16.19.8230722, every family, the object bytes in it (#ADR-0078), into a
scratch directory.

- **Coverage.** 3,165,814 entry texts, each with its object. None failed to parse, and none held
  no object or several. 3,164,576 rows of `object_bytes`: 1,238 texts share a row with a text of
  another class. 687,647 keyframes, and 20 dictionaries, 19 of a class and the general one,
  6,191,309 bytes.
- **Against `objects`** (`reforge history check-objects`, 105 s): 2,955,843 objects' digests are
  object keys under their text's class. The other 209,971 were all first held at builds of legacy
  bins and hold a kind legacy numbering writes as another byte. None is unexplained.
- **Against the clone's trees** at builds 378 and 756: every 97th text's object, 2,685 and 4,739,
  is what `ltk_meta` writes of the text the search of the tree finds. `object_bytes` takes 81.9
  and 82.7 µs at the median and 185 µs at the 99th percentile, the text by its blob through the
  clone 128 and 162 µs, and by a search 7.3 and 9.2 ms. Before the keyframe's row came with the
  object's, in one query, `object_bytes` took 98.8 and 103.5 µs.
- **Size.** 4,769,964,032 bytes, against 3,665,747,968 without the object bytes: 1,104,216,064
  more. The payload is 866,156,174 bytes: the frames, 859,964,865 (keyframes 392,041,369, the
  other versions 467,923,496), and the dictionaries. `text_bytes` holds 85 MB of blob ids,
  classes and rows. With `zstd --ultra -22 --long=31 -T1`: 1,730,947,676 bytes in 3,241 s,
  against 1,007,913,337 without; the frames are zstd already.
- **Pages.** The same file rewritten by `VACUUM` at 4, 16 and 32 KiB pages: 4,824,715,264,
  4,707,926,016 and 4,666,130,432 bytes. Without the object bytes at 32 KiB: 3,620,503,552. At
  32 KiB the growth is 1,045,626,880 bytes, 179 MB over the payload against 238 MB at 8 KiB, and
  `object_bytes` reads as fast: 101.8 and 102.3 µs at the median with two queries, against 98.8
  and 103.5 µs at 8 KiB. The writer keeps 8 KiB.
- **Time and memory.** Every build took 3,837 s on 23 threads, beside a zstd on one: the walk
  2,656 s and the write 1,179 s, the tables of lives about 240 s on one core and the object bytes
  about 900 s on five to six. The peak was 25.5 GiB of working set and 26.0 GiB of private bytes.
  The index of the first 752 builds took 4,696 s, the walk
  3,416 s and the write 1,277 s, at 25.4 GiB of working set and 26.0 GiB of private bytes. Every
  build without the object bytes took 3,231 s at 19.4 GiB. A first build of all 757, which made
  an allocation for each object and each frame, took 9,553 s at 39.3 GiB of private bytes, 24
  minutes of it on one core before the dictionaries were trained; its objects, written again in a
  fresh process by the writer of the batches, gave the same 3,164,576 frames in 286 s.
- **Dictionaries.** Trained in 0.04 to 3.7 s each, 20 s in all on one core: the largest from
  11,181 samples, 106 MB of the class of 368,970 keyframes and 3.48 GB.
- **Append.** The last five builds appended to the index of 752: 32.5 s for the process, at 2.20
  GiB of working set and 6.43 GiB of private bytes, against 14.1 s without the object bytes. It
  equals the whole index of 757 in every table, the object bytes compared decoded with their
  keyframes and their texts (344 s).
- **Determinism.** Two whole builds of the first ten builds write the same file, byte for byte,
  and so do two of all 757, the first by the writer of an allocation per object:
  `history.sqlite`, `stamp.json` and `builds.tsv`.
- **The first ten builds**: 40 s to write, 68,088 texts and as many rows of object bytes. 3,033
  are keyed; the other 65,055 are all of legacy bins and hold a renumbered kind. The first five
  with the next five appended equal the ten built whole.

## Extra units beside a champion's main unit (2026-10-09)

`scripts/corpus/extra_units.py` over the reports of `baseline-port-3`, with no port run and no
archive opened, for the 5,121 mods whose archives the datasets hold. 16 archives left the
datasets on 2026-10-09: a series of uploads of one icon file per Ashe skin beside the game's own
bins, and one more upload of the same uploader. 35 reports end in a fatal diagnostic, which
leaves 5,086 ports.

**What it measures.** A champion whose port ships more than one unit has a main unit: the unit
at skin0, else the one with the most own work. Each other unit it ships is an extra unit. An
extra unit's own work is compared with the main unit's by bytes: an own file by its content, an
own object by the SHA-256 of the object the package writes, an own clip by a bank of the main
unit's at the same path or with the same bytes. Own clips in a bank of the base's sound unit do
not count, since every skin bin of the game names the base's units. A mod is multi-skin when an
extra unit ships an own model, skeleton or texture that the main unit does not ship, a leftover
when no extra unit holds own work that the main unit does not hold, and ambiguous otherwise. A
family is the archives that share more than half of the smaller one's own files, by bytes.

**How often.** 459 of the 5,086 ports ship 2,604 extra champion units, in 314 families.
`Left::GameSkin` leaves 683 units in 33 mods.

| bucket | mods | families | extra units | at the main unit's origin slot |
|---|---:|---:|---:|---:|
| leftover | 55 | 46 | 311 | 3 |
| multi-skin | 361 | 237 | 2,068 | 20 |
| ambiguous | 43 | 36 | 225 | 3 |

Unit by unit, 431 extra units hold no own work that the main unit does not hold: the leftovers'
311, 83 in 15 multi-skin mods and 37 in 3 ambiguous ones. 1,883 ship an own model, skeleton or
texture, and 290 hold other own work. The reports of `baseline-port-1` (2026-10-04) give the same
55 leftovers and the same 43 ambiguous mods, and 5 more multi-skin mods: 3 end in a fatal
diagnostic in `baseline-port-3`, and in 2 the skin0 unit leaves there as the game's own skin.

**The three mods of the report.** PROJECT Urgot 1.5.0 (`976ade55971f`) is a leftover. Its skin3
unit's two own files are the main unit's icons by bytes. The unit also names the base's effects
bank, with 117 own clips that the main unit's audio block does not name; PROJECT Urgot 1.5.3
names the same bank bytes in its one unit, at skin0. Ben 10 Ultimate Alien Udyr
(`7eda40f07563`) is ambiguous. Its units at skins 3, 5 and 6 each hold 4 own objects that the
main unit does not write, and the units at skins 3 and 5 hold 33 and 9 own clips in banks at
their skins' paths. The main unit's renamed banks hold 160 and 97 clips, 33 and 9 of them own,
in other bytes. The reports hold no clip's bytes, so whether those are the main unit's clips is
open. The port carries none of the main unit's 75 own clips (no events bank of the client's
version to cut), and carries the 33 and the 9 at skins 3 and 5. The Lee Sin restoration
(`bb677a3d9e9d`) is multi-skin: each of its 22 extra units ships own models, skeletons or
textures, 86 files in all.

**Leftovers.** Of the 311 units, 259 have a bin of an older build's bytes, last shipped at 14.x
for 30, 15.x for 171 and 16.x for 58; 50 a bin of the install's bytes; 2 an own bin. 247 units
in 32 mods sit beside a main unit placed as the base, 61 in 20 mods beside one at a premium
slot, 2 in 2 mods beside a paste, and 1 beside one placed at the skin whose clips it plays. They
are the units #ADR-0020 drops: faults like PROJECT Urgot's, and spreads of the main unit's work
over the game's bins. `Left::GameSkin` keeps them for what they share with the main unit: 277
units in 41 mods name own files, each the main unit's by bytes; 22 units in 10 mods reach own
objects that the main unit writes too; 58 reach the game's objects of another slot. It keeps 23
units in 5 mods whose every shipped unit, the main one with them, holds no own work: it leaves a
game skin only beside a unit that carries the mod's work. 2 units in 2 mods have a bin that does
not read, with no entry and no reach, and ship with the reason `unjudged`.

**Multi-skin.** 166 mods ship an own model or skeleton that the main unit does not ship, 195 own
textures alone. In 194 the files sit in the unit's skin folder or are its rig. 76 of the 78
mods with "old" in their name are here, each skin's old model restored. The other 2 are one
family of Nidalee voice mods whose extra units are older builds' bins: one a leftover, whose
units name the main unit's own voice package (151 own clips) and nothing else of their own, and
one ambiguous through 154 own effects clips at one unit's skin.

**Ambiguous.** No extra unit of the 43 mods ships an own file that the main unit does not. They
hold own objects (148 units, 7,700 objects) or own clips in banks at their own skins' paths (42
units, 2,897 clips). Most are chroma effects mods, each chroma's unit with effects objects of its
own, and sound mods with each skin's own clips.

| held, not the main unit's | 0 | 1 to 5 | 6 to 20 | 21 to 100 | more |
|---|---:|---:|---:|---:|---:|
| own objects: units (mods) | 77 (8) | 25 (9) | 8 (5) | 115 (25) | 0 |
| own clips of its skin: units (mods) | 183 (38) | 6 (2) | 1 (1) | 29 (5) | 6 (6) |

**Each signal alone.** The leftovers' extra units the signal keeps, and the multi-skin mods'
units with an own model, skeleton or texture that it drops:

| a unit leaves when | leftover units kept (mods) | multi-skin units dropped (mods) |
|---|---:|---:|
| it sits at the main unit's origin slot | 308 (52) | 16 (16) |
| its bin is the game's bytes | 2 (2) | 785 (153) |
| its bin is an older build's bytes | 52 (13) | 98 (20) |
| its mesh and skeleton are the game's | 50 (11) | 345 (104) |
| it reaches no own object | 22 (10) | 743 (130) |
| at most 10% of the objects it reaches are own | 16 (6) | 913 (166) |
| at most 5 own objects are not the main unit's | 0 | 1,377 (214) |
| no own object or object of another slot, own files and clips the main unit's | 74 (20) | 0 |
| own objects, own files and own clips of its skin all the main unit's, by bytes | 0 | 0 |

The last row is the leftover bucket's own test, so it separates the certain buckets by its
making; over the ambiguous mods it drops 37 units in 3. No signal outside the byte comparison
comes near it. The main unit's origin slot keeps 308 of the 311 leftover units, and the bin's
bytes drop 785 multi-skin units. The current rule's object test keeps 74 leftover units in 20
mods: their own objects are the main unit's by bytes, or they reach the game's objects of another
slot. A bound on own objects separates only after the file test. With the file and clip tests,
a bound of 5 drops 60 units in 12 ambiguous mods, all extra units of 5. Of Ben 10's units it
drops the one at skin 6; the clips of the units at skins 3 and 5 hold them.

**Families.** Archives whose own files differ in bytes stay apart: three versions of one chroma
effects mod share no own file and count as three families.

## The baselines at e709c75 (2026-10-09)

Made again over the 5,121 mods of the datasets, with the program at `e709c75`, against the
16.19.8207193 copy and the history index of 757 builds in format 1, its events banks keyed by
file (#ADR-0081), at 24 in parallel with one thread each. 16 archives left the datasets that day
(see "Extra units beside a champion's main unit").

| baseline | command | done | ended by a fatal diagnostic | failed | time | differs from the one replaced |
|---|---|---:|---|---:|---|---:|
| `baseline-port-4` | `port --no-write` | 5,086 | 35: one `archive.no-skin`, 15 `package.empty`, 19 `package.shader-missing` | 0 | 943 s | 5,086 mods |
| `baseline-repair-4` | `repair --no-write --again` | 5,093 | 28: one `archive.no-skin`, 27 `package.shader-missing` | 0 | 1,293 s | 5,093 mods |

- **The differences** from `baseline-port-3` and `baseline-repair-3`: every report's evidence
  names the program 0.1.3 against 0.1.2. Beyond it, one port differs (`21c1e6eb6091`): one of
  its game objects is first held at 15.17.7060568 against 15.1.6450085, in the index of format
  1 against the one `baseline-port-3` read. The repair ran beside builds of the workspace and a
  port of 459 mods, so its time is longer than the machine's.
- **The edge cases** of both: 670 cases seen, as before, and 105 declared and not seen. The 5
  more are the cases of `unit.left-with-work` and of `off-target`, which no baseline shows. The
  reduced corpus is the same 64 mods.

## A port changes its target slot only (2026-10-09)

#ADR-0082, over the 459 mods whose ports ship units beside a champion's main unit (see "Extra
units beside a champion's main unit") and over the reduced corpus, compared with
`baseline-port-4`. Every report's port options hold `keep_other_slots`, and that field is the
only difference of 128 of the 459: they have no skin0 unit of the author's and keep every unit.

- **What leaves.** The other 331 differ where units leave at a slot the port does not change,
  2,018 units, and in nothing else.
- **What `unit.left-with-work` names.** 1,408 units in 269 mods hold own models, skeletons or
  textures the units at the target do not hold, and 214 units in 58 mods hold own objects, other
  files or clips alone, not supported for now. Together: 975 models, 967 skeletons, 11,591
  textures, 618 other files, 8,701 objects and 24,061 clips.
- **The three mods of the report.** PROJECT Urgot 1.5.0 and Ben 10 Ultimate Alien Udyr ship
  skin0 alone. Urgot's skin3 leaves with 117 own clips, and Udyr's skins 3, 5 and 6 with 4 own
  objects each and 33 and 9 own clips, all not supported for now. The Lee Sin restoration ships
  skin0 and names the models, skeletons and textures of its 22 other units, which the option
  ships.
- **The reduced corpus.** 9 of the 64 ports differ beyond the options, each where units leave at
  other slots, 127 units. No repair differs.
- **Time.** The 459 mods in 302 s at 8 in parallel, beside the repair baseline. The reduced
  corpus in 16 s for the port and 17 s for the repair.

## Names other skins hold (2026-10-09)

`object.link-dangling` names, of the names a unit reaches that nothing loaded at its target
holds, the ones the game's bins of other skins of the character hold (#GAME-BEHAVIOR-0006), and
what copies of their objects would bring: each object and, link by link, every object it names
that the target lacks. The port with the target slot rule over the 1,976 mods whose
`baseline-port-4` reports hold the finding, 435 s at 24 in parallel, summed by
`scripts/corpus/held_elsewhere.py`.

- **How often.** 2,252 shipping units in 1,756 mods reach 9,160 names that nothing loaded at
  their target holds. The game's bins of other skins of the character hold 1,585 of them, in 179
  units of 164 mods: 1,493 `VfxSystemDefinitionData`, 54 `StaticMaterialDef`, 30
  `GearSkinUpgrade` and 8 `ContextualActionData`. #GAME-BEHAVIOR-0006 leaves open whether a gear
  upgrade's link that nothing loads crashes the client.
- **Where.** 112 of those units, in 102 mods, find the names at their premium origin, and 106
  units, in 91 mods, are pastes, whose source slot is their target.
- **What copies would bring.** 1,585 objects, the names' own: no object reached from them names
  another that the target lacks, and none names a name that nothing holds. They name 5,136
  files. By the objects a unit's copies would bring: 1 to 5 for 124 units, 6 to 20 for 27, 21 to
  100 for 28.
- **The other names.** No skin's bins hold the other 7,575 at the target build. PROJECT Urgot
  1.5.0's 2 and Ben 10 Ultimate Alien Udyr's 1 are among them, and the history holds none of the
  three at any build.
- **The reduced corpus.** 5 ports differ from the run of the target slot rule, in those fields
  alone.

## Copies of other skins' names (2026-10-09)

#ADR-0083, over the 164 mods whose ports reach names only the game's bins of other skins hold
(see "Names other skins hold") and over the reduced corpus, each compared with the run before
the rule.

- **What is copied.** 1,585 objects in 179 units, the names measured: 1,493
  `VfxSystemDefinitionData`, 54 `StaticMaterialDef`, 30 `GearSkinUpgrade` and 8
  `ContextualActionData`. 112 units copy from their premium origin. The names left dangling go
  from 2,456 to 871, by those 1,585.
- **The checks.** No check and no package diagnostic of the 164 differs: the patched bins
  apply, read back and keep their types with the copies in them. One port ends with
  `package.shader-missing`, as it did before.
- **The reduced corpus.** 5 ports differ, each where it copies: 29 copies, the unit's bin that
  holds them, the patched skin bins whose objects name them, and the package's names.
- **Time.** The 164 mods in 48 s at 24 in parallel.

## Events banks of a version the client does not load (2026-10-09)

#ADR-0084, over the 10 mods whose `baseline-port-4` reports leave own clips with no events bank
to cut, each run with the program of the copy rule and with this one, and over the reduced
corpus.

- **Carried.** 445 own clips in 6 mods, their events cut from the game's bank at the path an
  older events bank of theirs held. Ben 10 Ultimate Alien Udyr's 75, in three units whose
  banks are skin 3's, skin 5's and the base's of builds 415 to 572 under paths of their own;
  PROJECT Urgot 1.5.0's 56, skin 3's bank in the base's folder; and 69, 100, 139 and 6 in four
  more.
- **Not carried.** 76 own clips in 4 mods: no event of the game's bank at the target build
  plays them, and they are reported so.
- **The checks.** No check diagnostic of the 10 differs, and none ends in a fatal diagnostic.
- **The reduced corpus.** Ben 10 Ultimate Alien Udyr alone differs from the run of the copy
  rule, by its 75 clips.

## The baselines at 8cacfff (2026-10-09)

Made again over the 4,909 mods in scope once the chroma VFX packs and the restorations were
classed apart (#ADR-0085, #ADR-0086), with the program at `8cacfff`, against the 16.19.8207193
copy and the history index of 757 builds in format 1, at 24 in parallel with one thread each.
They are the first runs of the datasets with the port's rules of #ADR-0082, #ADR-0083 and
#ADR-0084.

| baseline | command | done | ended by a fatal diagnostic | failed | time | differs from the one replaced |
|---|---|---:|---|---:|---|---:|
| `baseline-port-5` | `port --no-write` | 4,879 | 30: one `archive.no-skin`, 13 `package.empty`, 16 `package.shader-missing` | 0 | 936 s | 4,908 mods |
| `baseline-repair-5` | `repair --no-write --again` | 4,884 | 25: one `archive.no-skin`, 24 `package.shader-missing` | 0 | 1,172 s | 0 mods |

- **The comparison** is over the 4,909 mods both baselines hold. The 212 mods classed apart
  leave the baselines. The repair ran beside the port's comparison at 4 in parallel.
- **The port's differences.** 4,501 ports differ in their options alone, which hold
  `keep_other_slots` (#ADR-0082). The other 407 differ where the three rules and the names of
  `object.link-dangling` (see "Names other skins hold") act, and each is a mod of those runs'
  lists but two.
  - 1,454 units in 245 mods leave as `off-target`, and `unit.left-with-work` is raised in 209
    mods. Two of the 245 are not among the mods of "Extra units beside a champion's main unit":
    Aurora's sub-character `auroraspirits` leaves at skin1 (`7c374313c165`), and an Ezreal mod
    that `baseline-port-4` ends with `package.shader-missing` (`cad95ff580d3`) leaves its 20
    units at other slots and ships.
  - 161 mods copy 1,549 objects, each mod in one unit. Their reports are those of the run of
    "Copies of other skins' names", field for field. 12 more mods hold copies in the plan of
    units that leave, and raise no `object.copied`.
  - The 10 mods of "Events banks of a version the client does not load" give the reports of that run,
    field for field.
  - `object.link-dangling` differs in 364 mods.
- **The repair** gives the reports of `baseline-repair-4` in all 4,909 mods: the three rules are
  the port's.
- **The edge cases**, derived by `cases.py` from the reports of both: 668 cases seen and 104
  declared and not seen. 7 cases are new: `object.copied`, the two of `off-target`, the three of
  `unit.left-with-work`, and `audio.carried` of events from an older bank's path. 9 are gone:
  `audio.not-carried why=no-events-bank`, which #ADR-0084 leaves to no mod, and 8 that only mods
  now classed apart showed: the four of `check.other-skin`, `check.type-mismatch` with `Pointer`
  found and wanted, and `retype` of a `List` wanted as `F32` and as `Vec3`. `check.other-skin` and
  `no-events-bank` join the cases not seen, and the five of the target slot rule leave them.
- **The reduced corpus**: 68 mods show the 668 cases, 1.65 GB of archives. 19 of them are the
  only mod to show a case. 51 of the 64 mods before stay, and 6 of the 64 are classed apart. Run
  twice per command, the port in 33 s twice and the repair in 48 s twice, as the baselines ended
  them: in the port 1 `archive.no-skin`, 3 `package.empty` and 1 `package.shader-missing`, in the
  repair 1 `archive.no-skin` and 2 `package.shader-missing`. `compare.py` reports no difference
  between the two runs, nor between a run and the baseline's reports of the 68 mods. One mod
  takes 27 s of the port and 43 s of the repair: `4a554cb95fa0`, the only mod that shows
  `check.type-mismatch` with `Embed` found and wanted.

## A copy's own files (2026-10-09)

#ADR-0088 and the review's fixes of the same day (the load cap tested before a bin is read,
copies keyed by group and name, no `target.chosen` for a unit in a conflict, the copy's slot
and object read off one chain), over the reduced corpus and over the 164 mods whose ports copy
(see "Copies of other skins' names"; 161 are in scope), each compared with `baseline-port-5`
and `baseline-repair-5`.

- **The reduced corpus.** The repair differs in no mod but the program's version in the
  evidence. The port differs in 4 mods: in all 4 the files a copy names get a verdict (62
  `refs` rows); in 2 a copied object's link to a file the mod ships is rewritten to the file's
  place; and one, an Evelynn skin at skin 6, ships 5 own textures its copied material names,
  which left the port as not carried before. Port in 41 s, repair in 57 s, ended as the
  baselines.
- **The 161 mods with copies.** 152 ports differ in the verdicts of the files their copies name
  (4,858 `refs` rows), 26 in the bytes of a bin whose copy links a file the mod ships, and 25
  ship 261 own files their copies name, with `object.carried` and `package.texture-contained`
  changed in those. One port that left 60 files orphaned leaves 40. The 1,585 copies name
  5,136 files; the mods hold 261 of them. 56 s at 24 in parallel.
- **The cap note.** `catalog.capped` is raised by `fix`, the first stage to read a character's
  catalog, and by no other stage; no mod raises it.

## The camera-facing event (2026-10-09)

`graph-face-target` (#GAME-CHANGE-0026, #GAME-BEHAVIOR-0028) over the reduced corpus, port and
repair, each compared with the run of "A copy's own files": no mod of the 68 holds an event of
the old class, and the reports differ in the evidence's list of fixups alone. Port in 38 s,
repair in 51 s, ended as the baselines. The fixup's effect is seen in game, in the record's
evidence, and over the datasets at the next baseline.

## The gear condition (2026-10-10)

`contextual-gear-equip` (#GAME-CHANGE-0027) over the reduced corpus, port and repair, each
compared with the run of "The camera-facing event". One mod of the 68 holds a condition of the
old class. Its two conditions are converted, `check.type-mismatch` is gone from its report, and
its repair edits the rule's object in the 101 bins that copy it (#MOD-ARTIFACT-0001). The other
reports differ in the evidence's list of fixups alone. Port in 39 s, repair in 57 s, ended as the
baselines.

The record's 6 mods, by list, each run with the fixup and with `--no-fixup`: 5 have their
conditions converted (11 in the ports), and `check.type-mismatch` leaves the repairs of all 5 and
the ports of 2. In the sixth, the one bin that holds the conditions is one nothing loads
(`file.unused`), so it is not carried.

## The class of each struct (2026-10-10)

The type check over the reduced corpus, port and repair, each compared with the run of "The gear
condition", first with each struct of a collection judged by its class, then with
`check.class-undeclared` (#MOD-ARTIFACT-0039) added. Port and repair ended as the baselines.

- **Each struct of a collection** gives the reports of the baseline in all 68 mods.
- **The undeclared class.** The port differs in one mod, the repair in 9:
  - the rework the record names, port and repair: its 3 structs, `0xfd47f6e6` in
    `PersistentEffectConditionData.OwnerCondition` and `0x538189bf` twice in
    `FloatComparisonMaterialDriver.mValueA`. `check.done` counts 42 fields fewer, theirs.
  - 6 repairs that ship a champion's root bin whole (#MOD-ARTIFACT-0032): one
    `ItemRecommendationContextList` object each, a class the census history holds last at 16.5.
    Its 3 fields leave `check.field-undeclared`, which they were the whole of, and `check.done`
    counts 3 fields fewer.
  - one repair of a whole root bin and a sub-character's: 46 objects of 8 classes the census
    holds last between 15.19 and 16.9, `ItemRecommendationContextList` among them. Their 289
    fields were the whole of its `check.field-undeclared`.
  - one repair with an object of `0xbc13bcf6` in a skin bin, with no field, its entry hash its
    class hash. Neither the census history nor the meta database holds the class.

## Shader variants the cache lacks (2026-10-10)

`package.shader-variant-missing` (#GAME-BEHAVIOR-0029), against the 16.19.8207193 copy and the
history index of 757 builds in format 1, at 24 in parallel with one thread each.

- **The reduced corpus**, port and repair, with the check on `b011081` and again on `33d101e`,
  each compared with master at the same commit: the reports are equal in all 68 mods. No mod of
  the 68 holds a material whose variant the cache lacks. Against `baseline-port-5` and
  `baseline-repair-5` the reports differ only as the runs above have them. Port in 36 s, repair
  in 59 s, ended as the baselines.
- **The datasets**, with the check on `b011081`, not a baseline. The port: 4,870 done, 39 ended
  by a fatal diagnostic, those of `baseline-port-5` and 9 `package.shader-variant-missing`, in
  1,055 s. The repair: 4,875 done, 34 ended, those of `baseline-repair-5` and the same 9, in
  1,365 s. The 9 are the only mods whose exit differs from the baselines'.

| shader | file | mods | what the nearest variant the file holds differs in |
|---|---|---:|---|
| `Shaders/SkinnedMesh/Leona_Texture_Transition` | pixel | 7 | `OFF_VALUE_ON_TEXTURE=1`, a static switch the materials leave off |
| `Shaders/SkinnedMesh/TFT_FixedUVSpace_Bloom` | pixel | 2 | `SKINNED_MATERIAL_ADDITIVE=1`, a macro every material of Riot's on the shader sets |

## The nearest shader variant (2026-10-10)

The fixup `shader-variant` (#ADR-0092), on the working tree at `31a289a`, against the
16.19.8207193 copy and the history index of 757 builds in format 1, over five of the 9 mods
`package.shader-variant-missing` stopped: three Zed skins, the Mordekaiser and the Warwick
(`shader-variant-five-{port,repair}`).

- Port and repair: 5 done, 0 ended by a fatal diagnostic, each in 2 s. The fixup set
  `OFF_VALUE_ON_TEXTURE` on in the Zed materials' own `switches`, where each held it off (one
  material in two mods, two in the third), and added `SKINNED_MATERIAL_ADDITIVE=1` to the
  `shaderMacros` of the Mordekaiser's one material and the Warwick's two. No other fixup changed.
- Against `shader-variant-full-{port,repair}-after` the five differ in every section after the
  fixups: the earlier reports are those of a run the check ended.
- The reduced corpus holds no mod the check stops, so a run over it shows nothing of the fixup.

## What a blob store holds (2026-10-10)

The file rows of the 4,909 reports of `baseline-port-5`, read for #ADR-0093: each file's
size and SHA-256, the archive's bytes from the datasets on disk. No run was made.

| | datasets, 4,909 mods | reduced corpus, 68 mods |
|---|---:|---:|
| archive bytes | 69.7 GB, median 8.7 MB, largest 440 MB | 1.65 GB, median 11.0 MB, largest 328 MB |
| raw bytes, inflated and unpacked | 235.1 GB, median 28.0 MB, largest 948 MB | 5.73 GB, median 42.7 MB, largest 780 MB |
| files | 1,491,566, median 178, largest mod 6,550 | 34,774, median 252, largest mod 6,276 |
| distinct contents within each mod | 206.0 GB | 5.15 GB |
| distinct contents across the set | 130.4 GB in 417,503 ids | 5.03 GB in 19,317 ids |
| mods with another's content set | 246 in 119 groups | none |

The reduced corpus is chosen to cover cases, so the sharing between mods shows only over the
datasets: 45 % of the raw bytes.

## The blob store and the raw snapshot (2026-10-10)

The branch `run-state-store` at `b582589` (the blob store, #ADR-0093; the bytes as views; the
raw snapshot, #ADR-0094), against the 16.19.8207193 copy, at 24 in parallel with one thread
each, over the reduced corpus. The 757-build index of `baseline-port-5` and
`baseline-repair-5` was no longer on disk, so the runs use the app's own index of 758 builds
(head `d991890173fb`, 16.20.8248524), and the change is compared with its base commit
`ebdd720` (master's head) run over the same index (`raw-snapshot-base-{port,repair}`).

| run | command | done | ended by a fatal diagnostic | time | base commit's time |
|---|---|---:|---|---|---|
| `raw-snapshot-port` | `port --no-write` | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 34 s | 32 s |
| `raw-snapshot-repair` | `repair --no-write --again` | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 45 s | 48 s |

- **Against the base commit** (`raw-snapshot-{port,repair}-vs-base`): the reports are equal in
  all 68 mods, port and repair. The store and the snapshot are invisible in the output, as
  #ADR-0093 requires. The run's store is a memory store, and the entries' bytes reach it once
  per distinct content.
- **Against the baselines** (`raw-snapshot-{port,repair}-vs-baseline-5`): every mod differs in
  the history's head, the evidence's program and inputs, and the evidence's fixup list; the
  rest are the changes landed on master since `8cacfff` (the shader variant fixup among them)
  and the one build the index gained, which the base commit's run shows alike.
- **Time.** The difference of 2 s either way is within the noise of a run of 34 to 48 s; the
  one hash per file is now taken in the store's put, where the reader took it before.

## The baselines at ea2dd79 (2026-10-10)

Made again over the 4,909 mods in scope with the program at `ea2dd79`, master's head, against
the 16.19.8207193 copy and the app's history index of 758 builds in format 1 (head
`d991890173fb`, newest build 16.20.8248524), at 24 in parallel with one thread each. The
757-build index the baselines at `8cacfff` used is no longer on disk, so they are replaced
rather than extended: every later run is compared with these.

| baseline | command | done | ended by a fatal diagnostic | failed | time | differs from the one replaced |
|---|---|---:|---|---:|---|---:|
| `baseline-port-6` | `port --no-write` | 4,879 | 30: one `archive.no-skin`, 13 `package.empty`, 16 `package.shader-missing` | 0 | 922 s | 4,909 mods |
| `baseline-repair-6` | `repair --no-write --again` | 4,884 | 25: one `archive.no-skin`, 24 `package.shader-missing` | 0 | 1,197 s | 4,909 mods |

The exits are those of the baselines replaced, mod for mod. The comparison
(`baseline-6-vs-5-{port,repair}`) differs in every mod by the history's head and by the
evidence's program, inputs and list of fixups; the rest is the commits since `8cacfff` and the
index's one build, each as its own run recorded it:

- **The index's build** (16.20.8248524): `provenance` life and paths in 147 and 35 ports, the
  game's facts in 55, the reworks in 9, and `rework.restoration` in 2 of each.
- **A copy's own files** (#ADR-0088): the verdicts of the files a copy names in 156 ports
  (4,932 `refs` rows), and 25 ports that ship 261 own files their copies name, with their
  renames, chunks, package paths, what is left behind, `object.carried` and
  `package.texture-contained`.
- **The gear condition** and **the nearest shader variant** (#GAME-CHANGE-0027, #ADR-0092):
  `fixup.changed` and the fixups' kinds in 14 mods of each command, and in the repair the 101
  bins that copy the gear rule's object, edited in 5 mods.
- **The class of each struct** (#MOD-ARTIFACT-0039): `check.class-undeclared` in 231 repairs,
  `check.field-undeclared` gone from 198 and `check.done` counting fewer in 205;
  `check.type-mismatch` in 8 repairs and 5 ports, the gear condition's among them.

## The load snapshot (2026-10-10)

The branch `run-state-store` with the report's new inputs and the load snapshot (#ADR-0094),
against the 16.19.8207193 copy and the app's index of 758 builds (head `d991890173fb`), at 24
in parallel with one thread each, over the reduced corpus, compared with `baseline-port-6` and
`baseline-repair-6`. Every run writes its load snapshot at the end of `load` and resumes from
none; the resumed run is the test's (`tests/porter.rs`).

| run | command | done | ended by a fatal diagnostic | time | the raw snapshot's run |
|---|---|---:|---|---|---|
| `load-snapshot-port` | `port --no-write` | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 35 s | 34 s |
| `load-snapshot-repair` | `repair --no-write --again` | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 49 s | 45 s |

- **Against the baselines** (`load-snapshot-{port,repair}-vs-baseline-6`, expected:
  `run schema_build`, `run names`): the 68 mods of each command differ in the report's inputs
  alone, 136 differences each, all expected. The inputs name the schema's newest build,
  8248524, and the hash tables' stamp: the five tables of the hash cache at version
  2026-10-05, each by its manifest's SHA-256, and no text table. Nothing else differs, and
  the exits are the baselines', mod for mod.
- **Time.** The times are within a few seconds of the raw snapshot's runs over the same mods;
  another session worked in the same tree during the runs, so the 4 s of the repair are not
  read as the snapshot's cost.

## The snapshots through plan (2026-10-10)

The branch `run-state-store` at `63fe167`: the run's memory layer, and a snapshot after
`fix`, `facts`, `decide` and `plan` (#ADR-0094), with the report's `port` part typed. The
same inputs as "The load snapshot", over the reduced corpus, compared with `baseline-port-6`
and `baseline-repair-6`. Every run writes each snapshot at the end of its stage and resumes
from none.

| run | command | done | ended by a fatal diagnostic | time | the load snapshot's run |
|---|---|---:|---|---|---|
| `run-snapshots-port` | `port --no-write` | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 35 s | 35 s |
| `run-snapshots-repair` | `repair --no-write --again` | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 48 s | 49 s |

- **Against the baselines** (`run-snapshots-{port,repair}-vs-baseline-6`, expected as "The
  load snapshot"): the 68 mods of each command differ in the report's two new inputs alone,
  136 differences each, all expected. The typed `port` part writes what the JSON value
  wrote, and the exits are the baselines', mod for mod.
- **Resumed from each snapshot.** A program of the library's API, not kept, took each of the
  68 archives through a port and a repair straight, and again from each snapshot the run
  kept, each written as JSON and read back, its bytes flushed from the run's layer into a
  store: a port from the raw snapshot and after `load`, `fix`, `facts`, `decide` and `plan`,
  a repair from the raw snapshot and after `load` and `fix`. Every resumed run gives the
  report of the straight run, which holds the digest of every chunk of the package, in all
  68 mods. The mod `load` ends at `archive.no-skin` keeps the raw snapshot alone.
- **Time.** The times are the load snapshot's runs' within a second: the bins `fix` writes
  again, the facts and targets cloned, and the port's values written into the layer cost
  nothing a run of the reduced corpus shows.

## The snapshots through check (2026-10-10)

The branch `run-state-store` at `554b5aa`: a snapshot after `build` and after `check`
(#ADR-0094); a run folded over its snapshots, every stage ending by writing its own, a run
judged with an earlier run's evidence among them, which gives none out; the targets, the
inventory, the package's part of the report and what each patch came to read from their
snapshots; each stage's settings declared (`Settings::declared`, `earliest_changed`); and the
snapshots written and read at one JSON edge. The same inputs as "The load snapshot", over the
reduced corpus, compared with `baseline-port-6` and `baseline-repair-6`. Every run writes each
snapshot at the end of its stage and resumes from none.

| run | command | done | ended by a fatal diagnostic | time | the snapshots through plan |
|---|---|---:|---|---|---|
| `check-snapshot-port` | `port --no-write` | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 34 s | 35 s |
| `check-snapshot-repair` | `repair --no-write --again` | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 51 s | 48 s |

- **Against the baselines** (`check-snapshot-{port,repair}-vs-baseline-6`, expected as "The
  load snapshot"): the 68 mods of each command differ in the report's two new inputs alone,
  136 differences each, all expected. The exits are the baselines', mod for mod.
- **Resumed from each snapshot.** A program of the library's API, not kept, took each archive
  through a port and a repair to the end of `check`, wrote each snapshot as JSON and read it
  back, flushed the run's layer into a store, and resumed after every stage the run has.
  Every resumed run gives the report of the straight run: after `load`, `fix`, `build` and
  `check` in 128 runs (63 ports, 65 repairs), after `facts`, `decide` and `plan` in 63 ports.
- **Sizes.** The snapshots as JSON, of the 67 mods `read` does not end, and the time to read
  the largest of each stage back from its JSON, the least of five reads, one thread:

  | stage | port: mean | port: largest | read | repair: mean | repair: largest | read |
  |---|---:|---:|---:|---:|---:|---:|
  | load | 464 KB | 4.1 MB | 22 ms | 379 KB | 2.7 MB | 13 ms |
  | fix | 504 KB | 5.9 MB | 12 ms | 501 KB | 5.9 MB | 12 ms |
  | facts | 616 KB | 12.2 MB | 25 ms | | | |
  | decide | 94 KB | 1.0 MB | 0.7 ms | | | |
  | plan | 308 KB | 1.9 MB | 10 ms | | | |
  | build | 191 KB | 1.3 MB | 1.7 ms | 312 KB | 2.9 MB | 8.5 ms |
  | check | 111 KB | 1.1 MB | 0.7 ms | 85 KB | 1.0 MB | 0.5 ms |

  The build and check snapshots are of the mods that reach them, 63 ports and 65 repairs.
  The largest facts snapshot is the mod whose plan snapshot is the largest; the largest
  decide, build and check snapshots of a port are one mod's. The raw snapshot is not JSON
  and not measured.
- **Time.** The port is the run through plan's within a second. The repair is 3 s slower,
  and a check run of the same code before the settings and the JSON edge landed took 50 s:
  a repair's second run now keeps its snapshots as its state, the package's part of the
  report among them, which it did not before.

## A cancel and progress in steps (2026-10-10)

The branch `run-state-store` at `cc78439`: a stage reads the run's cancel before each of its
steps and reports each step to a progress listener (#ADR-0097), and the command sets the
cancel on the terminal's interrupt. The same inputs as "The load snapshot", over the reduced
corpus, compared with `baseline-port-6` and `baseline-repair-6`. No run is cancelled and no
listener is registered: the cost measured is the check of the flag, the count of the steps
and the call that finds no listener. The base commit `7c0e579` (the snapshots through check)
ran over the same mods in the same session, each command twice, alternated with the branch's
runs.

| run | command | done | ended by a fatal diagnostic | time | sum of the mods' times |
|---|---|---:|---|---|---|
| `cancel-port`, `cancel-port-2` | `port --no-write` | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 33 s, 33 s | 327.5 s, 323.9 s |
| `cancel-base-port`, `cancel-base-port-2` | the same, at `7c0e579` | 63 | the same | 33 s, 33 s | 323.2 s, 322.9 s |
| `cancel-repair`, `cancel-repair-2` | `repair --no-write --again` | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 50 s, 50 s | 320.8 s, 322.7 s |
| `cancel-base-repair`, `cancel-base-repair-2` | the same, at `7c0e579` | 65 | the same | 50 s, 50 s | 321.2 s, 321.3 s |

- **Against the baselines** (`cancel-{port,repair}-vs-baseline-6`, expected as "The load
  snapshot"): the 68 mods of each command differ in the report's two inputs alone, 136
  differences each, all expected. A run not cancelled writes no `cancelled` field, and the
  exits are the baselines', mod for mod.
- **Against the base commit** (`cancel-{port,repair}-vs-base`): the reports are equal in all
  68 mods of each command.
- **Time.** The runs take the base commit's wall time to the second, 33 s a port and 50 s a
  repair. The sums of the mods' own times differ by at most 4.6 s in 323 s, 1.4 %, and the
  base commit's two runs of a port differ from the branch's second by 1 s: the check and the
  progress call do not show above the run-to-run spread.

## A unit's own target and a file kept (2026-10-10)

The branch `run-state-store` at `a294f68`: a target given to one unit, read by `decide`, and a
stale copy kept by its WAD and path, read by `load`, `plan` and `build` (#ADR-0098), both
taken by the command. The same inputs as "The load snapshot", over the reduced corpus,
compared with `baseline-port-6` and `baseline-repair-6`. No run gives a unit a target or keeps
a file: the reports hold neither when none is given.

| run | command | done | ended by a fatal diagnostic | time |
|---|---|---:|---|---|
| `answers-port` | `port --no-write` | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 35 s |
| `answers-repair` | `repair --no-write --again` | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 52 s |

- **Against the baselines** (`answers-{port,repair}-vs-baseline-6`, expected as "The load
  snapshot"): the 68 mods of each command differ in the report's two inputs alone, 136
  differences each, all expected. The exits are the baselines', mod for mod.
- **Against the cancel runs** (`answers-{port,repair}-vs-cancel`, with `cancel-port` and
  `cancel-repair`): the reports are equal in all 68 mods of each command.
- **Time.** 35 s a port and 52 s a repair, within 2 s of the cancel runs' 33 s and 50 s.

## The raw snapshot as a cache (2026-10-10)

The branch `run-state-store` at `de0142b`: the ingest store (`cacache` 13.0.0, one file per
blob under its SHA-256), the raw snapshot as JSON, and a run that reads its archive through a
cache of raw snapshots (#ADR-0093, #ADR-0095), taken by the command as `--store`. The same
inputs as "The load snapshot", over the reduced corpus. Each command ran three ways: into a
fresh store under `data/corpus/` (the populating run), again with that store (the hitting
run), and with none; then the hitting run and the run with none once more, alternated. Each
command had a store of its own, so that each has a populating run.

| run | command | store | done | ended by a fatal diagnostic | time | sum of the mods' times |
|---|---|---|---:|---|---|---|
| `raw-cache-port-fill` | `port --no-write` | fresh | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 45 s | 607.8 s |
| `raw-cache-port-hit`, `-hit-2` | the same | populated | 63 | the same | 31 s, 31 s | 314.9 s, 313.2 s |
| `raw-cache-port`, `-2` | the same | none | 63 | the same | 31 s, 31 s | 313.4 s, 316.6 s |
| `raw-cache-repair-fill` | `repair --no-write --again` | fresh | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 61 s | 570.2 s |
| `raw-cache-repair-hit`, `-hit-2` | the same | populated | 65 | the same | 51 s, 48 s | 366.2 s, 304.6 s |
| `raw-cache-repair`, `-2` | the same | none | 65 | the same | 47 s, 47 s | 304.8 s, 304.9 s |

- **Against the baselines** (`raw-cache-{port,repair}{-fill,-hit,}-vs-baseline-6`, expected as
  "The load snapshot"): the 68 mods of each run differ in the report's two inputs alone, 136
  differences each, all expected. The exits are the baselines', mod for mod.
- **With each other** (`raw-cache-{port,repair}-{fill,hit}-vs-memory`,
  `raw-cache-{port,repair}-hit-vs-fill`): equal in all 68 mods of each command, and the
  report files of the populating and the hitting run are the run's with no store byte for
  byte. The cache is invisible in the output, as #ADR-0095 requires. That a hitting run reads
  the entries was checked on one archive: a mod name written into its entry by hand is the
  name the next run's report holds.
- **The store after a populating run**, the same for either command: 19,433 files of
  5,136,424,701 bytes (5,073,340 KiB allocated, as `du` counts it), in 16,957 directories.
  19,365 are blobs, 5,133,110,290 bytes: 19,346 distinct contents of the WADs' files, 12 of
  the extras' first bytes and 7 images. 68 are entries, one per archive, 3,314,411 bytes, the
  largest 411,345. The two-level fanout of `cacache` makes nearly a directory per blob at
  this count.
- **Time.** Populating costs 14 s of wall time over a port and over a repair, and nearly
  doubles the mods' summed time: the cost of one file per blob on Windows that #ADR-0093
  names, a create, a write and a rename per blob. A hitting run takes the time of a run with
  no store: summed, 313 to 317 s a port and 305 s a repair. The first repair after its store
  was populated took 61 s more summed, and the second the time of a run with none.
  `reforge load` over the corpus (`raw-cache-load{,-hit}{,-2}`) takes 15 s and 206 to 209 s
  summed with the store and without. On the largest archive of the corpus (328 MB), one
  process, `reforge load` takes 4.2 s with no store, 17.3 s populating and 3.9 to 4.1 s
  hitting. Reading a mod's blobs back, each checked against its hash, costs nearly what
  inflating and unpacking the archive does, and both are small beside opening the inputs and
  the stages after `read`.

## A read streamed into the store (2026-10-10)

The branch `run-state-store` at `db3091e`: an archive file read entry by entry, a packed WAD
stored as it is mounted over its part of the file, a deflated one inflated once into the
store's spool, each chunk put into the store as it is read, and a cap on one chunk of a packed
WAD (#ADR-0100). The base is master's head `e61b209`, built in a worktree of its own. The same
inputs as "The load snapshot". Peak memory is the process's peak working set, read from its
handle after it exits.

- **The chunk cap.** Over the 1,491,566 file rows of `baseline-port-6`, the largest file is
  304,352,832 bytes, ten rows are over 256 MiB and none is over 512 MiB. The cap is 512 MiB,
  that size rounded up to a power of two.
- **One mod of 256 MB with three packed WADs, one a map's**, all three stored as they are
  (121.9 MB, 125.1 MB and 8.5 MB), whose 1,748 chunks are 88,224,835 bytes at the largest.
  Each run twice, one process at a time; the two runs agree within 2 MiB. `read` alone is a
  program of the library's API, not kept, that calls `archive::read_file` into the store.

  | what | store | base: peak | base: time | branch: peak | branch: time |
  |---|---|---:|---:|---:|---:|
  | `port --no-write` | none | 921 MiB | 3.7 to 3.9 s | 923 MiB | 3.7 to 4.0 s |
  | `port --no-write` | fresh | 979 MiB | 8.2 to 8.8 s | 979 MiB | 8.0 to 8.4 s |
  | `port --no-write` | populated | 979 MiB | 3.7 to 3.9 s | 979 MiB | 3.9 to 4.3 s |
  | `read` alone | none | 862 MiB | 0.7 s | 430 MiB | 0.7 s |
  | `read` alone | fresh | 862 MiB | 2.0 s | 174 MiB | 2.0 s |

  `read` alone holds the archive and the WAD it inflates at the base; on the branch it holds
  the files' bytes in a store in memory, and with a store on disk two copies of the largest
  chunk, the buffer `ltk_wad` decompresses it into and the view the store is given. A port's
  peak is `load`'s and the stages' after it, which hold every file's bytes until `write`, so
  it does not move until a file node holds an id (#ADR-0100). The reports of the eight port
  runs are equal byte for byte.
- **The reduced corpus**, each command without a store and with a fresh one:

  | run | command | store | done | ended by a fatal diagnostic | time |
  |---|---|---|---:|---|---|
  | `stream-port`, `-2` | `port --no-write` | none | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 33 s, 32 s |
  | `stream-port-store` | the same | fresh | 63 | the same | 47 s |
  | `stream-repair`, `-2` | `repair --no-write --again` | none | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 55 s, 49 s |
  | `stream-repair-store` | the same | fresh | 65 | the same | 69 s |

  Against the baselines (`stream-{port,repair}{,-store}-vs-baseline-6`, expected as "The load
  snapshot"), the 68 mods of each run differ in the report's two inputs alone, 136
  differences each, all expected, and the exits are the baselines', mod for mod. With each
  other (`stream-{port,repair}-store-vs-memory`) and with the cache's runs at `de0142b`
  (`stream-{port,repair}-vs-cache`), equal in all 68 mods; the report files with a store and
  without are equal byte for byte. The store after a populating run holds 19,365 blobs of
  5,133,110,290 bytes, as the cache's did, and no scratch file. The times are those of the
  cache's runs, 31 s and 45 s a port and 47 s and 61 s a repair, within the spread of two
  runs.

## A file node by id (2026-10-10)

The branch `run-state-store` at `322e8ce`: a file node holds the content id of its bytes and a
stage fetches them where it reads them, a carried file named by its id in the package and
fetched by the writer, a cap on what a stage holds at once, and a run from the store by the
archive's SHA-256 (#ADR-0100). The same inputs as "The load snapshot". Peak memory is
measured as in "A read streamed into the store".

- **One mod of 256 MB with three packed WADs, one a map's**, `port --no-write`, each run
  twice, one process at a time; the two runs agree within 2 MiB. The before is the branch at
  `db3091e`, from "A read streamed into the store". The reports of the six runs are equal
  byte for byte.

  | store | before: peak | after: peak | after: time |
  |---|---:|---:|---:|
  | none | 923 MiB | 917 to 919 MiB | 3.9 to 4.4 s |
  | fresh | 979 MiB | 559 to 560 MiB | 8.2 to 8.8 s |
  | populated | 979 MiB | 559 MiB | 3.8 to 4.9 s |

  With a store the files' bytes leave memory once each stage has read them; the rest of the
  peak is the environment, the parsed bins and what the stages make. A run with no store
  holds every file in its memory store, as #ADR-0100 says.
- **The bytes each stage fetched over the reduced corpus**, from a counting store under the
  run's layer, in a program of the library's API, not kept, that reads each archive into a
  store in memory and starts the run from its raw snapshot. A stage a fatal diagnostic ended
  is not counted. The 68 archives hold 19,365 distinct blobs of 5,133,110,290 bytes.

  | stage | port: fetches | port: bytes | repair: fetches | repair: bytes |
  |---|---:|---:|---:|---:|
  | load | 65,683 | 13,422,376,110 | 38,552 | 7,689,454,447 |
  | fix | 1,695 | 465,850,674 | 1,695 | 465,850,674 |
  | facts | 1,795 | 665,841,380 | | |
  | decide | 0 | 0 | | |
  | plan | 28 | 154,466,407 | | |
  | build | 4,624 | 1,414,782,304 | 5,845 | 1,557,136,131 |
  | check | 0 | 0 | 239 | 20,807,636 |

  `load` reads every file once for its kind and, in a port, once more for the inventory,
  besides the bins it parses: its fetches are a whole file each for a few bytes of it. The
  most a stage held at once is its largest file read: 157,525,890 bytes in `load`,
  32,786,671 in `facts` and `plan`, 22,369,776 in `build`, 4,728,210 in `fix` and 1,962,531
  in `check` (`held-port`, `held-repair`, a build with no working-set cap, at `13cca10`'s
  code). The cap is 1 GiB.
- **The reduced corpus**, each command without a store from the archives (`final-port`,
  `final-repair`), into a fresh store (`hash-port-store`, `hash-repair-store`), and from that
  store by the archive's SHA-256 (`hash-port-from`, `hash-repair-from`, `--from-store`):

  | run | command | store | done | ended by a fatal diagnostic | time |
  |---|---|---|---:|---|---|
  | `final-port` | `port --no-write` | none | 63 | 5: one `archive.no-skin`, 3 `package.empty`, one `package.shader-missing` | 33 s |
  | `hash-port-store` | the same | fresh | 63 | the same | 57 s |
  | `hash-port-from` | the same | populated, by hash | 63 | the same | 43 s |
  | `final-repair` | `repair --no-write --again` | none | 65 | 3: one `archive.no-skin`, 2 `package.shader-missing` | 48 s |
  | `hash-repair-store` | the same | fresh | 65 | the same | 66 s |
  | `hash-repair-from` | the same | populated, by hash | 65 | the same | 57 s |

  Against the baselines (`*-vs-baseline-6`, expected as "The load snapshot"), the 68 mods of
  each run differ in the report's two inputs alone, 136 differences each, all expected, and
  the exits are the baselines', mod for mod. With each other, the report files without a
  store, with a fresh one and from the store by hash are equal byte for byte. The store after
  the populating run holds the 19,365 blobs and no scratch file.
