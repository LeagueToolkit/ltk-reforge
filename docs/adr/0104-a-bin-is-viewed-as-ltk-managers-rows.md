# ADR-0104: A bin is viewed as LTK Manager's rows

- **Status:** Accepted (2026-10-11)
- **Related:** #ADR-0046, #ADR-0093, #ADR-0103; `docs/design/ROADMAP-UX.md`, "The Runs page"

## Context

The file viewer drew a bin as text: every object dumped with its names spelled, the object a
jump asked for first between two fences. A jump from a type check's place (#ADR-0103) could
land on the object and no deeper, since a line of text has no address for a field. A skin's
object runs to a thousand lines and more, which a person scrolls through rather than reads.

LTK Manager's bin editor already draws a bin as rows: the backend holds the parsed tree and
answers a node's children a page at a time, each row addressed by its object's entry and its
hash path, and the front end draws the rows as a virtualized tree. Its hash path is the form
`ltk_meta`'s `ValuePath` writes, which is the form a type check's place carries. Both projects
are LeagueToolkit's, under GPL-3.0-or-later.

The options were:

- keep the text, and mark the field asked for in it;
- write a tree of reforge's own;
- copy LTK Manager's rows and tree, and cut them to a viewer.

## Decision

A bin a person opens in the file viewer is drawn as LTK Manager's bin editor draws one: rows
in a tree, read-only, in place of the text (the user's call, 2026-10-11).

- The code is copied from LTK Manager and cut to a viewer: no edits, no declarations, no
  links to open, no class cards. Every copied file says so in its module comment, with the
  path it came from in LTK Manager and what was cut.
- The types on the wire keep LTK Manager's names (`BinRow`, `BinRows`, `BinValue`, `RowNode`,
  `PropertyKind`) and its casing, so the two codebases read side by side.
- A bin is read by its content id from the run's tiers and held parsed, a few at a time, with
  no open and close pair: the bytes under an id never change, and a bin let go is parsed again.
- Names come from reforge's speller, the run's names over the inputs' tables, and every text a
  person reads on a row is cleaned (#ADR-0093).
- A jump to a place is a reveal of the row its entry and path name: the tree opens the rows
  above it, asks for the pages it needs and scrolls the row to its top.

## Consequences

- The copied code is kept in step with LTK Manager by hand. A change there that the viewer
  wants is copied again, starting from the paths the module comments name.
- The app depends on `@tanstack/react-virtual`, at the version LTK Manager pins.
- The text dump of an object stays where the History page shows one (`dump_object`), and
  nowhere else.
- A row carries no declared kind: the schema's line for a field and the mark of a mismatch are
  the type check's rule, and drawing them on a row is a decision of its own.
