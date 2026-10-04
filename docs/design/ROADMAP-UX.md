# Roadmap of the app

The plan for the first release of reforge as a desktop app, and what a person starting that
work needs to know of the library. The decisions are #ADR-0046 (a standalone app on LTK
Manager's stack), #ADR-0047 (the history index, distributed and grown on the user's machine),
#ADR-0048 (the hash tables on the machine) and #ADR-0049 (an AI agent as a caller, proposed).
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
  (`appearance.theme`).
- The theme (`app/src/styles/`) and the components are LTK Manager's, copied. Its lint
  rules apply as its own, so the warnings it gives on its components are given here too.
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
| the inputs of a run | `Env::builder()` with an `Install`, a `History`, `Names` and a `Schema`. `mimir_cache_dir()` and `Schema::manager_cache()` name where LTK Manager keeps the hash cache and the schema database. An input that is absent or does not read is a diagnostic, and the run goes on with less |
| a repair | `Repairer::new(&env)`, with its fixups, checks and caps, gives a `Run` |
| a port | `Porter::new(&env)`, with a `Policy` and the policies to compare with |
| a run a stage at a time | `Run::read`, `load`, `fix`, `gather`, `decide`, `plan`, `build`, `check`, `write`, or `Run::through(stage)`. A stage runs once, and the run holds every result: `module()`, `changes()`, `facts()`, `targets()`, `port()`, `package()` |
| the result as data | `Run::report()`: a `Report` (`reforge.report/1`) with a part per stage reached, each serialisable |
| the fixups as a list | `FixupSet::catalog()`: each `FixupInfo` has an id, a title, the records it implements, its phase and its default |
| a choice of fixups | `FixupSelection`: the ids turned off and on and the rules added, as JSON that is saved and loaded. A `Rule` (remove, rename or move a field) is data |
| what a fixup changed | `Changes`: per fixup, object and field path, the value before and after, and whether it was moved, removed, lost or had no conversion |
| the findings | `Diagnostics`, an ordered log of typed diagnostics, each with a code, a severity, a subject and data. `Diagnostics::listen` gives each one as it is raised. `docs/design/CODES.md` lists every code |
| a finding that stops the write | `DiagnosticPolicy`: codes or areas a caller sets to block. None blocks by default (#ADR-0003) |
| the output | `ProjectDir` and `Modpkg`, the two `PackageWriter`s. A package holds the evidence of its run as its readme (#ADR-0041) |
| text that is safe to show | a mod's name, description and paths are `Spelled` and reach a person through `clean`. A mod is untrusted input: [ARCHITECTURE.md](ARCHITECTURE.md), "Untrusted input" |

A record a fixup or a finding names (`#GAME-CHANGE-0005`, `#MOD-ARTIFACT-0017`) is a file
under `docs/`, written for a person: what the game or a tool does, and the repair. The app can
show it where it shows the fixup.

## What the library lacks for the app

- **Progress inside a stage, and a way to stop one.** A stage runs to its end. Most mods take
  one to five seconds through every stage, the slowest of the datasets about 45.
- **The report's types as TypeScript.** The parts of a `Report` are Rust types with `Serialize`
  only. The `port` part is a JSON value with no type of its own.
- **A summary for a person.** The report is JSON. What a person reads of a run is the app's to
  render.
- **A run over several mods.** A `Run` takes one archive. The caches of an `Env` grow with
  every mod, so a caller that runs many builds an `Env` for each or for a few.
- **A reader of the embedded evidence.** No run takes a package as its input
  ([ROADMAP.md](ROADMAP.md), "The fixer: left before a first release").

Each is a change to `reforge` made with its test (#ADR-0045), on the branch of the app or
before it.

## The inputs

| input | where it comes from | without it |
|---|---|---|
| the game install | the user picks the folder, or the app finds it as LTK Manager does | no run |
| the hash tables | the ones on the machine, LTK Manager's first (#ADR-0048) | a run names less: a chunk no table spells ships under its hash |
| the schema database | LTK Manager's copy on the machine | the types come from the install's bins alone |
| the history index | fetched prebuilt, or built from a clone (#ADR-0047) | the run is judged against the install alone, and says so |

The app says which inputs it read and how old each is, since a report differs with them.

## The history index

#ADR-0047. To build, in this order:

1. **A run with no index**, with what is judged with less shown to the user.
2. **A prebuilt index fetched and unpacked**: the SQLite file compressed, as a file of a GitHub
   release, with a stamp that names the published repository. The file is 2.75 GB and some
   880 MB compressed.
3. **The builds after the index's head added** on the user's machine: `Indexer::append` of
   `reforge-census`, which reports its progress. It reads the census history, so it needs the
   commits after the head.
4. **The census history cloned by the app** at the user's choice, some 5 GB. A user with a
   clone builds and appends from it and needs no download.

Not settled: who builds and publishes the prebuilt index, how the commits after an index's
head are fetched with no full clone, and how one bin entry's text is fetched with no clone.

## The first version

1. The shell of the app: the crate, the front end, the typed commands, the settings.
2. The inputs found, shown and changed by the user.
3. A repair: pick a mod, list the fixups with their records and turn each on or off, run,
   show what each fixup changed and every finding, write the `.modpkg`.
4. The history index, as far as step 2 of the list above.
5. A selection of fixups saved and loaded, and a rule added in the app.
6. A port, with the decision and its candidates shown.

Two faults of the crates reforge reads and packs with come before a release
([ROADMAP.md](ROADMAP.md), "Upstream"): a crafted archive can end the process, which a tool
run on downloaded mods has to withstand.

## An AI agent

Proposed, not decided (#ADR-0049): an agent as a third caller, for a mod the default settings
do not port, as a skill over the command or as an MCP server. It changes what a run is given
(a selection, a rule, a policy, a target) and not what a run does. Nothing of it is in the
first version.
