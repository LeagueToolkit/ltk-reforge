# MOD-ARTIFACT-0023: A bank with header id zero

- **Source:** LtMAO's bank writer, by its source; that the datasets' banks are its output is
  inferred from their shape
- **Affects:** `.bnk` files, the id in the `BKHD` chunk
- **Related:** none

## What is found

A Wwise bank whose header id is zero. A bank begins with a `BKHD` chunk: the four letters, the
chunk's length, the bank's version, then the bank's id, each a 32-bit number. The id is the
four bytes at offset 12.

A bank of the game carries the hash of its own name there: the 32-bit FNV-1 hash (multiply,
then xor) of the lowercased stem of its file name. `braum_base_sfx_audio.bnk` carries
3580963463.

Almost every bank found with id zero is a media bank, `*_audio.bnk`. It is told from a game
bank by the id alone.

## Why it is there

The bank is one a tool built from the author's sounds, and the tool wrote no id. LtMAO's bank
writer, which builds a bank from a folder of `.wem` files, writes every bank so: a `BKHD` of
length 32 with version 134 and id 0, then the same 24 bytes (`3E 5D 70 17`, the language id of
the effects banks, then 0, then 250, then twelve zeros), copied from a game bank of version 134.
Only a `DIDX` and a `DATA` chunk follow, the media packed with no alignment, and no `HIRC`: the
bank holds sounds and no events, as a media bank does. It writes the effects language into a
voice bank too.

## What reads it, and how

- **The client.** A play action names the bank it expects its sound in by the bank's id. What
  the client does with a bank whose header says zero is not established. No dated check in game
  is recorded for it.
- **LTK Manager** reports a zero id as a fault of the mod, by ltk-forge's docs.

## The repair

The id a game bank of that name carries: the FNV-1 hash of the lowercased stem of the bank's
file name, written at offset 12. No other byte of the bank changes
(`audio.bank-id-set`).

A bank whose path no name table spells has no name to hash, and keeps its zero
(`audio.bank-unnamed`).

## Evidence

- The baseline run (2026-10-03): 1,453 of the 5,137 mods carry a loaded bank with header id
  zero, 1,902 banks. 1,771 are effects banks (`_sfx_`), 128 voice banks (`_vo_`) and 3 others.
  1,896 names end `_audio`. None is an events bank. No such bank is at a path no table spells.
- The hash: the FNV-1 hash of `braum_base_sfx_audio`, computed 2026-10-04, is 3580963463, the
  id ltk-forge's notes give for the game's bank of that name. The ids the run gave to three
  other banks are the FNV-1 hashes of their names.
- That every game bank carries the hash of its name is ltk-forge's statement. The census history
  holds a bank's clips and events and not its header, so it is not checked here.
- ltk-forge's notes record a check in game (2026-09-30) of the opposite case: a bank under a
  file name that does not hash to its header id plays.
- LtMAO's bank writer is read from its source (version 28032026.0, read 2026-10-06). Whether
  the banks of the datasets hold its 24 fixed bytes is not checked: the reports do not give a
  bank's header.
