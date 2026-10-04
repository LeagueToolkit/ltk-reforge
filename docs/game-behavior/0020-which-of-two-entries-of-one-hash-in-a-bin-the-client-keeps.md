# GAME-BEHAVIOR-0020: Which of two entries of one hash in a bin the client keeps

- **Status:** Open
- **Builds:** none checked
- **Affects:** `.bin`: a bin's table of contents and its objects
- **Related:** none

## Behaviour

Not established. A bin's table of contents lists each object by its entry hash. A bin may list
one entry hash more than once, with an object for each listing. Which of those objects the
client keeps, the first, the last or both in turn, has not been checked.

Assumed: the client keeps the last object listed under the hash. That is what `ltk_meta`'s
reader gives: its tree holds one object per entry hash, and a later object replaces an earlier
one at the earlier one's place in the bin's order.

## In a mod

- Under the assumption a reader takes the last object under a hash only: an earlier one is
  not judged, fixed or carried.
- If the client keeps the first, the object judged and ported is not the one the author saw
  in game.
- When the two objects are equal, the assumption changes nothing. How many of the bins hold
  two that differ is not counted.

## Evidence

- `ltk_meta`'s reader keeps the last object under an entry hash. That is the library's
  behaviour, not the client's.
- The baseline run (2026-10-03): 128 of the 5,137 mods hold a bin that lists an entry hash
  more than once.
- No check in game is recorded. A check would settle it: a probe mod whose skin bin lists one
  entry twice, the two objects differing in one visible field, seen in game.
