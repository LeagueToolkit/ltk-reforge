# MOD-ARTIFACT-0009: A WAD packed whole as one entry

- **Source:** a mod manager's import of a WAD file, and packers not established
- **Affects:** the archive: an entry `WAD/<wad>` that is a file
- **Related:** #MOD-ARTIFACT-0006, #MOD-ARTIFACT-0007

## What is found

An entry `WAD/<wad>` that is a file, where other mods carry a folder of that name with one
entry for each chunk. Its bytes are a WAD.

The name is a WAD's when `.wad` is its last or next-to-last extension: `Lux.wad.client`,
`Lux.wad`, `Lux.en_US.wad.client`. A file `Yuumi.wad.client.pyntex.json` is no WAD.

No chunk of it has a path of the archive's spelling. A WAD holds each chunk under its path hash
only, as a chunk named by its hash is in a folder (#MOD-ARTIFACT-0007).

## Why it is there

One source is established. A mod manager imports a WAD file as a mod and writes `from wad` in
the mod's description. Almost every mod so described holds the WAD as one entry.

What packed the other mods this way is not established.

## What reads it, and how

A reader mounts the entry's bytes as a WAD and takes its chunks, each at its path hash. The
client reads a WAD the same way and needs no path.

- A chunk's path comes from a name table or stays unknown.
- The whole WAD is one entry, so a cap on an entry's size applies to the WAD
  (`archive.entry-too-large`).
- Some such entries are stored with a wrong checksum (#MOD-ARTIFACT-0006).

How each mod manager reads it is not established.

## The repair

The WAD's chunks, each at its path hash, as a folder's entries give them. No diagnostic is
raised.

- A WAD that does not mount gives no chunk (`archive.wad-unreadable`). So does one with more
  chunks than the cap, and one with a chunk that does not read or is over the cap.
- When the archive holds a WAD both as a file and as a folder, the two are one WAD. At a path
  both hold, the first in archive order stays.

## Evidence

- The baseline run (2026-10-03): 4,932 of the 5,137 mods carry a WAD packed whole, and 4,923 of
  them carry no WAD as a folder. 5,421 of the 5,671 WADs are packed whole.
- Those WADs hold 1,361,908 chunks. The run's name tables spell all but 268,220 of the hashes,
  and 3,110 mods hold a chunk with no known path.
- 669 mods of that run say `from wad` in their description. 667 of them carry a WAD packed
  whole.
