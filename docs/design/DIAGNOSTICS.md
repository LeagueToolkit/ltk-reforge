# Diagnostics

Status: proposed 2026-10-03. Nothing is built. The decisions this rests on are #ADR-0003, a
diagnostic does not stop the write, and #ADR-0008, a code names what the finding is about. Every
code is in [CODES.md](CODES.md). What ltk-forge does, and why it is not enough, is in
[REVIEW.md](REVIEW.md), "Diagnostics".

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

The kinds: `run`, `input` (the install, the history, the names, the schema), `wad`, `file`,
`bin`, `object`, `character`, `unit`, `layer`, `chunk` (a path of the output).

A place inside an object is not a subject. It is an item of `data`: the object and the field
path.

## One per subject

A stage raises one diagnostic per code and subject, with every instance as an item of `data`. A
fixup that moves 400 fields in a unit raises one `fixup.changed` whose `data` lists them.

- The message gives the count and the first few items. One function writes "the first N and M
  more".
- The report holds every item. A caller may cap the items per diagnostic for a corpus run. The
  total stays.

## Through the run

- **One log.** `Diagnostics` is an ordered, append-only log the run owns. The order is the order
  raised, which is the same for the same inputs.
- **A sink per stage.** A stage gets `diagnostics.at(Stage::Load)`. A fixup or a check gets a
  sink that also fills `by` and `record`. A finding type whose every finding rests on one
  record names it itself (`Diag::RECORD`), and the sink's record comes first.
- **A stage returns its result.** Only a `fatal` diagnostic ends the run, as
  `Err(Halted(id))`.
- **Results point at their reason.** Where a diagnostic explains a state, the state holds its
  id: a unit left out of the port, a file not shipped, a bank not carried. A caller goes from
  "this unit is not in the output" to why.
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

No code blocks by default (#ADR-0003). A policy is data and is saved beside the choice of fixups,
so a GUI or a corpus run keeps its own: a corpus run blocks on every `check` error, a person
porting one mod blocks on nothing.

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
  "message": "41 files hold the bytes the game shipped at their paths at earlier builds; not carried",
  "data": { "files": ["%f3", "%f9"], "total": 41 }
}
```

- `by` and `record` are present when set.
- `data` is the code's type as serde writes it. Its fields are added, never renamed.
- Text a mod controls is cleaned as everywhere: `clean` for a terminal, JSON escapes in a file.
- The same objects, one per line, are the stream a listener gets.
- `reforge diagnostics` lists every code with its area, channel, severity and one line of what it
  means. [CODES.md](CODES.md) holds the same table, and a test compares the two.

## With the output

`write` puts the report beside what it writes:

- `reforge-report.json`: the whole report, every diagnostic;
- `reforge-report.md`: for a person. Each unit with where it went and why, then the errors, then
  the warnings, then the notes counted by area.

Open: whether a `.modpkg` carries the summary inside it, where a mod manager can show it.

## Declaring one

The module that raises a diagnostic declares its type:

```rust
/// Files at game paths holding the bytes the game shipped there at earlier builds.
#[derive(Serialize)]
pub struct StaleFiles {
    pub wad: WadId,
    pub files: Vec<FileId>,
}

impl Diag for StaleFiles {
    const CODE: Code = Code::new("file.stale", Channel::Input, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file").text("hold the bytes the game shipped at their paths at earlier builds; not carried");
    }
}
```

`Message` spells ids through the `Speller`, so a message never holds a hash a table can name.
Each module lists its codes in one table, which the catalogue and the test read.
