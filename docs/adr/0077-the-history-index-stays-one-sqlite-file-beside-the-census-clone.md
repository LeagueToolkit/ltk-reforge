# ADR-0077: The history index stays one SQLite file beside the census clone

- **Status:** Accepted (2026-10-07)
- **Related:** #ADR-0005, #ADR-0036, #ADR-0037, #ADR-0043, #ADR-0057, #ADR-0060, #ADR-0067;
  `docs/design/HISTORY.md`

## Context

The porter's next items (`docs/design/ROADMAP.md`, "The porter: next", items 3 and 4) ask the
game at a chosen build: the file at a path, the bin there with its links and its objects, the
text of an object, the objects a skin entry reaches by its links, and the kind a field had. The
index of format 0 answers what the game ever shipped, with one life per key that spans the
builds between a key's first and last. An object's text at a build is found by a search of
that build's tree in the census clone.

The options were to keep the index as it is and grow it, to move the questions to a graph
engine, or to keep a store of the history's trees beside the index. Graph engines and a
separate tree store were tried and dropped: every question of a stage is a keyed read or a
short chain that ends in one object read. The graph engines made those reads slower and sped up
only an analyst's scans over the whole history. A tree store holds again what the clone holds.

## Decision

The index stays one SQLite file, and the census clone stays the only store of entry content (the
user, 2026-10-07).

- A question at one build reads a table whose rows are unbroken lives, so the rows alive at the
  build are the build's. The keys stay those of #ADR-0037.
- The index holds an entry text's blob id and never its text. `reforge-census` reads the text
  from the clone by the id.
- An append on the user's machine may take a minute or two.
- No graph engine, and no store of trees beside the clone.

## Consequences

- The index's format is 1, and an index of format 0 is rebuilt (#ADR-0036).
- An object's text at a build is one keyed read and one object read from the clone, a quarter
  of a millisecond where the search of the tree took ten
  ([RUNS.md](../design/RUNS.md), "The history index, format 1, every build").
- A clone that lacks a text's blob does not answer for it. The clone an append fetches holds
  the texts of the builds after the index's head (#ADR-0057); a whole clone, or one started
  from a release's archive (#ADR-0060), holds every text.
- The index grows by a third, from 2.75 GB to 3.67 GB at 757 builds, the placements and the
  objects of every bin the most ([RUNS.md](../design/RUNS.md), same section).
- An append reads the `_wad.yaml` of every WAD a build changes, and the fetch asks for it.
