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
| the app | a standalone desktop app (#ADR-0046), released as v0.1.4 (2026-10-09), with the history index distributed prebuilt (#ADR-0047) and holding every object's bytes (#ADR-0078) | [ROADMAP-UX.md](ROADMAP-UX.md) |

The stages were built as a rewrite of ltk-forge and each was compared with it over the 5,137
mods of the datasets. The last comparison is the record of parity ([RUNS.md](RUNS.md), "The
port").

## The fixer: left before a first release

The fixer repairs 4,884 of the 4,909 mods of the datasets in scope ([RUNS.md](RUNS.md), "The
baselines at 8cacfff"). One holds no skin, and 24 end with no package: a material of theirs
links a shader Riot removed with no relink (#ADR-0069). A repaired mod repaired again changes
nothing ([RUNS.md](RUNS.md), "The baselines and the edge cases"). Eight
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

1. **The behaviour carried over from ltk-forge**: done. Each of its thirteen items was measured,
   then kept with the measurement or changed as a rule of its own ([RUNS.md](RUNS.md), "The
   behaviour carried over from ltk-forge"). What remains is under
   [Carried over](#carried-over-from-ltk-forge).
2. **Ports checked in game** where none was: done. A voice layer, a `hud` layer and a bank unit
   cut from the mod's own events bank were checked and work, the last with the mod's own buses
   carried (#ADR-0064) and the game unit it left out left out (#ADR-0063)
   ([RUNS.md](RUNS.md), "Ports in game").
3. **A game source that answers from the history**: not built. The target install stays
   required: it answers what the target build holds, and the index what builds held
   (#ADR-0080).
4. **The author's change taken against the game at the mod's own build**, applied onto the
   target build (#ADR-0080). `Base` is where the comparison says which build. A sub-character's
   game value of the mod's build, the health bar style among them, is handled here (#ADR-0067).
5. **What ltk-forge held for later**: sounds over the game's (#GAME-BEHAVIOR-0016); a name
   made to hash to a field no table names (#ADR-0031); event names without a list; the
   history's references and entry texts; mip chains; material overrides that name no submesh.

## After

- What the app's plan holds beyond its release: [ROADMAP-UX.md](ROADMAP-UX.md).
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
- How a port names a file whose name holds a variant tag of the game beyond the four shapes
  of #MOD-ARTIFACT-0026: the port's baseline ships 23,753 such names in 959 mods, 62% of them
  bytes the game shipped. A pass that drops every tag the history knows, or one parameterised
  by the shape of a class of names, is to be explored (the user, 2026-10-06).
- Whether a sub-character's copies (#ADR-0083) come from its own skins' bins alone, as built, or
  from its champion's other skins' bins as well. The walk loads the champion's bins at the
  target beside the sub-character's, so only a name the champion holds for another skin alone
  is at stake; 103 `object.link-dangling` findings in 88 ports are on sub-characters
  (`baseline-port-5`), and which of their names the champion's other skins hold is not
  measured.
- *fixer* Whether `vfx-texture-mult` moves `uvScrollAlphaMult` and `isRandomStartFrameMult`,
  which the client does not read (#GAME-BEHAVIOR-0027), or removes them with nothing lost, as
  #ADR-0068 removes what a complex emitter does not read. The same for an old simple emitter's
  linger fields, which `vfx-linger` moves into a `Linger` a simple emitter does not read
  (#GAME-BEHAVIOR-0024). Either way the emitter draws the same.

### Carried over from ltk-forge

Behaviour reforge has because ltk-forge had it and the comparison held reforge to it. No
record and no measurement of reforge's own says it is right.

- A skin is looked for up to slot 120 (`MAX_SLOT`), ltk-forge's value. Champions reach 116 at
  16.19, and the `jade_*` characters, which no group takes (#ADR-0071), have skin bins up to 305.
- A file under 4,096 bytes and an object of fewer than 4 properties do not date a mod
  (`MIN_FILE`, `MIN_PROPERTIES`): ltk-forge's thresholds, with a reason and no measurement
  behind the values.

### Known limits

*fixer* marks the ones a repair shows.

- *fixer* An emitter's vector written as a float, in mods that restore old effects, has no
  conversion (#MOD-ARTIFACT-0036, 36 mods): no build of the census history holds one, so no
  conversion of Riot's is read. `retype` leaves the value, and the client drops it.
- *fixer* `retype` takes a value for one that never showed only when its kind differs from
  the build's. An embed or a pointer of another class than the build reads is left, where the
  mod's own build did not read it either (#ADR-0065): the floats of #MOD-ARTIFACT-0036, in mods
  dated 15.10 or later, among them.
- *fixer* A mod made before 13.24 holds no `0x6aec9e7a` on its camera-aligned meshes, which
  Riot's conversion set on every aligned `VfxPrimitiveMesh` of the game (#GAME-CHANGE-0025). No
  fixup sets it. What the flag changes on screen is not checked in game.
- A child particle set names its system by `effect`, a link, or by `effectKey`, a hash a
  resolver's `resourceMap` maps (LTK Manager's VFX renderer plan, section 2.33). Whether each key
  a port carries still resolves in the resolvers it writes is not checked.
- *fixer* A mod's own `CustomShaderDef` is kept, and the client crashes at the first draw of a
  material that uses it (#GAME-BEHAVIOR-0022). No mod of the datasets defines one.
- *fixer* A material on a game shader whose switches and macros ask for a variant the shader
  cache lacks is packaged, and the client crashes at its first draw (#GAME-BEHAVIOR-0029). No
  check reads the cache, and how many mods of the datasets hold one is not counted.
- *fixer* A repair keeps a skin bin's link to a shared bin at its path before 16.10, which the
  client skips (#GAME-CHANGE-0012). Riot's bins link the `_multi_` path from 16.10.
- *fixer* A repair ships a mod's champion root bin whole, with the champion's gameplay data of
  the mod's build (#MOD-ARTIFACT-0032), in 259 mods of the datasets.
- *fixer* 40 game paths the hash cache spells hold an empty part (`//`). `ChunkPath` refuses
  them, so such a chunk stays unnamed.
- *fixer* A `ChunkPath` may be longer than a file name on disk: 1,264 game paths are over 512
  bytes. A writer of a project directory names such a chunk by its hash.
- An event cut from a mod's own events bank can hold the id of an event of a game bank the
  block keeps, and the report counts shared ids against the game's bank at the unit's events
  path alone. Which of two effects banks defining one id the client keeps is not established
  (#GAME-BEHAVIOR-0012). One mod of the datasets carries 26 such events (#ADR-0075).
- 84 sound units have an events bank of version 134 that plays their clips, and nothing
  converts a bank from 134 to 145.
- The three earliest reworks of #GAME-CHANGE-0001 (Mordekaiser, Pantheon, Fiddlesticks) are
  known by their joint names alone. Their submeshes and dropped paths are not measured.
- A port to skin0 can name a premium skin of the champion in a value a check of the base skin
  reads (#ADR-0062), in three cases `check.other-skin` reports: a game file in a premium
  skin's folder that is no skin's mesh or skeleton (4 mods of the datasets, 2026-10-05, the
  old base model in skin 5's folder), a unit that fits no slot (#ADR-0016), and a target
  forced to the base. Whether one of the six values is enough for that check to refuse a mod,
  or two are needed, is not established (the user recalls two, 2026-10-05).
- A character a champion's skins preload is its sub-character when the champion's WAD ships its
  skin bins (#ADR-0071), even under another champion's name, and it follows the champion's
  target. One Xayah mod of the datasets (`8afd81d86710`) ships `taliyahwallchunk` bins in
  Xayah's WAD, likely a tool's copy of the game's at the mod's build, and the port patches
  Taliyah's wall chunk at skin4, Xayah's target, which plays with Taliyah's skin4: a wrong patch
  shipped. Telling such a copy from the author's work is the game at the mod's own build
  ([The porter: next](#the-porter-next), item 4); until then, a sub-character of another
  champion's name could follow nothing and be left to the game with a finding.
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
| #MOD-ARTIFACT-0036, emitter vectors as floats in restored old effects | `retype`, which leaves such a value as it is |
| #GAME-CHANGE-0025, a flag on camera-aligned meshes | nothing: no fixup sets the flag ([Known limits](#known-limits)) |
| #GAME-BEHAVIOR-0016, the game's event plays beside a mod's event of the same hook | nothing: sounds over the game's are held for later ([The porter: next](#the-porter-next), item 5) |
| #GAME-BEHAVIOR-0026, Riot's bins hold a field only where it differs from its default | `strip` and `retype`, which judge a field by the schema and not by whether a game object holds it |
| #GAME-BEHAVIOR-0027, some declared VFX fields are loaded and not read | nothing: `vfx-texture-mult` moves them ([Decisions not taken](#decisions-not-taken), [Known limits](#known-limits)) |
| #MOD-ARTIFACT-0005, banks added with events of their own | the port's audio, which cuts a unit's events from the mod's own events bank (#ADR-0030) |
| #MOD-ARTIFACT-0014, the game's own skins shipped beside the mod's | the port's plan, which leaves such a unit to the game's bin (`unit.left`, #ADR-0020, #ADR-0082) |
| #MOD-ARTIFACT-0015, bins as text or JSON at a bin's path | the inventory, which reports `inventory.bin-text` (#ADR-0066) |
| #MOD-ARTIFACT-0027, a voice stub of an older bank version | the port's audio, which writes every bank with a header of the client's version |
| #MOD-ARTIFACT-0030, a skin bin split into concat and StaticMat bins | nothing: the copies in them are judged as #MOD-ARTIFACT-0001 and #MOD-ARTIFACT-0029 are |
| #MOD-ARTIFACT-0032, a champion's root bin shipped whole | nothing: a repair ships it whole ([Known limits](#known-limits)) |
| #MOD-ARTIFACT-0033, effects moved into a champion's vfx bin | nothing: the objects load as the skin's own from a bin of another name |
| #MOD-ARTIFACT-0034, transparent placeholders at missing texture paths | nothing: no pass takes a placeholder off or points its reference back at the game's path |
| #MOD-ARTIFACT-0035, buses in a mod's media bank | the port's audio, which carries the buses the game's init bank does not define (#ADR-0064) |
| #MOD-ARTIFACT-0037, chroma VFX packs | nothing in `crates/`: the class table `scripts/corpus/corpus.py` reads classes them apart (#ADR-0085) |
| #MOD-ARTIFACT-0038, restorations of a champion's older content | nothing in `crates/`: the class table `scripts/corpus/corpus.py` reads classes them apart (#ADR-0086) |
| #GAME-BEHAVIOR-0029, a material whose shader variant the cache lacks crashes at its draw | nothing: no check reads the shader cache ([Known limits](#known-limits)) |
