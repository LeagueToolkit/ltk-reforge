# ADR-0098: A target per unit and a keep per file are settings

- **Status:** Accepted (2026-10-10); the keep read by `decide` and `plan` superseded by
  #ADR-0099
- **Related:** #ADR-0023, #ADR-0046, #ADR-0076, #ADR-0082, #ADR-0090, #ADR-0094

## Context

A port's policy chooses a target for every unit at once: `auto`, `base`, `origin` or `slot`,
with one level of animation reuse. Stale copies of game files are dropped or kept for the
whole mod (#ADR-0023). The guided view (#ADR-0090) asks about one unit and one file at a time,
and its ADR requires that an answer maps to a setting the library takes and that the app adds
no rule of its own (#ADR-0046). It names a target per unit as the library change made for the
view.

The units that the policy's defaults leave for a person are those of #ADR-0076 and
#ADR-0082: a unit that fits no slot and stays at skin0, units that tie on a slot and each
leave it, a slot the port would not change. A file the defaults decide for a person is a
stale copy: dropped unless the whole mod keeps them.

## Decision

The settings gain a target per unit and a keep per file, each an override of the policy's
choice for that one unit or file (the user's call, 2026-10-10).

- A per-unit target names the unit by its character and the slot the mod ships it at, and
  gives the slot it ships to, or that it is left out. It is the policy's fixed slot for that
  one unit and nothing else; the units not named follow the policy.
- A per-file keep names a file by its WAD and path, and keeps it where the run would drop it
  as a stale copy. The files not named follow the stale setting.
- Both are read by the stages that decide them, decide and plan, and by no earlier stage, so a
  change of either resumes from the load (#ADR-0094).
- A per-unit target the work does not fit raises `target.forced-unfit`, as a forced target
  does today. A per-file keep of a file that is not stale raises nothing and keeps nothing.
- The settings are data, in the run request and in the report's settings, so that a kept run
  says what was answered.

## Consequences

- The guided view's questions about a unit and about a stale file each map to one of these.
- The command takes both as flags, since the library's settings are the command's.
- The report's decisions say when a target came from a per-unit setting, as they say when it
  came from the policy, so that a comparison of two runs shows the answer.
