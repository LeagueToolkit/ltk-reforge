# Roadmap

The order of work for the rewrite of ltk-forge, the decisions it waits on, and the records to
carry over. The design is in [ARCHITECTURE.md](ARCHITECTURE.md), the review it answers in
[REVIEW.md](REVIEW.md). Proposed 2026-10-03.

## Rules of the rewrite

- **forge is the oracle** (#ADR-0004). forge ports 5,136 of 5,137 mods in scope and its output
  is confirmed in game. Each step is compared with forge over the corpus: unit fates and
  targets, diagnostics, package contents.
- **The output need not be the same.** A step ends when the deviation is small and each kind of
  difference has been looked at. A change of behaviour that is meant is an ADR. A small
  deviation is noted with the run that measured it.
- **The comparison is built in forge.** forge is changed directly for it: the corpus scripts,
  the table of codes ([CODES.md](CODES.md)), whatever a report needs to be compared.
- **Parity, then change.** What forge held for later (forged field names, the history's refs,
  mip chains) starts after the last comparison step.
- **Records before code.** A step writes the records its code cites, then the code. Evidence
  moves from forge's module comments and design docs into the record.
- **Bottom up, with an early way out.** The model and the inputs first. `repair` is the first
  run that writes a package, so the output side is exercised before the port exists.

## Decisions

Taken on 2026-10-03, where no other date is given.

| decision | record |
|---|---|
| The stages are types, as modules of one crate | #ADR-0001 |
| Every loaded object is fixed, and the set stays an argument | #ADR-0002 |
| A diagnostic does not stop the write; a caller's level may | #ADR-0003 |
| forge is the oracle; the comparison is built in forge; a small deviation is accepted | #ADR-0004 |
| The census reader is carried over as a crate of its own; the index starts at format 0 | #ADR-0005 |
| The pass list, stub passes and the own resolver are not carried over | #ADR-0006 |
| Fixups run by phase, then by build | #ADR-0007 |
| A diagnostic code names what the finding is about | #ADR-0008 |
| The crates and the command are named reforge | #ADR-0009 |
| A history key is the head of a SHA-256 | #ADR-0037 |
| Of two reduced copies of a texture the larger is taken (2026-10-04) | #ADR-0038 |
| A package ships only what is wired up (2026-10-04) | #ADR-0039 |
| Of several files for one asset, the mod's own bytes win (2026-10-04) | #ADR-0040 |
| A package embeds the evidence of the run that made it (2026-10-04) | #ADR-0041 |
| The census history is read from its `history-v2` branch (2026-10-04) | #ADR-0042 |
| How the game client behaves is a fourth series of records, `docs/game-behavior/` | `AGENTS.md`, `docs/agents/records.md` |
| A GUI is a caller of the library: fixups and checks are listed, turned on and off, and added as data where a table says it all | [ARCHITECTURE.md](ARCHITECTURE.md), "Callers" and "Fixups" |

## Steps

Each step lists what is built, the records it writes, and its exit.

### 0. Ground

Done 2026-10-03.

- The decisions made during forge: #ADR-0010 to #ADR-0036, with the records they rest on.
- `CONTEXT.md`, the glossary.
- In forge: the runner, the summary, the diff and the reducer tracked under `scripts/corpus/`,
  each input an argument, with the reduced corpus list beside them.
- The baseline: forge at `d6cd66f` over the 5,137 mods in scope, in forge's runs log and in
  [RUNS.md](RUNS.md).

Exit, met: the diff of forge against itself over the reduced corpus (27 mods, 9 s) is empty.

### 1. Model and inputs

- `reforge-magic` and `reforge-wwise`, carried over. The bank reader refuses a bank whose clips
  together are larger than the data they are read from.
- `model`: ids, hashes, keys, checked paths, the `Mod`, the bin reader, read access to values.
- `diag`: the log, sinks, filters, levels, the catalogue, the diagnostics in the report
  ([DIAGNOSTICS.md](DIAGNOSTICS.md)).
- `archive`, `game`, `history` with `SqliteHistory` on the index's format 0 (#ADR-0005,
  [HISTORY.md](HISTORY.md)).
