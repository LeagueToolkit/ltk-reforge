# Measured runs

Runs over the corpus, oldest first, each with its date and what it measured. A run about one
record goes in that record's evidence. What forge measured before the rewrite is in forge's own
runs log.

## The baseline (2026-10-03)

forge at `d6cd66f` over the corpus, with forge's tracked runner (`scripts/corpus/port_run.py`
there), against the 16.19.8207193 install copy, the history index of 755 builds and the meta
schema from LTK Manager's cache. Each step of the rewrite is compared with it (#ADR-0004).

- **Scope.** 8,604 mods classed, 5,137 in scope (#ADR-0033). None failed.
- **Time.** 1,633 s at ten in parallel. 16,306 s summed over the mods, median 2.4 s, at most
  43.5 s.
- **Outcome.** 5,136 port. One halts: no WAD ships a champion's skin.

Champion units' targets under the default policy:

| target | reason | units |
|---|---|---:|
| skin0 | base | 3,455 |
| skin0 | paste cleaned | 464 |
| skin0 | fits no slot | 3 |
| skinN | slot | 5,748 |
| skinN | fallback to the premium origin | 497 |
| skinN | the skin whose clips it plays | 84 |
| skinN | unjudged | 2 |
| none | made before a rework | 17 |
| none | superseded | 4 |

Errors, by mods: a value of another type than the build declares in 41, made before a rework in
15, a name unresolved in 3, fits no slot in 3. Fields lost to a layout change in 30 mods, a value
with no conversion in 1.

Leaving the port: 1,960 chroma units folded, 31 mods re-shipping the game's own skins. 5 new
sub-character bins. Audio: 5,017 units over the game's, 568 of their own, 1,859 voice units in
the voice layer and 1,102 in a locale layer.

**The reduced corpus.** 27 mods show the 149 features of the run, in 65 s summed, 9 s at eight
in parallel. Two runs of it show no change in unit fates, targets, reasons or diagnostics.

**forge's reports are not byte for byte the same across two runs.** 9 of the 27 differ, in one
place: the order of a clip's joint hashes in the rig facts. A comparison reads that list as a
set.
