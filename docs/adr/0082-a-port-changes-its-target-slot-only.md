# ADR-0082: A port changes its target slot only

- **Status:** Accepted (2026-10-09)
- **Related:** #ADR-0012, #ADR-0013, #ADR-0015, #ADR-0018, #ADR-0020, #ADR-0023, #ADR-0076,
  #MOD-ARTIFACT-0001, #MOD-ARTIFACT-0014

## Context

A mod targets one skin. Its archive often holds skin bins at other slots as well: the game
shares files and bins between skins, so a change made for one skin reaches the bins of others,
and the tools that pack a mod carry those bins along (#MOD-ARTIFACT-0014). Shipped as units,
they patch skins the mod never targeted. PROJECT Urgot 1.5.0 and Ben 10 Ultimate Alien Udyr,
played on 2026-10-09, show artifacts on their other skins: their ports patch skin 3, and Udyr's
skins 5 and 6, with the objects of those bins.

#ADR-0020 drops the game's own bins shipped beside a mod's work and ports each distinct cluster
of a character at its own slot (#ADR-0012). Over the reports of the corpus, 459 of 5,086 ports
ship 2,604 units beside a champion's main unit (`docs/design/RUNS.md`, "Extra units beside a
champion's main unit"). Comparing each with the main unit by bytes tells 55 mods whose extra
units hold nothing of their own from 361 whose extra units ship own models, skeletons or
textures, and leaves 43 open. That test fixes PROJECT Urgot and keeps every unit of the other
416, and does not decide Ben 10 Ultimate Alien Udyr.

## Decision

A port changes its target slot only (the user's call, 2026-10-09). What sits under another slot
sits there because of how the game shares files between skins, and shipping it changes unrelated
skins; what a unit holds does not say which skin its author meant.

- **The target.** It is chosen among the author's units: after the copies, the game's own skins
  (#ADR-0020) and the chromas that are the same work (#ADR-0018) leave. The character's target
  slot is where its skin0 unit goes (#ADR-0013, #ADR-0015), and the unit that holds that target
  ships (#ADR-0076). Every other unit of the character leaves, and a sub-character's units follow
  their champion's.
- **No skin0 unit.** When no unit of the author's sits at skin0, every unit ships as before. That
  case is decided later.
- **A finding.** A unit that leaves while it holds own work the target's unit does not hold is
  named in a finding, with that work, so that a person knows to pass the option. Own models,
  skeletons and textures are the author's work for that unit's slot, and the option ships them.
  Own objects or own clips alone are not supported for now: whether they were made for that slot
  or put there by the sharing is not known, and the finding says so.
- **The option.** It keeps every unit at its own slot as before, for a mod that works on several
  skins, such as a restoration of each skin's old model. It sits beside `--keep-stale`
  (#ADR-0023), which such a mod may need for its old files. Under the option #ADR-0018 and
  #ADR-0076 apply to every unit as before; without it a chroma's unit leaves whatever it holds.
  A mod found to work on several skins could ship its other slots as layers of their own,
  which a player turns on in the mod manager; that is for later (the user, 2026-10-09).
- **What the target's unit names in other skins.** What the target's unit names that the game
  holds only for another skin is to be carried under the target's namespace. How is a decision of
  its own, after a measurement and on a record of what the client does with such a name.

## Consequences

- No port of a mod with a skin0 unit of the author's changes another slot. PROJECT Urgot 1.5.0
  and Ben 10 Ultimate Alien Udyr ship skin0 only.
- It supersedes #ADR-0020's third consequence, that several clusters of one character are each
  ported, and #ADR-0012's second, that a mod writing a different bin for each slot keeps every
  unit. A copy still leaves, and a mod made only for a premium slot still stays there.
- Without the option, the rule changes 331 of the 459 ports that ship extra units, and 2,018
  units leave (`docs/design/RUNS.md`, "A port changes its target slot only"): 1,408 with own
  models, skeletons or textures in 269 mods, 214 with own objects, other files or clips alone in
  58, and 396 with nothing of their own. The restorations named "Old" and the chroma packs are
  among the mods that lose skins unless the option is passed.
- 128 mods with no skin0 unit keep every unit, 20 of them with extra units that hold nothing of
  their own, until that case is decided.
- Work of the target's unit that only another unit carries is lost until the target's unit
  carries it. Ben 10 Ultimate Alien Udyr's skin0 unit carries none of its own clips (no events
  bank of the client's version to cut), and its sounds reach the game only through skins 3 and 5.
