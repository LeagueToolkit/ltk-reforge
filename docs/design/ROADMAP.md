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

All taken on 2026-10-03.

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
| How the game client behaves is a fourth series of records, `docs/game-behavior/` | `AGENTS.md`, `docs/agents/records.md` |
| A GUI is a caller of the library: fixups and checks are listed, turned on and off, and added as data where a table says it all | [ARCHITECTURE.md](ARCHITECTURE.md), "Callers" and "Fixups" |

## Steps

Each step lists what is built, the records it writes, and its exit.

### 0. Ground

- The decisions made during forge, as ADRs ([the list](#adrs)).
- `CONTEXT.md`, the glossary: unit, slot, S, O, T, copy, spread, representative, own, stale,
  carried, layer, namespace, fixup, and the stage names.
- In forge: the corpus scripts tracked, with their inputs as arguments, and the reduced corpus
  list beside them.
- A baseline: forge over the reduced corpus and the datasets, the run entered in forge's runs
  log.

Exit: the diff of forge against itself over the reduced corpus is empty.

### 1. Model and inputs

- `reforge-magic` and `reforge-wwise`, carried over. The bank reader takes a budget.
- `model`: ids, hashes, keys, checked paths, the `Mod`, the value visitor, the view types, `Dump`.
- `diag`: the log, sinks, filters, levels, the catalogue, the diagnostics in the report
  ([DIAGNOSTICS.md](DIAGNOSTICS.md)).
- `archive`, `game`, `history` with `SqliteHistory` on forge's index format.
- `reforge-census`: the walk, the fold, the index writer and append, carried over (#ADR-0005). The
  index is built from the published census history.
- `reforge-testkit`.
- The test that holds the modules to their order.

Records: the archive's mod artifacts; the content keys.

Exit: `reforge history probe` passes against the install. The index reforge builds answers the
questions as forge's does.

### 2. Load

- File fixups: legacy formats, moved assets.
- Binless mods, WAD roles, characters, units, loads, file uses.
- `Dating`, `Inventory`, `FileProvenance`.

Records: the format and path game changes; the WAD and packaging mod artifacts.

Exit: over the datasets, the units, the file uses and the `archive`, `inventory`, `wad`, `file`
and `bin` diagnostics compared with forge's.

### 3. Fixups and the first package

- The engine, `Rule`, `FixupSelection`, the catalog.
- Every built-in fixup, each with its record.
- `package`: `Package`, `InPlaceBuilder`, the round-trip and type checks, `ProjectDir`, `Modpkg`,
  the report beside the output.
- `reforge repair`.

Records: every schema game change; the tool-artifact mod artifacts.

Exit: over the datasets, a repaired mod's bins compared with forge's, value for value. A
repaired mod repaired again changes nothing. The 14 in-game sample mods repair with every bin
verified.

### 4. Facts and the decision

- `Facts`, `Policy`, `Targets`, the report.
- `reforge inspect`: the report with no package.

Records: the reworks; the copy and paste practices; the target ADRs.

Exit: over the datasets, each unit's origin, target and reason compared with forge's under each
animation-reuse level.

### 5. The port

- `Shipping`, `Reach`, `FileRefs`, `Layout`, `Renames`, `AudioPlan`, `Port`.
- `PortBuilder`: overrides, the port's bins, voice and HUD layers, the TEX container.
- The apply check.
- `reforge port`.

Records: the audio and layout ADRs; the audio mod artifacts; the game behaviors.

Exit: over the datasets, the package contents compared with forge's: chunk paths and bytes, and
each override applied to the game's bin. The 14 in-game sample mods are checked in game by a
person.

### 6. Closing

- The remaining differences against forge, each fixed, noted or made an ADR.
- Two runs of every reduced-corpus mod give equal bytes.
- The design docs edited to what was built.

### 7. After parity

In forge's order of priority: sounds over the game's; forged field names in PTCH paths; then
what forge held (event names without a list, the history's refs and entry texts, mip chains,
`tex-align`'s crash condition, material overrides that name no submesh). The GUI is a caller of
`reforge` and can start once step 3's catalog and selection exist.

## Records to write

Titles as the records will carry them. Numbers are given when a record is written, in the step
that cites it. Sources are forge's docs and the module that holds the evidence.

### Game changes

| the change | build | forge's source |
|---|---|---|
| File references as chunk hashes | 16.17 | `TRANSFORMS.md`, `transform/retype.rs` |
| The material sampler's fields renamed | 15.3, 16.13, 16.17 | `transform/migrate.rs` |
| A material's default technique retired | 13.5 | `transform/technique.rs` |
| Emitter filters in `Filtering` | 12.17 | `transform/gather.rs` |
| Emitter linger fields in `Linger` | 12.18, 12.20 | `transform/gather.rs` |
| Emitter bound-object scales in `FlexShapeDefinition` | 12.6, 12.21 | `transform/gather.rs` |
| Emitter simple-particle fields in `LegacySimple` | 12.19, 12.20 | `transform/gather.rs` |
| Emitter texture mult in `textureMult` | 13.1 | `transform/gather.rs`, `transform/reshape.rs` |
| Primitive mesh camera alignment on both axes | 13.8 | `transform/meshalign.rs` |
| Emitter shape as `SpawnShape` and `EmitterPosition` | 14.1 | `transform/spawnshape.rs` |
| Emission surface in `EmissionSurface` | 15.18, 15.22 | `transform/surface.rs` |
| Audio queue links as hashes | 16.18 | `TRANSFORMS.md` |
| A material's type as one byte | 16.18 | `TRANSFORMS.md` |
| A game-mode field made optional | 15.20 | `TRANSFORMS.md` |
| Textures in the TEX container, static meshes as `.scb` | to establish | `transform/legacy.rs`, `texcontainer.rs` |
| Variant tags dropped from asset names | to establish | `transform/moved.rs` |
| Resolution prefixes dropped from texture names | to establish | `transform/moved.rs` |
| A `_base` suffix on the base mesh and skeleton | to establish | `transform/moved.rs` |
| Shared skin bins in the character's folder | to establish | `transform/moved.rs` |
| Champion reworks | 13 builds, 10.11 to 16.6 | `TARGETS.md`, `tables/reworks.tsv` |

### Game behaviors

| the behaviour | status in forge's docs | forge's source |
|---|---|---|
| A bin value of another type than the build registered is dropped without a word | stated | `TRANSFORMS.md` |
| A bin's link resolves to the mod's file, else the game's, else is skipped | stated | `INPUTS.md` |
| A file is there for a champion when a WAD mounted with it holds the path | stated | `INPUTS.md`, `analysis/refs.rs` |
| A texture is read by its bytes' magic, not its extension | stated | `TRANSFORMS.md` |
| An old particle's `.troy` is read as the `.troybin` beside it | stated | `WRITER.md` |
| A block-compressed texture not in whole blocks is drawn | seen in the game's own files | `TRANSFORMS.md` |
| The client loads a skin bin the game does not ship for a sub-character | confirmed in game, 2026-09-30 | `TARGETS.md` |
| A graph event finds its resolver by the skin's name, not by the entry's link | confirmed in game, 2026-09-30 | `WRITER.md` |
| A graph event does not look in the additional resolvers | confirmed in game | `WRITER.md` |
| A clip's track for a joint the skeleton lacks is skipped | inferred from mods that play in game | `TARGETS.md` |
| Spell icons are named by the champion's root bin and show under every skin | confirmed in game, 2026-10-01 | `WRITER.md` |
| A PTCH record's path names are hashed lowercased | to confirm in game | `WRITER.md` |
| Spell and buff sound hooks are composed from the skin's tags | settled in game, 2026-09-23 to 2026-09-26 | `AUDIO.md` |
| Of two banks defining one id the client keeps one, for voice the later unit's | settled in game | `AUDIO.md` |
| A bank adds event ids only when its file name holds `_events` | settled in game | `AUDIO.md` |
| A play action finds its bank by the id in the header, not by the file name | confirmed in game, 2026-09-30 | `AUDIO.md` |
| Events banks load at version 145 only, and an older stub blocks its package | confirmed in game, 2026-10-01 | `AUDIO.md` |
| The game's event plays beside a mod's event of the same hook | seen in game, 2026-10-01 | `AUDIO.md` |
| Which of two entries of one hash in a bin the client keeps | open | `DIAGNOSTICS.md` |

### Mod artifacts

| what is found | forge's source |
|---|---|
| Zip entries stored with a wrong checksum | `INPUTS.md`, `forge-frontend/src/archive.rs` |
| Chunks named by their path hash | `INPUTS.md` |
| Repack duplicates with a numeric suffix | `INPUTS.md` |
| A WAD packed whole as one entry | `INPUTS.md` |
| A mod packed into its champion's locale WAD | `INPUTS.md` |
| A WAD named for no champion | `INPUTS.md` |
| Two WADs for one champion | `RUNS.md` |
| Mods with no skin bin | `WRITER.md` |
| A skin bin copied to every slot | `TARGETS.md` |
| A premium skin pasted into the base slot | `TARGETS.md` |
| The game's own skins shipped beside the mod's | `TARGETS.md` |
| Stale copies of game files | `INPUTS.md` |
| Bins as text or JSON at a bin's path | `INPUTS.md` |
| Executables, sources and leftovers in the archive | `INPUTS.md` |
| Repath tags in file and folder names | `WRITER.md`, `transform/relocate.rs` |
| Emitter fields nested in `textureMult` | `transform/strays.rs` |
| `textureMult` wrapped twice | `transform/reshape.rs` |
| A birth translation inside `VfxShapeLegacy` | `transform/spawnshape.rs` |
| Fields no game build declared | `transform/artifacts.rs` |
| Integer fields written too wide | `TRANSFORMS.md` |
| Values converted by the manager at install | `transform/retype.rs` |
| A bank with header id zero | `transform/bank.rs` |
| A voice stub of an older bank version | `AUDIO.md` |
| Banks added with events of their own | `AUDIO.md` |
| A unit of the game's name on banks of its own | `AUDIO.md` |

### ADRs

The decisions forge's docs and session notes record as the user's, with the date given there.

| the decision | date | forge's source |
|---|---|---|
| A mod for a premium slot stays there | 2026-09-29 | `TARGETS.md`, notes |
| A skin0 unit goes to skin0 first, then to its premium origin | 2026-09-29 | `TARGETS.md`, notes |
| Borrowed clips are judged by `own-rig` | 2026-09-29 | `TARGETS.md`, notes |
| The port holds the author's one unit and never writes copies back | 2026-09-29 | `TARGETS.md`, notes |
| A borrowed effects texture is legitimate and never flagged | 2026-09-29 | `ROADMAP.md`, `TARGETS.md` |
| A fixup is a general transform of the mod, and the choice of target is a policy apart from the facts | 2026-09-29 | notes |
| Voice goes to a layer of its own | 2026-09-29 | `AUDIO.md` |
| The output's layout | 2026-09-29 | `WRITER.md` |
| Path edits as declarations are deprioritised | 2026-09-29 | `ROADMAP.md` |
| The corpus covers champion skin mods | 2026-09-29, 2026-09-30 | `TOOLING.md` |
| Textures keep their pixels | 2026-09-30 | `ARCHITECTURE.md` |
| A unit that plays mostly one skin's clips is a mod for that skin | 2026-09-30 | `TARGETS.md`, notes |
| A unit that fits nothing is written at skin0 with an error | 2026-09-30 | `TARGETS.md` |
| A sub-character with no bin at T gets a new one | 2026-09-30 | `TARGETS.md` |
| A chroma that is the same work folds into its parent | 2026-09-30 | `TARGETS.md` |
| A name and an author do not identify a mod | 2026-09-30 | `TOOLING.md` |
| No archive is extracted to disk | 2026-10-01 | notes |
| Work made for a model before its rework is not ported | 2026-10-01 | `TARGETS.md` |
| A stale copy is no override | 2026-10-01 | `INPUTS.md` |
| Spell icons and HUD textures go to a layer of their own | 2026-10-01 | `WRITER.md` |
| A bank the author added is cut from the mod's own events bank | 2026-10-01 | `AUDIO.md` |
| A field no table names gets a forged name | 2026-10-01 | `WRITER.md` |
| A mod broken by the format is not accommodated | 2026-10-01 | `TOOLING.md` |
| Picture comparison is held | 2026-10-01 | `ROADMAP.md` |
| The census format is closed | 2026-10-03 | `CENSUS.md` |

Policies forge's docs state without saying who decided. Each is confirmed with the user before
it is written as an ADR.

| the policy | forge's source |
|---|---|
| The resolver keeps the game's name (tried in game, 2026-09-30; #ADR-0006 rests on it) | `WRITER.md` |
| A value no build since the mod's own read is removed, not revived | `TRANSFORMS.md` |
| The game's sounds stay and the mod's units come after them | `AUDIO.md` |

## Open, carried from forge

- Sounds over the game's: a port plays the game's event beside the mod's.
- Whether a unit that fits no slot is written at all.
- A shared schema crate with LTK Manager.
- A prebuilt history index, distributed.
- The test realm's builds in the history.
