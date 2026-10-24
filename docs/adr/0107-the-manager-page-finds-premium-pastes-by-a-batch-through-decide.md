# ADR-0107: The Manager page finds premium pastes by a batch through decide

- **Status:** Accepted (2026-10-24)
- **Related:** #ADR-0013, #ADR-0091, #ADR-0106, #MOD-ARTIFACT-0002, #MOD-ARTIFACT-0024;
  [docs/design/ROADMAP-UX.md](../design/ROADMAP-UX.md), "The Manager page"

## Context

Many mods at the base slot are a premium skin pasted into it (#MOD-ARTIFACT-0002). A person
with a library of them wants to know which before porting, since a paste that does not fit the
base becomes a mod for the premium skin (#ADR-0013), and a mod manager's own check of the base
skin blocks a paste on positive evidence: a mesh or skeleton file that is one of the champion's
other skins', or a skin name, an audio tag or a bank unit naming `<Champion>Skin<N>`.

reforge's facts stage already reads a unit's origin (#ADR-0013): the mesh, the skeleton, the
texture, the skin name, the audio tags, the resolver and the graph, each against every skin
of the champion at every build of the history, weighed strong or weak, with a verdict of base,
premium, custom or unknown. A manager's check sees one install's files and three names.

The options were a check of the manager's kind over the archive's bins alone, a scan that runs
the facts stage, or nothing.

## Decision

The Manager page offers "Find premium pastes" over the ticked mods (the user's call,
2026-10-24). It runs them as a batch of ports stopped after `decide`, which runs no plan, no
build and no write and keeps no run in the store, and shows each unit's origin beside the mod:
the verdict, the premium skin, the weight and the aspects that decided.

- A batch takes the stage it runs through, `write` unless asked. A run that ends at an earlier
  stage as asked has the outcome `scanned`, and its row carries each unit's origin.
- A **paste** on the page is a unit whose origin is a premium skin with strong evidence from
  its mesh or its skeleton file: the manager's rule, read off reforge's facts. The other
  verdicts are shown as they are, so a person sees a paste by name alone too.
- The page keeps the scan's rows until the library is read again, and filters the table to
  pastes on request. Ticking is the person's: a scan changes no tick.

## Consequences

- A library is scanned in seconds per mod, and a person ports pastes knowing where each will
  land.
- The batch loop serves both; the Batch page itself keeps running through `write`.
- The origin view carries the weight and the aspects for every decision from now on, in the
  run page's views too.
