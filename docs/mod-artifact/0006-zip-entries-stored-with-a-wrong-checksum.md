# MOD-ARTIFACT-0006: Zip entries stored with a wrong checksum

- **Source:** some packers; which ones is not established
- **Affects:** the archive: entries written with the stored method
- **Related:** #MOD-ARTIFACT-0009

## What is found

A zip entry written with the stored method, uncompressed, whose recorded CRC-32 is not the
CRC-32 of its bytes. Every entry seen with it holds a WAD packed whole (#MOD-ARTIFACT-0009).

The zip alone does not tell it from damage: a wrong checksum over intact bytes and a right
checksum over damaged bytes fail the same check. The content tells. A stored entry's bytes sit
in the archive as they are, and a damaged WAD does not mount (`archive.wad-unreadable`).

A compressed entry that fails its checksum is not this artifact. It is taken as damaged and
skipped (`archive.entry-corrupt`).

## Why it is there

Not established. ltk-forge's docs say that some packers write the checksum wrong, and name
none. Which value such a packer records is not established either: ltk-forge's reports do not
hold it.

## What reads it, and how

A zip reader that checks checksums reads the entry to its end and then fails it: the `zip` crate
returns a checksum error. A reader that takes a stored entry's bytes without the check reads the
content whole. What each mod manager does with such an entry is not established.

## The repair

The entry's bytes as stored, read without the check, with a note (`archive.stored-crc-wrong`).
A stored entry that does not read that way either is skipped (`archive.entry-unreadable`).

Nothing of the artifact reaches a package: a package holds chunks, not the archive's entries.

## Evidence

- The baseline run (2026-10-03): none of the 5,137 mods carries it.
- ltk-forge's full datasets run (2026-09-30): 1 of 5,544 mods, on the entry of a packed WAD.
- ltk-forge's in-game sample mods (reports of 2026-10-01): 4 of 15, each on the entry of a packed
  WAD. The four WADs mount and their mods port.
