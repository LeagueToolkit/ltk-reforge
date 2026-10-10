# Diagnostics

The decisions this rests on are #ADR-0003, a diagnostic does not stop the write, and #ADR-0008,
a code names what the finding is about. Every code is in [CODES.md](CODES.md). What ltk-forge
does, and why it is not enough, is in [REVIEW.md](REVIEW.md), "Diagnostics".

A diagnostic is what a run found about a mod or about its own inputs. A run carries every
diagnostic from the first stage to the report. A caller filters them, and a person reads them
with the output.

## A diagnostic and a fault

- A **diagnostic** is about the inputs: the archive, the mod in it, the install, the history, the
  schema. A mod that does not parse, a value with no conversion and a stale index are
  diagnostics. A run with diagnostics still returns its results.
- A **fault** is about the tool or the machine: a disk that fails, a bug. A fault is a Rust
  error and ends the run. It is never a diagnostic.

A parser that panics on a mod's bytes is caught and becomes a diagnostic about that file.

## What one holds

| field | holds |
|---|---|
| `id` | its place in the run's log, from 0 |
| `code` | a stable name, `area.finding` ([Names](#names)) |
| `severity` | `note`, `warning`, `error` or `fatal` |
| `channel` | what it speaks to: `run`, `input`, `decision`, `output` |
| `stage` | the stage that raised it: `read`, `load`, `fix`, `facts`, `decide`, `plan`, `build`, `check`, `write` |
| `subject` | what it is about, as a typed reference ([Subjects](#subjects)) |
| `by` | the fixup or check that raised it, when one did |
| `record` | the record it rests on, when it has one: a game change, a game behavior or a mod artifact, by its code |
| `data` | the finding itself, a typed value |
| `message` | one line for a person, rendered from `data` |

`code`, `channel` and `severity` are fixed by the type of `data`. The rest is filled
when it is raised.

## Severity

Severity says what the output loses against the mod as its author saw it. It does not say what
the run did next.

| severity | means |
|---|---|
| `note` | Nothing is lost. It says what was found or done: a copy dropped, a field moved, a file left out that nothing loads. |
| `warning` | The output may differ from what the author saw, or an input limits how far the mod was judged. |
| `error` | Something of the mod is not in the output, or the output is known to be wrong somewhere: a unit that fits no slot, a value the client will skip, a name that resolves to nothing. |
| `fatal` | There is no output: the archive is not a zip, it holds no skin. |

A code has one severity. A finding that is sometimes a note and sometimes a warning is
two codes.

## Names

A code is `area.finding`: two parts, lowercase, words joined by hyphens.

- **The area is the thing the finding is about**, in the glossary's words. It is never the name
  of a stage, a module or a function, so the code survives when the code moves.
- **The finding** is an adjective or a participle for a state (`file.stale`,
  `target.infeasible`), a past participle for something the run did (`fixup.changed`,
  `unit.left`).
- **A code is permanent.** It keeps its meaning and is never reused. A finding whose meaning
  changes gets a new code.
- **One code, one type.** Each code has one `data` type. Fields are added to it, never renamed
  or retyped.

| area | about |
|---|---|
| `archive` | the zip and its entries |
| `inventory` | what the archive carries besides game files |
| `wad`, `character` | a WAD of the mod and the characters in it |
| `file` | a file of the mod: unused, stale, at a former path, in an older format |
| `bin` | a bin: unreadable, a link to nothing, a mod with none |
| `rework` | what a champion's rework says of the mod |
| `fixup` | what the fixups changed and what they could not |
| `copies`, `origin`, `rig`, `target` | the facts and the decision |
| `unit` | a unit in the port: left out, retargeted, given a new bin, ported |
| `object` | what an entry reaches: carried, dangling, unreached |
| `ref` | a file the output names: found nowhere, not mounted with the champion |
| `audio`, `hud` | the mod's sounds, its spell icons and HUD textures |
| `package` | what the build could not express, and what it made |
| `check` | what a check found in the package |
| `write` | what was written, or refused |
| `install`, `names`, `history`, `schema` | the run's other inputs |

Fixups share their codes. A fixup is told apart by `by`, so a rule a person adds needs no new
code:

| code | severity | means |
|---|---|---|
| `fixup.changed` | note | values kept, in the form the target build reads |
| `fixup.removed` | note | values removed with nothing lost |
| `fixup.lost` | warning | values removed that the mod's own build read |
| `fixup.unconvertible` | warning | values with no conversion; the client skips them |

A decision has one code too: `target.chosen`, with the reason in its data.

ltk-forge's codes that name a pass (`survey.*`, `lower.*`, `strip-undeclared.*`,
`vfx-filtering.*`) get a name by this scheme. [CODES.md](CODES.md) gives ltk-forge's code beside
each.

## Subjects

A subject is a reference, not a string. In the report it carries an id to join with the
report's tables and a name for a person:

```json
{ "kind": "unit", "id": "%u0", "name": "varus skin0" }
```

The kinds: `run`, `input` (the install, the history, the names, the schema), `entry` (an entry
of the archive, by the name the archive spells), `wad`, `file`, `bin`, `object`, `character`,
`unit`, `chunk` (a path of the output). A layer is no subject: a finding about one holds it in
`data`.

A place inside an object is not a subject. It is an item of `data`: the object and the field
path.

## One per subject

A stage raises one diagnostic per code and subject, with every instance as an item of `data`. A
fixup that moves 400 fields in a unit raises one `fixup.changed` whose `data` lists them.

- The message gives the count and the first few items. One function writes "the first N and M
  more".
- The report holds every item. No cap on the items of a diagnostic is built.

## Through the run

- **One log.** `Diagnostics` is an ordered, append-only log the run owns (`Run::log`). The
  order is the order raised, which is the same for the same inputs. What the inputs raise
  while the environment is built goes in the same log, before the archive is read
  (`Run::log_from`).
- **A sink per stage.** A stage gets `diagnostics.at(Stage::Load)`, from the `Run` method that
  runs it. A fixup or a check gets a sink that also fills `by` and `record`. A finding type
  whose every finding rests on one record names it itself (`Diag::RECORD`), and the sink's
  record comes first.
- **A stage returns its result.** Only a `fatal` diagnostic ends the run, as
  `Err(Halted(id))`. Every stage of a `Run` asked after it answers the same `Halted`, and the
  report is built all the same.
- **Results point at their reason.** Where a diagnostic explains a state, the state holds its
  id: a unit left out of the port holds the id of its character's `unit.left`. A caller goes
  from "this unit is not in the output" to why. The files not shipped are counted by reason
  in the data of `package.ported` and `package.in-place`, and a unit's banks not carried are
  listed with why in its `audio.not-carried`. Neither holds an id.
- **Nothing raises into a sink that is thrown away.** The decision's diagnostics are made from
  the chosen decisions. The policies compared against make none.
- **A listener hears each one as it is raised**, so a GUI shows a run as it goes.

## Filters and levels

A `Filter` selects diagnostics by any of: code or area, severity at least, channel, stage,
subject (a unit, a character, a file), `by`, record. The command, the report and a GUI use the
same type. A test asks by type: `diagnostics.of::<StaleFiles>()`.

A `DiagnosticPolicy` says what a caller does with what a filter selects:

| action | effect |
|---|---|
| `hide` | not printed; kept in the report |
| `block` | `write` refuses while one is in the log |

No code blocks by default (#ADR-0003). `Run::write` asks the policy before any writer runs,
and raises `write.blocked` with the diagnostics that refuse it. A policy is data that
serialises, as a choice of fixups does, so a GUI or a corpus run keeps its own: a corpus run
blocks on every `check` error (`DiagnosticPolicy::strict`), a person porting one mod blocks on
nothing. The command takes a filter as text: `--block check,severity=error`.

## Serialised

In the report, `diagnostics` is an array of:

```json
{
  "id": 12,
  "code": "file.stale",
  "severity": "note",
  "channel": "input",
  "stage": "load",
  "subject": { "kind": "wad", "id": "%w0", "name": "Varus.wad.client" },
  "record": "#MOD-ARTIFACT-0004",
  "message": "2 files (0.0 MiB) at the game's paths, holding the bytes the game shipped there at earlier builds; not carried",
  "data": { "wad": "%w0", "files": ["%f11", "%f14"], "bytes": 15535 }
}
```

- `by` and `record` are present when set.
- `data` is the code's type as serde writes it. Its fields are added, never renamed.
- Text a mod controls is cleaned as everywhere: `clean` for a terminal, JSON escapes in a file.
- A listener hears each `Diagnostic` as it is raised. `Diagnostic::to_json` writes the same
  object, for a stream of one per line.
- `reforge diagnostics` lists every code with its severity and its channel.
  [CODES.md](CODES.md) holds each code with its severity, what it means and ltk-forge's code
  for the same finding. A test holds that every declared code is in it with the severity it
  is declared with.

## With the output

`write` puts the report beside what it writes: `reforge-report.json` in a project directory,
`<project>_<version>.reforge-report.json` beside a `.modpkg`. It is the whole report of the
run as it stands before the write, every diagnostic in it. The report a command writes with
`--report` is of the run after the write, so it also holds `write.written`.

A summary for a person (each unit with where it went and why, then the errors, the warnings
and the notes counted by area) is not built.

Open: whether a `.modpkg` carries the summary inside it, where a mod manager can show it.

## Declaring one

The module that raises a diagnostic declares its type:

```rust
/// Files at game paths holding the bytes the game shipped there at earlier builds.
#[derive(Serialize)]
pub struct StaleFiles {
    pub wad: WadId,
    pub files: Vec<FileId>,
    pub bytes: u64,
}

impl Diag for StaleFiles {
    const CODE: Code = Code::new("file.stale", Channel::Input, Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(4));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message) {
        f.count(self.files.len(), "file");
        mib(f, self.bytes);
        f.text(" at the game's paths, holding the bytes the game shipped there at earlier builds; not carried");
    }
}
```

`Message` holds the line as parts, text and the ids and hashes in it, and spells the ids and
hashes through the names it is rendered with (`Render`). A message never holds a hash a table
can name, and a finding raised before a name was learnt prints with it. Each module lists its codes in one table (`CODES`), which
`reforge::codes()` joins for the catalogue and the test.
