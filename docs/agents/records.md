# Records

The four numbered series under `docs/`: what each holds, how a file is shaped, and how it is
cited. The rules they share with every doc are in the root `AGENTS.md`, under "Docs".

| series | folder | a record is | cited as |
|---|---|---|---|
| ADR | `docs/adr/` | a decision | `#ADR-0001` |
| game change | `docs/game-change/` | a change Riot made to the game | `#GAME-CHANGE-0001` |
| game behavior | `docs/game-behavior/` | how the game client behaves | `#GAME-BEHAVIOR-0001` |
| mod artifact | `docs/mod-artifact/` | what tools, mod managers and authors' practices put in mods | `#MOD-ARTIFACT-0001` |

## Every record

- **File.** `NNNN-<slug>.md`: four digits, the next free number of its series, and the title in
  lower case with hyphens. The number is the order the record was written in, not the order
  things happened in.
- **A number is permanent.** It is never reused and no record is renumbered, because code and
  docs cite it. A record that turns out wrong is corrected. One that is withdrawn keeps its file
  and says so in its status.
- **Heading.** `# GAME-CHANGE-0001: <title>`: the code without `#`, then the title.
- **Citation.** The code with `#`, in a comment, a design doc or another record:
  `#GAME-CHANGE-0001`. A record does not list what cites it: grep does.
- **One thing per record.** Two changes at one build are two records. One change that reached the
  game over several builds is one record.
- **Look first.** Before writing a record, search the series for the field, class or extension.
- **Facts and choices apart.** A game change, a game behavior and a mod artifact state facts.
  Where handling one took a choice, the choice is an ADR that cites the record.
- **No pass names.** A record does not say which code handles it. The citation in the code does.

## ADR

A choice that could have gone another way: a policy, a scope, a trade. A decision the user made
is one, with its date.

The title is the decision as a declarative sentence: `A mod for a premium slot stays there`.

```markdown
# ADR-0001: <the decision>

- **Status:** Proposed | Accepted (YYYY-MM-DD) | Superseded by #ADR-NNNN
- **Related:** the records and docs it rests on

## Context

What made a decision necessary: the facts, and the options that were open.

## Decision

What was chosen, in the first sentence. Then its bounds.

## Consequences

What follows from it, the costs included.
```

An accepted ADR is not rewritten. A later ADR supersedes it, and only the status line of the
earlier one changes. A later ADR that supersedes one consequence names it on the status line:
`Accepted (YYYY-MM-DD); <the consequence> superseded by #ADR-NNNN`.

## Game change

Something Riot changed in the game at a build, which a mod made before that build does not
follow: a file format, a field's name or type, a struct's layout, where a file lives.

The title is a noun phrase naming the change: `Textures in the TEX container`.

```markdown
# GAME-CHANGE-0001: <the change>

- **Build:** the first build that has it
- **Affects:** the formats, classes or fields
- **Related:** other records

## Before

What the game held up to the build.

## After

What it holds from the build on.

## Riot's conversion

What the game's own data did across the build: how each old value maps to the new form, what
got a default, what was dropped.

## In a mod

How a mod made before the build shows it, and what the client does with that at a later build:
reads it, skips it, draws it wrong, crashes.

## Evidence

Where each statement above comes from: the census, the schema, a check in game. Counts and
checks carry their date.
```

## Game behavior

Something the game client does with what it is given, which a port or a repair rests on: how it
finds a file, what it does with a value of another type, which of two banks it keeps, how it
resolves a name. The client here is the game's own program, not a mod manager and not an author.

It is a game behavior when no change is the point. When Riot changed the behaviour at a build
and older mods show the old one, that is a game change, and the two records name each other.

The title is one declarative sentence naming the behaviour: `A texture is read by its magic`,
`A graph event finds its resolver by the skin's name`.

```markdown
# GAME-BEHAVIOR-0001: <the behaviour>

- **Status:** Confirmed in game (YYYY-MM-DD) | Inferred | Open
- **Builds:** the builds it was checked at
- **Affects:** the formats, classes or fields
- **Related:** other records

## Behaviour

What the client does, as one rule, then its bounds: what is known not to follow it.

## In a mod

What follows for a mod: what loads, what is skipped without a word, what plays twice.

## Evidence

How it was established: the check in game with its date and what was seen with and without the
mod, or the data it is inferred from. An inference says what would confirm it.
```

A behaviour that is only inferred is recorded as `Inferred`. One nobody has established, which
the code has to assume either way, is recorded as `Open` with what is assumed.

## Mod artifact

Something found in mods that comes from the modding side, not from the game: from a tool, a mod
manager or a common practice of authors. Three kinds:

- **What a tool writes into a mod**: a field no game build declared, a bank ported wrong, a
  repath layout, an added hash table.
- **How a mod manager packaged, installed or read mods**, which the mods made for it rely on: a
  zip entry written with no CRC because the files carry their own checksum, an edge case the
  manager accepted.
- **What authors commonly do**: copy game files into the mod, edit a field a certain way.

One mod's one-off mistake is not a record. A pattern across mods is.

The title is a noun phrase naming what is found: `Emitter fields nested in textureMult`,
`Zip entries with no CRC`.

```markdown
# MOD-ARTIFACT-0001: <what is found>

- **Source:** the tool, the manager or the practice, when known
- **Affects:** the archive, formats, classes or fields
- **Related:** other records

## What is found

How to recognise it in a mod or its archive, and how to tell it from deliberate work.

## Why it is there

What the tool did, what the manager did or accepted, or what authors do it for.

## What reads it, and how

The client, a current manager or a strict reader: reads it, skips it, rejects it, draws it
wrong, crashes.

## The repair

The form a mod made today has for the same thing. `None` when what the author meant is not in
the file.

## Evidence

How many mods carry it, over which set and when. Checks in game, with their date.
```