- `reforge-census`: the walk, the fold, the index writer and append, carried over (#ADR-0005). The
  index is built from the published census history.
- `reforge-testkit`.
- The test that holds the modules to their order.

Records: the archive's mod artifacts (#MOD-ARTIFACT-0006 to #MOD-ARTIFACT-0009); the history's
keys (#ADR-0037).

Exit, met: `reforge history probe` passes against the install, and the index reforge builds
holds the rows forge's holds, in every table ([RUNS.md](RUNS.md), "The history index").

### 2. Load

- File fixups: legacy formats, moved assets.
- Binless mods, WAD roles, characters, units, loads, file uses.
- `model`: the view types of a skin's objects, and `Dump`.
- `game`: `GameView`.
- `Dating`, `Inventory`, `FileProvenance`.
- `report`: the `Report` of a run as far as it went, and `reforge load`, which writes it.

Records: the format and path game changes (#GAME-CHANGE-0007 to #GAME-CHANGE-0012); the WAD
and packaging mod artifacts (#MOD-ARTIFACT-0010 to #MOD-ARTIFACT-0016).

Exit, met: over the datasets, the WADs, files with their use, characters, units, dating,
inventory, file provenance and the load-stage diagnostics are forge's in every mod, but for six
listed deviations ([RUNS.md](RUNS.md), "The load stage").

### 3. Fixups and the first package

- The engine, `Rule`, `FixupSelection`, the catalog.
- `model`: the visitor that changes values.
- Every built-in fixup, each with its record.
- `package`: `Package`, `InPlaceBuilder`, the round-trip and type checks, `ProjectDir`, `Modpkg`,
  the report beside the output.
- `reforge repair`.

Records: the schema game changes (#GAME-CHANGE-0013 to #GAME-CHANGE-0021); the tool-artifact
mod artifacts (#MOD-ARTIFACT-0017 to #MOD-ARTIFACT-0023).

Exit, met: over the datasets a repaired mod's bins are forge's, value for value, but for six
listed deviations; a repaired mod repaired again changes nothing; the 15 in-game sample mods
repair with every rewritten bin read back ([RUNS.md](RUNS.md), "The repair").

### 4. Facts and the decision

- `model`: the readers of a rig's files. `game`: the typed questions of #ADR-0043 on
  `GameView`.
- `Facts`, `Policy`, `Targets`, the report.
- `reforge inspect`: the report with no package.

Records: a clip's track for a joint the skeleton lacks (#GAME-BEHAVIOR-0005); a premium skin's
submeshes in an own mesh and another skin's clips on an own rig (#MOD-ARTIFACT-0024,
#MOD-ARTIFACT-0025). The reworks, the copy and paste practices and the target ADRs were written
in step 0.

Exit, met: over the datasets, of 11,256 units 11,254 have forge's origin, target and reason
under each animation-reuse level, and every difference of the facts is under one of four listed
deviations ([RUNS.md](RUNS.md), "Facts and the decision").

### 5. The port

- `Shipping`, `Reach`, `FileRefs`, `Layout`, `Renames`, `AudioPlan`, `Port`.
- `PortBuilder`: patches, the port's bins, voice and HUD layers, the TEX container.
- The apply check.
- `reforge port`.

- The audio: the facts' `Sounds`, `OwnClips`, `AudioPlan` with the literal sound names, the
  banks and the audio records of `PortBuilder`, the voice layers of #ADR-0028, and the game's
  events bank and a bank's clips as typed questions of `GameView` (#ADR-0043).

Records: a bin's links and names (#GAME-BEHAVIOR-0006), the WADs mounted with a champion
(#GAME-BEHAVIOR-0007), a graph event's resolver (#GAME-BEHAVIOR-0008), the `.troybin` read for
a `.troy` (#GAME-BEHAVIOR-0009), a PTCH record's path names (#GAME-BEHAVIOR-0010); repath tags
(#MOD-ARTIFACT-0026); the client's rules for banks and events (#GAME-BEHAVIOR-0011 to
#GAME-BEHAVIOR-0017); a voice stub of an older version and a unit that stands in for the
game's (#MOD-ARTIFACT-0027, #MOD-ARTIFACT-0028).

Exit, met: over the datasets, the package contents compared with forge's, chunk paths and
bytes and each patch applied to the game's bin ([RUNS.md](RUNS.md), "The port"). The 14
in-game sample mods port with no finding of a check. Five ports were checked in game by the
user and work ([RUNS.md](RUNS.md), "Ports in game").

### 6. Closing

- `Porter`, `Repairer` and `Run`, the objects a caller drives the stages through
  ([ARCHITECTURE.md](ARCHITECTURE.md), "Shape"). Built: the four commands run on them, and
  their reports, output and exit codes are those of the commands that called the stages
  themselves ([RUNS.md](RUNS.md), "The commands on a run").
- The remaining differences against forge, each fixed, noted or made an ADR. Over the datasets
  no port differs from forge's outside the accepted deviations ([RUNS.md](RUNS.md), "The
  port").
- Two runs of every reduced-corpus mod give equal bytes. The reports of two runs are equal,
  with the digest of every chunk and every patch in them ([RUNS.md](RUNS.md), "The port").
  Still to do: the `.modpkg` two runs write of one mod differs in its bytes
  ([RUNS.md](RUNS.md), "The commands on a run").
- The design docs edited to what was built. Done with `Porter`: each doc of `docs/design/`
  read against the code, every record code cited in `crates/` and `docs/` naming a file.

### 7. After parity

In forge's order of priority: sounds over the game's; forged field names in PTCH paths; then
what forge held (event names without a list, the history's refs and entry texts, mip chains,
`tex-align`'s crash condition, material overrides that name no submesh). The GUI is a caller of
`reforge` and can start once step 3's catalog and selection exist.

## Records to write

Titles as the records will carry them. Numbers are given when a record is written, in the step
that cites it. Sources are forge's docs and the module that holds the evidence.

Written: #GAME-CHANGE-0001 to #GAME-CHANGE-0021; #GAME-BEHAVIOR-0001 to #GAME-BEHAVIOR-0017;
#MOD-ARTIFACT-0001 to #MOD-ARTIFACT-0028.

### Game changes

| the change | build | forge's source |
|---|---|---|
| File references as chunk hashes | 16.17 | `TRANSFORMS.md`, `transform/retype.rs` |
| The material sampler's fields renamed | 15.3, 16.13, 16.17 | `transform/migrate.rs` |
| A material's default technique retired | 13.5 | `transform/technique.rs` |
| Emitter bound-object scales in `FlexShapeDefinition` | 12.6, 12.21 | `transform/gather.rs` |
| Primitive mesh camera alignment on both axes | 13.8 | `transform/meshalign.rs` |
| Emission surface in `EmissionSurface` | 15.18, 15.22 | `transform/surface.rs` |
| Audio queue links as hashes | 16.18 | `TRANSFORMS.md` |
| A material's type as one byte | 16.18 | `TRANSFORMS.md` |
| A game-mode field made optional | 15.20 | `TRANSFORMS.md` |

### Game behaviors

| the behaviour | status in forge's docs | forge's source |
|---|---|---|
| A bin's link resolves to the mod's file, else the game's, else is skipped | stated | `INPUTS.md` |
| A file is there for a champion when a WAD mounted with it holds the path | stated | `INPUTS.md`, `analysis/refs.rs` |
| An old particle's `.troy` is read as the `.troybin` beside it | stated | `WRITER.md` |
| A block-compressed texture not in whole blocks is drawn | seen in the game's own files | `TRANSFORMS.md` |
| A graph event finds its resolver by the skin's name, not by the entry's link | confirmed in game, 2026-09-30 | `WRITER.md` |
| A graph event does not look in the additional resolvers | confirmed in game | `WRITER.md` |
| A PTCH record's path names are hashed lowercased | to confirm in game | `WRITER.md` |
| Spell and buff sound hooks are composed from the skin's tags | settled in game, 2026-09-23 to 2026-09-26 | `AUDIO.md` |
| Of two banks defining one id the client keeps one, for voice the later unit's | settled in game | `AUDIO.md` |
| A bank adds event ids only when its file name holds `_events` | settled in game | `AUDIO.md` |
| A play action finds its bank by the id in the header, not by the file name | confirmed in game, 2026-09-30 | `AUDIO.md` |
| Events banks load at version 145 only, and an older stub blocks its package | confirmed in game, 2026-10-01 | `AUDIO.md` |
| The game's event plays beside a mod's event of the same hook | seen in game, 2026-10-01 | `AUDIO.md` |
| A sound plays only from a bank that holds the nodes above it | settled in game | `AUDIO.md` |
| Which of two entries of one hash in a bin the client keeps | open | `DIAGNOSTICS.md` |

### Mod artifacts

| what is found | forge's source |
|---|---|
| Repath tags in file and folder names | `WRITER.md`, `transform/relocate.rs` |
| Emitter fields nested in `textureMult` | `transform/strays.rs` |
| `textureMult` wrapped twice | `transform/reshape.rs` |
| A birth translation inside `VfxShapeLegacy` | `transform/spawnshape.rs` |
| Fields no game build declared | `transform/artifacts.rs` |
| Integer fields written too wide | `TRANSFORMS.md` |
| Values converted by the manager at install | `transform/retype.rs` |
| A bank with header id zero | `transform/bank.rs` |
| A voice stub of an older bank version | `AUDIO.md` |
| A unit of the game's name on banks of its own | `AUDIO.md` |

### ADRs

The decisions forge's docs and session notes record as the user's are written: #ADR-0010 to
#ADR-0036, each with the date of the decision.

Policies forge's docs state without saying who decided. They stay open until the port is built
and each is checked in game (the user, 2026-10-03). None is an ADR before that.

| the policy | forge's source |
|---|---|
| The resolver keeps the game's name (tried in game, 2026-09-30; #ADR-0006 rests on it) | `WRITER.md` |
| A value no build since the mod's own read is removed, not revived | `TRANSFORMS.md` |
| The game's sounds stay and the mod's units come after them | `AUDIO.md` |

## Open

- A run on the census history alone, with no install, and against the game at any build the
  history holds. The index has the tables and the questions for the two things a port reads
  from the game that the history gained on its `history-v2` branch (#ADR-0042): the joints of
  an animation clip, and an events bank's header and objects ([HISTORY.md](HISTORY.md), "The
  questions"). Still to do: a game source that answers from the history.
- The history holds no vertex positions, so a source backed by it does not answer which
  submeshes of an own mesh are copied from another skin (#ADR-0043). No digest of rounded
  positions is asked of the census (the user, 2026-10-04): a rounding rule frozen into the
  history serves one check, and stays in the data when the rule proves wrong, where an exact
  hash keeps its use.
- Of the sample mods ported by reforge, five are checked in game ([RUNS.md](RUNS.md), "Ports
  in game"). The others are not. No unit cut from the mod's own events bank was checked in
  game in a port of reforge's.
- The two reasons the build has for not writing a unit's audio (the game's entry has no bank
  unit to model one on; a skin bin written whole takes no layer) are carried over from forge
  and met by no mod of the datasets: a unit with either has no audio block at its target, and
  the plan carries none of its clips.
- Sound units forge leaves out that a port could carry (measured 2026-10-04, for step 5): 64
  units in 18 mods have a second events bank of the client's version that plays their clips,
  and forge reads only the first; 84 units have an events bank of version 134 that plays their
  clips, and nothing converts a bank from 134 to 145.
- An object of a class the build dropped is left as it is (`ItemRecommendationContextList`,
  gone at 16.6, in 227 mods' root bins).
- A list of floats where the build reads a list of vectors, and the reverse, has no conversion
  (46 mods). Forge has none either.
- A package embeds the evidence of its run (#ADR-0041), and that is not built: the format of
  the embedded data, where it sits in a `.modpkg` and in a project directory, and the reader
  that takes it in place of judging the package's bytes. `reforge repair --again` carries the
  date and the restorations from the first run in memory until then.
- A birth translation in a `VfxShapeLegacy` is still stripped in the 206 mods dated 14.1 or
  later (276,644 values), where the mod's own build never read the old `shape`.
- The integer kinds a mod holds too wide are the game's own earlier kinds, narrowed from 9.22 on
  (#MOD-ARTIFACT-0021). They are game changes with no record of their own.
- A copy of a game file under another path is looked for among the first 64 chunks of its size
  in the champion's WAD. How often the cap is reached is not measured.
- A `ChunkPath` may be longer than a file name on disk: 1,264 game paths are over 512 bytes. A
  writer of a project directory names such a chunk by its hash.
- 40 game paths the hash cache spells hold an empty part (`//`). `ChunkPath` refuses them, so
  such a chunk stays unnamed.
- The three earliest reworks of #GAME-CHANGE-0001 (Mordekaiser, Pantheon, Fiddlesticks) are
  known by their joint names alone. Their submeshes and dropped paths are not measured. Over
  the datasets 74 mods of theirs are judged against the rework and none is left to the game
  for it ([RUNS.md](RUNS.md), "Facts and the decision").
- `unit.new-bin` is raised with the decision, as forge's `target.sub-no-slot-bin` is. The build
  says in `unit.ported` that the bin is written whole, and raises `unit.new-bin-failed` where
  a record does not apply to it.
- The base an author's object is compared with is the game at the target build (`Base`). The
  comparison with the game at the mod's own build, applied onto the target build (#ADR-0043),
  is not built.
- A repath tool's `.tft_set<N>` tag stays in a ported file's name (#MOD-ARTIFACT-0026): the
  rule carried over from forge takes `.tftset<N>`, which no mod was seen to carry. Dropping
  the tag changes the paths a port ships against forge's.
- A name made to hash to a field no table names (#ADR-0031) is not built: a HUD edit on such a
  path is left out, as in forge.
- What a unit reaches is reported (`object.link-dangling`, `object.unreached`,
  `object.game-carried`, `ref.dangling`, `ref.unmounted`) for every representative unit with
  a target, the ones that then leave the port as the game's own skin or a folded chroma
  included, as in forge.
- A port that holds nothing ends the run at the build (`package.empty`). Whether a mod whose
  every unit left the port is a fatal finding or a report with no package is not decided.
- The joints of a game skin's skeleton are asked only for the slots a unit was compared with,
  and two units on one slot never rank the same, both as in forge ([RUNS.md](RUNS.md), "Facts
  and the decision"). Whether to ask the joints of any slot, and what a conflict is, is not
  decided.
- A game source that answers the typed questions from the history (#ADR-0043) is not built.
- A text bin at a bin's path is left as it is (#MOD-ARTIFACT-0015). Whether to parse it and
  write the bin is a decision not taken.
- Whether an old emitter's simple-particle fields are moved into `LegacySimple` or removed:
  Riot removed the struct from most of its own emitters at 12.20 (#GAME-CHANGE-0004).
- `ltk_wad` reserves a chunk's declared size while it mounts a WAD with sub-chunks. A packed
  WAD that declares a size the machine cannot give aborts the process. The fix is `ltk_wad`'s.
- Sounds over the game's: a port plays the game's event beside the mod's.
- Whether a unit that fits no slot is written at all.
- A shared schema crate with LTK Manager.
- A prebuilt history index, distributed.
- The test realm's builds in the history.
