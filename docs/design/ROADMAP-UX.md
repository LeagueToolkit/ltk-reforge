# Roadmap of the app

The plan for the first release of reforge as a desktop app, and what a person starting that
work needs to know of the library. The decisions are #ADR-0046 (a standalone app on LTK
Manager's stack), #ADR-0047 (the history index, distributed and grown on the user's machine),
#ADR-0057 (the census history fetched as far as an append reads it), #ADR-0060 (the census
history bootstrapped from a released archive), #ADR-0059 (the League Toolkit caches updated
when asked and marked when behind), #ADR-0061 (what the app downloaded removed when asked,
proposed), #ADR-0049 (an AI agent as a caller, proposed),
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
- The pages share one column, centred, at the width the default window draws them at. A wider
  window buys margins, as LTK Manager's pages do (its `docs/ux/HOME.md`, "Layout").
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
| the hash tables | the cache the League Toolkit programs share, else a folder the user picks; downloaded or updated by the app when asked (#ADR-0059) | a run names less: a chunk no table spells ships under its hash |
| the schema database | the copy the League Toolkit programs share, downloaded or updated by the app when asked (#ADR-0059), else a file the user picks | the types come from the install's bins alone |
| the history index | fetched prebuilt and taken forward from the census history (#ADR-0047, #ADR-0057) | the run is judged against the install alone, and says so |

The app says which inputs it read and how old each is, since a report differs with them: the
install's build, the newest build the schema and the history index describe, and what opening
each one raised. On a first start with no game chosen, the app looks for one with
`ltk_mod_core`, as LTK Manager does, and chooses the one it finds.

On this machine (2026-10-04), the inputs open in 1.7 s with the name-hash index already built,
and a repair of a mod of 27 MB runs through check in 0.4 s and through write in 0.5 s. The
report of that run is the command's, byte for byte, with the same inputs.

## The inputs page

The page has two groups, and a button that asks every server again how the inputs stand.

- **What a run reads**: the game, the hash tables, the schema database and the history index,
  a row each.
- **What the history index is made from**: the app's clone of the census history, and the
  downloads a stop kept. No run reads either, and the group says so.

The history index is a SQLite database and the census history a Git repository, and each row
names its kind beside its title. They are two rows in two groups because they are brought up to
date in different ways and cost different amounts: the index is what a run needs, and the clone
is only where its new builds are fetched into.

A row holds, in this order:

1. Its title, and a badge of how it stands: up to date, an update published, none, being
   checked, or not opening.
2. The path read, with a button that shows the folder in the system's file manager
   (`reveal_place`, which takes the name of a place and never a path).
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
   release's archive (#ADR-0060), to read an entry's text at any build and to build an index
   with no download.

The census history is public, `https://github.com/LeagueToolkit/ltk-census-history`, and the
index is published as a release of it: `stamp.json`, `builds.tsv` and `history.sqlite.zst`,
and beside them the census history as an archive, `ltk-census-history.git.tar.zst`, at the
index's head (`history-v2-snapshot-<n>`). The stamp names the public repository. The app takes
the newest release that holds the index's three files and checks each against the SHA-256
GitHub lists for it. The database downloads into `downloads` in the app's data folder, where a
download that stops is kept and the next one resumes it while GitHub sends the same file
(`reforge_app::download`); it is unpacked once it is whole, with a window of up to 2 GiB of
memory as `--long=31` writes it. The app opens the index and installs it in its data folder, a
folder per release (`reforge_app::index`). The settings then name it, and a folder they no
longer name is removed at the next start. The published database of 767 MB downloaded and
unpacked in 42 s on this machine (2026-10-05).

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
| keep each DDS as it is, out of TEX's container (#ADR-0055) | `Repairer`, `Porter` | yes | yes |
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
  starts and ends, and each finding as it is raised. A stage that gives a part sends it as it
  ends, built alone with the library's `ReportBuilder` as `Run::report` builds it: `fix` the
  changes, `decide` the decisions, `build` the package. The window shows each part as it
  comes, in the same place it shows the ended run's (`reforge_app::run`, `drawn`). A run's
  details are the library's `tracing` spans, one per stage with its time, its debug events,
  and what other crates warn of (`reforge_app::runlog`), shown folded. Progress inside a stage
  waits on the library.
- Every finding opens to its details. A fixup's `fixup.*` finding opens to the values it
  counts, object by object, each with its path and its value before and after. Another
  finding opens to its fields, and an id among them (`%f12`) shows the name the report's
  tables give it.
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
