# Architecture

This is the design of reforge. Each section is edited to what the code is as the code lands.
The reasons are in [REVIEW.md](REVIEW.md), the review of ltk-forge this design answers. The
order of work, and what is next, are in [ROADMAP.md](ROADMAP.md).

## Shape

A run takes one mod archive through nine stages. Each stage is a type. A stage takes the
earlier stages' results by reference or by value and returns its own. No stage changes what an
earlier one returned, and the compiler enforces the order.

```
archive bytes
   │ read      RawSnapshot   the untrusted zip, under caps; its bytes in the blob store by content id
   │ load      Mod           WADs, files, bins, objects, characters, skin units; what the client loads
   │ fix       FixedMod      values as the target build reads them; Changes
   │ facts     Facts         what is true of the mod: copies, provenance, origin, rig, clips
   │ decide    Targets       the policy's choice of T per unit, with the alternatives
   │ plan      Port          the units that ship, renamed and placed; audio; HUD
   │ build     Package       layers, chunks, patches, manifests, hash tables
   │ check     diagnostics   the package applied as the overlay applies it; types
   ▼ write                   a project directory, a .modpkg, a .fantome
```

`repair` is the same run without facts, decide and plan: `FixedMod` builds a `Package` in place.

| stage | reads | may not |
|---|---|---|
| read | the archive's bytes | touch the game, the history or the disk; hold bytes outside the store |
| load | `RawSnapshot`, the store, the environment | judge values inside objects |
| fix | `Mod`, the schema, the mod's dating | know a target slot |
| facts | `FixedMod`, the environment | choose; change the mod |
| decide | `Facts`, the policy | read the mod or the game |
| plan | `FixedMod`, `Facts`, `Targets`, the environment | serialise |
| build | `Port` or `FixedMod`, the game's bins | touch the disk |
| check | `Package`, the environment | change the package |
| write | `Package` | read anything else |

The user's four parts map to them: fixups are `fix` and the file fixups inside `load`, analysis
is `facts`, restructuring is `plan`, lowering and writing are `build`, `check` and `write`.

A caller drives the stages through a `Porter` and its `Run` (`reforge::porter`):

```rust
let env = Env::builder().install(install).history(history).names(names).schema(schema).build()?;
let porter = Porter::new(&env).policy(policy).fixups(FixupSet::standard());

let mut run = porter.run();
run.read(&bytes, file_name)?; // &RawSnapshot; the bytes are in the run's store
run.load()?;                  // &Mod; the run holds the dating, the inventory, the file provenance
run.fix()?;                   // &FixedMod; the run holds the Changes
run.gather()?;                // &Facts, the stage `facts`
run.decide()?;                // &Targets
run.plan()?;                  // &Port
run.build()?;                 // &Package
run.check()?;                 // the checks' diagnostics; the run holds what each patch came to
run.write(&[&ProjectDir::new(out)], &DiagnosticPolicy::default())?;
let report = run.report();
```

