# Architecture

This is the design the rewrite builds. Each section is edited to what the code is as the code
lands. The reasons are in [REVIEW.md](REVIEW.md). The order of work, and which steps are
built, are in [ROADMAP.md](ROADMAP.md).

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
   │ build     Package       layers, chunks, patches, manifests, hash tables
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
let env = Env::builder().install(install).history(history).names(names).schema(schema).build()?;
let porter = Porter::new(&env).policy(policy).fixups(FixupSet::standard());

let archive = porter.read(bytes, file_name)?;
let mod_ = porter.load(archive)?;
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

A stage ends early in two ways. A fatal diagnostic leaves nothing to write, and the stage
returns `Halted` with its id. An input that does not answer (a game chunk that does not read,
a history query that fails) is a fault, a typed error of the stage: a run judged against an
input it could not read does not answer as if the input held nothing.

Each method hands its stage the stage's sink and options. `porter.read` is
`Archive::read(bytes, file_name, &caps, &mut sink)`: the archive's bytes are borrowed, so the
caller holds them once and drops them after the call.

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
| `reforge-wwise` | Wwise banks and packages, read and written; a bank whose clips are larger than its data is refused |
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
| `game` | the target side: `Game` and `Install`, `GameView`, `Names`, `Schema` |
| `history` | `History`: the questions the stages ask, and the definition of the SQLite index ([HISTORY.md](HISTORY.md)) |
| `env` | `Env`: what a run is judged against, built once |
| `load` | `Mod` from an `Archive`: file fixups, binless mods, characters, units, loads; and what is known of the files before any value is read: `Dating`, `Inventory`, `FileProvenance` |
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
  came from (`FileSource::Archive`, `Made { from, by }`, `Game`), a unit's loads and entry.
  A target, a destination and a dropped state are in `Targets` and `Port`.
- **One file per path in a WAD.** The WAD format holds one chunk per path hash, so a `Mod` holds
  at most one file per WAD and path hash. Adding a file where one exists adds nothing and says
  which file is there. A step that would add one (a file fixup, the game's bins for a mod with
  no skin bin) leaves the mod's own file in place.
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

`Env` is what a run is judged against: the game, its size index, the history, the names and
the schema. It is immutable and built once by `Env::builder()`.

- **`Game`** is a trait for a chunk store: contains, size, read, the WADs holding a path, the
  build. `Install` implements it. `GameView` wraps a `Game` and caches what callers repeat: a
  path's content id, a parsed bin with the files it names, the bins of a character's skin at a
  slot (`GameSkin`), the catalog of a character's skins. The caches live as long as the `Env`
  and have no cap, so a caller that runs many mods builds an `Env` for each or a few. The chunks of one WAD by uncompressed size are a `SizeIndex`, a value
  beside the `Game` and no method of it, so a fake game builds one from the same chunks.
- **`History`** is a trait with one method per question in forge's query table. `SqliteHistory`
  reads the index, `NoHistory` answers nothing, and the testkit has one in memory.
- **`Names`** holds the tables (Mimir, text tables) and does not change. Names a run learns
  (paths the archive spells, names the port makes) are in a `NameBook` the run owns. A `Speller`
  reads both in one order for every kind of hash: the run's names, Mimir, the text tables. A
  name a table gives is checked against its hash on load, so a wrong line names nothing.
- **`Schema`** answers the target build's type of a class's field: the meta database, else the
  install's bins. A revision the reader does not understand refuses the whole database: a
  skipped revision would read as a field no build declares.

No input is found by itself. The Mimir cache directory and the Manager's schema file each have
a function that names the usual place, and the caller passes the result to the builder. An
input that is absent or does not read is a diagnostic of the `run` channel, raised by the
input's `report`. Its reason may name a path of the machine, which is the one place a report
differs between machines.

Options are not in `Env`. Each stage takes its own: `Caps` and `LoadOptions` for read and load,
`Policy` for decide, `PortOptions` for plan.

## Fixups

A fixup brings old or tool-damaged data to what the target build reads. Each is an object that
names the records it implements. `reforge fixups` lists them.

```rust
pub trait BinFixup {
    fn info(&self) -> &FixupInfo;
    fn applies(&self, types: &TargetTypes) -> bool;    // the target build has the new shape
    fn classes(&self) -> Classes;                      // the structs it is called for
    fn fix(&self, cx: &mut FixCx<'_>, s: StructMut<'_>);
}

/// What a caller lists and toggles: serialisable, with no code in it.
pub struct FixupInfo {
    pub id: FixupId,            // "vfx-filtering"
    pub title: String,
    pub records: Vec<Record>,   // game changes and mod artifacts, by their codes
    pub phase: Phase,           // where it runs: Files, Undo, Layout { build }, Conform, Strip
    pub default: bool,          // in the standard set
}
```

