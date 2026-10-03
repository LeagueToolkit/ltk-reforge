# Architecture

Status: proposed 2026-10-03. Nothing is built. This is the design the rewrite builds, and each
section is edited to what the code is as the code lands. The reasons are in
[REVIEW.md](REVIEW.md), the order of work in [ROADMAP.md](ROADMAP.md).

## Shape

A run takes one mod archive through nine stages. Each stage is a type. A stage takes the
earlier stages' results by reference or by value and returns its own. No stage changes what an
earlier one returned, and the compiler enforces the order.

```
archive bytes
   │ read      Archive       the untrusted zip, in memory, under caps
   │ load      Mod           WADs, files, bins, objects, characters, skin units; what the client loads
   │ fix       FixedMod      values as the target build reads them; Changes
   │ facts     Facts         what is true of the mod: copies, provenance, origin, rig, clips
   │ decide    Targets       the policy's choice of T per unit, with the alternatives
   │ plan      Port          the units that ship, renamed and placed; audio; HUD
   │ build     Package       layers, chunks, overrides, manifests, hash tables
   │ check     diagnostics   the package applied as the overlay applies it; types
   ▼ write                   a project directory, a .modpkg
```

`repair` is the same run without facts, decide and plan: `FixedMod` builds a `Package` in place.

| stage | reads | may not |
|---|---|---|
| read | the archive's bytes | touch the game, the history or the disk |
| load | `Archive`, the environment | judge values inside objects |
| fix | `Mod`, the schema, the mod's dating | know a target slot |
| facts | `FixedMod`, the environment | choose; change the mod |
| decide | `Facts`, the policy | read the mod or the game |
| plan | `FixedMod`, `Facts`, `Targets`, the environment | serialise |
| build | `Port` or `FixedMod`, the game's bins | touch the disk |
| check | `Package`, the environment | change the package |
| write | `Package` | read anything else |

The user's four parts map to them: fixups are `fix` and the file fixups inside `load`, analysis
is `facts`, restructuring is `plan`, lowering and writing are `build`, `check` and `write`.

A caller drives the stages through one object:

```rust
let env = Env::builder().game(install).history(history).names(names).schema(schema).build()?;
let porter = Porter::new(&env).policy(policy).fixups(FixupSet::standard());

let mod_ = porter.load(Archive::read(bytes, &caps)?)?;
let fixed = porter.fix(mod_);
let facts = porter.facts(&fixed);
let targets = porter.decide(&facts);
let port = porter.plan(&fixed, &facts, &targets);
let package = porter.build(&port)?;
porter.check(&package);
package.write(&ProjectDir::new(out))?;
```

`Porter::port(bytes)` runs them all. Stopping early is calling fewer methods. There is no pass
registry, no `after` edge and no stub.

