# ADR-0049: An AI agent is a caller of reforge

- **Status:** Proposed
- **Related:** #ADR-0003, #ADR-0024, #ADR-0046; `docs/design/ARCHITECTURE.md`, "Callers",
  "Fixups", "Untrusted input"

## Context

A run with the standard fixups and the default policy ports most mods. Where it does not, a
person changes what the run is given: turns a fixup off, adds a rule, names a target, accepts
a level of animation reuse. That takes knowledge of the game's data most users of a mod tool
do not have, in the command and in a UI alike.

The user proposed (2026-10-04) to let an AI agent do that work with the user: when the
default fails and changing rules by hand, in a config or in the UI, is too complex.

What an agent needs is what any caller needs, and the library gives it as data: the report of
a run as far as it went, every diagnostic with its code and its record, the fixups and checks
as lists, a selection of fixups as JSON, the policy, and a run that stops after any stage.

Two forms are open:

- **A skill over the command.** Guidance for an agent that runs `reforge` and reads its
  reports. The command and the reports exist. The agent needs a shell.
- **An MCP server.** A program that offers the same actions as typed tools to any client that
  speaks the protocol, with no shell. It is a caller to build and keep, beside the command and
  the app.

## Decision

Proposed: an AI agent is a third caller of `reforge`, beside the command and the app. Which
form is built is not decided.

Whatever the form, these hold:

- **The agent changes what a run is given, not what a run does.** It proposes a selection of
  fixups, rules, a policy and a target. The result is the data a person could have written,
  and it is saved and shown as such.
- **A mod is untrusted input to the agent too.** A name, a description or a path of a mod
  reaches the agent as cleaned text marked as the mod's, never as an instruction.
- **No archive is extracted for the agent** (#ADR-0024). It reads reports and dumps.
- **The agent writes a package only through a run**, with the checks on, and the report says
  an agent chose the settings.

## Consequences

- The report and the diagnostics are the agent's whole view of a mod, so what a person would
  look up in a hex view has to be a question the command answers.
- A rule or a policy an agent proposed is held to the same contract as one a person wrote.
  The engine already bounds what a rule can express.
- Guidance for the agent is a set of records and design docs it can read: which finding means
  what, and which setting answers it. The records are written for that already.
- An agent that ports what the default refuses can ship a mod that looks right and is not.
  What it may override, and what stays refused, is part of the decision still to take.