- **The engine owns the frame.** `FixupSet::apply` walks every loaded object once per fixup, in
  the set's order. `FixCx` gives the target build's types (`declares`, `type_of`), whether the
  mod's build read a field (`read_at_source`, from the mod's dating), the names, and the calls
  that record what a fixup did.
- **The target's types** are `TargetTypes`: the meta schema, else what the install's own bins
  hold for a class and field.
- **A change is a record.** `Change { fixup, object, path, before, after }`, where `after` is a
  value, a move to another path, removed, lost or unconvertible. A fixup writes no sentence and
  sets no flag. The object's edited state, the report's rows and the four `fixup.*` findings
  (changed, removed, lost, unconvertible, each raised once per fixup) come from the changes.
- **A fault is not a change.** A fixup that cannot ask its inputs (a name table that does not
  read) hands the fault to the context, and `apply` returns it as an error.
- **Table rules are data.** A `Rule` is a serialisable value: remove a field, rename fields, or
  move fields into a child struct, in one class, with its record. The engine runs a `Rule` as a
  `BinFixup`. The built-in rules are written with builders that make the same values:

  ```rust
  Rule::move_fields("VfxEmitterDefinitionData")
      .into_field("Filtering", "VfxEmitterFiltering")
      .keep(["keywordsExcluded", "keywordsRequired", "spectatorPolicy", "censorPolicy"])
      .at_build(Version::new(12, 17))
      .record(game_change)
  ```

  A rule acts only on a field the target does not read where it is, so no rule a caller writes
  can change a value twice.

- **The rest is Rust.** A fixup that combines values, chooses a class from the schema, recurses
  or conforms a whole object to the schema implements the trait in code: mesh alignment, the
  spawn shape, the emission surface, the sampler's fields, the default technique, the
  texture-mult strays, the hoist and the wrap, retype, strip. Retype's conversions are a
  catalogue of named items, one function each. A `Rule` grows a new shape only when a second
  fixup needs it. Both kinds are listed, ordered and
  turned on and off the same way.
- **One record, one fixup.** Where forge put a game change and a tool artifact in one pass, they
  are two fixups: the spawn shape and the misplaced birth translation, the wrap and the hoist.
  Retype is the exception: one fixup that follows every type change, with a record per
  conversion.
- **The order comes from each fixup's phase** (#ADR-0007). `Undo` takes tool damage out before the layouts
  it would confuse. `Layout` follows Riot's changes, the oldest build first. `Conform` brings
  values to the schema. `Strip` removes what the build does not declare. A fixup added later
  takes its place by its phase and build, with no list to edit. A test pins the standard order.
  Two fixups for a tool's damage run later than `Undo`, each because an earlier place would
  change the result: the misplaced birth translation runs after the spawn shape, whose value
  the mod's build read, and the hoist runs after the texture-mult rule, which joins fields to
  the outer struct.
- **A set is chosen, not hard-wired.** `FixupSet::standard()` holds every built-in fixup whose
  `default` is true. `FixupSelection` is the choice as data: the ids turned off, the ids turned
  on, the rules added. `FixupSet::from(&selection)` builds the set, and a selection is saved and
  loaded as a file. `FixupSet::catalog()` lists every built-in fixup's `FixupInfo`.