- **A `Porter` holds what a caller chose**, built once and used for any number of runs: the
  policy and the policies compared with it, the targets given to units, the fixups, the
  checks, the caps, the reworks, whether stale copies are carried and which files are kept
  each alone, whether a bank's events are cut from the game's events bank only.
  `Porter::new` takes the standard of each. They are its `Settings`, data a
  caller reads (`Porter::settings`): `declared(stage)` is the part a stage reads, as its
  snapshot holds it (`StageSettings`), and `earliest_changed(other)` the first stage, in run
  order, whose declared part differs, which is the stage a rerun under `other` resumes from
  (#ADR-0094).
- **A `Run` takes one archive through the stages and owns what passes between them**: the
  log, the names learnt (`NameBook`), each stage's snapshot, and beside them the parsed
  objects the stages work on: the mod, the dating, the file provenance, what the fixups
  changed, the facts, the port and the package. The inventory, the targets, the package's
  part of the report and what each patch came to are read from their snapshots. A caller
  reads each by its
  accessor (`archive`, the raw snapshot, `load_snapshot`, `store`, `module`, `fixed`,
  `dating`, `changes`, `facts`, `targets`, `port`, `package`, `applied`, `log`, `names`)
  after any stage. `Run::log_from` starts a run from a log of the caller's: one that holds
  what the inputs raised when the environment was built, or one with a listener.
- **Each method hands its stage the stage's sink and options** and gives the stage's result.
  `run.read` is `archive::read(bytes, file_name, &caps, &store, &mut sink)`: the archive's
  bytes are borrowed, so the caller holds them once and drops them after the call. The
  run's store is a memory layer of its own over the caller's store (`Run::in_store`, an
  empty `MemoryStore` unless given), and every byte a stage makes, the entries' among them,
  goes into the layer and none into the store under it (#ADR-0093). `Run::into_layer` gives
  the layer's blobs, which the caller flushes into a store it keeps; a run dropped leaves
  nothing. `run.read_file` reads an archive file entry by entry and never holds it whole
  (`archive::Archive::file`, #ADR-0100): its SHA-256 is taken in one pass, and a packed WAD
  stored as it is is mounted over its part of the file, which `ltk_wad` reads chunk by chunk.
  A deflated packed WAD is inflated once into the store's spool (`BlobStore::spool`) and
  mounted from there: the ingest store writes it to a scratch file in its directory, named by
  the process's id and a random part, and a store in memory holds it in memory. Its chunks are the blobs; the spool
  is removed once they are read, and kept by neither store. Each chunk, loose or packed, goes
  into the store as it is read and is dropped, the reader keeping its id alone, so a read
  that a fatal diagnostic ends leaves the chunks read before it in the store, named by no
  snapshot.
- **A run reads archives through a cache when given one** (`Run::in_cache`, a
  `snapshot::RawCache`; #ADR-0095). `read` looks the archive up by its SHA-256 before it
  inflates anything: on a hit the raw snapshot is the result, its findings raised again and
  the caller's file name in it; on a miss it reads the archive into the cache's ingest store
  (`IngestStore`), not the run's layer, and keeps the snapshot there. The ingest store sits
  under the caller's store, so the stages after `read` find the archive's bytes through the
  layer. The run takes the cache, not the porter: it is a store the run reads through, as
  `in_store`'s is, and no setting a snapshot names. A run with a cache and one without give
  one report. `reforge port`, `repair`, `load` and `inspect` take the cache's directory as
  `--store`, and the corpus scripts as `--store` or `REFORGE_STORE`; without it a run holds
  the archive in memory alone. With `--store` the commands take the archive's SHA-256 in
  place of its path: the run starts from the raw snapshot the store holds of it
  (`Start::Raw`), its file name the one it was read under, and a store that holds none is
  refused. The corpus scripts' `--from-store` gives each mod by its SHA-256, so a corpus run
  reads no archive. `load` opens the mod from the
  raw snapshot and the store. The cache evicts nothing: `RawCache::entries` lists its
  entries from their files' names, `entry_bytes(entry)` sums the blobs one names, and
  `clear(sha256)` and
  `clear_unnamed(keep)` remove an archive's entries with the blobs no other entry names and
  their facts (#ADR-0093).
- **A stage reads a mod's bytes from the store on use** (#ADR-0100). The mod and its package
  hold the run's store as `Blobs` (`Run::blobs`), and a file node holds the content id of its
  bytes and their size, not the bytes. A stage fetches a file's bytes where it reads them
  (`Mod::bytes`, `FileNode::bytes`) and drops them, or holds them for its own duration in a
  local: `load` the head of each file for its kind and size and the whole of each bin it
  parses, the file fixups what they
  make files from, `facts` the meshes, skeletons, clips and banks it judges, `fix` a bin it
  writes again, `plan` the banks it cuts, `build` the textures it puts in TEX's container and
  the bins it writes again or reports, `check` the bins and banks it reads back. The parsed
  bins stay on the bin nodes: they are the working set. When a store under the run's memory
  layer is on disk (`BlobStore::on_disk`), the run reads it through an `Lru`, which holds
  what it read up to a quarter of the working-set cap and drops the least recently read
  first, so a file two stages read is read from disk once; a run over stores in memory reads
  them as they are. A read of the ingest store under the
  run is not checked against the id (`BlobStore::get_unchecked`): the store holds only what
  a run put there or `read` found whole. A file the run makes, a file fixup's or a game bin
  taken as the mod's, goes into the run's layer when it is made. A fetch the store does not
  answer ends the stage at that fetch with `RunError::Store`, the stages before it keeping
  their results, and the stage keeps no snapshot. Before the first writer starts, the store is
  asked for every chunk the writers fetch; a writer a fetch ends leaves nothing, since it
  renames its staged files into place only when they are whole. What a stage
  holds of its fetches is counted while it holds them (`Fetched`), and a stage that held more
  at once than the caps' working set (`Caps::working_bytes`) raises
  `store.working-set-capped` at its last step and runs on. The run keeps the raw snapshot, and each stage from `load` on
  ends by writing its snapshot (#ADR-0094): `load_snapshot`, `fix_snapshot`,
  `facts_snapshot`, `decide_snapshot`, `plan_snapshot`, `build_snapshot`, `check_snapshot`.
  The snapshots are what the run holds of its stages, and the parsed objects beside them a
  cache. `Run::snapshots` gives them together as `Snapshots`, what a caller keeps to resume
  after any of them, and `Snapshots::ids` every content id they name, each once, in order:
  what a store must hold for that resume, and what a caller that sweeps its store marks. A
  run judged with an earlier run's evidence (`Run::known`) gives none:
  the evidence is no input a snapshot names, and a run resumed from one would not be judged
  with it.
- **A run starts from a snapshot in place of `read`** (#ADR-0094). `run.resume(from)`, or
  `Porter::resume(from, store)`, takes a `Start`: the raw snapshot, after which `load` runs;
  the load snapshot, after which `fix` does; or `Snapshots`, after the stage the variant
  names (`Start::Fix`: `facts` runs next in a port, `build` in a repair; `Start::Facts`:
  `decide`; `Start::Decide`: `plan`; `Start::Plan`: `build`; `Start::Build`: `check`;
  `Start::Check`: `write`). The
  load snapshot reopens the mod from the store's bytes and parses its bins
  again; the fix snapshot reads the objects of each bin `fix` wrote again from its new
  bytes (`FixedMod::reopen`) and its changes from the snapshot; the facts snapshot gives
  what the facts found, and the game the catalogs again (`Facts::reopen`); the decide
  snapshot gives the targets; the plan snapshot gives the port, its values read back from
  the store (`PortValues`); the build snapshot gives the package, its chunks named by id and
  read from the store where they are used, its patches and thumbnail read back from it; the
  check snapshot gives what each patch came to. The
  parsed
  objects are a cache of the snapshots. A run resumed from any of them writes the report
  and the package of the run that wrote them.
- **A snapshot is refused** with a `ResumeError` that says why: a raw snapshot of another
  reading, or read under other caps, which it records (#ADR-0095); a later one of another
  format or program (version and commit), or one whose stage read an input or a setting
  that is not the run's, which it names (`Input`, `Setting`); a resume after a stage whose
  snapshot the `Snapshots` lack (`Missing`), or that a repair has not (`NotInARepair`); a
  store that lacks bytes a snapshot names (`Store`), the check snapshot's patched bins among
  them, which no stage reads again and the store is asked for at the resume. The checks that
  read no bytes are `Run::start_holds`, which `resume` makes before it reopens anything, so
  a start refused by a setting reads nothing from the store. The
  caller then starts from an earlier
  snapshot. The commit is `REFORGE_COMMIT` in the build's environment when set (CI builds
  a tarball of the commit, with no `.git`), else the one the build script reads from git. A
  build with neither has none, and a snapshot it wrote is refused (`Uncommitted`): its
  version alone does not say which code wrote it. Each stage declares what it reads, and its snapshot
  holds exactly that:

  | stage | snapshot | inputs | settings |
  |---|---|---|---|
  | read | `RawSnapshot` | the archive | the caps |
  | load | `LoadSnapshot` | `LoadInputs`: the install's build, the history, the hash tables' stamp | `LoadSettings`: the mode, the file fixups, the stale keep, the files kept, the reworks |
  | fix | `FixSnapshot` | `FixInputs`: the schema | `FixSettings`: the fixups on bin values, and the table rules among them as data |
  | facts | `FactsSnapshot` | none of its own | `FactsSettings`: the reworks |
  | decide | `DecideSnapshot` | none | `DecideSettings`: the policy, the policies compared with it, the targets given to units |
  | plan | `PlanSnapshot` | none of its own | `PlanSettings`: the stale keep, the files kept, the game's events only, the other slots kept |
  | build | `BuildSnapshot` | none of its own: the install's shader bin and shader cache, the game's skin bins a port patches and a sub-character's skin0 bin it copies, the game's files and the history a repair's texture is judged stale against, the hash tables that spell a repair's paths and a patch's fields | `BuildSettings`: the DDS keep, the stale keep, the files kept |
  | check | `CheckSnapshot` | none of its own: the install's bins the patches apply to, every bin a patched bin loads and the shader bin; the target build's types, from the schema and the install | `CheckSettings`: the checks |

  `load` marks the stale copies, and `plan` and `build` judge a file again by the same rule,
  so the files kept (#ADR-0098) are `load`'s setting as the stale keep is, and a change of
  them resumes from the raw snapshot. The targets given to units are `decide`'s alone, and a
  change of them resumes after `facts`.
- **A stage runs once.** A stage ran when the run holds its snapshot (`Run::ran`), and its
  method asked again gives the result the run holds. The stage types keep the order in
  their signatures. A method called before the result it reads is held answers
  `RunError::NotReached` with both stages.
- **Stopping early is calling fewer methods.** `run.through(stage)` folds the stages up to
  that one over the snapshots: each stage the run holds no snapshot of runs from the one
  before it and ends by writing its own, so a run resumed after a stage and a run that ran
  it go on alike. `Porter::port(bytes, file_name)` reads and runs them all but `write`.
  There is no pass registry, no `after` edge and no stub.
- **`run.report()` is the report of the run as far as it went**: a part for each result the
  run holds, and every diagnostic, written with the names learnt up to then.
- **`run.write(writers, policy)`** asks the caller's `DiagnosticPolicy` first (#ADR-0003),
  then gives every writer the package and the report of the run as it stands before the
  write.

`Repairer` is the same with no policy: its run has no `gather`, `decide` and `plan`
(`RunError::NotInARepair`), `load` gathers the dating alone, and `build` builds in place
(#ADR-0011). `run.again()` takes a repair's package through `read`, `load`, `fix` and `build`
a second time, in memory, judged with the `Evidence` of the first run (its dating and its
restorations, #ADR-0041), and holds what the second run changed.

A stage ends early in three ways, each a `RunError`. A fatal diagnostic leaves nothing to
write: the stage answers `Halted` with the diagnostic's id, as every stage asked after it
does, and `through` and `port` end there with no error. A cancel ends the stage under way
after the step it is on (#ADR-0097): the stage answers `Cancelled` with its stage, as every
stage asked after it does, writes no snapshot, and `through`, `port` and `repair` end there
with no error. An input that does not answer (a game chunk that does not read, a history
query that fails) is a fault, with the stage's own typed error as its source: a run judged
against an input it could not read does not answer as if the input held nothing.

- **A cancel is a flag the caller hands the run** (`Cancel`, an atomic shared by its clones):
  `Run::cancel`, or `Porter::cancel` and `Repairer::cancel` for each of their runs. A cancel
  is no setting, and no snapshot names it. The run's log carries it to each stage's sink.
  The log holds the stage a cancel ended (`Run::cancelled_at`), and the report names it
  (`cancelled`), beside `halted`. A cancelled run writes nothing and is not repaired again;
  the snapshots of the stages that ended are whole, and a new run resumes from the last.
  `reforge port` and `repair` set the cancel on the terminal's interrupt, write the report
  of a cancelled run as a halted run's, and exit with 4 (2 is a halted run's); a second
  interrupt ends the process at once.
- **A stage reports its progress in its steps.** A stage counts its steps before the first
  (`Steps`, or `Parts` for a stage of named parts) and begins each through its sink, which
  reads the cancel and tells the log's progress listener (`Diagnostics::listen_progress`) a
  `Progress`: the stage, the step's kind and name as a `Step`, its number from 1 and the
  count. Progress holds no clock, and a step is named only for a log with a listener. The
  command registers none.
- **A step runs to its end.** `read` reports its steps and reads no cancel: its bytes are
  under the caps. The steps, each named:

  | stage | a step | named |
  |---|---|---|
  | read | one WAD, begun at its first entry | `wad`: its name |
  | load | one part (`LOAD_PARTS`): the WADs and files opened, the file fixups, the game's bins for a WAD with no skin bin, the survey, the client's load, the dating, and in a port the inventory and the file provenance | `part` |
  | fix | one fixup over the objects of one bin | `fixup`: its id and the bin's path |
  | facts | one result (`FACTS_PARTS`): the catalogs, the copies, the objects' provenance, the origins, the rigs, the clips, the reworks, the sounds, the candidates | `part` |
  | decide | one policy's decisions: the porter's, then each compared with it | `policy`: as `auto/own-rig` |
  | plan | one step of the plan (`PLAN_PARTS`): the reach and the file refs under the mod's names, the shipping, the renames, the audio, the reach and the file refs at the target, the units linked, the HUD | `part` |
  | build | in a port one unit, one bank, one file or one champion's HUD; in a repair one file | `unit`, `file`: its path, `hud`: the champion |
  | check | in a port the patches applied; then one check | `part`, `check`: its id |

  A step of `facts` or `plan` is one of its results, not one unit: each result is found for
  every unit at once, and reads the results before it.

The stages are fixed. What a caller turns on and off is inside two of them: the fixups in `fix`
and `load`, and the checks in `check`. Both are lists of objects that describe themselves
([Fixups](#fixups)).

A finding of the type check counts values and names one place per row. The place of every
value, with its path inside its object, is asked for apart (#ADR-0103): `Run::type_places`
walks the package again as `check` does and raises nothing.

## Callers

`reforge` is a library, and the command is one caller of it. A GUI is another (the user's wish,
2026-10-03): it lists the fixups and checks from their descriptions, lets a person turn each on
or off and add a rule, runs a stage at a time, and shows each stage's result. What that needs
from the library:

- every stage result after `read` prints (`Dump`) and has its part of the report, which
  serialises;
- a run stops after any stage and holds every result up to it (`Run`);
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
| `reforge-magic` | what a file is by its first bytes and name: `HeadKind`, what the first `HEAD_LEN` bytes say, the longest prefix any check reads, and the name read after it |
| `reforge-wwise` | Wwise banks and packages, read and written; a bank whose clips are larger than its data is refused |
| `reforge` | the stages, `Report`, `Porter`, `Repairer` |
| `reforge-census` | the census history (format 2) read with gix; builds and appends the history index (#ADR-0005); with the feature `fetch`, fetches the history as far as an append reads it (#ADR-0057) |
| `reforge-cli` | the `reforge` command |
| `reforge-app` | the desktop app's Tauri side: its commands, settings and generated bindings; its front end is `app/` ([ROADMAP-UX.md](ROADMAP-UX.md)) |
| `reforge-testkit` | builders for games, archives, bins, schemas and histories; a dev-dependency |

The modules of `reforge`, each using only the ones above it:

| module | holds |
|---|---|
| `model` | ids and arenas, hashes and keys, checked paths, the `Mod` and `Blobs`, the run's store its files' bytes are fetched from, bin value access and the walk over it, the view types of a skin's objects, text cleaning, `Dump`, the readers of a rig's files, an events bank's header and objects, the layers, a record's code, what a fixup says of itself (`FixupInfo`) |
| `diag` | diagnostics: the log, the sinks, filters and levels ([DIAGNOSTICS.md](DIAGNOSTICS.md)); a stage's steps: the run's `Cancel`, which the log carries to the stages, and the `Progress` it tells a listener |
| `store` | `BlobStore`: bytes by content id (#ADR-0093), read checked or unchecked, the first bytes alone with the size (`get_head`), and a spool for bytes too large to hold, read in part (#ADR-0100); `MemoryStore`, `Layered`, a store over another, and `IngestStore`, the ingest tier: a directory of one file per blob, which processes share. Every store answers the empty blob, `ContentId::EMPTY`, without holding it, so no file of no bytes is written. `FactTable` and `MemoryFacts`: rows of facts per blob by id, whose kind is text the table reads nothing into. `Lru`, a store over another that holds what it read up to a bound in bytes |
| `archive` | the only reader of untrusted zips; `RawSnapshot`, what `read` gives (#ADR-0094): the archive's WADs, files, extras and image named by content id, its bytes in the store, and what the read raised |
| `game` | the target side: `GameSource` and `GameView`, which answers it, `Game` and `Install`, `SkinCatalog`, `Names`, `NameBook` and `Speller`, `Schema` |
| `history` | `History`: the questions the stages ask, and the definition of the SQLite index ([HISTORY.md](HISTORY.md)) |
| `env` | `Env`: what a run is judged against, built once |
| `load` | `Mod` from a `RawSnapshot`: file fixups, binless mods, characters, units, loads; and what is known of the files before any value is read: `Dating`, `Inventory`, `FileProvenance`; the table of reworks |
| `fixups` | the fixup engine and every rule; a fixed bin written again as bytes (`FixedMod::rewrite`), and a fixed mod reopened from them |
| `facts` | `Facts` and each result in it: copies, object provenance, origins, rigs, clips, reworks, sounds; `UnitJudge` |
| `port` | `Policy`, `Targets`; `Port` and the steps of its plan: `Shipping`, `Reach`, `FileRefs`, `Layout`, `Renames`, `OwnClips`, `AudioPlan`, `HudPlan`; `Base`; `PortValues`, what a port holds that its report does not |
| `package` | `Package`, its builders for a port and for a repair, `PtchBuilder`, `Wired`, `TexContainer`, `Overlay`, the checks, the writers, `EvidenceDoc` |
| `peek` | `Peek`: an archive read alone, its characters by path and its thumbnail, for a page that shows a mod before a run |
| `report` | `Report` and its rows; `LoadPart`, the rows `load` gives |
| `snapshot` | the snapshot after each stage from `load` on, the mod as the stage left it named by content id (#ADR-0094), with what it was judged under: `LoadSnapshot` with `LoadInputs` and `LoadSettings`, `FixSnapshot` with `FixInputs` and `FixSettings`, `FactsSnapshot` with `FactsSettings`, `DecideSnapshot` with `DecideSettings`, `PlanSnapshot` with `PlanSettings`, `Program`; `Snapshots`, a run's together; how each reopens, and `ResumeError`; `RawCache`, the raw snapshots by archive with their bytes in an ingest store (#ADR-0095) |
| `porter` | `Porter`, `Repairer` and `Run`: the stages driven in order ([Shape](#shape)), and started from a snapshot; `codes()`, every code a run can raise |

A test (`tests/layers.rs`) reads every `crate::` path of the sources and fails when a module
uses one below it in this table. The same test fails when `facts` or `port` reads a game chunk
(#ADR-0080).

`reforge-census` keeps gix, `ltk_ritobin` and the patched `ltk_*` crates out of `reforge`: a run
asks the history questions and never walks the census. It depends on `reforge` for the index's
tables (`history`) and the bin value helpers (`model`).

## The model

`model` holds the mod as the archive ships it. It keeps forge's entities and ids (`%w`,
`%f`, `%b`, `%o`, `%c`, `%u`) and the creation order that makes a dump deterministic.

- **A `Mod` is frozen after `fix`.** `FixedMod` is a newtype only `FixupSet::apply` makes,
  or `FixedMod::reopen` from the bins a fix wrote again (#ADR-0094).
  `Facts::gather` takes `&FixedMod`, so values are compared only at the target's types.
- **No field belongs to a later stage.** A node holds what `load` found: a file's use, where it
  came from (`FileSource::Archive`, `Made { from, by }`, `Rewritten { by }`, `Game`), a unit's
  loads and entry. A target, a destination and a dropped state are in `Targets` and `Port`.
- **One file per path in a WAD.** The WAD format holds one chunk per path hash, so a `Mod` holds
  at most one file per WAD and path hash. Adding a file where one exists adds nothing and says
  which file is there. A step that would add one (a file fixup, the game's bins for a mod with
  no skin bin) leaves the mod's own file in place.
- **One object per entry hash in a bin.** Of the objects a bin lists under one entry hash, the
  first is kept, as the client keeps it (#GAME-BEHAVIOR-0020), and the bin is reported.
- **The port is new data.** `plan` clones the objects a unit carries and renames the clones.
  The mod's objects stay as the fixups left them.
- **Closed sets are enums.** A layer is `Layer::Base`, `Voice` with its locale when it is one
  locale's, or `Hud`. A unit that leaves the port has a `Left` reason. A rig check has an
  `Aspect`.
- **A file holds an id, not bytes** (#ADR-0100). A file node holds the content id of its
  bytes and their size, and gives the bytes from the run's store when a stage asks
  (`FileNode::bytes`, `Mod::bytes`), through `Blobs`, the run's store as the model names it:
  `BlobAccess`, which every store answers, a fetch and a keep by id. Nothing on the node keeps
  them. A file's kind and size are what is known of its blob (`Blobs::facts`): the kind what
  its first `HEAD_LEN` bytes say (`HeadKind`), read alone (`BlobStore::get_head`), and no
  kind needs more of the file, since every check of `reforge-magic` reads within the head.
  What a run learns of a blob it keeps, so the inventory reads each file's kind with its name
  and no bytes. A bin is read whole where it is parsed, which is no part of its kind. Beside
  the store is a table of facts per blob (`FactAccess`, #ADR-0100): a row by id holds the
  size and the kind as text, stamped with the matcher's generation
  (`reforge_magic::GENERATION`). A row of this generation is read in place of the head; a
  missing row, or one of another generation or a kind that does not read, means the head is
  read and the row written again. The rows a run learnt are written together at the end of
  `load`, whatever ended it (`Blobs::keep_facts`). A run in a cache reads and writes the
  cache's table (`FactTable`, one SQLite file in the ingest directory, WAL mode, a write
  tried again while another process holds the lock), and any other run a table in memory
  (`MemoryFacts`), so a stage does not know which. A blob many mods share is read once. A fetch the store does not answer gives no bytes: it unwinds with `Missing`, which no
  panic hook sees, and the run takes it back where it ends the stage with a fault
  (`Missing::catch`). The unwind is the way out because a fetch sits deep in every stage's
  readers, under signatures that have no store fault to give. The id is remembered too
  (`Blobs::missing`), for a fetch inside a reader whose panic guard took the unwind.
- **Bytes are views.** Fetched bytes, the image's and a chunk's a stage made are a
  `ByteView` (`model::ByteView`): the type the blob store hands out (#ADR-0093), shared on
  clone and sliced without a copy.
- **Paths are checked types.** `ChunkPath` is a path inside a WAD, lowercased, with no `..`, no
  root and no drive. `WadName` is one file name with its stem and locale parsed. Only these
  reach a writer. A name spelled by a mod is `Spelled` and is cleaned when printed.
- **View types read a class once.** `SkinEntry` wraps a bin object and gives a skin entry's
  fields by name, its `AudioBlock` among them. The keys of a resolver's map, an object's links
  and the files it names are functions of `model::value`. Class and field names are constants
  in one module (`model::known`).
- **One walk.** A visitor over bin values gives each struct with its object, its path and its
  class, read-only or mutable. Links, file references, renames and fixups use it.

## The environment

`Env` is what a run is judged against: the game, the history, the names and the schema. It is
immutable and built once by `Env::builder()`.

- **`GameSource`** is the game a run is judged against, the target install, asked typed
  questions (#ADR-0080). Every stage reaches the game through it (`Env::game()`), and none of
  its questions gives a chunk's bytes:
  - `contains(path)`, `wads(path)`, `wad_of(path)`: whether the game has a file at a path, and
    the WADs holding it, the one it is read from first;
  - `holds(path, size, content)`: whether the game holds these bytes at a path;
  - `copies(wad, size, content, besides)`: the other paths of a champion's WAD holding these
    bytes, where a copy of a game file under another path is looked for;
  - `catalog(character)`: a `SkinCatalog`, every skin the game has for the character with its
    class, the slot it counts as (a chroma counts as its parent), its name, its mesh, skeleton
    and texture, its resolver and graph, its audio block and its clips; the objects of the
    skins' bins by entry, by the digest of their bytes and by the digest of their bytes after
    the entry hash; the slots whose bins hold each entry; and the slots at which the game has a
    skin bin. A skin's bins are its skin bin and what that links, breadth first, `MAX_CHAIN` at
    most (#ADR-0070);
  - `bin(path)`: the game's bin at a path, parsed, with the files its objects name and each
    object by its entry; `skin(character, slot)`: the bins of a character's skin at a slot
    (`GameSkin`);
  - `skeleton(path)`: the joints' names in order, and how many a vertex may bind to;
  - `mesh_shape(path)`: the submeshes with their vertex positions rounded to 1/16 unit, every
    position, and the highest joint index a vertex binds to;
  - `clip_joints(path)`: the hashes of the joints a clip animates;
  - `events_bank(path)`: an `EventsBank`, the header and the objects of the events bank at a
    path. The history's `events_banks` answers with the same type, per file, with the WADs
    that held it;
  - `bank_clips(path)`: the clips of the bank or the package at a path, each by its id, its
    size and the key of its bytes, which is what the census records of a bank's clips;
  - `shader_variants(path)`: a `ShaderVariants`, the variants the shader cache's table at a
    path holds programs for: the base defines, name and value pairs a key is made of, and the
    key of each variant. `shader_file(objectPath, stage)` names the table of a game shader's
    pixel or vertex programs in `ShaderCache.dx11.wad.client` (#GAME-BEHAVIOR-0029).

  A file that is absent, and one that does not read as what is asked, answer `None`. A file the
  game holds and does not give is a `GameError`. What a build before the target held is asked
  of the history, not of the game.
- **`GameView`** answers from a chunk store: `Game`, a trait for contains, size, read, the WADs
  holding a path and the build, which `Install` implements. It reads, parses and keeps what
  callers repeat: a path's content id, a parsed bin, a character's skins. The caches live as
  long as the `Env` and have no cap, so a caller that runs many mods builds an `Env` for each or
  a few. The chunks of each champion WAD by uncompressed size are a `SizeIndex`, the install's
  own or one a fake game builds from the same chunks, which `copies` reads.
- **`History`** is a trait with one method per question ([HISTORY.md](HISTORY.md), "The
  questions"). `SqliteHistory` reads the index, `NoHistory` answers nothing, and the testkit
  has one in memory.
- **`Names`** holds the tables (Mimir, text tables) and does not change. Names a run learns
  (paths the archive spells, names the port makes) are in a `NameBook` the run owns. A `Speller`
  reads both in one order for every kind of hash: the run's names, Mimir, the text tables. A
  name a table gives is checked against its hash on load, so a wrong line names nothing. Its
  `NamesStamp` identifies what loaded (#ADR-0094): the manifest's version and SHA-256 of each
  table of the hash cache that opened, and the SHA-256 of each text table file read.
- **`Schema`** answers the target build's type of a class's field: the meta database, else the
  install's bins. A revision the reader does not understand refuses the whole database: a
  skipped revision would read as a field no build declares.

A stage names the inputs it reads by what identifies each (#ADR-0094). `load` reads the
install, the history and the names: `LoadInputs` is the install's build, the history's head,
newest build and count of builds, and the names' stamp. A load snapshot carries it, and a run
resumes from the snapshot only where the environment's is the same. The schema is read from
`fix` on, so a load snapshot does not name it; the report's inputs do.

No input is found by itself. The Mimir cache directory and the Manager's schema file each have
a function that names the usual place, and the caller passes the result to the builder. An
input that is absent or does not read is a diagnostic of the `run` channel, raised by the
input's `report`. Its reason may name a path of the machine, which is the one place a report
differs between machines.

Options are not in `Env`. Each stage takes its own: `Caps` and `LoadOptions` for read and load,
`Policy` for decide, `PortOptions` for plan. A `Porter` holds a caller's choice of them and
gives each stage its own ([Shape](#shape)).

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
  mod's build read a field (`read_at_source`, from the mod's dating), the character whose skin
  entry the object is (`entry_of`), the target game's bins (`game_bin`), the names, and the
  calls that record what a fixup did.
- **The target's types** are `TargetTypes`: the meta schema, else what the install's own bins
  hold for a class and field, and the install's version, `16.19`, which orders builds across
  hotfixes where a build's number does not. A hash's stored width, 4 or 8 bytes
  (#GAME-CHANGE-0028), is not in the schema: a field the schema declares takes it from the
  install's bins, where they hold the field in the same kind.
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

- **The rest is Rust.** A fixup that combines values, chooses a class from the schema, recurses,
  reads where a struct is held or conforms a whole object to the schema implements the trait in
  code: the camera-facing event, which changes the class of a struct in a map, the gear
  condition, which changes the class of a struct in a list and renames one of its fields, mesh
  alignment,
  the spawn shape, the emission surface, the sampler's fields, the default
  technique, the texture-mult strays, the simple-particle fields (a table rule's move, and a
  removal from a complex emitter), the shader relink, the shader variant, which judges a
  material against the target's shader cache, the hoist and the wrap, retype, strip. Retype's conversions are a
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

`Facts` is a struct of typed results, gathered once in a fixed order by `Facts::gather`.

```rust
pub struct Facts {
    pub roster: Roster,
    pub catalogs: Catalogs,
    pub copies: Copies,
    pub objects: ObjectProvenance,
    pub origins: Origins,
    pub rigs: Rigs,
    pub clips: Clips,
    pub candidates: SideTable<SkinUnit, UnitCandidates>,
    pub reworks: Reworks,
    pub sounds: Sounds,
}
```

- Each result has a constructor that takes what it is found from:
  `Clips::find(&fixed, &env, &catalogs, &copies, &origins, &rigs)`. The dependency graph is the
  signatures. Where the inputs are more than four, they are the fields of an inputs type
  (`OriginInputs`, `RigInputs`).
- There is no cache and no invalidation. The mod is frozen, so a result stays true.
- **What a snapshot keeps** is `FactsFound`, every result but the roster and the catalogs,
  which `Facts::reopen` reads off the mod and asks of the game again. It holds what the
  report leaves out of a result: the joints of each clip of a rig, and the clips of each
  bank of a unit's sounds, which the report counts.
- The game is asked typed questions (#ADR-0080, [The environment](#the-environment)). No code of
  `facts` reads a chunk.
- **Copies.** Units of one character are related when their bins are the same bytes, equal once
  each bin's own slot names are normalised, near (80% or more of the normalised objects shared,
  and the same mesh, skeleton and texture) or of the same rig. Related units form clusters, and
  each has one representative: slot 0, else a member that is no chroma and whose entry names
  its own slot, else the lowest slot. A cluster that covers a skin other than the
  representative's own is a spread. A sub-character whose bins are the game's has no role.
- **Object provenance.** Each loaded object is the game's byte for byte, the game's under
  another name, or the mod's own: asked of the catalog first, then of the history.
- **Origin.** Evidence and verdict are two steps: `OriginEvidence` is gathered, `Origin::judge`
  reads it. Evidence is typed: its aspect, the skin it points at, its weight and what it rests
  on.
- **Rigs.** For each representative with a skin entry: where its mesh, its skeleton and each
  clip of its graph come from (the mod, the game, the game at the path Riot moved the file to,
  nowhere), the game skins each matches, whether it is the mod's own bytes; the submeshes of an
  own mesh against the base's and each candidate's mesh; the skeleton's joints. The game's
  skeleton of every skin of the character is read, so a unit is judged by its joints at any
  slot.
- **Clips.** `ClipsAt` sorts a unit's clips at one slot: the slot's own, the mod's own that
  move a joint of the skeleton, the mod's own that move none, the ones only other skins play
  (by skin, with how their tracks meet the unit's skeleton), the ones nothing judges.
- **A per-slot question is a method.** `facts.judge(character, unit)` gives a `UnitJudge`, and
  `UnitJudge::at(slot)` the rig checks and clip facts of the unit at any slot, so the policy
  can ask for a slot no one listed. `candidates` holds the answers for the slots a unit is
  likely to go to: the base, its own, its premium origin, and for a sub-character its
  champion's.
- **A check is typed.** A `RigCheck` is an `Aspect` and a `Fit`: an `Outcome` (fits, fits with
  a fix, does not fit, unknown) and a `Why`, an enum with the data of each reason. The text of
  a `Why` says what the check tested.
- **Reworks.** Each reworked champion of the mod is judged made before or after its rework,
  from `load`'s count of its files around the rework and from the dating.
- **Sounds.** For each representative unit whose entry holds an audio block: its tags, its
  bank units, and every copy the archive ships of a bank a unit names, WAD by WAD, with its
  role (media, events, a header alone, a package), its version and its locale. Each clip of a
  copy is the game's when the history holds its bytes at any build, else the mod's own.
  Without a history a clip is the game's when the game's bank at the same path holds it
  under the same id, and an older clip of the game then passes as the mod's own. A bank a
  repath tool moved counts at the game's path. The units of the game's skin at the unit's own
  slot that its block leaves out are those the history shows the skin held when the mod was
  made, by the dating's bound (#ADR-0063). A mod the history does not date leaves none out,
  and neither does a block that names none of the game's units there: it was made from another
  skin's. A game unit is left out only when the block holds no unit of its kind, voice or
  effects: one that does swapped it for another skin's unit, whose game banks a port does not
  carry.
- **The roster** is the mod's groups, characters and units as far as a policy needs them: ids,
  source slots, whether a unit has an entry. With it `Facts` holds everything `decide` reads.
- **Faults.** A game chunk that does not read and a history that does not answer end
  `gather` with a `FactsError`.
- **Diagnostics** are raised by `Facts::report`, from the results: the copies, the origins
  left open, the submeshes copied, the rig files that do not parse and the banks that do not
  read.
- Facts that do not read object values (`Dating`, `Inventory`, `FileProvenance`) are gathered on
  `&Mod` before `fix`, which needs the dating. They are in `load`, since `fixups` comes before
  `facts` in the module order. `Facts::gather` takes the dating and the file provenance as
  inputs.

## Decide

`Policy::decide(&Facts) -> Targets` is a pure function. A `Policy` is how the target is chosen
(`TargetPolicy`: by the facts, the base, the origin, the unit's own slot, a slot given) and the
level of animation reuse (`AnimReuse`). It reads neither the mod nor the game.

- `Targets` holds one `Decision` per representative unit: the chosen slot or none, a `Reason`,
  the unit's origin, each `Candidate` weighed with its checks (the rig checks of the facts,
  then the clips judged at the policy's level), whether the origin is on the base's rig, and
  the chromas of the target.
- **The order of the choice** under `TargetPolicy::Auto`: a unit made for a model before its
  rework gets no target (#ADR-0019); a sub-character follows its champion (#ADR-0013,
  #ADR-0017); a unit with no rig stays unjudged at its slot; a unit in a premium slot that is no
  spread stays there (#ADR-0012); every other unit goes to skin0 where it fits, else to its
  premium origin, else to the skin whose clips it mostly plays (#ADR-0015), else to the
  premium skin whose skeleton file it takes (#ADR-0062), else stays on skin0 for a person to
  decide (#ADR-0016).
- **A target given to one unit** (`UnitTarget`, #ADR-0098) names the unit by its character
  and the slot the mod ships it at, and gives the slot it ships to or none.
  `Policy::decide_with(&Facts, &[UnitTarget])` takes it in place of the policy's choice for
  that unit, after the rework's rule and before a sub-character's following its champion, and
  judges it as the policy's fixed slot (`Considered::PerUnit`): the reason is
  `Reason::PerUnit`, and a slot the work does not fit is flagged as a forced one is. A
  champion's unit given a target takes its sub-characters along. Every unit not named follows
  the policy, and a target that names no unit of the mod changes nothing. `Targets` holds the
  targets as given, and the policies compared with it decide without them.
- **Two units on one slot.** One keeps it: a unit whose entry reads, then the unit written for
  the slot, then a skin0 unit, then one that is no spread. The others get no target. When the
  best two are of one rank, each unit on the slot leaves it to the game (#ADR-0076).
- **The diagnostics are made from the decisions afterwards**, by `Targets::report`. A decision
  keeps what they need: what it chose before another unit took the slot, the unit that took
  it, whether a forced target does not fit, the slot at which the game has no bin of a
  sub-character.
- **Comparing policies is deciding again.** `Targets::compared(&facts, &policies)` adds what
  each other policy chooses, as an `Alternative` per unit. `Policy::other_levels` gives the
  same target policy at the other levels of animation reuse, which is what `reforge inspect`
  reports.

## Plan

`Port::plan(&PortInputs, &mut sink)` turns the mod and its targets into a `Port`. Each step is
a type with one job, each prints and serialises, and a step reads only the steps before it.
The port is linked against the game as a linker links a program: the skin entry, the resolver
and a graph equal to the game's keep the names the game finds them by, and every other carried
object takes a name of its own, which every link to it follows.

1. **`Shipping`**: the units that ship, and why each other unit leaves, as a `Left`: a copy,
   superseded, made before a rework, a sub-character left to the base or equal to the game's,
   no target, the game's own skin shipped again by a spread, a chroma folded into its parent
   (#ADR-0018, #ADR-0020), at a slot the port does not change (#ADR-0082). A unit that leaves
   holds the id of the `unit.left` diagnostic of its character. The game's own skin and the
   folded chroma are judged from what each unit reaches and names under the mod's own names,
   so `Reach` and `FileRefs` are found once before `Shipping` and once after the names moved.
   Then the slot a champion's skin0 unit goes to, among the units still shipping, is the only
   slot the port changes: the units of the champion and of its sub-characters at other slots
   leave unless `PortOptions::keep_other_slots`, and `unit.left-with-work` names the own work
   they hold that the units at the target do not, by bytes: files by content, objects by body,
   own clips. A skin0 unit that left for any reason but a copy or a superseded one fixes no
   slot, and every unit ships at its own (#ADR-0087).
2. **`Renames`**, first part: each shipping unit's `Skin<S>` entry and resolver names become
   `Skin<T>`'s, and its graph's when the mod defines one. These names are replaced in every
   object of the mod, the objects of other groups and of no unit among them: a bin of one unit
   that also holds objects under another's source names keeps them apart from its own renamed
   entry ([RUNS.md](RUNS.md), "The behaviour carried over from ltk-forge"). A rename that only
   swaps one game object for another is not taken by the objects of a unit whose rig does not
   fit its target.
   `AudioPlan` is planned here, and the literal sound names are rewritten once step 3 is
   found.
3. **`Reach`**, per unit at T: the objects its entry reaches through its load, each with a
   `Fate` (the entry, the resolver, the graph, the game's own byte for byte, carried), the
   names that resolve to the game's bins at T or to the champion's unit, the names nothing
   holds, the game bins the port keeps as links, and the objects nothing reaches
   (#GAME-BEHAVIOR-0006). A game object naming a file the mod overrides is carried, with every
   game object on the way to it. A resolver under the slot's own name stands in for a
   resolver link nothing loads at T (#ADR-0056). A name only the game's bins of another skin
   of the character hold is a `Copied`: the game's object, of the unit's premium origin when
   its bins hold it, else of the lowest slot, and the walk goes on through it. The plan names
   each copy under T's names, renames every link to it, and ships it in the unit's bin
   beside its carried objects (#ADR-0083). A file a copy names ships under the namespace when
   the mod holds it, and stays at the game's path otherwise (#ADR-0088).
4. **`FileRefs`**, per unit: each file a ported object or a copy names, as a `FileVerdict`: the game's at
   a path (in a WAD mounted with the champion, #GAME-BEHAVIOR-0007), the mod's own and
   shipped, held only by a WAD that is not mounted, or dangling. A stale copy is the game's
   (#ADR-0023). Of a skin entry only the fields a port carries count. What `Reach` and
   `FileRefs` find is reported for the units that ship (#ADR-0072).
5. **`Layout`**: the one owner of every output name and path (#ADR-0027). It gives the
   namespace, a unit's directory, a file's place by its first user, an object's name, the
   paths of the port's bins and patches, and keeps the names made for the hash tables. No path
   it makes has a file stem of sixteen hex digits (#ADR-0054).
6. **`Renames`**, second part: the name each carried object takes under the namespace, by
   group. A unit's objects follow the renames of its group only: a champion and its
   sub-characters at one target. A resolver's keys stay.
7. **`Port`**: the objects the port answers for, cloned, with the renames and the places
   applied; the files with their places; the units; the HUD edits; the audio.

- **The mod's objects are not changed.** `Objects` gives a step the clone an earlier step
  changed, else the mod's own.
- **`HudPlan`** (#ADR-0029): for each champion, the files of the mod that override what the
  game's root bin names, each with its place, and the edits of the deepest fields that name
  them. An edit on a path a table does not spell is left out (#ADR-0031 is not built).
- **`OwnClips`** is the own clips of each unit's banks, from the facts' sounds. `Shipping`
  reads it: a unit with an own clip no other unit carries is not the game's own skin, and a
  chroma with an own clip its parent's unit lacks does not fold.
- **`AudioPlan`** is planned after the first renames. The game's audio block at the target
  stays, and each media bank or voice package of the mod with own clips becomes one bank
  unit after the game's:
  - a bank at a path the game's units at the target load holds clips under the game's ids,
    which the game's events play (#GAME-BEHAVIOR-0012). Its unit is a media bank of the own
    clips and an events bank with no object;
  - a bank at any other path gets a unit with events of its own. The events that reach an
    own clip are cut from the game's events bank at the unit's events path, else from the
    mod's own events bank of the client's version (#ADR-0030), the first whose events reach an
    own clip (#ADR-0075), else from the game's events bank at the target build at a path where
    one of the unit's older events banks was the game's (#ADR-0084). Every other object of the
    source stays (#GAME-BEHAVIOR-0017). A kept event is renamed under the mod's tag,
    `<base tag>Mod<id>`, token by token, and the tag goes after the game's
    (#GAME-BEHAVIOR-0011);
  - a unit of the mod with the name of a game unit and none of its banks takes that unit's
    place in the block (#MOD-ARTIFACT-0028);
  - a game unit the mod's block leaves out (Facts, "Sounds") is left out of the block, at the
    unit's own slot only: a block made for another skin names that skin's units
    (#ADR-0063). A unit with no own clip then still gets an audio plan, of the units left
    out alone.

  A bank made is a `reforge_wwise::Bank` the build writes. Its header is the header of an
  events bank the game's units at the target load, so it is of the client's version
  (#GAME-BEHAVIOR-0015). Its id is the hash of its unit's name (#GAME-BEHAVIOR-0014), and
  its file is named by its role, `sfx_audio.bnk` and `sfx_events.bnk` (#GAME-BEHAVIOR-0013).
  A media bank made from a mod's bank holds the buses that bank defines and the game's init
  bank does not, beside the own clips (#ADR-0064).
  Voice goes to the layer `voice`, and a voice bank of a locale WAD to `voice-<xx-xx>` in that
  WAD (#ADR-0028). A locale WAD's bank of a unit that is not voice goes where the champion WAD's
  would (#ADR-0073). A locale copy with the champion WAD's own clips is carried once, from the
  champion's WAD, and one with other own clips is not (#ADR-0074). A clip the game shipped is
  never carried.
- **Literal sound names.** Once `Reach` is found at the target, the event names the carried
  objects and the graph of a unit spell out are rewritten token by token: a tag the target's
  block does not hold becomes the mod's tag where the base layer's units kept that event
  under it, else the base's tag where the game's units at the target list that event, else
  it stays. A layer's renamed event is never named, so a literal name does not depend on a
  layer being on.
- **The base of a comparison is a question of its own.** `Base` answers "the game's object
  under this entry" for every comparison of the plan and of the build: whether a reached
  object is the game's, whether a graph is the game's, whether a carried field differs
  (#ADR-0080). It answers from the target build. The code that compares does not say which
  build.
- **The game is asked typed questions** (#ADR-0080): a parsed bin at a path, the catalog, the
  WADs holding a path. No code of `port` reads a chunk.
- **Faults.** A game chunk that does not read and a history that does not answer end `plan`
  with a `PlanError`.
- `PortOptions` holds whether stale copies are carried, the files kept each alone, and
  whether a bank's events are cut from the game's events bank only.

## Package

`package` holds the output and everything after it.

- **`Package`** is layers of chunks and patches, the hash tables, the project's config and the
  evidence of the run that built it. A
  chunk is keyed by `(WadName, ChunkPath)`, names its bytes by content id and size, and says
  where they come from (`Origin`). A chunk a stage made holds its bytes; a file of the mod kept
  as it is holds none and is never read to be carried: the package reads it from the run's
  store (`Package::bytes`) where a check, the report or a writer uses it (#ADR-0100). Origin: a file
  of the mod kept as it is, a bin written again with what it was built to hold, a texture put
  in TEX's container, a bin a port made, a bank a port made. A `Patch` is a typed manifest entry and its PTCH: the
  game bin it patches, the PTCH's place in its layer, the links to add, and what it is of (a
  unit's skin bin, a champion's root bin). `Package::manifest(layer)` is the one place a
  layer's `game_data.yaml` is written as text: a writer writes it, and the overlay loads it.
- **Builders.** `PortBuilder` makes a package from a `Port`, `InPlaceBuilder` from a `FixedMod`.
  `PtchBuilder` collects whole objects and records by field path, and spells each field of a
  path (#GAME-BEHAVIOR-0010). A path through a field no table names is refused.
- **A port's package** (`PortBuilder`, #ADR-0026). Per unit: a PTCH on the game's `skin<T>.bin`
  with one record per carried field of the skin entry that differs from the base (`Base`),
  the icons and loading screens only when they name a file the port ships; the resolver's
  map, the game's keys with the mod's laid over, written whole under the game's name
  (#GAME-BEHAVIOR-0008, #GAME-BEHAVIOR-0019, #ADR-0006); a bin of the carried objects and a
  bin of the graph when it ships; the links the patched bin lacks. A sub-character the game ships no skin bin of at
  the target gets a whole `skin<T>.bin` in place of a patch: the game's skin0 bin renamed,
  the records applied, the links added (#ADR-0017). A unit whose target the game has no bin
  for is not written. The files ship at their places, the spell icons and HUD textures in the
  layer `hud` with a patch of the root bin (#ADR-0029). The hash tables hold the names the
  plan made.
- **A port's audio.** The unit's patch sets `skinAudioProperties.bankUnits` to the game's
  units with the base layer's units after them, and `tagEventList` likewise when the plan
  adds a tag. Each unit added is a copy of the game's first unit with its name, bank paths,
  events and voice flag set, so its types are the build's. Each voice layer has a patch of
  its own on the same bin, `overrides/<character>-skin<T>-<layer>.ptch`, that sets the whole
  block: the game's units, the base layer's, then the layer's (#ADR-0028). The banks ship as
  chunks of their layer and WAD.
- **Only what is wired up** (#ADR-0039, `Wired`). Either builder ships a file only when the
  client reaches it: a bin the client loads, a file a loaded bin names, a file that overrides a
  path the game's own bins read, a file a run made for one of those. A bin at a former path
  that nothing loads, a file nothing names, a file that only a value a fixup removed named, and
  a stale copy stay behind, each counted by its reason in the package's finding. A caller that
  asks to keep stale copies gets them, and one that names a file to keep (`KeptFile`, by its
  WAD and path, #ADR-0098) gets that file where it is a stale copy; a file named that is no
  stale copy is judged as any other. A port ships a file only when an object it answers for
  names it with the mod's own bytes: a file the game holds where the champion plays is named
  at the game's path, and a file no ported object names stays behind, both counted. A bank a
  port made ships only with the record that names its unit. A bank of the mod whose own
  clips ship in a bank made is counted apart.
- **A repair's package** (`InPlaceBuilder`) holds those files at their own WAD and path. A bin
  an object of which a fixup changed is written again through `ltk_meta`'s delta writer, which
  copies every other object byte for byte, and whole where the delta writer refuses the bin's
  numbering. Every other file keeps its bytes. A file no table names goes under its hash at its
  WAD's root (#MOD-ARTIFACT-0007). A package that would hold nothing ends the run.
- **A material the client crashes on ends the run.** A package of either kind in which a
  material's pass links a shader the target's shader bin lacks is not built
  (`package.shader-missing`, #ADR-0069). The fixups have relinked what Riot's materials relinked.
  Nor is one in which a material's pass, on a shader of the game's shader bin, asks for a variant
  the target's shader cache lacks (`package.shader-variant-missing`, #GAME-BEHAVIOR-0029): the
  pass's defines are made from the shader's definition in the game's bin and the material's
  switches and macros (`model::material`), and the key of each technique and each child
  technique is looked up in the tables of both the pixel and the vertex programs
  (`game::asked_variants`). A shader the mod's objects define, and one with no table in the
  cache, is not judged. The fixup `shader-variant` has moved each material that one define
  brings to a compiled variant (#ADR-0092), by the same reading.
- **A repaired package is not the whole evidence of the mod.** The files left behind are part
  of what `load` judged by: a mod's date, and that a WAD restores a model from before a rework.
  A package repaired again from its own bytes alone dates differently in about half the mods,
  and judges a restoration's old files stale. So a package holds the evidence of the run that
  built it (#ADR-0041).
- **The evidence** is an `EvidenceDoc`, a document of the format `reforge.evidence/1`. It
  holds the program and its version, the kind of run, the SHA-256 of the archive, the target
  build and the history's head, the dating (the bound with its position in that history, the
  piece that sets it by its path hash or entry hash and by the digest the history is asked
  by, and the two counts), the WADs that restore a model from before a rework, the ids of the
  fixups the run ran under, and for a port the policy and each unit that ships with its
  character, its source slot, its target and the reason. In a README, a note says whether the
  README holds the run's report and how many bytes the report's JSON has; the document a run
  holds, and the report's part, have no note. It holds no id of the run's mod, no
  clock and no path of the machine, and no list by file or by object. `Run::build` sets it on
  the package from what the run holds. A run judged with an earlier run's evidence
  (`Run::known`) sets none: the piece of that dating is an id of the earlier run's mod.
- **The thumbnail** is the mod's `META/image.png` (#ADR-0051). `read` holds the entry whole,
  up to 5 MiB. `Run::build` decodes it as a PNG, a JPEG or a WebP and encodes it again as
  WebP, and sets it on the package. An image that is not carried raises
  `package.thumbnail-dropped`, and the package has none.
- **The evidence sits in the project's `README.md`.** `ltk_modpkg` gives a `.modpkg` these
  meta chunks and no other: the metadata, a thumbnail, a readme, a license text and the hash
  tables. The readme and the license text are copied byte for byte, and the license text is
  the terms the mod is shared under, so the readme takes the document. `ltk_mod_project`'s
  packer takes the readme from `README.md` at the project's root. The file is a
  heading and three sentences of fixed text with the program's version, the kind of run, the
  target build and the archive's SHA-256, then one fenced block whose info string is the
  format and whose body is the document as JSON, its keys in the order of the fields. A mod's
  text is inside the block only, in a JSON string, with every character a terminal acts on
  escaped. An archive's own readme is an entry outside its WADs, which no package carries, so
  the README of a project is the run's alone.
- **The report sits in the README too** (#ADR-0102), since the file beside a package is lost
  when the package travels alone. After the evidence's block come a line for a person and a
  second fenced block, its info string `reforge.report+zstd/1`, its body the report's text as
  the writer writes it beside the package (`Report::to_json`), compressed as one zstd frame
  at level 9 with its checksum and encoded as standard base64 in lines of 96 characters
  (`package::report_block`). The block is left out when its body passes 768 KiB, or when the
  README with it would pass the reader's 1 MiB; the evidence's note says which. The largest
  report measured, 11.5 MB of JSON, gives a body of 544 KiB.
- **`EvidenceDoc::parse`** reads the document back from a README's text: `None` for a README
  with no block of evidence, and an `EvidenceError` for a README over 1 MiB, a block of
  another format number, a block that is not closed, a second block, a body that is not the
  JSON of a document, and a field whose value is not what the field holds (a digest, a build,
  a position outside the history named, a count above its total, a policy). A field it does
  not know is ignored. It holds the report's block to its shape, one and closed, and does
  not decode it. **`Readme::parse`** reads both in one pass: the report's body is decoded with
  whitespace ignored, at any level and any line length, up to 64 MiB of JSON, and refused
  when it does not decode, is no UTF-8 or is not what the note states. A report's block in a
  README with no evidence is no report, and a block of another report format is a block of
  another language. No stage calls either: a run reads a mod archive, and judges it by its
  bytes.
- **`TexContainer`** puts a DDS at a `.tex` path in TEX's container where its blocks go in as
  they are (#ADR-0022), after either builder. A texture TEX does not hold as it is stays the
  DDS it is (#GAME-BEHAVIOR-0003), one not in whole blocks among them (#GAME-BEHAVIOR-0018).
  A run that keeps DDS puts none in TEX's container (#ADR-0055).- **`Overlay`** is a package's patches applied as LTK Manager's overlay applies them: each
  layer's manifest loaded with `ltk_game_data`, its edits applied to the game's bin, the base
  layer first and each other layer over the base layer's result. It holds, per patch, the bin
  after it with what the patch changed, the edits skipped, and for the base layer everything
  the patched bin loads. The checks read it, and the report lists each patched bin object by
  object.
- **Checks** implement `Check`: the patches applied (`Apply`), the types of every value
  written and of every field a patch changes, the class of each struct a value holds among
  them, and that every struct and object written is of a class the target build declares
  (`Types`, #MOD-ARTIFACT-0039), a bin and a bank the run wrote read
  back, the bank at the client's version (`RoundTrip`), a champion's patched base skin
  naming no other skin of the champion (`BaseSkin`, #ADR-0062). `Apply` holds that every record
  applies, that every link a patch adds leads to a bin, that every name and file a patched
  entry reaches through the port's objects resolves in what its group loads, and that every
  bank a voice layer's audio block names is the game's or ships in the layer or in the base
  layer. Names and files unresolved in the mod already, and loose
  ends of the game's own entry and resolver, are counted apart. A file the client finds by a
  fallback of its own is there (#GAME-BEHAVIOR-0009). Each has a
  `CheckInfo` and is chosen as a fixup is: a `CheckSelection` is the choice as data, a
  `CheckSet` the checks of a run. Each says what it covered. What a check finds is a
  diagnostic and does not stop `write` (#ADR-0003): `permit` asks the caller's
  `DiagnosticPolicy`, and raises `write.blocked` where it refuses.
- **Writers** implement `PackageWriter`: `ProjectDir`, `Modpkg`, `Fantome`. A writer writes
  into a new directory and renames it into place, with the report beside the package:
  `<out>/<project>/` with `reforge-report.json` in it, and `<out>/<project>_<version>.modpkg`
  or `<out>/<project>_<version>.fantome` with `<project>_<version>.reforge-report.json` beside
  it. The `.fantome` carries every layer and its game data in league-mod's layout
  (#MOD-ARTIFACT-0040, #ADR-0105); a manager that predates the layout installs its base layer
  alone. A write that fails leaves nothing, and a
  directory that is no mod project is not replaced. The evidence of a package that has one is
  written as `README.md` at the project's root with the report's block, and the packer packs
  it as the `.modpkg`'s readme, `_meta_/readme.md`, stored as it is. The thumbnail of a package that has one is written as
  `thumbnail.webp` at the project's root and named in the config, and the packer packs it as
  `_meta_/thumbnail.webp`. A layer's patches go under
  `content/<layer>/overrides/`, with its `game_data.yaml` beside them. A chunk with a part of its path longer
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
- A caller filters by code, severity, channel, stage, subject, record and the fixup or check
  that raised it, and may set a filter to block the write. None blocks by default.
- The report is written beside the package, and the package's README embeds it
  ([Package](#package)).

## Report and dump

- Every stage result after `read` implements `Dump`: `Mod`, `Changes`, `Facts`, `Targets`,
  `Port` with each step of the plan, `Package`. `reforge load --dump` prints the mod as loaded
  and, with `--fix`, what the fixups changed, `reforge inspect --dump` the facts and the
  decisions, `reforge port --dump` the plan and the package, `reforge repair --dump` the
  package. `dump_object` writes one bin object as text, its fields with their kinds and its
  names spelled, which the app's History page shows an object with.
- `Report` is a serialisable struct with a schema version (`reforge.report/1`). It is built from
  parts, each optional, so a run that stops early reports what it has. `Run::report` builds it
  from what the run holds ([Shape](#shape)). The parts: the mod, the inputs (the install
  and its build, the history's head, the schema's newest build, the hash tables' stamp), the
  WADs, files, characters and units, the dating, the inventory, the file provenance, and every
  diagnostic with its data. A later stage adds its part: `fix` what each fixup changed, `facts`
  the copies, the objects that are the game's, the origins with their evidence, the rigs, the
  clips at each candidate slot, the candidates, the reworks and the sounds, `decide` the policy
  with each decision and what the other policies choose, and the targets given to units as
  given when there are any, `plan` the `Port` as serde writes it,
  without the values `PortValues` keeps
  (the units that ship and why the others left, the reach, the file verdicts, the renames, each
  unit and each file's place, the audio, the HUD edits), `build` and `check` the package with
  every chunk's digest and, for a bin, the digest of each object as
  `ltk_meta` writes it, and every patch with the game bin it patches and the digest of each
  object of that bin after the patch, and as the part `evidence` the document the package
  embeds ([Package](#package)), field for field but the README's note on the report. A run writes the package part and the
  evidence from its build snapshot, with each patch's outcome from its check snapshot
  (`AppliedRow`, `PackageRow::apply`). `reforge repair --again` adds what repairing
  the package a second time, in memory, changed.
- The rows `load` gives, the WADs, files, characters and units, the dating, and in a port the
  inventory and the file provenance, are `LoadPart`. The report holds them under the same
  names, and a load snapshot carries them with what the rows do not say: where each bin's
  links lead and why a bin did not read, the groups, the files a table named, the names
  learnt, and the findings of `read` and `load` as `Logged` ([DIAGNOSTICS.md](DIAGNOSTICS.md),
  "Through the run"). `wads()` gives each WAD's files by path hash with the id of their
  bytes. Its format is `SNAPSHOT_FORMAT`, 2, the format of every snapshot.
- Every snapshot is JSON, the raw snapshot among them. `snapshot::to_json` writes one,
  cleaned as the report is, and `snapshot::from_json` reads one back as the stage asked for:
  they are the one place a snapshot meets its format. A snapshot type (`StageSnapshot`) holds
  its fields and names its stage, and knows nothing of the format. The raw snapshot carries
  no `SNAPSHOT_FORMAT`: its reading version, `RAW_VERSION`, is raised when its shape
  changes, and with its caps it is refused as the snapshot `read` gave is (#ADR-0095). A
  WAD's files are a list in hash order, each hash in sixteen hex digits.
- The fix snapshot holds each WAD's files by path hash as `fix` left them, a bin a fixup
  changed under the id of the bytes `FixedMod::rewrite` wrote, which a repair's package
  ships; what each fixup changed, a field path with the class of each struct on it, which
  the report's hash form leaves out; the names learnt; and the findings from `read` to the
  end of `fix`.
- The facts snapshot holds what the facts found (Facts, "What a snapshot keeps") and the
  findings to the end of `facts`. Its files are the fix snapshot's: `facts` changes none.
- The decide snapshot holds the `Targets` with what the compared policies choose, and the
  findings to the end of `decide`. Its files are the fix snapshot's.
- The plan snapshot holds the port as the report writes it, and `PortValues`: what the
  report leaves out and `build` writes, each by the content id of its bytes in the store,
  the objects the port answers for and each unit's copies as bins of one object, each HUD
  edit's value, each bank made. A bank is kept as its bytes. Each WAD's files as the port
  places them, the banks made among them; the names learnt; and the findings to the end
  of `plan`. The plan's working state that no later stage reads is not kept
  (`port::values`).
- The build snapshot holds the package's part of the report as `build` gave it, no patch
  applied (`PackageRow`), and the package by the content id of its bytes: each layer's chunks
  by WAD and path, each patch's PTCH, the thumbnail. Beside each id is what a check and a
  writer read that the bytes do not say: the project's config, the hash tables, the
  evidence, each chunk's size and where its bytes come from, a patch's links and the skin entry it
  patches, and for a bin the run wrote, the entries it was built with and each object
  written as a bin of that object alone (`model::bin::write_one`); and the findings to the
  end of `build`.
- The check snapshot holds what applying each patch came to, in the package's order, as the
  report writes it (`AppliedRow`), with the bytes of the bin after the patch by content id,
  and the findings to the end of `check`. Its package is the build snapshot's: `check`
  changes none. What the checks read of the overlay besides, the fields each patch changed
  and every object a patched bin loads, is not kept: `write` reads none of it.
- The corpus scripts (`scripts/corpus/`) compare the reports of a run with the baseline's
  reports of the same mods (#ADR-0044).

## Untrusted input

- `archive` is the only module that reads a zip, under the caller's `Caps`. It reads an
  archive file entry by entry, holds an archive in memory once, and puts the entries' bytes
  into the blob store under their content id and nowhere else (#ADR-0093, #ADR-0100). On
  disk that is the ingest store, a file per blob named by its id. A deflated packed WAD is
  inflated there into a scratch file named by the process's id and a random part, never the
  archive's, removed once its chunks are read and on an error too. When the store is next
  opened, a scratch file whose process is gone is removed, and one older than a day by its
  own modified time whatever its process; a running process's file stays. The raw cache's entry beside the blobs is named by the archive's SHA-256, the reading
  and the caps, and holds the raw snapshot's JSON, the mod's names in it as JSON strings. The
  table of facts per blob beside them holds ids, sizes and kinds, and no name.
- No parser of mod bytes ends a run by a panic. A reader of this repository's own returns an
  error on any input (banks, file kinds). A reader from another crate runs where its panic is
  caught (bins, packed WADs, meshes, skeletons, clips, the mod's image).
- What a parser allocates is bounded by the bytes it is given: the archive's caps bound what is
  inflated, and a bank whose clips are larger than its data is refused. A failed allocation
  aborts the process and no panic guard catches it, so a size a reader from another crate
  reserves before it reads is checked first: a packed WAD whose header declares more chunks
  than its bytes can list is refused before `ltk_wad` mounts it, and one over the cap after
  that. The cap, 262,144, is well above the 90,012 chunks of the game's largest WAD. A
  stage's working set, the bytes of the mod it holds at once from the store, has a cap of its
  own, which a stage passes with a note and no refusal (#ADR-0100). The
  mod's image is decoded under limits on its width, its height and the memory the decoder
  asks for (#ADR-0051).
- A mod's text reaches a terminal only through the `Speller` and `clean`.
- A package's README is a file anyone can edit, so the evidence in it is read as a mod's input
  is: under a size cap, with a typed error for every refusal and no panic.
- The app's project file (#ADR-0096) carries a mod's bytes and is untrusted input as an archive
  is: its entries counted, its JSON and each blob read under the caps, a blob refused when its
  bytes are not the content its name spells, and nothing extracted under its names
  (`docs/design/ROADMAP-UX.md`, "The app's store").
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

- **Every edge case has a test of its own** (#ADR-0045). An edge case is a diagnostic code, a
  variant of a closed set, a built-in fixup, or a record the code cites. Its test builds the
  smallest fixture that shows it, calls the one stage that decides it, and asserts the typed
  result and the diagnostic by type. It reads no dataset, no game copy and no history index.
  [CASES.md](CASES.md) holds each case's baseline from the corpus.
- **A case's test is in the file of the stage that decides it**, in the crate's `tests/`
  directory: `archive.rs`, `load_*.rs`, `fixups_*.rs`, `facts_*.rs`, `port_decide.rs` for the
  decision, `port_plan.rs` and `port_audio.rs` for the plan, `package_*.rs` for the build,
  the checks and the writers, `porter.rs` for a run. A test's name is a sentence that says
  the case. A test that shows a record's case cites the record by its code.
- **`tests/cases.rs` is the guard.** It reads the sources and the test files, and fails when
  no test names a code of `reforge::codes()`, a variant of a closed set it lists, a fixup of
  `FixupSet::catalog()` or a record the sources cite. Its module comment states what counts
  as naming one. It holds the list of the cases with no test, each with why. The list only
  shrinks: the guard also fails when a test names a listed case. A closed set added to the
  code is added to the guard's sets.
- `reforge-testkit` builds a game with its skins, an archive, bins, rig files, banks, a schema
  from rows and a history from facts, with no file on disk. It has one run type per stage a
  test starts from (`LoadRun`, `FixRun`, `FactRun`, `PlanRun`, `PortRun`, `PackRun`), each
  taking an archive up to that stage, a game one chunk of which does not read and a history
  that does not answer, for the faults.
- A failed assertion on a run prints the run's dump and its diagnostics. No dump is compared
  with a stored one.
- `tests/porter.rs` holds `Porter`, `Repairer` and `Run`: a port and a repair, a run a stage
  at a time, a run stopped after each stage, a halted run's report, that two runs of one
  archive give the same report, which holds the digest of every chunk, and that a run
  resumed from its load snapshot, read back from JSON, or from its raw snapshot writes the
  report and the project of the run straight through, byte for byte, and is refused when an
  input, a setting, the program or the format is another.
- `tests/layers.rs` holds the modules to their order, and `tests/catalogue.rs` holds
  [CODES.md](CODES.md) to the codes declared.
- The corpus compares reforge with forge (#ADR-0004). [RUNS.md](RUNS.md) holds each run.

## What forge has and this does not

| forge | here |
|---|---|
| the pass registry, `after` edges, `--passes` | the stage methods of a `Run`, and `Run::through(stage)`; a command per place to stop: `load`, `inspect`, `port`, `repair`; a `FixupSelection` to choose fixups |
| stub passes and milestones | the roadmap |
| `AnalysisManager`, `Preserved` | `Facts`, gathered once on a frozen mod |
| `scope`, `scope.unrepaired` | every loaded object fixed; each change in the report says whether a unit's entry reaches its object |
| a diagnostic as a sentence with counts in it | a typed value, one per code and subject |
| `Module.output`, marks on input nodes | `Targets`, `Port`, `Package` |
| `own-resolver` | not built: the resolver keeps the game's name (#ADR-0006) |
| `Names` learning through `&Env` | `Names` immutable, a `NameBook` per run |
| emit skipped without `--out` | the caller writes or does not |
