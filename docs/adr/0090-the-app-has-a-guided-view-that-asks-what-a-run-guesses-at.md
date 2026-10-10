# ADR-0090: The app has a guided view that asks what a run guesses at

- **Status:** Accepted (2026-10-09); each run from an answer a new run from the archive
  superseded by #ADR-0094
- **Related:** #ADR-0003, #ADR-0023, #ADR-0050, #ADR-0052, #ADR-0076, #ADR-0082, #ADR-0084,
  #ADR-0085, #ADR-0086; `docs/design/ROADMAP-UX.md`, "The second version"

## Context

The app shows one run in a simple and an advanced view (#ADR-0050). The simple view runs with
the standard settings and shows what came of it. The advanced view shows every setting the
library takes, in the library's words, and a person changes them and runs again.

The corpus work settled several rules by a default where the library cannot tell what the
author meant: a port changes its target slot only, and an option keeps every slot (#ADR-0082);
a unit that fits nothing stays at skin0 for a person to decide; units that tie on a slot each
leave it (#ADR-0076); stale copies of game files are dropped unless an option keeps them
(#ADR-0023); an events bank of a version the client does not load gives way to the game's
(#ADR-0084); a restoration or a chroma pack is a mod made for several skins on purpose
(#ADR-0085, #ADR-0086). Each default is right for most mods and wrong for some, and the
finding that says so reaches a person only as a line among the findings, with the option that
changes it somewhere in the advanced view.

## Decision

The app has a third view, the guided one, between the simple and the advanced (the user's
call, 2026-10-09). It runs the mod through the plan stage with the standard settings, then
stops and asks one question for each finding of a kind the library decided by a default, with
the choices that finding allows, and runs again with the answers. A run with nothing to ask
ends as the simple view's does.

- A question is derived from a finding and names the mod's own facts: the slots the mod
  changes and what each holds, the files that are an older build's bytes, the candidates a
  unit fits and their checks, the bank the events come from.
- An answer maps to a setting the library takes: the policy, a target, the option that keeps
  every slot, the stale copies kept, the events cut from the game's bank only, a check turned
  off, a finding allowed to block the write or not. The app adds no rule of its own
  (#ADR-0046).
- A target per unit is a library change made for this view, since the policy chooses for every
  unit at once.
- The errors that refuse the write are asked about one by one, each with what writing anyway
  loses, in place of the simple view's one button (#ADR-0003).

## Consequences

- #ADR-0050's two views become three; its consequence that a person who needs a choice goes to
  the advanced view is superseded for the choices the guided view asks.
- Each run from an answer is a new run from the archive. The early stages run again; a cache
  of them is a library change for later.
- A finding that the guided view asks about is one the stage raises with the data the question
  needs. A question with no finding behind it is not asked.