- **Every loaded object is fixed** (#ADR-0002). The report lists the changes on objects the port
  ships. `FixupSet::apply` takes the set of objects, so narrowing it is a change at one call.
- **File fixups** (`FileFixup`: legacy formats, moved assets, a bank's header id) carry the same
  `FixupInfo` and are chosen the same way. They work on the file set: each adds files and
  changes none, so the file it starts from stays for what the mod's own bins name. The first two
  run inside `load`, before characters and units are found, legacy formats first: moved assets
  skips a file a twin was made of. A chunk of a packed WAD has no path of its own: for the
  fixups it takes the path a bin of the mod spells for its hash, and gives it back after them,
  so a repair makes the twin the same archive with spelled paths would get. A file fixup names
  the records it implements, one for each
  rename or format, and answers where it would put a file at a path (`destinations`), which is
  how the step for mods with no skin bin knows a file is covered.

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
  `&Mod` before `fix`, which needs the dating. They are in `load`, since `fixups` comes before
  `facts` in the module order.

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

- **`Package`** is layers of chunks and patches, the hash tables and the project's config. A
  chunk is keyed by `(WadName, ChunkPath)` and says where its bytes come from (`Origin`): a file
  of the mod kept as it is, a bin written again with what it was built to hold, a texture put
  in TEX's container. A patch is a typed manifest entry and its PTCH. The manifest is
  serialised once, by the writer.
- **Builders.** `PortBuilder` makes a package from a `Port`, `InPlaceBuilder` from a `FixedMod`.
  `PtchBuilder` collects records by field path.
- **Only what is wired up** (#ADR-0039, `Wired`). Either builder ships a file only when the
  client reaches it: a bin the client loads, a file a loaded bin names, a file that overrides a
  path the game's own bins read, a file a run made for one of those. A bin at a former path
  that nothing loads, a file nothing names, a file that only a value a fixup removed named, and
  a stale copy stay behind, each counted by its reason in the package's finding. A caller that
  asks to keep stale copies gets them.
- **A repair's package** (`InPlaceBuilder`) holds those files at their own WAD and path. A bin
  an object of which a fixup changed is written again through `ltk_meta`'s delta writer, which
  copies every other object byte for byte, and whole where the delta writer refuses the bin's
  numbering. Every other file keeps its bytes. A file no table names goes under its hash at its
  WAD's root (#MOD-ARTIFACT-0007). A package that would hold nothing ends the run.
- **A repaired package is not the whole evidence of the mod.** The files left behind are part
  of what `load` judged by: a mod's date, and that a WAD restores a model from before a rework.
  A package repaired again from its own bytes alone dates differently in about half the mods,
  and judges a restoration's old files stale. The report beside the package holds the date and
  the restorations of the run that made it.
- **`TexContainer`** puts a DDS at a `.tex` path in TEX's container where its blocks go in as
  they are (#ADR-0022), after either builder.
- **Checks** implement `Check`: the patches applied as the overlay applies them, the types of
  every value written (`Types`), a rewritten bin read back (`RoundTrip`). Each has a
  `CheckInfo` and is chosen as a fixup is: a `CheckSelection` is the choice as data, a
  `CheckSet` the checks of a run. Each says what it covered. What a check finds is a
  diagnostic and does not stop `write` (#ADR-0003): `permit` asks the caller's
  `DiagnosticPolicy`, and raises `write.blocked` where it refuses.
- **Writers** implement `PackageWriter`: `ProjectDir`, `Modpkg`. A writer writes into a new
  directory and renames it into place, with the report beside the package: `<out>/<project>/`
  with `reforge-report.json` in it, and `<out>/<project>_<version>.modpkg` with
  `<project>_<version>.reforge-report.json` beside it. A write that fails leaves nothing, and a
  directory that is no mod project is not replaced. A chunk with a part of its path longer
  than a file name may be is named by its hash in a project directory, and its path goes in
  the project's table of paths. The `.modpkg` is packed from a project directory by
  `ltk_mod_project`'s packer, with no ignore file read.

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

- Every stage type implements `Dump`. `reforge load --dump` prints the mod as loaded.
- `Report` is a serialisable struct with a schema version (`reforge.report/1`). It is built from
  parts, each optional, so a run that stops early reports what it has: the mod, the inputs, the
  WADs, files, characters and units, the dating, the inventory, the file provenance, and every
  diagnostic with its data. A later stage adds its part: `fix` what each fixup changed, the
  build the package with every chunk's digest and, for a bin, the digest of each object as
  `ltk_meta` writes it. `reforge repair --again` adds what repairing the package a second time,
  in memory, changed.
- The corpus tooling in forge compares a report with forge's report of the same mod, section by
  section, and keeps the list of accepted deviations with the reason for each.

## Untrusted input

- `archive` is the only module that reads a zip, under forge's caps. It holds the archive's
  bytes once.
- No parser of mod bytes ends a run by a panic. A reader of this repository's own returns an
  error on any input (banks, file kinds). A reader from another crate runs where its panic is
  caught (bins, packed WADs, meshes, skeletons, clips).
- What a parser allocates is bounded by the bytes it is given: the archive's caps bound what is
  inflated, and a bank whose clips are larger than its data is refused. A failed allocation
  aborts the process and no panic guard catches it, so a size a reader from another crate
  reserves before it reads is checked first: a packed WAD whose header declares more chunks
  than its bytes can list is refused before `ltk_wad` mounts it, and one over the cap after
  that. The game's own WADs hold up to some 200,000 chunks, so the cap is above that.
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
