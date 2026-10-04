# Roadmap of the app

The plan for the first release of reforge as a desktop app, and what a person starting that
work needs to know of the library. The decisions are #ADR-0046 (a standalone app on LTK
Manager's stack), #ADR-0047 (the history index, distributed and grown on the user's machine),
#ADR-0048 (the hash tables on the machine), #ADR-0049 (an AI agent as a caller, proposed),
#ADR-0050 (two views of one run) and #ADR-0052 (the app ports, a repair is a preset).
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
  (`appearance.theme`, `inputs.game`, `output.folder`).
- The theme (`app/src/styles/`) and the components are LTK Manager's, copied. reforge's own
  red, taken from Ornn's base splash art, is `app/src/styles/reforge.css` over them. LTK
  Manager's lint rules apply as its own, so the warnings it gives on its components are given
  here too.
- The crate's modules: `inputs` opens the inputs once and raises what they found into every
  run's log, as the command raises it when it opens them; `state` holds them open for the
  settings as they stand; `run` takes a request (the archive, the last stage, the settings,
  the folder written in) through a new `Repairer` run; `view` reads the views of a run from
  the report.
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
| a run a stage at a time | `Run::read`, `load`, `fix`, `gather`, `decide`, `plan`, `build`, `check`, `write`, or `Run::through(stage)`. A stage runs once, and the run holds every result: `module()`, `changes()`, `facts()`, `targets()`, `port()`, `package()`. `Run::stages()` lists the stages of the run's kind, `Run::ran(stage)` says whether one ran to its end, and `Run::halted_at()` names the stage a fatal finding ended |
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

The app says which inputs it read and how old each is, since a report differs with them: the
install's build, the newest build the schema and the history index describe, and what opening
each one raised. On a first start with no game chosen, the app looks for one with
`ltk_mod_core`, as LTK Manager does, and chooses the one it finds.

On this machine (2026-10-04), the inputs open in 1.7 s with the name-hash index already built,
and a repair of a mod of 27 MB runs through check in 0.4 s and through write in 0.5 s. The
report of that run is the command's, byte for byte, with the same inputs.

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

The census history is public, `https://github.com/LeagueToolkit/ltk-census-history`, and the
index is published as a release of it: `stamp.json`, `builds.tsv` and `history.sqlite.xz`,
the release tag naming the branch (`history-v2-index-<build>`). The stamp names the public
repository. The app takes the newest release that holds the three files, checks each against
the SHA-256 GitHub lists for it, unpacks the database as it downloads, opens the index, and
installs it in its data folder, a folder per release (`reforge_app::index`). The settings
then name it, and a folder they no longer name is removed at the next start.

Not settled: how the commits after an index's head are fetched with no full clone, and how
one bin entry's text is fetched with no clone.

## Two views of one run

#ADR-0050. The simple view ports the mod through every stage with the standard settings and
writes the `.modpkg`. The advanced view shows the stages in order, each with whether it ran,
its result and its findings, and runs up to a stage the user chooses. It ports too, and a
repair is a preset of it beside the standard settings (#ADR-0052).

The settings a run takes, each where the library keeps it, and nothing beside them:

| setting | where | repair | port |
|---|---|---|---|
| a fixup on or off, and rules added | `FixupSelection` | yes | yes |
| a check on or off | `CheckSet` | yes | yes |
| the target policy (auto, base, origin, slot, a skin) and the level of animation reuse | `Policy` | | yes |
| keep stale copies | `Repairer`, `Porter` | yes | yes |
| cut events from the game's bank only | `Porter` | | yes |
| which findings block the write | `DiagnosticPolicy` | yes | yes |

The decide stage gives, per unit, the chosen skin, the reason, every candidate with its checks,
and what the other levels of animation reuse choose. The advanced view shows all of it, and the
user picks another policy and runs again.

- The simple view's one button runs every stage with `severity=error` blocking the write. A
  mod with no error is written by that run. A mod with one is not: the view shows every
  finding, errors first, and a second button writes it with nothing blocking, as a new run.
- The app holds no rule about a mod. It shows what the library returned and passes settings in.
- Every finding shows its code. A fixup and a finding that name a record link to it.
- A mod's name, description and paths reach the screen through `clean`: the crate reads its
  views of a run from `Report::to_json`, which writes every character a terminal acts on as an
  escape.
- Wanted, and a library change: a target per unit. The policy chooses for all units at once.
- A port compares the policy with the other levels of animation reuse (`Porter::compare`),
  and the decide stage's view runs any of them with one button.
- The stages a view lists are the library's, `Run::stages()` of a run of the kind
  (`run_stages`); the front end keeps no list of its own.
- The mod card shows the package's thumbnail: the WebP the library encoded from the mod's
  image (#ADR-0051), never the archive's own bytes.
- The app runs the stages one at a time, as `Run::through` does, and tells the window as each
  starts and ends, and each finding as it is raised. A run's details are the library's
  `tracing` spans, one per stage with its time, its debug events, and what other crates warn
  of (`reforge_app::runlog`), shown folded. Progress inside a stage waits on the library.
- A rule is edited as its JSON is written, and a choice of fixups is saved and loaded as the
  selection file the command reads with `--fixups` (`reforge_app::selection`).
- A run with an input absent asks first: with no game it does not start, and with no history
  index, hash tables or schema it says what is judged with less and offers the index's
  download, until the user tells it not to ask (`prompts.runWithLess`).

## The first version

1. The shell of the app: the crate, the front end, the typed commands, the settings.
2. The inputs found, shown and changed by the user.
3. A repair in the simple view: pick a mod, run every stage, show what each fixup changed and
   every finding, write the `.modpkg`.
4. The advanced view of a repair: the stages, a run up to one of them, the fixups and checks
   turned on and off, the findings that block the write.
5. The history index, as far as step 2 of the list above.
6. A selection of fixups saved and loaded, and a rule added in the app.
7. A port in both views, with the decision, its candidates and the policy.

Steps 1 to 7 are built.

Two faults of the crates reforge reads and packs with come before a release
([ROADMAP.md](ROADMAP.md), "Upstream"): a crafted archive can end the process, which a tool
run on downloaded mods has to withstand.

## An AI agent

Proposed, not decided (#ADR-0049): an agent as a third caller, for a mod the default settings
do not port, as a skill over the command or as an MCP server. It changes what a run is given
(a selection, a rule, a policy, a target) and not what a run does. Nothing of it is in the
first version.