The stages are fixed. What a caller turns on and off is inside two of them: the fixups in `fix`
and `load`, and the checks in `check`. Both are lists of objects that describe themselves
([Fixups](#fixups)).

## Callers

`reforge` is a library, and the command is one caller of it. A GUI is another (the user's wish,
2026-10-03): it lists the fixups and checks from their descriptions, lets a person turn each on
or off and add a rule, runs a stage at a time, and shows each stage's result. What that needs
from the library:

- every stage result prints (`Dump`) and serialises;
- fixups and checks are listed with an id, a title, the record they implement and their default
  state;
- a choice of fixups is data that is saved and loaded, not code;
- a change a fixup made is a record with the object, the field path, and the value before and
  after, so a person sees what turning it off would keep;
- nothing in the library prints or reads arguments.

## Crates and modules

The stages are modules of one crate, `reforge` (#ADR-0001). A part with a boundary of its own is
a crate.

| crate | holds |
|---|---|
| `reforge-magic` | what a file is by its first bytes and name |
| `reforge-wwise` | Wwise banks and packages, read under a budget and written |
| `reforge` | the stages, `Porter`, `Repairer`, `Report` |
| `reforge-census` | the census history (format 2) read with gix; builds and appends the history index (#ADR-0005) |
| `reforge-cli` | the `reforge` command |
| `reforge-testkit` | builders for games, archives, bins, schemas and histories; a dev-dependency |

The modules of `reforge`, each using only the ones above it:

| module | holds |
|---|---|
| `model` | ids and arenas, hashes and keys, checked paths, the `Mod`, bin value access, text cleaning |
| `diag` | diagnostics: the log, the sinks, filters and levels ([DIAGNOSTICS.md](DIAGNOSTICS.md)) |
| `archive` | `Archive`: the only reader of untrusted zips |
| `game` | the target side: `Game` and `Install`, `GameView`, `Names`, `Schema`, tables of what is known about the game |
| `history` | `History`: the questions the stages ask, over the SQLite index |
| `load` | `Mod` from an `Archive`: file fixups, binless mods, characters, units, loads |
| `fixups` | the fixup engine and every rule |
| `facts` | `Facts` |
| `port` | `Policy`, `Targets`, `Port` |
| `package` | `Package`, its builders for a port and for a repair, the checks, the writers |
| `report` | `Report`, the dump |

A test reads the crate's `use` lines and fails when a module uses one below it in this table.

`reforge-census` keeps gix, `ltk_ritobin` and the patched `ltk_*` crates out of `reforge`: a run
asks the history questions and never walks the census. It depends on `reforge` for the index's
tables (`history`) and the bin value helpers (`model`).

## The model

`model` holds the mod as the archive ships it. It keeps forge's entities and ids (`%w`,
`%f`, `%b`, `%o`, `%c`, `%u`) and the creation order that makes a dump deterministic.

- **A `Mod` is frozen after `fix`.** `FixedMod` is a newtype only `FixupSet::apply` makes.
  `Facts::gather` takes `&FixedMod`, so values are compared only at the target's types.
- **No field belongs to a later stage.** A node holds what `load` found: a file's use, where it
  came from (`FileSource::Archive`, `MadeFrom { file, rule }`, `Game`), a unit's loads and entry.
  A target, a destination and a dropped state are in `Targets` and `Port`.
- **The port is new data.** `plan` clones the objects a unit carries and renames the clones.
  The mod's objects stay as the fixups left them.
- **Closed sets are enums.** A layer is `Layer::Base | Voice(Option<Locale>) | Hud`. A unit that
  leaves the port has a `Left` reason. A rig check has an `Aspect`.
- **Paths are checked types.** `ChunkPath` is a path inside a WAD, lowercased, with no `..`, no
  root and no drive. `WadName` is one file name with its stem and locale parsed. Only these
  reach a writer. A name spelled by a mod is `Spelled` and is cleaned when printed.
- **View types read a class once.** `SkinEntry`, `Resolver`, `Graph` and `AudioBlock` wrap a bin
  object and give its fields by name. Class and field names are constants in one module.
- **One walk.** A visitor over bin values gives each struct with its object, its path and its
  class, read-only or mutable. Links, file references, renames and fixups use it.

## The environment

`Env` is what a run is judged against. It is immutable and built once by `Env::builder()`.

- **`Game`** is a trait for a chunk store: contains, size, read, the WADs holding a path, the
  build. `Install` implements it. `GameView` wraps a `Game` and caches what callers repeat: a
  path's content id, a parsed bin, a character's skin at a slot (`GameSkin`), the catalog of a
  character's skins.
- **`History`** is a trait with one method per question in forge's query table. `SqliteHistory`
  reads the index, `NoHistory` answers nothing, and the testkit has one in memory.
- **`Names`** holds the tables (Mimir, text tables) and does not change. Names a run learns
  (paths the archive spells, names the port makes) are in a `NameBook` the run owns. A `Speller`
  reads both in one order for every kind of hash: the run's names, Mimir, the text tables.
- **`Schema`** answers the target build's type of a class's field: the meta database, else the
  install's bins.

Options are not in `Env`. Each stage takes its own: `Caps` and `LoadOptions` for read and load,
`Policy` for decide, `PortOptions` for plan.

## Fixups

A fixup brings old or tool-damaged data to what the target build reads. Each is an object that
names the record it implements.

```rust
pub trait BinFixup {
    fn info(&self) -> &FixupInfo;
    fn applies(&self, schema: &Schema) -> bool;        // the target build has the new shape
    fn classes(&self) -> Classes;                      // the structs it is called for
    fn fix(&self, cx: &mut FixCx<'_>, s: StructMut<'_>);
}

/// What a caller lists and toggles: serialisable, with no code in it.
pub struct FixupInfo {
    pub id: FixupId,            // "vfx-filtering"
    pub title: String,
    pub record: Option<Record>, // #GAME-CHANGE-0007, #MOD-ARTIFACT-0003
    pub phase: Phase,           // where it runs: Undo, Layout { build }, Conform, Strip
    pub default: bool,          // in the standard set
}
```

- **The engine owns the frame.** `FixupSet::apply` walks every loaded object once per fixup, in
  the set's order. `FixCx` gives the schema (`declares`), whether the mod's build read a field
  (`read_at_source`, from the mod's dating), and `change`, which records what moved.
- **A change is a record.** `Change { fixup, object, path, before, after }`. A fixup writes no
  sentence and sets no flag. The object's edited state and the report's counts come from the
  changes, grouped by fixup.
- **Table rules are data.** A `Rule` is a serialisable value: remove a field, rename fields, or
  move fields into a child struct, in one class, with its record. The engine runs a `Rule` as a
  `BinFixup`. The built-in rules are written with builders that make the same values:

  ```rust
  Rule::move_fields("VfxEmitterDefinitionData")
      .into_field("Filtering", "VfxEmitterFiltering")
      .keep(["keywordsExcluded", "keywordsRequired", "spectatorPolicy", "censorPolicy"])
      .at_build("12.17")
      .record(GameChange(7))
  ```

- **The rest is Rust.** A fixup that combines values, chooses a class from the schema, recurses
  or conforms a whole object to the schema implements the trait in code: mesh alignment, the
  spawn shape, the emission surface, the texture-mult strays, reshape, retype, strip. A `Rule`
  grows a new shape only when a second fixup needs it. Both kinds are listed, ordered and
  turned on and off the same way.
- **One record, one fixup.** Where forge put a game change and a tool artifact in one pass, they
  are two fixups.
- **The order comes from each fixup's phase** (#ADR-0007). `Undo` takes tool damage out before the layouts
  it would confuse. `Layout` follows Riot's changes, the oldest build first. `Conform` brings
  values to the schema. `Strip` removes what the build does not declare. A fixup added later
  takes its place by its phase and build, with no list to edit. A test pins the standard order.
- **A set is chosen, not hard-wired.** `FixupSet::standard()` holds every built-in fixup whose
  `default` is true. `FixupSelection` is the choice as data: the ids turned off, the ids turned
  on, the rules added. `FixupSet::from(&selection)` builds the set, and a selection is saved and
  loaded as a file. `FixupSet::catalog()` lists every built-in fixup's `FixupInfo`.
- **Every loaded object is fixed** (#ADR-0002). The report lists the changes on objects the port
  ships. `FixupSet::apply` takes the set of objects, so narrowing it is a change at one call.
- **File fixups** (`FileFixup`: legacy formats, moved assets, a bank's header id) carry the same
  `FixupInfo` and are chosen the same way. They work on the file set. The first two run inside
  `load`, before characters and units are found.

The contract is forge's: idempotent, all or nothing per property, names kept. A test applies the
standard set twice to every fixture and expects no change the second time. A rule a user adds is
held to the same contract by the engine: a `Rule` cannot express a rewrite that is not
idempotent.

## Facts

`Facts` is a struct of typed results, gathered once in a fixed order.

```rust
pub struct Facts {
    pub copies: Copies,
    pub files: FileProvenance,
    pub objects: ObjectProvenance,
    pub clips: ClipProvenance,
    pub origins: Origins,
    pub rigs: Rigs,
    pub reworks: Reworks,
    // ...
}
```

- Each result has a constructor that takes what it needs:
  `Origins::find(&fixed, &env, &copies, &files, &objects)`. The dependency graph is the
  signatures.
- There is no cache and no invalidation. The mod is frozen, so a result stays true.
- Evidence and verdict are two steps: `OriginEvidence` is gathered, `Origin::judge` reads it.
- A per-slot question is a method. `UnitJudge::at(slot)` gives the rig checks and clip facts of
  a unit at any slot, so the policy can ask for a slot no one listed.
- Facts that do not read object values (`Dating`, `Inventory`, `FileProvenance`) are gathered on
  `&Mod` before `fix`, which needs the dating.

## Decide

`Policy::decide(&Facts) -> Targets` is a pure function. It returns one `Decision` per
representative unit: the chosen slot, a `Reason`, and each candidate with its checks. The
diagnostics are made from the decisions afterwards, so comparing other policies is calling
`decide` again.

## Plan

`plan` turns the mod and its targets into a `Port`. Each step is a type with one job, and each
prints.

1. **`Shipping`**: the units that ship, and why each other unit leaves: a copy, no target, the
   game's own skin, a chroma folded into its parent.
2. **`Reach`** and **`FileRefs`**, per group at T: what each entry reaches, what is the game's,
   what is carried, where each file is found.
3. **`Layout`**: the one owner of every output name and path. It gives the namespace, a unit's
   directory, a file's place by its first user, an object's name.
4. **`Renames`**: S to T names and namespaced names, as a map.
5. **`AudioPlan`**: the units and banks the port adds, the events renamed.
6. **`Port`**: the carried objects cloned with the renames and file references applied, the
   files with their places, the audio, the HUD edits.

## Package

`package` holds the output and everything after it.

- **`Package`** is layers of chunks and overrides, the hash tables and the project's config. A
  chunk is keyed by `(WadName, ChunkPath)`. An override is a typed manifest entry and its PTCH.
  The manifest is serialised once, by the writer.
- **Builders.** `PortBuilder` makes a package from a `Port`, `InPlaceBuilder` from a `FixedMod`.
  `PtchBuilder` collects records by field path.
- **Checks** implement `Check`: the overrides applied as the overlay applies them, the types of
  every value written, a rewritten bin read back. Each has a `CheckInfo` and is chosen as a
  fixup is. What a check finds is a diagnostic and does not stop `write` (#ADR-0003).
- **Writers** implement `PackageWriter`: `ProjectDir`, `Modpkg`. A writer writes into a new
  directory and renames it into place, with the report beside the package.

## Diagnostics

[DIAGNOSTICS.md](DIAGNOSTICS.md) holds the design. In short:

- A diagnostic is a typed value, declared by the module that raises it. Its code is
  `area.finding`, named for what it is about and not for the stage that found it (#ADR-0008).
  [CODES.md](CODES.md) lists every code.
- One ordered log runs through every stage. A stage returns its result whatever it found. Only
  a diagnostic that leaves nothing to write ends the run.
- A result that a diagnostic explains holds the diagnostic's id: a unit left out, a file not
  shipped.
- A caller filters by code, severity, channel, stage, subject and fixup, and may set a code to
  block the write. None blocks by default.
- The report is written beside the package, whole and as a summary for a person.

## Report and dump

- Every stage type implements `Dump`. `reforge ... --print <stage>` prints it.
- `Report` is a serialisable struct with a schema version. A projection gives
  the fields the corpus tooling compares with forge.

## Untrusted input

- `archive` is the only module that reads a zip, under forge's caps. It holds the archive's
  bytes once.
- Every parser of mod bytes runs through one guard that catches a panic and charges a budget:
  bins, packed WADs, meshes, skeletons, clips, banks.
- A mod's text reaches a terminal only through the `Speller` and `clean`.
- A writer takes `ChunkPath` and `WadName`, never a string.

## Code shape

- A stage or a step is a type with methods. Its inputs are fields.
- A function that takes more than four parameters, or passes its context on, becomes a method of
  a context type.
- Behaviour that varies is a trait: `Game`, `History`, `BinFixup`, `FileFixup`, `Check`,
  `PackageWriter`, `Diag`.
- A value built from many optional parts has a builder: `Env`, `Porter`, the table fixups,
  `PtchBuilder`, the testkit's fixtures.
- A closed set is an enum. A value with a rule is a newtype.
- Libraries return typed errors (`thiserror`). `anyhow` is for the command.
- `cargo fmt`, `fs-err`, `camino` and `tracing` spans per stage, as the workspace declares.

## Tests

- `reforge-testkit` builds a game, an archive, bins, a schema from rows and a history from facts,
  with no file on disk.
- A test calls the stage it tests and asserts on typed results and diagnostics by type.
- Each stage has golden dumps for the fixtures.
- Two runs of one input give the same package bytes and the same report.
- The corpus compares reforge with forge (#ADR-0004).

## What forge has and this does not

| forge | here |
|---|---|
| the pass registry, `after` edges, `--passes` | stage methods; `--stop-after <stage>`; a `FixupSelection` to choose fixups |
| stub passes and milestones | the roadmap |
| `AnalysisManager`, `Preserved` | `Facts`, gathered once on a frozen mod |
| `scope`, `scope.unrepaired` | every loaded object fixed; the report filtered by what ships |
| a diagnostic as a sentence with counts in it | a typed value, one per code and subject |
| `Module.output`, marks on input nodes | `Targets`, `Port`, `Package` |
| `own-resolver` | not built: the resolver keeps the game's name (#ADR-0006) |
| `Names` learning through `&Env` | `Names` immutable, a `NameBook` per run |
| emit skipped without `--out` | the caller writes or does not |
