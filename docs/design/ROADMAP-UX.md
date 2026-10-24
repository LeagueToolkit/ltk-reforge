# Roadmap of the app

The plan for the first release of reforge as a desktop app, and what a person starting that
work needs to know of the library. The decisions are #ADR-0046 (a standalone app on LTK
Manager's stack), #ADR-0047 (the history index, distributed and grown on the user's machine),
#ADR-0057 (the census history fetched as far as an append reads it), #ADR-0060 (the census
history bootstrapped from a released archive, superseded by #ADR-0078: the index holds every
entry's object bytes, so a run needs no clone), #ADR-0059 (the League Toolkit caches updated
when asked and marked when behind), #ADR-0061 (what the app downloaded removed when asked),
#ADR-0101 (the game installs listed, detected and added, one chosen),
#ADR-0049 (an AI agent as a caller, proposed),
#ADR-0050 (two views of one run), #ADR-0090 (a third view, the guided one) and #ADR-0052 (the
app ports, a repair is a preset).
The library's design is in [ARCHITECTURE.md](ARCHITECTURE.md), the order of the other work in
[ROADMAP.md](ROADMAP.md).

## What the app is

A caller of `reforge`, as the command is (#ADR-0046). It holds no rule of its own about a mod:
it finds the inputs, drives a run, shows each stage's result, and writes the package.

- It lives in this repository: the crate `crates/reforge-app`, beside `reforge-cli`, and its
  front end in `app/`. `reforge` stays free of Tauri and of anything a UI needs alone, so a
  library type the front end shows is mirrored in the crate with `specta::Type`
  (`reforge_app::catalog`).
- `pnpm tauri dev` in `app/` runs it. `app/scripts/tauri.mjs` points the Tauri CLI at the
  crate, which it does not find from `app/` by itself.
- A command answers `IpcResult`, `{ ok, value }` or `{ ok, error }`, and an error is a code
  with fields and no sentence, as LTK Manager's are: the front end owns every sentence, in
  `app/messages/`. The TypeScript bindings, `app/src/lib/bindings.gen.ts`, are generated from
  the commands (`pnpm generate:types`) and committed; a test of the crate fails when they
  differ from the commands.
- The settings are one JSON file in the app's data directory, a key per setting id
  (`appearance.theme`, `inputs.game`, `inputs.installs`, `output.folder`, `output.format`).
- The theme (`app/src/styles/`) and the components are LTK Manager's, copied. reforge's own
  red, taken from Ornn's base splash art, is `app/src/styles/reforge.css` over them. LTK
  Manager's lint rules apply as its own, so the warnings it gives on its components are given
  here too.
- The pages share one column, centred, at the width the default window draws them at. A wider
  window buys margins, as LTK Manager's pages do (its `docs/ux/HOME.md`, "Layout"). A page of
  panes side by side, the Runs page and the Records page, and the Batch page with its table,
  take the whole width.
- The crate's modules: `inputs` opens the inputs once and raises what they found into every
  run's log, as the command raises it when it opens them; `state` holds them open for the
  settings as they stand; `run` takes a request (the kind, the archive, the last stage, the
  settings, the folder written in) through a new `Porter` or `Repairer` run and reads what the
  views draw from its report; `view` passes every string of a report through `clean` on the
  way.
- The stack is LTK Manager's: Tauri 2, React and TypeScript, Vite, Tailwind, Base UI
  components, TanStack router, query, form and table, zustand, zod, paraglide for messages,
  and `tauri-specta` for typed commands. Where a choice of library or convention comes up,
  LTK Manager's guidance for its two sides and its design system decide it.
- The fixer comes first. The porter follows in the same app once its own list in
  [ROADMAP.md](ROADMAP.md) allows.

## What the library gives a caller

`crates/reforge-cli/src/run.rs` is the working example: it builds the environment, drives a
run and writes the report, in some two hundred lines.

| the app needs | the library has |
|---|---|
| the inputs of a run | `Env::builder()` with an `Install`, a `History`, `Names` and a `Schema`. `mimir_cache_dir()`, `meta_cache_dir()` and `Schema::shared_copy()` name the hash cache and the schema database the League Toolkit programs share. An input that is absent or does not read is a diagnostic, and the run goes on with less |
| a repair | `Repairer::new(&env)`, with its fixups, checks and caps, gives a `Run` |
| a port | `Porter::new(&env)`, with a `Policy` and the policies to compare with |
| a run a stage at a time | `Run::read`, `load`, `fix`, `gather`, `decide`, `plan`, `build`, `check`, `write`, or `Run::through(stage)`. A stage runs once, and the run holds every result: `module()`, `changes()`, `facts()`, `targets()`, `port()`, `package()`. `Run::stages()` lists the stages of the run's kind, `Run::ran(stage)` says whether one ran to its end, and `Run::halted_at()` names the stage a fatal finding ended |
| a way to stop a run | `Run::cancel(Cancel)`: a flag set from any thread ends the stage under way after the step it is on (#ADR-0097). `Run::cancelled_at()` names the stage, and the snapshots of the stages before it stand |
| progress inside a stage | `Diagnostics::listen_progress`: each step of a stage as it begins, a `Progress` with the stage, the step's kind and name, its number and the count (#ADR-0097) |
| a mod's bytes | `BlobStore` (#ADR-0093): bytes by content id, a memory store, a layered store, an LRU and the ingest store on disk (`cacache`). A run writes into a memory layer of its own (`Run::in_store`, `Run::into_layer`), reads a file's bytes on use (#ADR-0100) and never holds the mod whole; `read` streams the archive from its file |
| a run's state | a snapshot after every stage (#ADR-0094), `Run::snapshots()`, JSON at one edge. `Run::resume(Start)` starts after any stage from them, refusing with a `ResumeError` that names the input, setting or program that differs, or the id a store lacks. `Run::start_holds(&Start)`, or `start_holds_at(&Snapshots, stage)` on borrowed snapshots, makes every check of `resume` that reads no bytes, and `resume` makes them before it reopens anything. `Snapshots::ids()` lists every content id they name. `Settings::declared(stage)` and `earliest_changed` say which stage a changed setting resumes from |
| a mod ingested once | `RawCache` (#ADR-0095, #ADR-0100): the raw snapshot kept per archive beside the ingest store, a run started from it by the archive's SHA-256, and the kind of each blob kept by the matcher's generation. `entries()` lists the archives it holds from their files' names, and `entry_bytes(entry)` sums one entry's blobs; `clear(sha256)` and `clear_unnamed(keep)` remove archives with the blobs no other names |
| the guided view's answers | a unit's own target and a file kept (#ADR-0098, #ADR-0099), settings the run declares and the command takes |
| the result as data | `Run::report()`: a `Report` (`reforge.report/1`) with a part per stage reached, each serialisable |
| the fixups as a list | `FixupSet::catalog()`: each `FixupInfo` has an id, a title, the records it implements, its phase and its default |
| a choice of fixups | `FixupSelection`: the ids turned off and on and the rules added, as JSON that is saved and loaded. A `Rule` (remove, rename or move a field) is data |
| what a fixup changed | `Changes`: per fixup, object and field path, the value before and after, and whether it was moved, removed, lost or had no conversion |
| the findings | `Diagnostics`, an ordered log of typed diagnostics, each with a code, a severity, a subject and data. `Diagnostics::listen` gives each one as it is raised. `docs/design/CODES.md` lists every code |
| a finding that stops the write | `DiagnosticPolicy`: codes or areas a caller sets to block. None blocks by default (#ADR-0003) |
| the output | `ProjectDir`, `Modpkg` and `Fantome`, the three `PackageWriter`s; the app writes the one `output.format` names, the `.fantome` by default (#ADR-0105). A package holds the evidence of its run as its readme (#ADR-0041) |
| text that is safe to show | a mod's name, description and paths are `Spelled` and reach a person through `clean`. A mod is untrusted input: [ARCHITECTURE.md](ARCHITECTURE.md), "Untrusted input" |

A record a fixup or a finding names (`#GAME-CHANGE-0005`, `#MOD-ARTIFACT-0017`) is a file
under `docs/`, written for a person: what the game or a tool does, and the repair. The app
bundles the records and opens one from the fixup or the finding that names it ("The Records
page").

## What the library lacks for the app

- **The report's types as TypeScript.** The parts of a `Report` are Rust types with `Serialize`
  only; the app's bindings are generated from its own views of them.
- **The app's store, its pages and its project.** The app keeps runs ("The app's store") and
  shows them ("The Runs page"), a rerun resumes from a kept run's snapshots, the ingest tier
  is cleared by mod, two runs are compared, a project file is exported and imported, and a
  package's evidence opens as a run. The reader of the embedded evidence that opens a
  package is the app's, and draws a view; it starts no run.
- **A summary for a person.** The report is JSON. What a person reads of a run is the app's to
  render.
- **A run over several mods.** A `Run` takes one archive. A caller that runs many loops over
  them against one `Env`, as the app's batch does ("The Batch page"). The caches of an `Env`
  grow with every mod, by an amount not measured, so the batch opens its inputs again after a
  fixed number of mods.
- **A reader of the embedded evidence.** No run takes a package as its input
  ([ROADMAP.md](ROADMAP.md), "The fixer: left before a first release").

Each is a change to `reforge` made with its test (#ADR-0045), on the branch of the app or
before it.

## The inputs

| input | where it comes from | without it |
|---|---|---|
| the game install | one of a list (#ADR-0101): the live install and the PBE the app detects (the Riot client's list of installs, a running client as `ltk_mod_core` finds it, the usual folders on every drive), and the folders the user added that open as an install | no run |
| the hash tables | the cache the League Toolkit programs share, else a folder the user picks; downloaded or updated by the app when asked (#ADR-0059) | a run names less: a chunk no table spells ships under its hash |
| the schema database | the copy the League Toolkit programs share, downloaded or updated by the app when asked (#ADR-0059), else a file the user picks | the types come from the install's bins alone |
| the history index | fetched prebuilt and taken forward from the census history (#ADR-0047, #ADR-0057) | the run is judged against the install alone, and says so |

The app says which inputs it read and how old each is, since a report differs with them: the
install's build, the newest build the schema and the history index describe, and what opening
each one raised. On a first start with no game chosen, the app lists the installs and chooses
the live one, else the first found. An install is listed by the build its
`content-metadata.json` states, and none is opened to list it; the chosen one opens with the
other inputs. The chosen install is `inputs.game`, the added ones `inputs.installs`.

On this machine (2026-10-04), the inputs open in 1.7 s with the name-hash index already built,
and a repair of a mod of 27 MB runs through check in 0.4 s and through write in 0.5 s. The
report of that run is the command's, byte for byte, with the same inputs.

## The inputs page

The page has two groups, and a button that asks every server again how the inputs stand.

- **What a run reads**: the game, the hash tables, the schema database and the history index,
  a row each.
- **What the history index is made from**: the app's clone of the census history, and the
  downloads a stop kept. No run reads either, and the group says so. It is folded under a
  disclosure titled "For maintainers", below the findings, closed unless the user left it open.

The history index is a SQLite database and the census history a Git repository, and each row
names its kind beside its title. They are two rows in two groups because they are brought up to
date in different ways and cost different amounts: the index is what a run needs, and the clone
is only where its new builds are fetched into.

A row holds, in this order:

1. Its title, and a badge of how it stands: up to date, an update published, none, being
   checked, or not opening. The game's row says none is chosen, or that the chosen install
   does not open.
2. The path read, with a button that shows the folder in the system's file manager
   (`reveal_place`, which takes the name of a place and never a path; the places are the
   folders of the hash tables, the schema database, the history index and the census clone, a
   listed install, the package the last run wrote, which the run state remembers, and the
   package a kept run wrote, which its row names). The game's row lists the installs instead
   (#ADR-0101), one line each: a mark on the chosen one, where it was found (live, PBE,
   running, added), its build or that it does not open, its path with the same button (a
   listed install's path is the one place named by a path, and any other is refused), and on
   an added one a button that takes it off the list. A click on a line chooses that install.
   The row's actions find the installs again and add a folder, which is refused when it is no
   install.
3. Short facts: whether the folder is the shared one or one the user chose, the build, the
   tables, the bytes on disk.
4. An offer for each thing the user can have done, as a box with what it does, what it costs
   and one button. Work under way shows its progress in the same box.

The index row has up to three offers: the builds after its head added from the census history,
the newest release downloaded in its place, and an index of the app's that the settings do not
name put back in use. The filled button is the cheapest that brings the index up to date.

The index, the clone and the kept downloads each show their bytes on disk and a button that
removes them, asked about first (#ADR-0061, `reforge_app::storage`). The work on the history
(a download, an update, a bootstrap, a removal) runs one at a time, and the buttons of the
others are off while one runs. A page left and come back to shows the work still under way.

## The Mod page

#ADR-0091. The page shows what an archive holds before a run: the load report, rendered
(`reforge_app::explore`). It takes the window's whole width, and adds no rule about a mod
(#ADR-0046).

**What it runs.** `explore_mod` takes a port's run with the standard settings through `load`,
as `run_mod` makes one: over the store's tiers, from the raw snapshot the ingest tier holds of
the archive when there is one, and as one run at a time with the runs of `run_mod`
(`RUN_BUSY` while one is under way). A read of the archive it makes goes into the ingest tier
as any run's does. Nothing goes to the store's rows: the app holds the last mod explored in
memory (`state::Explored`), with its raw snapshot, its load snapshot and the run's memory
layer, until the next explore replaces it. A run halted in `read` or `load` keeps what it has,
and its view names the halting finding; an input that does not answer is the view's `fault`.

**What it shows.** At the top, the mod card the run page shows, from `peek_mod` as soon as the
mod is picked or dropped on the window, with "Choose mod", "Explore" and "Run this mod". Run
this mod takes the archive and the standard settings (`explore_run`) into the run page as the
Runs page's Run again does; nothing runs until the person presses Run. While the explore runs
the page says so. Once explored, three tabs:

- **Overview**: the build the mod was made at, else "not dated", with the file or object that
  dated it and how many pieces the history dates, at the bound among them; the characters with
  their kind, WAD, unit count and a mark when every skin bin is the game's; the units with
  their character, slot and skin bin, the slot the entry's name spells when it differs, and
  how many bins load for them; the WADs with their role, layout, files, named files, locale and
  a mark for a restoration; the files per kind, from the inventory, in the kind's order, each
  kind in a word with the library's name of it on hover; and
  the files per provenance fact (the install's bytes, earlier builds', the mod's own), in the
  order the library declares the facts. Every text is the report's, read through `clean`.
- **Files**: the Runs page's files browser over the files at `load` (`explore_files`), each
  marked the same, with a badge of its provenance fact and a filter by fact beside the filter
  by path and kind; a file opens in the Runs page's viewer (`explore_file`), over the explored
  run's layer, the run tier and the ingest tier, a bin's rows named through the load's names. The listing is the function a kept run's files go through
  (`store::load_files`, `store::Tiers`).
- **Findings**: the findings of `read` and `load`, the worst first.

## The app's store

#ADR-0093, #ADR-0096. The app keeps runs in one folder of its data folder, `store/`
(`reforge_app::store`):

| path | holds | made by |
|---|---|---|
| `ingest/` | the ingest tier: each archive's raw snapshot and the bytes `read` made of it | the library's `RawCache` |
| `run-tier/` | the run tier: the bytes the runs made, by content id | `RunTier`, a `fjall` keyspace keyed by the id's 32 bytes |
| `runs.sqlite3` | the rows | `AppStore` |

The rows are three groups after #ADR-0094's, each naming the one before it by key:

| table | key | holds |
|---|---|---|
| `raw` | the archive's SHA-256 | the reading's version and the caps, so the ingest tier's entry is named; the path and file name of the last read; the mod's name, author and version as the archive spells them |
| `load` | the hex of a SHA-256 over the JSON of the archive's SHA-256, the reading's version, the caps, the program, the load inputs and the load settings, each as the load snapshot holds it | the load snapshot |
| `run` | a number, in the order written | the load it starts from, none for a package opened; the archive's SHA-256, so the listing and the sweep reach its `raw` row without a load; where it came from (`origin`: `run`, `project` or `package`) and the path a project or a package was opened from; the kind, the request's settings, the schema's build, the program, the package written, whether a person marked it kept, the time the store wrote it, the snapshot of each later stage reached, the report and the view the run ended with |
| `run_blob` | a run and a content id | the ids of the run's memory layer and every id its snapshots name, so a run resumed from an older one keeps the older one's bytes from its sweep |

A snapshot, the report and the view are their JSON in one zstd frame. The raw snapshot is not a row:
it is the ingest tier's entry.

**The format.** A `meta` table holds the rows' format (`store::FORMAT`, 2), raised with every
change of the tables (`SCHEMA`). At open, after the format is read, every table `SCHEMA`
declares is read with `PRAGMA table_info` and its columns compared with the declared ones by
name and in order, the declared ones as SQLite reads `SCHEMA` in a database in memory: a file
an earlier build made under the same number would otherwise open and fail on a missing
column. A file of another format, of other columns, or that is no database is left as it is
and the store stays closed, outdated (`StoreState::Outdated`), with what a reset loses read
from the file by queries any shape answers: its format (0 when unread), its runs, the runs
marked kept, the rows' bytes and the run tier's bytes, each 0 when its query fails. An
outdated store is never migrated (#ADR-0096): it is reset. While it is closed, outdated or
failed (another instance holds it, or a file does not open), runs are not kept, and the
commands that need it answer `STORE_OUTDATED` or `STORE_FAILED`.

**The reset** (`reset_store`, `RunStore::reset`) removes the rows' file and SQLite's files
beside it, not moved aside, since the person asked for it and copies would pile up, and opens
the store anew, which sweeps every blob of the run tier, since no row names one. The ingest
tier stays: an archive's data is still good. It works in every state: outdated, open (a
person who wants a clean store) and failed (a retry). It holds the store's lock until the
store is open again, so no command sees it half made, and it is refused while a run is under
way, and holds the run job, so none starts while it resets.

**The save.** A run is kept when it writes its package, and when its request asks to keep it
(`RunRequest::keep`) and it ends at its `through` stage, with no package: the guided view's
run through `plan` ("The guided view"). With the store, a run reads through
both tiers, and starts from a kept run ("The rerun") or from the raw snapshot the ingest
tier holds of its archive's SHA-256 when there is one, `read` then shown as ran. Once the
write succeeds, its memory layer is flushed into the run tier and synced, then the rows are
written in one transaction: the raw row, the load row unless one of its key is there, the run
row and its `run_blob` rows. A run through `check` and no further that its request does not
ask to keep, a write the settings refuse and a run with no snapshots keep nothing. The view of the run names its number
(`RunView::saved`). A save that fails leaves the run as it ran, unkept, and logs why.

**The rerun.** A run whose archive the ingest tier holds resumes from a kept run where one
holds (#ADR-0094), a run kept at its plan on request among them. Its load key is computed before it runs, by the function that keys a
saved load (`store::load_key`), from the raw snapshot, this program, the load inputs open and
the load settings its settings declare. The newest run of that load and of the request's
kind is the one resumed from (`AppStore::newest_kept_run_of`), and its row alone is
unpacked: a repair's snapshots do not resume a port. The start is the latest stage the
snapshots hold that is at or before the request's `through`, so a resume never runs past the
request. A start is checked before anything is reopened: one run checks it, then each
earlier stage down to `load` (`Run::start_holds_at`, `run::kept_start`), with no byte read
from the store, and resumes once, from the first start that holds. A refusal (a setting, an
input or the program that differs, or at the resume a store that lacks an id) is logged at
debug; when no start holds or the resume is refused, the run starts from the raw snapshot.
The stages taken from the snapshots are not run: the watch
hears each as `StageEvent::Resumed` with what it gave the views, and its `StageView` says
`resumed`, which the stage strip shows as "from store" beside the stage's state and the live
progress in place of its time. A resumed run's
report is the report of the same run from the archive. Its layer holds only what its own
stages made, so its `run_blob` rows list every id its snapshots name too: the bytes the older
run made stay while the newer names them, and an id the ingest tier holds is absent from the
run tier, which the sweep ignores.

**The ingest tier.** The ingest tier is cleared by mod, as its own action (#ADR-0093,
#ADR-0096). `RawCache::entries` lists its entries, each an archive's SHA-256, reading and
caps, from the names of their files in one directory read; `RawCache::entry_bytes` sums the
blobs one entry names, so the settings page sums the entries no `raw` row names alone and a
confirm dialog the entries of its mod alone. Clearing a mod removes every entry of its archive
and the blobs no other entry names, with their facts. "Clear unused" clears every archive no
`raw` row names. A run whose entry is gone keeps its rows and opens read-only
(`RunRow::archive_ingested`): its files no tier holds answer `BLOB_MISSING`, and a run of
its archive reads the archive again, which ingests it again.

**The bound.** The store keeps 256 runs not marked kept (`RUN_BOUND`); past it, the lowest
numbers go first. A run marked kept is not counted.

**The sweep.** A save, a removal of a run, of every run of a mod or of every run, and a change
of a run's mark each cut the bound and delete, in the same transaction, the loads no run
refers to and the raw rows no load and no run refers to. When a run went, the run tier's blobs no
`run_blob` row lists are removed after the commit, and the tier compacts its tables, which
gives back its value log's space. Opening the store sweeps the run tier too, for a rows file
a reset removed or a save a crash ended between its flush and its rows. The rows and the run tier
are behind one lock, so a sweep never runs while a save flushes. The ingest tier is not swept.

**The project file** (#ADR-0096) is a kept run with its load, its raw snapshot and the bytes
they name, as one zip with the extension `.reforge-project`:

| entry | holds | stored |
|---|---|---|
| `project.json` | `format` (1); the `raw` row (SHA-256, reading, caps, path, file name, name, author, version); the raw snapshot's JSON; the load snapshot's JSON or null; the `run` row (kind, settings, schema build, program, package, origin, the time the store wrote it, each later snapshot, the report and the view, as their JSON or null); and `blobs`, each blob's id and size | deflated |
| `blobs/<64 hex>` | one blob, its bytes under the content id the name spells | stored: a mod's bytes are compressed textures and zstd chunks already |

`AppStore::export_project` writes one: its blobs are the ids the run's `run_blob` rows list
and the ids its raw snapshot names, each once, fetched from the run tier, else the ingest
tier. One that neither holds refuses the export (`BLOB_MISSING`) before anything is written,
and the file is written under a temporary name in its folder and renamed into place when
whole. A run opened from a package has no snapshots and no bytes and is refused
(`NO_PROJECT_OF`); a run imported from a project exports as a run of the app's does.

`AppStore::import_project` reads a project as a mod archive is read, since it is untrusted
input: its entries are counted and refused past the caps' `entries`; `project.json` is read up
to the caps' `entry_bytes` and its `format` must be 1 (`PROJECT_FORMAT` otherwise); every
other entry's name is `blobs/` and 64 lowercase hex digits, listed once in `blobs`, its size
as its header says within the caps' `chunk_bytes` and all of them within `total_bytes`, and
it is read whole and hashed; any other name, a name twice, an entry the list does not name or
names at another size, a listed blob no entry holds and bytes whose hash is not the name are
refused (`PROJECT_MALFORMED`). The snapshots are read by the library as a cache entry is: a
raw snapshot of another reading, or one of another archive or caps than the `raw` row, and a
snapshot of another format are refused, as is a snapshot that names a blob the list lacks.
The entries are read twice, once to check every blob and once to put them, so a refused file
leaves nothing in either tier. The blobs the raw snapshot names go into the ingest tier and
the raw snapshot into its cache; the rest go into the run tier, under the store's lock with
the rows, flushed before them as a save is. The rows are the `raw` row unless one is there,
its path the project's own (the archive may be elsewhere on this machine: Run again then
says it is not at its path), the load unless one of its key is there, the run with the origin
`project`, the file's path and the mark kept, and `run_blob` as a save writes it. The time
the store wrote the run is the exporting store's: an import makes no run. The settings and
the program of an imported run reach the comparison through `clean`.

**A package opened** (#ADR-0041, #ADR-0096) is a kept run with no load and no snapshots.
`AppStore::open_package` bounds the file by the caps' `archive_bytes`, holds the counts the
`.modpkg`'s header declares to its bytes before `ltk_modpkg` reserves room for them, and
mounts it where its panic is caught. The metadata gives the name, the author and the version,
and the readme (`_meta_/readme.md`, read up to the evidence's own cap) gives the evidence and
the report it embeds through `Readme::parse` (#ADR-0102); a package with no readme or no
evidence in it is refused (`NO_EVIDENCE`), and one that does not read, a report's block that
does not decode among it, answers `MOD_UNREADABLE`. The run's report comes from the package
first: the report the readme embeds, else the one the writer put beside the package,
`<stem>.reforge-report.json`, read within the caps' `entry_bytes` only when the readme embeds
none; either is taken when it reads and its evidence names the package's archive. Its view is
drawn from the report alone (`Drawn::of_report`): the mod, the findings, the decisions, what
the fixups changed with no class or field and the outcome from what a change left, no objects,
and the package without a thumbnail. The report holds no `write.written`: the writer writes
it before the write raises that. Without either, the run's report is the evidence wrapped as
`{"schema": "reforge.evidence-only/1", "evidence": …}` and its view the mod from the archive's
SHA-256 and the metadata and, for a port, each unit the evidence names with its slot and
reason. Either view lists every stage of the kind as not run. The rows are a `raw` row unless
one is there (its version 0, caps `{}`, path empty, file name the package's, since the
evidence names the archive by its SHA-256 alone; `archive_ingested` is false) and the run
with the origin `package`, the package's path as where it was opened from and as its package,
the mark kept, the kind and the program from the evidence, no schema build, since the build the
evidence's inputs name is the game's and not the schema database's, and the default settings
with the fixups the evidence names, since an opened package was asked for by no request and
the evidence holds no other setting. The fixups are the choice that gives the evidence's ids:
the standard set's ids it lacks are off, the catalog's others it names are on, and an id the
catalog lacks, such as a rule's, is left out (`FixupChoice::of_ids`). A comparison with the run
that wrote the package then shows the same fixups turned off and on.

The commands are `list_runs` (newest first, a mod's text through `clean`), `run_report`,
`run_view` (the view the run ended with, its number set), `run_files`, `run_file`,
`run_settings`, `compare_runs` and `compare_files` ("The Runs page"), `export_project`,
`import_project`, `open_package`, `keep_run`,
`remove_run`, `remove_mod_runs`, `remove_all_runs`, `mod_ingest_bytes`, `clear_mod_ingest`,
`clear_unused_ingest`, `store_view` (each tier's bytes, and the ingest bytes no run's
archive names, for the settings page), `store_state` (open, outdated with what a reset loses,
or failed with why) and `reset_store`. A removal, a clear or a reset while a run is under way
is refused. A store that does not open at start, such as one another instance of the app
holds or one another build made, leaves the runs unkept, and its commands answer why.

## The Runs page

#ADR-0091, #ADR-0096. The page lists the runs the store keeps and shows one, in two panes as
LTK Manager's Mods page: the list on the left, a third of the width, and the run picked on the
right. It takes the window's whole width, which the route asks of the shell (`staticData`'s
`wide`). The page adds no rule about a mod (#ADR-0046): it renders what the commands answer.

**The list** has a row per run, newest first: the mod's name, else the archive's file name,
the author, the kind (Repair or Port), when the store wrote it as a relative time ("3 min
ago", "yesterday", the date beyond a week) with the full time on hover, a pin when a person
marked it kept and a package when it wrote one. A run kept at its plan on request ("The
save") has no package, and says the last stage it reached ("through Plan"); its Show package
is off and Run again works as for any run. A run whose archive data the ingest tier
lacks has a badge "archive data cleared" (`RunRow::archive_ingested`); a package opened,
which never had any, has none. A box above it filters by name and author. The run picked is
the route's (`/runs?run=<number>`), so a link elsewhere opens the page at a run. With no run kept,
the page says none is kept and links to the run page. While the store is not open, the page
shows one line in place of the list, that the store cannot be read or why it did not open,
with a link to Settings, where it is reset.

**Open…**, a menu beside the box (and under the empty page's line), opens a file as a kept
run: "Import project" picks a `.reforge-project` (`import_project`), "Open package" a
`.modpkg` (`open_package`), and the run opened is selected. A project file or a package
dropped on the page does the same, routed by its extension as the run page takes a dropped
archive, with a line saying so while it is held over the window. A refusal shows under the
list's head.

**The detail** has a header with the package's thumbnail (the first letter of the name when
there is none), the name, the version, the author and the file name. A run imported has a
badge "imported", and a package opened "from package", each with the path it was opened
from on hover. A run whose archive data was cleared has the list's badge there too. Six
actions follow:

- **Keep** (`keep_run`).
- **Show package** (`reveal_place` with the place `run`, which names the run and never a path),
  off when the run wrote none.
- **Run again**: the run's kind, archive path and settings (`run_settings`) set in the run
  page's store as if picked and typed, the advanced view shown and the page opened; nothing
  runs until the person presses Run. Off, with a note saying so, when no file is at the
  archive's path, and for a package opened, which no request made. A run whose archive data
  was cleared runs again from the archive.
- **Export project…**: a path picked in a save dialog that starts at the mod's file stem with
  the extension `.reforge-project`, then `export_project` ("The project file"). Off, with a
  note saying so, for a package opened.
- **Compare…**: a dialog of the other kept runs, those of the same mod first under a heading
  of their own, then the rest, each drawn as the list draws it. The run picked is compared
  with this one ("The comparison"). Off while the store keeps no other run.
- **Remove**, a menu of three, each asked about first with what it frees: this run
  (`remove_run`), every run of this mod (`remove_mod_runs`, with how many), and this mod's
  archive data (`clear_mod_ingest`, with the bytes `mod_ingest_bytes` gives, at most what it
  frees, since a blob another archive names stays).

Under them are three tabs:

- **Result**: the run page's result over the view the run ended with (`run_view`): the stage
  strip and the findings, the decisions and what the fixups changed. Picking a stage that holds
  files opens the Files tab at it.
- **Files**: the files at each stage the run holds them for, `load`, `fix`, `plan` and
  `build` (`run_files`), picked in a row of the stages. Each WAD is a root with its file count
  and bytes, a folder per part of the paths with a count, and the files named by their hash
  alone under a last folder `?`, as LTK Manager's Workshop shows them. A file has its name, an
  icon of its kind, its size and its mark: a dot coloured for changed, added or removed, the
  word on hover, a removed file struck through. A box filters by path and kind, and a switch
  hides the files that are the same. A WAD opens at first and a folder closed, so a WAD of
  thousands of files draws its rows as they are opened. The file picked opens below the tree;
  a finding's Show in bin picks the bin at `build`, opens the folders down to it, scrolls the
  tree's box to it and opens it at the value's object.
- **Report**: the report the package embeds (`run_report`), as its JSON.

For a package opened, the Files tab is one line saying the run has no snapshots, and the
Report tab starts with that line above the report. Its comparison with another run is over
the views, as any run's; its files compare with none, so the comparison's Files card shows
its empty line.

**The comparison** (#ADR-0091) takes the tabs' place in the detail, with a Back button that
gives them back. It is a diff of what the two runs answered (`compare_runs`), and adds no rule
about a mod. Its header has two columns, the run that was open on the left and the one picked
on the right, each with its name, kind, when and package file name, and a note when the two
are of different archives by their SHA-256. Under it, one card each, an empty one saying so:

- **Settings and inputs**: the request's settings that differ, the targets given to units
  (`<character>:<slot>=<slot|none>`) and the stale copies kept (`<wad>/<path>`) each in the
  library's text form and sorted, so their order changes nothing; then what the runs were judged
  under that differs: the program and the install's build, the history's newest build and
  head and the hash tables, as the load snapshots hold them, and the schema's build. A key per
  row the window names; a value in mono, a switch as on or off, a fixup by its title with its
  id, the hash tables a line each with the table, its version and the start of its SHA-256.
- **Findings**: how many are in both, then the left run's findings the right lacks, marked
  with a minus, beside the right run's the left lacks, marked with a plus, each opening to its
  details as on the run page. A finding is matched by its code, its subject and its message,
  not its id, which another setting shifts.
- **Targets**, when both runs are ports: each unit of either, matched by its character and
  slot, with each run's skin and its reason in the run page's sentence; the rows that differ
  first, marked.
- **Fixups**: each fixup that changed something in either run, with each run's counts by
  outcome; the rows that differ first, marked.
- **Package**: the two packages side by side, each with its thumbnail and what it holds.
- **Files**: the Files tab over the stages both runs hold files for, the right run's files
  marked against the left run's at the same stage (`compare_files`), "Changed only" on at
  first, a mark's word saying it is against the other run. A run that lacks the stage answers
  `NO_FILES_AT_STAGE`.

**The marks.** A stage's files are marked against the nearest earlier stage the run holds files
for: `fix` against `load`, `plan` against `fix`, and `build` against `plan` in a port and
`fix` in a repair; `load`'s are all the same. `load` and `fix` name a file by its WAD and path
hash; `plan` and `build` by the path it is placed at, whose hash is the one compared, so a file
a port moves is removed at its old path and added at its new one. A WAD of a layer other than
the base one is named under it (`hud/Ahri.wad.client`). A chunk placed at its hash is read as
that hash (#MOD-ARTIFACT-0007). A file's size and kind come from the ingest tier's table of
facts where it holds the blob, else from the blob's first bytes and `reforge-magic`; a blob no
tier holds has the kind `missing`.

**The viewer** (`run_file`) fetches the file's bytes by their id from the run tier over the
ingest tier, and shows them by the kind their first bytes give, under a read-only badge with
the path, the kind in a word (bin, image, model, audio, text, other; the library's kind on
hover), the size and the content id:

| kind | shown as | cap |
|---|---|---|
| a bin, `PROP` or `PTCH` | rows in a tree, read apart (below) | 500 rows a page |
| a texture, TEX or DDS | its largest mip decoded by the app and written as a PNG, its side at most 1024 pixels, with its size and format | a side up to 8192 pixels |
| text | the text through `clean` | 1 MB |
| anything else | the first 256 bytes as hex, sixteen to a line | |

A file over 64 MB, and a texture that does not decode, shows as hex. A mod's bytes reach the
window only as text through `clean` or as a PNG the app encoded (#ADR-0093).

**A bin** is drawn as LTK Manager's bin editor draws one, read-only (#ADR-0104). The
backend reads the bin by its content id from the run's tiers (`bin_rows::BinSource`, the
run's layer over the store's for the last run and the mod explored) and holds the last eight
parsed (`bin_rows::BinViews`). `bin_open` answers the header (the objects, a `PROP`'s version
and dependency count, a `PTCH`'s records and deletions) and one row per object, then one target
row per object a `PTCH`'s records patch; `bin_children` answers the rows under one node, 500
a page; `bin_read` answers the first page under several nodes of one object, four pages a
call at most. A node is its object's entry and its hash path, the form a type check's place
writes, `#` and the record's position under a target. Names are the run's own, as far as its
snapshots go (the plan's book holds the entries and paths the port made over the load's, so a
made bin's objects spell), over the inputs' tables, every text cleaned. The front end (`modules/bin`) draws the rows as a
virtualized tree: the objects sorted by class then name, a bin of one object opened, a tag of
the kind in ritobin's words, guide lines, the arrow keys, Home and End, and `Ctrl+←` or the
header's collapse-all button to shut every row and scroll to the top. A value-family row
(`ValueColor`, `ValueFloat`, `ValueVector2`, `ValueVector3`) draws its constant, and a
colour its swatch and the band of its keys. A row's right-click menu holds LTK Manager's copies
and nothing else: the path (the object's path, then a colon and the property path below an
object), the name and the field hash of a property, the value where it reads as one string,
the hash of a hash or a link, and the class name and hash of a struct. The badge row adds the
bin's facts and a button that copies the bin's path.

A jump to a place (the Show in bin button) carries its entry and its path; the tree opens
the rows above it, asks for the pages it needs, scrolls the row to its top and marks it with
the accent's bar and tint. A path whose row the tree does not draw, such as the `[0]` of an
option drawing its leaf on its own row, lands on the deepest row above it. Each jump, a second
one to the row shown too, brings the viewer's top to the top of the page, under its padding.
The tree is a scroll box of its own, as tall as the page less its padding and the badge row,
so the page's end does not stop that scroll short; the file tree scrolls its own box to the
file, never the page. A page's tabs keep their panels mounted, so what a person opened in the
result is as they left it when they come back from the files. Bytes no tier
holds, such as a mod's cleared from the ingest tier, are refused as `BLOB_MISSING`.

**The settings page** has a storage group with a row for the store: its runs, kept runs and
mods, the bytes of the rows, the run tier and the ingest tier, the ingest bytes no run's
archive names (`StoreView::ingest_unused_bytes`), and two actions, each asked about first:
Clear unused archive data (`clear_unused_ingest`, off when there is none) and Remove all
(`remove_all_runs`). The run tier reserves 64 MB for its journal, which the row says. At the
row's end a small Reset store asks first that it removes every run, those marked kept too.
While the store is outdated, a warning box takes the counts' place: the store was made by
another version and cannot be read, a reset keeps runs again, what it removes (the runs, the
kept ones, and the bytes of the rows and the run tier) and that the archives' ingested data
stays, with a Reset store button in the danger tone, asked about first. While it failed, the
box gives why, and the button reads Try again. A run page's result for a run that wrote its
package while the store is not open says, in one muted line, that the run was not kept. A
keep, a removal, a clear, a reset and a run kept ask the list, the views, the store's state
and its counts again.

## The Records page

#ADR-0091. The page lists the numbered records under `docs/` and shows one, in two panes as the
Runs page, at the window's whole width. The records are the repository's own text: read-only
in the app, and edited in the repository.

**The bundle.** The app's build script reads the four series, `docs/adr`, `docs/game-change`,
`docs/game-behavior` and `docs/mod-artifact`, and includes every record's file in the binary,
in series order then number order, with its code and the title its heading gives
(`docs/agents/records.md`). A file whose name or heading does not have a record's shape fails
the build with its path. The binary grows by the records' text and no more. Two commands read
it: `records` lists every record's code, series, number, title and, for an ADR, its status
line as written; `record` opens one by its code, with or without the `#`, with its Markdown,
the codes its text names (`cites`) and the records whose text names it (`cited_by`), and
answers none for a code the bundle lacks.

**The list** groups the records by series under a heading each (Decisions, Game changes, Game
behaviors, Mod artifacts), which folds the group while the page is open. A line has the code,
the title and, for an ADR that is not accepted, the first word of its status (Proposed,
Superseded, Withdrawn). A box above it filters by code and title.

**The record** has the title, the code and the series, then the record's text rendered as
Markdown with its tables (`react-markdown` and `remark-gfm`), in the app's own classes. Each
code the text names outside code is a link to that record; a code the bundle lacks stays text,
and a link into the repository reads as its text. Under the text, "Cites" and "Cited by" list
the codes as links. With no record picked, the pane says to pick one.

**The links.** The page is routed by the code without its `#` (`/records/GAME-CHANGE-0005`),
so a link anywhere opens it at that record and the list marks it. A fixup's records in the
list of fixups and a finding's record are such links.

## The History page

#ADR-0091. The page asks the history index the settings choose what the game held, and takes
the window's whole width. It reads the index of the inputs as a run does (`OpenInputs`), and
spells every hash through the hash tables of the same inputs. It needs no census clone: the
index holds every object's bytes (#ADR-0078). It adds no rule (#ADR-0046): each command is
one question to the library's `History` (`reforge_app::lookup`), and a name reaches the page
through `clean`. With no index chosen every command answers `NO_INDEX`, and the page says so
with a link to the Inputs page.

**The queries.** `history_builds` lists the builds in position order with the head.
`history_lookup` takes a path, or its path hash as sixteen hex digits, and a build (the head
when none is given), and answers the path's life (`History::path`), the files the WADs held
there at the build (`placed`), the objects that named it (`named_by`, the first 256, and
whether more did) and the bin at the path at the build with its links and entries (`bin`).
`history_suggest` answers the first 64 paths the tables spell that start with a prefix of
three characters or more (`Names::first_paths_with_prefix`, which reads the hash cache no
further than it needs: collecting every path of a three-letter prefix took 4.4 s on this
machine, 2026-10-11). `history_object` takes a bin, a path or a hash, an entry, a name or
eight hex digits, and a build, and answers the object as text: `History::object_bytes`,
`model::bin::read_stored`, then `dump_object` with the tables' names. The text is capped at
2 MB (`MAX_BIN_TEXT`), cut at a line end and said to be cut. A build that holds no such object answers none, and bytes that do not read are
`HISTORY_OBJECT_UNREADABLE`.

**The page.** A search box takes a path or a hash and lists the suggestions under it as the
person types, picked with a click or with Enter; a select picks the build, newest first, the
head chosen; Look up asks. The result shows the path, its hash, its kind and its life ("from
8.20.2483196 to 16.20.8248524, 758 builds", or "never held"), then three cards: Held in, the
WADs with the content id shortened and whole on hover; Named by, each object a link that
opens it below; and Bin, when the path is a bin at the build, with its WAD, its links as chips
that look the linked path up at the same build, and its entries in a table with a filter,
each entry a link that opens it. The Object card holds the bin and the entry, filled by those
links or typed, and shows the object's text with its class, its WAD and its life, a cut text
ending in a line that says so, as the file viewer's does. "Compare
with" picks another build: the object at that build is asked for and the card shows a unified
line diff of the two texts in place of the text, removed lines in the danger tone, added
lines in the success tone, the rest dimmed. What was asked (the search, the build, the path
looked up, the object and the build compared) is kept in the page's store, as the run page
keeps its own, and the answers in the query cache, so the page shows the same when a person
leaves it and comes back; an index of another head starts it over.

**The diff** is the front end's (`modules/history/diff.ts`): the common head and tail are
matched, and the lines between them by their longest common subsequence. A text of more than
20,000 lines, or lines between the common head and tail too many for a table of 16 Mi cells,
reads "too long to compare".

On this machine, over the index of 758 builds and the shared hash cache (2026-10-11), a lookup
of a skin bin took 1 to 12 ms, a suggestion under 1 ms and an object at a build 0.4 to 5 ms;
opening the hash tables took 1.3 s, once for the inputs.

## The Batch page

#ADR-0091. A folder of mods, or files dropped, run as one job with one set of settings: a
table of outcomes, each row its own run. The page takes the window's whole width. It adds no
rule about a mod (#ADR-0046): each row is one run's answer, and a mod's name and file name
reach the table through `clean`.

**The loop.** A `Run` takes one archive, so a batch is a loop in the app (`reforge_app::batch`,
the command `run_batch`). It takes the run job as `run_mod` does, so a run and a batch never
run at once (`RUN_BUSY`), and settings the library does not take refuse it before it starts.
It opens the inputs once and runs each archive in order through `write`, as a request of the
run page's would: the output folder is the one chosen under Settings, else each archive's own
folder. A run that wrote its package is kept in the store as any run is. The window hears a
batch through `BatchProgress`, sent as each archive starts and as it ends with its row, and
none of a run's events: each run's watch counts its findings by severity and forwards nothing.
A run that does not start, or an input that does not answer, is that row's fault, and the
batch goes on.

**The bound on the environment.** The caches of an environment grow with every mod. After
every 16 archives (`batch::REOPEN_EVERY`) the batch lets go of the open inputs
(`OpenInputs::forget`) and opens them again for the next: the plain bound until the growth is
measured. Inputs that do not open again fault that archive alone, and the next tries again.

**The cancel.** Stop sets the run job's cancel (`cancel_run`, "Two views of one run"): the
archive under way ends after its step, as a row stopped in its stage, and the batch ends after
it with the rows so far (`BatchView::stopped`).

**The page.** A card "Mods" lists the archives, each once, with a remove icon per line, their
count and Clear: "Choose folder" adds a folder's `.fantome` and `.zip` files, not its
subfolders', sorted by name (`list_archives`); "Add files" adds the files picked; every
archive dropped on the window is added; the Manager page hands over the archives it ticked
(#ADR-0106). A card "Settings" picks the kind (Repair, Port), says
where the packages go with a link to Settings, and says the standard settings are used, with a
switch that takes the run page's advanced settings in their place, less the targets given to
units and the files kept, which name one mod's. Start is off with no
archives and while a run or a batch is under way, and is Stop while the batch runs, over a line
"3 of 12 · <archive>" and a bar. The table fills in as the archives end: the file name, the
mod's name, the outcome as a badge (written in the success tone; refused and halted, with the
halting code, in warning; a fault, with its detail on hover, and an archive that does not read
in danger; stopped muted), the run's errors, warnings and notes in their severities' colours,
the time, and "Open" to the run on the Runs page (`/runs?run=<number>`) when it was kept. When
the batch ends, a line counts the rows ("9 written, 2 refused, 1 failed", refused counting the
halted and failed the archives that do not read) with its time, and "Copy summary" puts a line
per mod on the clipboard: its name, version and author, then where each of a port's units
went (`Vex base→base`, `Janna base→skin7`) or how the run ended. The table stays until the
next Start or Clear.

## The Manager page

#ADR-0106. LTK Manager's library at the window's whole width, third in the sidebar after Mod,
read from the manager's data folder (`manager_library`) and never written: a table with a box
per mod, its name, version, champions, a status (a plain `.fantome` of a champion mod, already
carries layers, a `.modpkg`, unpacked in the manager with no archive, not a champion mod) and
the day it was installed. The plain `.fantome` files of champion mods are ticked when the
library is read; "Tick the plain champion mods" ticks them again; the box in the header ticks
and unticks every mod shown that has an archive. A filter box narrows the table by name, slug,
version or champion, whatever the case, and a line counts what is ticked and shown. "Read
again" reads the library again. "Port N in a batch" adds the ticked archives to the Batch
page's list and goes there; the batch's kind, settings and Start stay the Batch page's. A mod
with no archive is listed and has no box.

## The history index

#ADR-0047. To build, in this order:

1. **A run with no index**, with what is judged with less shown to the user.
2. **A prebuilt index fetched and unpacked**: the SQLite file compressed, as a file of a GitHub
   release, with a stamp that names the published repository. The file is 2.75 GB: 891 MB
   with xz up to 16.19.8207193, 767 MB with zstd at `--ultra -22 --long=31` up to
   16.19.8230722 (2026-10-05).
3. **The builds after the index's head added** on the user's machine, from the census history
   fetched as far as the append reads it (#ADR-0057).
4. **The census history cloned whole by the app** at the user's choice, bootstrapped from a
   release's archive (#ADR-0060), to build an index with no download. A run reads an object at
   any build from the index (#ADR-0078), so the whole clone serves no run, and #ADR-0078
   supersedes the bootstrap.

The census history is public, `https://github.com/LeagueToolkit/ltk-census-history`, and the
index is published as a release of it, the files of each format under names of their own
([HISTORY.md](HISTORY.md), "Distribution"), and beside them the census history as an archive,
`ltk-census-history.git.tar.zst`, at the index's head (`history-v2-snapshot-<n>`). The stamp
names the public repository. The app takes the newest release that holds its format's files
and checks each against the SHA-256 GitHub lists for it. With no such release the index row
offers no download, and with no index it reads as none: a mod is judged against the game
alone. The database downloads into `downloads` in the app's data folder, under the release's
tag and the file's name, where a download that stops is kept and the next one resumes it while
GitHub sends the same file (`reforge_app::download`); it is unpacked once it is whole, with a
window of up to 2 GiB of memory as `--long=31` writes it. The app opens the index and installs
it in its data folder, a folder per release and format (`reforge_app::index`). The settings
then name it, and a folder they no longer name is removed at the next start. On this machine
the published database of format 0, 767 MB, downloaded and unpacked in 42 s (2026-10-05), and
that of format 1, 1,731 MB and 4.77 GB unpacked, in 159 s (2026-10-08).

The census history's row offers the archive as a download of its own, which makes the app's
whole clone, `census-history` in its data folder, or makes it again (`reforge_app::archive`). The archive
downloads as the database does, and is read as a stream: its pack's four files go into a new
bare clone, under names built from the pack's hash, the commit of `history-v2` in its
`packed-refs` becomes the clone's branch, and nothing else of the archive is written. The new
clone takes the place of the whole clone held, and the update below fetches into it from then
on. From the published release it took 191 s (2026-10-05).

The inputs page asks the census history's server for the branch's tip and offers the update
when it is not the index's head (`reforge_app::census`). The update fetches into the app's
whole clone when there is one, else into a clone of its own, `census-partial` in its data
folder, which a bootstrap removes. It appends the new builds to a copy of the index in a
folder named for the new head, `head-<commit>`, and moves the settings to it. A download of a
release older than the index is not offered. For the two builds after 16.19.8207193, the fetch
read 616 KB and the append took 4 to 7 s, most of it the copy of the database (2026-10-05).

Not settled: how one bin entry's text at a build before the head is fetched with no whole
clone. The index names the text's blob (`History::text_blob`) and a whole clone holds it
(`GitRepo::entry_text_at`). No view asks for it yet.

## The hash tables and the schema database

#ADR-0059. The inputs page reads the list of tables of Mimir's newest release and names the
tables the cache lacks or holds older, with the size to download. The download runs Mimir's
own updater into the cache the settings choose, else the shared one (`reforge_app::hashes`).
The schema database is asked for with the tag of the shared copy, and downloaded into the
shared `meta` folder with its tag beside it, in LTK Manager's files (`reforge_app::schema`).
The inputs open again once either download ends.

The app checks both, and the census history's tip, at its start, and downloads nothing unasked.
An input a download would bring up to date is marked with a warning triangle that names what
is behind when hovered: beside Inputs in the list of pages, and on the run page as a link to
the inputs page. On the inputs page the input's row says it with its badge and its offer.

## Two views of one run

#ADR-0050, #ADR-0090. The run page has three views of one run, switched as Simple, Guided and
Advanced. The simple view ports the mod through every stage with the standard settings and
writes the `.modpkg`. The guided view ports it as far as the plan, asks about each choice the
run made by a default, and writes it with the answers ("The guided view"). The advanced view
shows the stages in order, each with whether it ran, its result and its findings, and runs up
to a stage the user chooses. It ports too, and a repair is a preset of it beside the standard
settings (#ADR-0052).

The settings a run takes, each where the library keeps it, and nothing beside them:

| setting | where | repair | port |
|---|---|---|---|
| a fixup on or off, and rules added | `FixupSelection` | yes | yes |
| a check on or off | `CheckSet` | yes | yes |
| the target policy (auto, base, origin, slot, a skin) and the level of animation reuse | `Policy` | | yes |
| one unit's own target, or none (#ADR-0098) | `UnitTarget` | | yes |
| keep stale copies | `Repairer`, `Porter` | yes | yes |
| keep one stale copy, by its WAD and path (#ADR-0098) | `KeptFile` | yes | yes |
| keep each DDS as it is, out of TEX's container (#ADR-0055) | `Repairer`, `Porter` | yes | yes |
| cut events from the game's bank only | `Porter` | | yes |
| ship the units at slots other than the target too (#ADR-0082) | `Porter` | | yes |
| which findings block the write | `DiagnosticPolicy` | yes | yes |

The decide stage gives, per unit, the chosen skin, the reason, every candidate with its checks,
and what the other levels of animation reuse choose. The advanced view shows all of it, and the
user picks another policy and runs again.

- The simple view's one button runs every stage with `severity=error` blocking the write. A
  mod with no error is written by that run. A mod with one is not: the view shows every
  finding, errors first, and a second button writes it with nothing blocking, as a new run.
- The app holds no rule about a mod. It shows what the library returned and passes settings in.
- Every finding shows its code. A fixup and a finding that name a record show the record's
  code as a link that opens it on the Records page ("The Records page").
- A mod's name, description and paths reach the screen through `clean`: the crate reads its
  views of a run from `Report::to_json`, which writes every character a terminal acts on as an
  escape.
- A unit's own target stands in place of the policy's for that unit alone (#ADR-0098); the
  app passes it as it passes the policy.
- A port compares the policy with the other levels of animation reuse (`Porter::compare`),
  and the decide stage's view runs any of them with one button.
- The stages a view lists are the library's, `Run::stages()` of a run of the kind
  (`run_stages`); the front end keeps no list of its own.
- The mod card shows the mod as soon as it is picked, from the library's read of the archive
  alone (`reforge::peek`, the command `peek_mod`): its name, author and version, the
  characters its paths name with their slots, and the package's thumbnail: the WebP the
  library encoded from the mod's image (#ADR-0051), never the archive's own bytes. A run's own
  name and thumbnail replace them. A mod that does not read shows the halting finding and the
  path. The card says where the package goes, beside the mod or in the folder chosen under
  Settings, as a link to Settings, where the folder is chosen.
- A written package is named in the result by its file name, its path on hover, with a button
  that shows it in its folder (`reveal_place` with the place `written`).
- A run that ended has a Files tab beside its result once it holds files: the Runs page's
  files browser over the stages it holds files for, `load`, `fix`, `plan` and `build`, a stage
  picked in a row, each file marked against the stage before. The app holds the last run of
  `run_mod` in memory (`state::LastRun`, `run::HeldRun`) until the next replaces it: its
  snapshots, and the bytes of its memory layer the store did not take, as the Mod page holds
  the mod explored. `last_files` lists a stage through the function a kept run's files go
  through, over that layer and the store's tiers, so a run the store did not keep, one that
  halted or did not write, is browsed as a kept one is; `last_file` opens a file in the Runs
  page's viewer, a bin's rows named through the run's names as far as its snapshots go.
- The app runs the stages one at a time, as `Run::through` does, and tells the window as each
  starts and ends, and each finding as it is raised. A stage that gives a part sends it as it
  ends, built alone with the library's `ReportBuilder` as `Run::report` builds it: `fix` the
  changes, `decide` the decisions, `build` the package. The window shows each part as it
  comes, in the same place it shows the ended run's (`reforge_app::run`, `drawn`). The app
  carries no progress inside a stage: the library reports it (#ADR-0097), and the app's run
  events do not carry it.
- A run under way is stopped with the library's cancel (#ADR-0097): the run page's Run button
  is Stop while one is under way, and `cancel_run` sets the cancel the run job made at its
  start. The stage under way ends after its step; the run answers with its report and the
  stage it stopped in (`RunView::cancelled`), writes nothing and is not kept. A cancel set
  after `check` stops the write. The live progress says which stage it is stopping in, and
  the result banner which stage it stopped in.
- Every finding opens to its details. A fixup's `fixup.*` finding opens to the values it
  counts, object by object, each with its path and its value before and after. Another
  finding opens to its fields, and an id among them (`%f12`) shows the name the report's
  tables give it. A hash among them, under a key that names one (`class`, `field`, `entry`,
  `hash`, `held`, a field of `held_by`), shows the name the run's names or the inputs'
  tables spell, the hash on hover; the report keeps hex (#ADR-0103). `spell_hashes` spells
  them for a run named as a `RunRef`: a kept run by its number, the last run, or the mod
  explored. Eight hex digits are tried as an entry, a field and a class, sixteen as a path,
  at most 4096 hashes an ask.
- The type check's three findings (`check.type-mismatch`, `check.field-undeclared`,
  `check.class-undeclared`) open to a row per class and field: `Class.field`, the two shapes
  as the message writes them (`Pointer<Class>`, `Map<Hash, Embed<Class>>`), the count, and the
  first place as `WAD · bin path · entry`. A row opens to the place of each of its values,
  `WAD · bin path · entry · path`, the path's fields and its map keys of eight hex digits
  spelled. Another map key is the mod's own text, shown as the app cleaned it, a string as
  JSON. The path is read in the hash form `ltk_meta`'s `ValuePath` writes; text of another
  form shows as it came. `type_places` resumes the run
  after `build` from its snapshots against the inputs as they stand and runs the check's walk
  again (#ADR-0103): asked once per run, when a row first opens, with a line saying the package
  is read again. A run the inputs or the program no longer resume says so (`NOT_RESUMED`). The
  rows are matched by their index, which the library keeps; when the package read again gives
  another count of rows, each row shows its first place alone. A run under way and the
  comparison of two runs name no run to ask, and their rows do not open. On the run page and
  the Runs page's run detail, a place whose bin is a file of the package at `build`, matched
  by its WAD's name and its path, has a Show in bin button: it opens the Files tab at `build`
  at that file, and the viewer's tree at the place's value, or at the object for a row's
  first place, which carries no path. A place in a game bin a patch changes is
  in no file of the package, and has none.
- "What changed" holds every value the fixups changed, by fixup or by object, filtered by a
  word of its object, field, path or value and by outcome. A change carries its class and
  field and its outcome from the run's own `Changes`, which the report lists in the same
  order. A fixup's notes are shown there, with their codes, and not among the findings; its
  warnings are in both.
- A rule is edited as its JSON is written, and a choice of fixups is saved and loaded as the
  selection file the command reads with `--fixups` (`reforge_app::selection`).
- A run with an input absent asks first: with no game it does not start, and with no history
  index, hash tables or schema it says what is judged with less and offers their downloads,
  until the user tells it not to ask (`prompts.runWithLess`).

**The guided view.** #ADR-0090, #ADR-0098, #ADR-0099. "Guide me" ports the mod with the
standard settings and an empty block list through `plan`, with the live progress the simple
view shows, and asks the store to keep that run (`RunRequest::keep`, "The save"). A run that halts shows the simple view's banner. The questions are the app
crate's (`reforge_app::guided`, the command `guided_questions`), derived from the view the
window holds, so the rule of what is asked is one place and the front end renders it. One
question is asked for each finding of a kind the library decided by a default, in this order,
and none when the finding is absent:

| finding | the choices | the default | the setting |
|---|---|---|---|
| `file.stale` | leave them out; keep every one; keep one file, offered by file up to 16 files | leave them out | `keep_stale`, `kept_files` |
| `target.infeasible`, `target.unjudged`, `target.sub-left`, `target.pre-rework` | each slot of the decision's candidates the unit fits; the slot the run chose when it fits none of them, for `unjudged` as the slot it stays at; the game's own skin | the run's target | `unit_targets` |
| `target.conflict` | one unit of the conflict takes the slot and the others leave it; each leaves it | each leaves it | `unit_targets` |
| `unit.left` with a unit at a slot the port does not change | the target slot only; every slot | the target slot only | `keep_other_slots` |
| `audio.carried` with a bank whose events are cut from the mod's own events bank (#ADR-0030) | that bank; the game's bank alone | that bank | `game_events_only` |
| `rework.restoration`, when the champion has more than one unit | as the skins it is made for; one skin, every unit of the champion but the first left to the game | as the skins it is made for | `unit_targets` |
| every finding of severity `error` | refuse the write; write with what the finding says is lost | refuse the write | `block` |

The stale question comes first, since its answer runs `load` again (#ADR-0099), and the errors
come last, one by one, in place of the simple view's one button (#ADR-0003). A question names
its subject and the mod's facts from the finding's data: the finding's message, the files that
are older bytes, the units at other slots with their own work from `unit.left-with-work`, the
banks and their events. A unit's question and a conflict's carry the decision's candidates as
data (`Question::candidates`), and the window draws each as the Targets card does, beside the
choice that targets it; a unit's slots no choice gives are drawn under the choices. Its default is what the run did and gives no setting, but for an error's, which blocks
it by its code and subject. An answer is a patch of the run's settings; the window merges the
answers into the standard settings in question order, a list appended to and a switch set, and
runs through `write`. A run with nothing to ask goes on to the write with the same settings.
The result shows as the simple view's, with the answers kept: "Change answers" derives the
questions of the run that wrote, keeps an answer where a question's id and its choice's id
match, and keeps a question that run no longer asks while its answer is not the default, since
that answer is why its finding went. The run through `plan` is kept, so the first write
resumes from it as "The rerun" says, after the latest stage whose settings the answers
leave as that run had them, after its load at worst; a written run is kept too, so each run
from changed answers resumes from it the same way. The advanced view's settings show the
targets given to units, the files kept and the other slots shipped that the last guided run
set, read only under the policy, each with Clear; they name one mod's units and files, so
picking another mod clears them and a batch leaves them out.

An events bank that gives way to the game's (#ADR-0084) is asked nothing: no setting changes
which bank its events are cut from.

## The first version

1. The shell of the app: the crate, the front end, the typed commands, the settings.
2. The inputs found, shown and changed by the user.
3. A repair in the simple view: pick a mod, run every stage, show what each fixup changed and
   every finding, write the `.modpkg`.
4. The advanced view of a repair: the stages, a run up to one of them, the fixups and checks
   turned on and off, the findings that block the write.
5. The history index, as far as step 4 of the list above, and the hash tables and the schema
   database downloaded.
6. A selection of fixups saved and loaded, and a rule added in the app.
7. A port in both views, with the decision, its candidates and the policy.

Steps 1 to 7 are built.

Two faults of the crates reforge reads and packs with come before a release
([ROADMAP.md](ROADMAP.md), "Upstream"): a crafted archive can end the process, which a tool
run on downloaded mods has to withstand.

## The second version

A review of the first version's pages (2026-10-09, screenshots of every page and state over a
recorded run) found what a person trips over: the advanced view stacks the settings, the
result, the stage detail, the policy, the rule editor and the toggles into one page with the
Run button in the middle; the mod card shows only a path until a run ends, since its name and
image come from the run's report; the result speaks the library's words (`%u0`, `reason:
base`, "kept" for a converted value) and shows a full path where a name would do; a choice is
typed where it could be picked (the filter syntax that refuses the write, a rule's fields, a
target skin by number); a written package has no next step; and the inputs page shows a
maintainer's rows to every user. The decisions are #ADR-0090 (a guided view) and #ADR-0091
(the pages that explore). The steps, in order:

1. **The pages that exist, made plain.** Each a change of its own:
   - the mod read on pick, or dropped on the window: name, author, version, image and units
     shown at once, from a read of the archive alone;
   - the result banner names the package, with a button that shows it in its folder, the path
     on hover;
   - the Targets card in words: where each unit ships and why, what fits, and the other levels
     of animation reuse folded under one line, shown where they differ;
   - the advanced view in tabs: Result, Settings, Fixups and checks, Rules, with the mod card
     and the Run button above them;
   - what refuses the write picked by severity and by area under its stage, the filter under
     a disclosure; a
     target skin picked by its name among the champion's skins;
   - ids in live findings spelled as each stage ends, and "converted" where "kept" stood;
   - the output folder in Settings, the mod card saying where the package goes;
   - the census history's group folded under a disclosure for maintainers.
2. **The Mod page** and the **History page** (#ADR-0091), which share a renderer of values with
   names, and the library's read of an archive alone and its spelled history queries. Built:
   the Mod page ("The Mod page") and the History page ("The History page"). The library
   gained the read of an object as the index stores it and the first paths of a prefix; the
   spelling of its answers and the diff of two objects are the app's.
3. **The guided view** (#ADR-0090), its answers the library's settings, a unit's own target
   and a file kept among them (#ADR-0098). Built: the targets given to units, the files kept
   and the other slots shipped in a run's request, the questions derived from a run's view,
   and the guided view between the simple and the advanced ("The guided view").
4. **Runs** and **Records** (#ADR-0091): the runs kept in the app's store with their
   snapshots and bytes (#ADR-0096), reopened and resumed, a package's evidence read, two runs
   compared, a run removed and the store swept; the records bundled and opened from a fixup
   or a finding. Built: the store, its sweep and the Runs page, with a run's result, its files
   at each stage and one file opened, and the store's row in the settings; a rerun resumed
   from a kept run's snapshots, a kept run run again from the Runs page, the ingest tier
   cleared by mod and of what no run names, two runs compared, the project file exported and
   imported, and a package's evidence opened as a kept run with no snapshots; the Records
   page, the records bundled and opened from a fixup or a finding.
5. **Batch** (#ADR-0091). Built: the Batch page, a loop of runs in the app ("The Batch
   page"), and the cancel of a run under way. The library gained nothing for it: a `Run`
   takes one archive, the app holds one environment across runs (`OpenInputs`), and the
   cancel was the library's (#ADR-0097).

A review of the pages step 4 added (2026-10-11, screenshots of every state over runs of two
mods, a run resumed from another, a project imported and a package opened) found and fixed:
the Runs page's filter squeezed to nothing beside Open… in a narrow window; the path an origin
badge names on hover running past the window; a run whose archive data was cleared saying so
in the list alone; the viewer naming a file's kind by the library's id and its content id with
no word; the comparison's columns squeezed by one long value, its hash tables as JSON, its
fixups by id, its targets' reasons as the library's ids, and its marks of a row that differs
pushing that row out of line; the settings page's storage row breaking a size from its unit;
and the stage strip's "from store" on a line of its own, which made that stage's box taller
than its row. Left: the schema's build reads as the schema's number for a run of the app and
as the install's build for a package opened, which the comparison shows as a difference; live
findings name objects by hash until the run ends; and the detail's six actions wrap at the
default window.

A review of the five pages and the guided view the second version added (2026-10-11,
screenshots of every state over the inputs of a run, three mods and an archive that does not
read, a batch with a row of every outcome, questions of every kind and a run stopped in `fix`)
found and fixed: the Mod page's files per kind named by the library's ids, its units'
character in lower case, an overview of empty cards for a read that halted, and its card's
actions squeezing the archive's path in a narrow window; the History page's Escape emptying the
search box, a content id out of line with its row, and its description apart from its title;
the Records page's list running past the window's bottom, its filter scrolling out of view, its
titles a word a line in a narrow window, their Markdown backticks shown, and a code linked in
the text broken at its hyphen; the Batch page's summary running its two sentences together, a
stopped batch said to have stopped before the last mod alone, its table's counts and times out
of line with their rows, and its description apart from its title; "Guide me" broken onto two
lines; and a unit question's candidates given as sentences in the library's words (its reason
id, the check's aspect and fit).

Each library change is one rule with its test (#ADR-0045), made before the page that needs it.

## Releases

`.github/workflows/build.yml` runs the checks and builds the app's installer and the command
for Windows, on Linux (#ADR-0053), on a pull request and on a push to master.
`.github/workflows/release.yml` calls it and attaches what it built to a GitHub release of
`v<version>`, the version in the workspace's `Cargo.toml`. A pushed tag that matches the version publishes the release. A run started by
hand from the Actions tab releases the head of the branch it runs on, as a draft unless told
otherwise, and GitHub tags a draft when it is published. A version is released from one commit,
so a release of new commits starts with a version bump.

The notes list the commit subjects since the last tag. The installer is not signed and the app
has no updater, so Windows asks before running the installer and a new version is installed by
hand.

## An AI agent

Proposed, not decided (#ADR-0049): an agent as a third caller, for a mod the default settings
do not port, as a skill over the command or as an MCP server. It changes what a run is given
(a selection, a rule, a policy, a target) and not what a run does. Nothing of it is in the
first version.
