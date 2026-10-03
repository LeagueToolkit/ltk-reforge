# MOD-ARTIFACT-0007: Chunks named by their path hash

- **Source:** not established
- **Affects:** the archive: entries under a WAD's folder
- **Related:** #MOD-ARTIFACT-0008, #MOD-ARTIFACT-0009

## What is found

An entry directly under a WAD's folder whose name is sixteen hex digits, with or without an
extension: `WAD/<wad>/<16 hex>` or `WAD/<wad>/<16 hex>.<ext>`. The digits are the chunk's path
hash. The other entries of the folder are named by their paths
(`WAD/<wad>/data/characters/...`).

It is told from a path by its shape and its place. The name up to its first dot is sixteen hex
digits, and it sits in no folder under the WAD's. A name of that shape inside a folder is a
path.

The extension is no part of the hash. An extension that is a number marks a repack duplicate
(#MOD-ARTIFACT-0008).

## Why it is there

A WAD holds each chunk under the hash of its path and holds no path. A path comes from a name
table, and a table does not spell every hash. Which tools write the hash as the entry's name,
and for which chunks, is not established.

## What reads it, and how

The client finds a chunk by its path hash and reads no name, so a chunk loads without a known
path. A reader of the archive has to take the digits as the hash: the name hashed as a path
gives a hash nothing looks for. How each mod manager reads such a name is not established.

## The repair

The chunk at the hash its name gives, the extension dropped. No diagnostic is raised. Its path
is spelled where a name table of the run holds the hash, and is unknown otherwise.

When an entry named by its path and an entry named by the hash of that path are both in a WAD,
the first in archive order stays and the second is skipped (`archive.chunk-duplicate`).

## Evidence

- The baseline run (2026-10-03): 30 of the 5,137 mods carry such chunks, 5,037 chunks in 34
  WADs. In 6 of the mods a WAD holds no chunk named by its path.
- The run's name tables spell 3,167 of the 5,037 hashes. 1,870, in 16 mods, have no known path.
- The count covers WADs shipped as folders. A report does not tell these chunks apart in a WAD
  that the archive also packs whole.
