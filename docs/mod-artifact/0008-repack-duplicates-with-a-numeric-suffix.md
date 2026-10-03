# MOD-ARTIFACT-0008: Repack duplicates with a numeric suffix

- **Source:** a repack; the tool is not established
- **Affects:** the archive: entries under a WAD's folder named by their path hash
- **Related:** #MOD-ARTIFACT-0007

## What is found

An entry directly under a WAD's folder named sixteen hex digits, a dot and a number:
`WAD/<wad>/<16 hex>.1`, `WAD/<wad>/<16 hex>.2`. The digits are a path hash
(#MOD-ARTIFACT-0007).

The number is everything after the first dot. `<16 hex>.bin` is a chunk named by its hash, and
so are `<16 hex>.bin.1` and `<16 hex>.1.bin`: their extensions are not numbers.

## Why it is there

ltk-forge's reader takes it as a duplicate a repack kept: a second chunk for one path hash,
differing from the first, written under the first's name with a number added. Which tool writes
it, and how a WAD came to two chunks for one hash, is not established.

## What reads it, and how

The client finds a chunk by its path hash and reads no name: the number means nothing to it. A
reader that hashes the name as a path puts the duplicate at a hash nothing looks for. How each
mod manager reads such a name is not established.

## The repair

The entry without the number. The numbered entry is skipped with a note
(`archive.repack-duplicate`).

- Its bytes are not compared with the first's.
- It is skipped whether or not the archive holds the entry without the number.

## Evidence

- The baseline run (2026-10-03): none of the 5,137 mods carries it.
- ltk-forge's first corpus run (2026-09-29): 2 of 1,029 mods, 3 entries, each with the number 1.
