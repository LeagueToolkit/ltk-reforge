# Domain docs

How an agent uses this repository's docs when it explores the code or plans a change.

## Before a change, read these

- **`CONTEXT.md`** at the repo root: the glossary.
- **`docs/design/`**: the doc for the area.
- **`docs/adr/`**: the decisions that touch the area.
- **The records the code cites.** A `#GAME-CHANGE-NNNN` or `#MOD-ARTIFACT-NNNN` beside the code
  names a file under `docs/game-change/` or `docs/mod-artifact/`. Read it before changing that
  code: the comment holds the code only, and the record holds the reason.

A file or folder that does not exist is no fault. Work without it, and do not create it ahead of
need: a glossary entry, an ADR or a record is written when a term, a decision or a fact is
settled.

## Layout

One context: one glossary and one of each series cover every crate.

```
/
├── AGENTS.md
├── CONTEXT.md              the glossary
├── crates/
└── docs/
    ├── adr/                0001-<slug>.md   decisions           #ADR-0001
    ├── game-change/        0001-<slug>.md   Riot's changes      #GAME-CHANGE-0001
    ├── mod-artifact/       0001-<slug>.md   the modding side    #MOD-ARTIFACT-0001
    ├── design/             the design overview, for a person
    └── agents/             this folder
```

## Finding things

| question | command |
|---|---|
| what rests on a record? | `grep -rn "#GAME-CHANGE-0003" crates docs` |
| which record is it? | `ls docs/game-change/0003-*` |
| is this fact recorded? | `grep -rli "<field, class or extension>" docs/game-change docs/mod-artifact` |
| which decisions touch this area? | `grep -li "<term>" docs/adr/*.md` |

A record's own heading holds its code without the `#`, so a search for `#GAME-CHANGE-0003` lists
the citations and leaves the record out.

## Use the glossary's words

A domain concept in a doc, a comment, a test name or a commit takes the glossary's term, not a
synonym. A concept the glossary lacks is either a word the project does not use, or a gap: say
which, and add the entry when the term is settled.

## Say when something conflicts

A proposal that contradicts an accepted ADR or a record says so, and why it is worth reopening:

> Contradicts #ADR-0007, worth reopening because the client's behaviour changed at 16.20.

A game change or a mod artifact that the game or a mod proves wrong is corrected in place, with
the evidence. An ADR is superseded by a new one.
