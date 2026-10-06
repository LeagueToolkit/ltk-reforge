# Roadmap

What is built, what is left before a first release of the fixer, what comes next for the
porter, and what is open. The design is in [ARCHITECTURE.md](ARCHITECTURE.md). A decision is
an ADR in `docs/adr/`. A measurement is in [RUNS.md](RUNS.md). The edge cases, with how many
mods of the corpus show each, are in [CASES.md](CASES.md).

## Rules

- **reforge is compared with its own baseline** (#ADR-0044). A change of behaviour is measured
  as the difference between the baseline and a run with the change. Each difference is meant,
  or it is a fault.
- **One rule at a time, with its test** (#ADR-0045). A change is made to one rule, its test
  changes with it, and the run over the reduced corpus shows what else moved. The full
  datasets are run for a new baseline only.
- **A fault found in game becomes a fixture and a test before it is fixed** (#ADR-0045).
- **Records before code.** A change writes the records its code cites, then the code.
- **A change of behaviour that is meant is an ADR.**
- **Not every mod is repaired or ported** (the user, 2026-10-04). A mod that cannot be done
  correctly ends with a finding that says why, and no package. How many mods a change stops
  is a result to look at, and no fault by itself.

## Built

Two programs on one set of stages. The **fixer** (`reforge repair`) reads a mod, fixes its
files and values for the current build and writes it back in place. The **porter**
(`reforge port`) goes on from the fixed mod: it decides which skin each unit is for, and writes
each as a patch of the game's bin with the mod's own objects and files under a namespace.

| part | holds | shown by |
|---|---|---|
| inputs | the archive reader, the install, the hash tables, the schema, the census reader and the history index | [RUNS.md](RUNS.md), "The history index", "Clip joints and events banks in the history index" |
| load | file fixups, binless mods, WAD roles, characters, units, what the client loads, dating, inventory, file provenance; `reforge load` | "The load stage" |
| fix | the fixup engine, rules as data, the selection, every built-in fixup; `reforge fixups` | "The repair" |
| package, in place | what is wired up, the round-trip and type checks, the project directory and the `.modpkg`; `reforge repair` | "The repair, of what is wired up" |
| facts and the decision | copies, provenance, origin, rig, clips, reworks, sounds; the policy and its targets; `reforge inspect` | "Facts and the decision" |
| the port | what ships, reach, file references, layout, renames, audio, patches, voice and HUD layers, the apply check; `reforge port` | "The port", "Ports in game" |
| the run | `Porter`, `Repairer` and `Run`, which the commands and any other caller drive the stages through | "The commands on a run" |
| the tests | a test for each edge case and the guard that holds the list (#ADR-0045); the corpus scripts and the baseline (#ADR-0044) | [CASES.md](CASES.md) |

The stages were built as a rewrite of ltk-forge and each was compared with it over the 5,137
mods of the datasets. The last comparison is the record of parity ([RUNS.md](RUNS.md), "The
port").

## The fixer: left before a first release

The fixer repairs the 5,137 mods of the datasets but one that holds no skin, and a repaired mod
repaired again changes nothing ([RUNS.md](RUNS.md), "The baselines and the edge cases"). Eight
repaired mods were checked in game by the user and work ([RUNS.md](RUNS.md), "Ports in game").
Every run embeds its evidence in the package it writes (#ADR-0041,
[ARCHITECTURE.md](ARCHITECTURE.md), "Package").

Left:

1. **The two faults of the crates reforge reads and packs with** ([Upstream](#upstream)). They
   are reported to the crates, and fixed there.
2. **The reader of the embedded evidence**, later (the user, 2026-10-04): a run that takes a
   package's evidence in place of judging the package's bytes. A package repaired again from
   its own bytes dates differently in about half the mods and judges a restoration's old
   files stale. `reforge repair --again` carries the date and the restorations in memory until
   then. The reader settles how a run is given a package, what bounds a date or a restoration
   the package's bytes cannot support, and what a run judged with an earlier run's evidence
   embeds.

What a repair leaves as it is, or could do better, is a fixup to add later (the user,
2026-10-04), and holds up no release: the items marked *fixer* under
[Decisions not taken](#decisions-not-taken) and [Known limits](#known-limits). Each is one
rule, added with its record and its test.

## The porter: next

In this order. Each item is one or more changes measured against the baseline.

1. **The behaviour carried over from ltk-forge** ([Carried over](#carried-over-from-ltk-forge)),
   each item changed or removed as a change of its own.
2. **Ports checked in game** where none was: done. A voice layer, a `hud` layer and a bank unit
   cut from the mod's own events bank were checked and work, the last with the mod's own buses
   carried (#ADR-0064) and the game unit it left out left out (#ADR-0063)
   ([RUNS.md](RUNS.md), "Ports in game").
3. **A game source that answers from the history**: a run with no install, and against the
   game at any build the history holds (#ADR-0042, #ADR-0043). The index holds what the typed
   questions need but vertex positions.
4. **The author's change taken against the game at the mod's own build**, applied onto the
   target build (#ADR-0043). `Base` is where the comparison says which build. A sub-character's
   game value of the mod's build, the health bar style among them, is handled here (#ADR-0067).
5. **What ltk-forge held for later**: sounds over the game's (#GAME-BEHAVIOR-0016); a name
   made to hash to a field no table names (#ADR-0031); event names without a list; the
   history's references and entry texts; mip chains; material overrides that name no submesh.

## After

- The first release, a standalone app (#ADR-0046), with the history index distributed
  (#ADR-0047): its plan is [ROADMAP-UX.md](ROADMAP-UX.md).
- The test realm's builds in the history.
- A shared schema crate with LTK Manager.

## Open

### Decisions not taken

Each is the user's. *fixer* marks the ones a repair depends on.

- *fixer* An object of a class the build dropped is left as it is
  (`ItemRecommendationContextList`, gone at 16.6, in 227 mods' root bins). Whether it is
  removed.
- *fixer* A list whose items have no conversion is left as it is, where any other value with
  no conversion that the mod's own build did not read is removed. Which of the two a list
  gets.
- Whether a unit that fits no slot is written at all. It is written at skin0 with an error
  (#ADR-0016).
- Whether a mod whose every unit left the port is a fatal finding (`package.empty`, as built)
  or a report with no package.
- The resolver keeps the game's name (tried in game, 2026-09-30; #ADR-0006 rests on it). A
  policy with no decision behind it.
- The game's sounds stay and the mod's units come after them. A policy with no decision behind
  it.
- `Run` checks the order of the stages when it runs, where the design had the compiler hold it
  ([ARCHITECTURE.md](ARCHITECTURE.md), "Shape"). Whether that stays.

### Carried over from ltk-forge

Behaviour reforge has because ltk-forge had it and the comparison held reforge to it. No
record and no measurement of reforge's own says it is right.

- A repath tool's `.tft_set<N>` tag stays in a ported file's name (#MOD-ARTIFACT-0026): the
  rule takes `.tftset<N>`, which no mod was seen to carry. Dropping the tag changes the paths
  a port ships.
- Two units of one character on one slot are ranked by a key that ends with the source slot,
  so two units never rank the same and `target.conflict` is never raised.
- The joints of a game skin's skeleton are asked only for the slots a unit was compared with.
  At another slot, a forced target, the check "the slot's joints" cannot hold.
- The names a retarget moves from a unit's slot to its target are replaced in every object of
  the mod, not only in the objects that unit's group loads.
- What a unit reaches is reported (`object.link-dangling`, `object.unreached`,
  `object.game-carried`, `ref.dangling`, `ref.unmounted`) for every representative unit with
  a target, the ones that then leave the port included.
- A bank of a locale WAD goes to that locale's voice layer whether or not its unit is voice.
- A locale WAD's copy of a bank whose own clips are not those of the champion WAD's copy is
  not carried (`audio.not-carried`).
- Of a unit's own events banks of the client's version, only the first is read. 64 units in
  18 mods have a second that plays their clips ([RUNS.md](RUNS.md), "Sound units of a mod's
  own").
- A champion's sub-characters are taken to be the directories in its WAD whose names start
  with the champion's name. No count over the game's characters backs it.
- Four outcomes no run reaches, each tested with an input made by hand: `target.conflict`, a
  unit with no decision, and the two reasons the build has for not writing a unit's audio
  (the game's entry has no bank unit to model one on; a skin bin written whole takes no
  layer).
- `unit.new-bin` is raised with the decision. The build says in `unit.ported` that the bin is
  written whole.
- The text of a rig check's reason (`Why`) is ltk-forge's wording of the same check.
- The bounds a run reads under are ltk-forge's values: the archive's caps, 64 bins followed
  from one root, 16 bins read of one game skin, 64 chunks of a file's size read to find its
  bytes under another path. One has a measurement behind it: the cap on a packed WAD's chunks
  is above the 200,000 the game's largest WADs hold.

### Known limits

*fixer* marks the ones a repair shows.

- *fixer* A list of floats where the build reads a list of vectors, and the reverse, has no
  conversion (46 mods).
- *fixer* A material keeps a link to a shader Riot removed, and the client crashes at load
  (#GAME-CHANGE-0024, #GAME-BEHAVIOR-0023). Riot's relinks are read, and Riot rewrote the
  material with its link, so the link alone is no conversion.
- *fixer* A mod's own `CustomShaderDef` is kept, and the client crashes at the first draw of a
  material that uses it (#GAME-BEHAVIOR-0022).
- *fixer* A repair keeps a skin bin's link to a shared bin at its path before 16.10, which the
  client skips (#GAME-CHANGE-0012). Riot's bins link the `_multi_` path from 16.10.
- *fixer* A repair ships a mod's champion root bin whole, with the champion's gameplay data of
  the mod's build (#MOD-ARTIFACT-0032), in 259 mods of the datasets.
- *fixer* A birth translation in a `VfxShapeLegacy` is stripped in the 206 mods dated 14.1 or
  later (276,644 values), where the mod's own build never read the old `shape`.
- *fixer* Of two entries of one hash in a bin the client keeps the first (#GAME-BEHAVIOR-0020),
  and the bin reader keeps the last. 128 mods of the datasets hold such a bin.
- *fixer* 40 game paths the hash cache spells hold an empty part (`//`). `ChunkPath` refuses
  them, so such a chunk stays unnamed.
- *fixer* A `ChunkPath` may be longer than a file name on disk: 1,264 game paths are over 512
  bytes. A writer of a project directory names such a chunk by its hash.
- 84 sound units have an events bank of version 134 that plays their clips, and nothing
  converts a bank from 134 to 145.
- The history holds no vertex positions, so a source backed by it does not answer which
  submeshes of an own mesh are copied from another skin (#ADR-0043). No digest of rounded
  positions is asked of the census (the user, 2026-10-04): a rounding rule frozen into the
  history serves one check, and stays in the data when the rule proves wrong, where an exact
  hash keeps its use.
- The three earliest reworks of #GAME-CHANGE-0001 (Mordekaiser, Pantheon, Fiddlesticks) are
  known by their joint names alone. Their submeshes and dropped paths are not measured.
- A port to skin0 can name a premium skin of the champion in a value a check of the base skin
  reads (#ADR-0062), in three cases `check.other-skin` reports: a game file in a premium
  skin's folder that is no skin's mesh or skeleton (4 mods of the datasets, 2026-10-05, the
  old base model in skin 5's folder), a unit that fits no slot (#ADR-0016), and a target
  forced to the base. Whether one of the six values is enough for that check to refuse a mod,
  or two are needed, is not established (the user recalls two, 2026-10-05).
- A HUD edit on a path with a field no table names is left out (#ADR-0031 is not built).
- The condition under which a block-compressed texture not in whole blocks crashes the client
  is not established (#GAME-BEHAVIOR-0018).

### Upstream

- `ltk_wad` reserves a chunk's declared size while it mounts a WAD with sub-chunks. A packed
  WAD of 334 bytes that declares a chunk of 256 MiB makes the reader ask for 256 MiB, and a
  size the machine cannot give aborts the process. An ignored test holds the case.
- The `.modpkg` two runs write of one mod has the same length and other bytes
  ([RUNS.md](RUNS.md), "The commands on a run"). The project directories are equal. The file
  is packed by `ltk_mod_project`.

### Records no code cites

Each names what the code rests on, and the citation goes in with the next change to that
code.

| record | the code that rests on it |
|---|---|
| #GAME-CHANGE-0022, integer fields in narrower kinds | the narrowing conversion of `retype` |
| #GAME-BEHAVIOR-0018, a block-compressed texture not in whole blocks is drawn | `TexContainer`, which leaves such a texture the DDS it is |
| #GAME-BEHAVIOR-0019, a graph event does not look in the additional resolvers | `PortBuilder`, which writes the resolver's map whole under the game's name |
| #GAME-BEHAVIOR-0020, of two entries of one hash the client keeps the first | the bin reader, which keeps the last, and `bin.entry-duplicate` |
