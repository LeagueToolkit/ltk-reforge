# GAME-BEHAVIOR-0010: A PTCH record's path names are hashed lowercased

- **Status:** Open
- **Builds:** 16.19
- **Affects:** the PTCH format: a record's path of field names
- **Related:** #ADR-0026, #ADR-0031

## Behaviour

A PTCH record names the field it sets by a path of plain names, `mSpell.mImgIconName`. Each name
on the path is lowercased and hashed with FNV-1a, and the hash finds the field: a pointer is
followed, an embedded struct is entered.

A record whose path leads to no field of the object is skipped, and the records after it
apply.

What is assumed: the name is only its hash, so any spelling that hashes to the field names it,
in any case. This is how `ltk_meta` applies a patch, which LTK Manager's overlay does when it
builds the game's bin with a mod's patch. The client itself applies a PTCH in a few UI cases
only, and how it hashes a name there was not read.

## In a mod

- A patch reaches the game through the overlay: the overlay applies it to the game's bin and
  writes the result. The client loads that bin and no PTCH.
- A field no table names cannot be addressed by a record, unless a name is made that hashes to
  it (#ADR-0031).

## Evidence

- `ltk_meta` hashes each segment of a record's path lowercased, and resolves it against the
  object as described.
- Ports whose records use the names the hash tables spell, in the tables' own case, work in
  game (2026-09-29 to 2026-10-01).
- Not checked: a record through a name made to hash to its field. What would confirm it: one
  such record in a port, and the field's value seen in game.
