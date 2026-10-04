# AGENTS.md

Guidance for coding agents in ltk-reforge, a porter for League of Legends skin mods and the
rewrite of ltk-forge. The docs are in `docs/`, one folder per kind ([Docs](#docs)). Read what
covers the area you change before changing it, and update it in the same change.

## Working here

- **Checks.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` come back clean.
- **Format with `cargo fmt`.** Run `cargo fmt --all` before a commit. The layout is rustfmt's,
  not a hand's.
- **Tests.** A small check of a private helper may sit inline. Anything with fixtures, fake games
  or several steps goes in the crate's `tests/` directory, fixtures built in the test.
- **League formats through `ltk_*` crates only.** Dependencies land in
  `[workspace.dependencies]`: crates.io first, else a git dependency pinned by `rev`, never a
  committed path dependency.
- **Lints.** Suppress one with `#[expect(lint, reason = "...")]`, never a bare `#[allow]`. A
  panic states what failed and with what value.
- **Mods are untrusted input.** Names, descriptions and entry names reach a terminal only through
  `clean` (`reforge::model`). No archive is extracted to disk, by reforge or by a script, not to
  a scratch folder either: entries are read in memory and only derived facts (counts, names,
  hashes) are written.
- **Schema questions.** A field's type at a build comes from `rito-meta`
  ([skills/meta-cli](skills/meta-cli/SKILL.md)) from 13.15 on, and from the census history
  before. A conversion is read off Riot's own across the build that changed it, never guessed,
  and recorded as a game change.
- **Scripts write LF.** A Python script that rewrites a tracked file opens it with
  `newline="\n"` and pipes to git as bytes: text mode on Windows writes CRLF.
- **Output is deterministic.** No clock, no machine state, no unordered map reaching output.
- **One rule at a time.** A change is made to one rule, with the test of its case, in the
  stage that decides it (#ADR-0045).
- **Corpus runs** use the scripts in `scripts/corpus/` and write under `data/corpus/`
  (gitignored). A change is run over the reduced corpus first and compared with the baseline,
  and over the full datasets only for a new baseline (#ADR-0044).
- **Code shape.** A stage is a type and a module of `reforge`, and a module uses only the
  modules of earlier stages ([docs/design/ARCHITECTURE.md](docs/design/ARCHITECTURE.md), "Code
  shape" and "Crates and modules").
- **"forge" means ltk-forge**, the retired program this rewrote (#ADR-0044). Nothing is
  compared with it, and it is not built, run or changed.
- Never commit or push unasked.

## Writing

Plain, literal language in replies, docs, comments and commit messages.

- One name for one thing. The short common word: use, not utilize or leverage; start, not
  initiate. No marketing adjectives (robust, seamless, powerful).
- Active voice, one fact per sentence. No metaphor or emphasis that carries no information:
  "adjust the setting", not "turn the dial".
- Keep technical terms where they are precise.

## Docs

| folder | one file holds | cited as |
|---|---|---|
| `docs/adr/` | a decision: a choice that could have gone another way, and why it went this way | `#ADR-0001` |
| `docs/game-change/` | a change Riot made to the game: the build, the before and after, Riot's own conversion | `#GAME-CHANGE-0001` |
| `docs/game-behavior/` | how the game client behaves: what it does with a file, a value or a name, and how that was established | `#GAME-BEHAVIOR-0001` |
| `docs/mod-artifact/` | what tools, mod managers and authors' practices put in mods: how to recognise it, why it is there, what reads it and how, the repair | `#MOD-ARTIFACT-0001` |
| `docs/design/` | the design, written for a person: how the parts fit and why | its file and section |
| `docs/agents/` | guidance for agents | |

The first four are numbered records, `NNNN-<slug>.md`. Their shape is in
[docs/agents/records.md](docs/agents/records.md). What to read before a change is in
[docs/agents/domain.md](docs/agents/domain.md).

- **Which folder.** A choice of the project's is an ADR. Something Riot changed at a build is a
  game change. How the game client behaves, where no change is the point, is a game behavior.
  A fact about the modding side (what a tool writes, how a mod manager packaged or read mods,
  what authors commonly do) is a mod artifact. How the parts fit is design.
- **A fact has one home.** Code, a design doc or another record that rests on a record names it by
  its code and stops. `grep -rn "#GAME-CHANGE-0003"` then lists everything that rests on it.
- **A record comes before its citation.** A code names a file that exists. When the fact has no
  record, the record is written in the same change.
- **Lifecycle.** A design doc, a game change, a game behavior and a mod artifact state what is
  true and are edited in place: when the code or the evidence departs from one, the section is
  rewritten, not appended to. They carry no `now`, `currently`, `used to` or `no longer`, and no
  story of what was tried. An accepted ADR is not rewritten: a later ADR supersedes it, and its
  status line says which. History is `git log`.
- **Decisions.** A decision the user made is an ADR, with its date.
- **Measurements** are facts about one run. They go, dated, in a record's evidence when they are
  about that record, and in the runs log (`docs/design/RUNS.md`) otherwise.
- Docs name no mod author or site and no local path.

## Comments

**A comment says why, not how.** The code says what it does. A comment carries what the code
cannot: the reason for a choice, a constraint from the client or the data, a workaround and its
cause, a magic value. The test is whether deleting it would let a reader change the code and break
something.

```
Bad   // Remove the field from the struct.
Good  // No build since the mod's own read it, so the author never saw it.

Bad   // Loop over the clips and count the borrowed ones.
Good  // A borrowed dance does not make the unit that skin's.
```

- **Most code needs none.** Clear names, small functions and named constants come first. Strip
  comments that narrate the next line.
- **Module (`//!`)**: what the file is for and the design behind it, for a person working inside
  it. One or two lines when there is no design to explain.
- **Declaration (`///`)**: for the caller. Every public item has one. The first line names the
  thing, a noun phrase or one declarative sentence on the domain's words: `/// The unit's clips
  judged against slot k.`, not `/// This function returns...`. A second paragraph only for what
  a caller needs and the signature cannot show.
- **Body**: only to split a long function into steps, or for a reason the code cannot show.
- **Cite a record, do not restate it.** When the reason is a decision, a game change, a game
  behavior or a tool artifact, the comment is its code, with a few words when the code alone
  would not say which part applies:

  ```
  Bad   // Riot turned these path strings into chunk links, so a string here is an older mod's.
  Good  // #GAME-CHANGE-0004: a string here is an older mod's.
  ```

- **A comment is smaller than the code it explains.** When the reason needs a page of design, the
  comment names the file and section: `` //! `docs/design/<FILE>.md`, "<section>". ``
- **Evidence is a fact worth keeping.** "Confirmed in game (2026-09-30)", a census count, the build
  a field changed at: these are why the code is as it is. Evidence for a record stays in the
  record. Evidence for this code alone stays in the comment.
- **Timeless.** No "used to", "the old X", "now", and no retelling of the change. That belongs in
  the commit. The exception is a workaround whose only reason is past behaviour someone would
  otherwise restore.
- **Self-contained.** A comment makes sense in a clean checkout: it does not lean on `data/` or
  other gitignored files.
- Third person, present tense, no `we` or `you`. Drop `note that`, `simply`, `just`.

## Commits

One conventional-commit subject line. No body, no trailers, no `Co-Authored-By`. The scope is
a module of `reforge` (`model`, `diag`, `load`, `fixups`, `facts`, `port`, `package`,
`porter`), a crate without its prefix (`census`, `magic`, `wwise`, `cli`, `testkit`) or,
outside one, the area (`docs`, `scripts`, `ci`, `workspace`).

**A subject names the change, it does not describe it.** A plain verb and a domain noun phrase,
about three to six words, in the codebase's vocabulary. No contrastive clause, no mechanism.

```
Bad   reforge: a skin's clips make a mod that skin's; a few borrowed clips do not block a slot
Good  feat(port): target by played clips

Bad   reforge: strip-known-artifacts, fields no game build ever had, removed and reported apart
Good  feat(fixups): strip known tool artifacts
```

A test name stays a sentence: it is the only place saying what the case is.

## CodeGraph

`.codegraph/` holds a local index (`codegraph init`, then `codegraph sync`). `explore`, `node`,
`callers` and `impact` answer most "where is this used" questions. Its blind spots here:

- A fixup or a check is reached through its trait and the set that holds it, not by a call to
  it. Its place in the order is its phase and build (#ADR-0007).
- `docs/` and `data/` are outside the index. Find reasons and record codes in `docs/` with grep.
