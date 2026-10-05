# MOD-ARTIFACT-0007: Chunks named by their path hash

- **Source:** the WAD extractors of the modding tools; which one wrote a given mod's entries is
  not established
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
table, and a table does not spell every hash. The extractors write a chunk no table names under
its hash as sixteen hex digits:

- `ltk_wad`'s extractor, with no extension;
- LtMAO's, with an extension it guesses from the bytes. It also writes a chunk whose name is over
  255 characters, or names a folder, as `<hex>.<ext>`, and lists each such name with its path in
  `hashed_files.json`;
- cslol-go's, with an extension guessed from the first bytes: any `r3d` magic gives `.scb`, so an
  animation or a voice package is named `.scb`, and a texture, a mesh or a skeleton `.bin`.
  Its fixer writes a `file` value it cannot name as sixteen hex digits with no extension.

Which tool wrote the entries of the mods that carry them is not established.

## What reads it, and how

The client finds a chunk by its path hash and reads no name, so a chunk loads without a known
path. A reader of the archive has to take the digits as the hash: the name hashed as a path
gives a hash nothing looks for.

LTK Manager reads the stem of every file under a WAD's folder, at any depth: a stem of sixteen
hex digits is the chunk's hash, and any other name is hashed as a path. The stem is the name up
to its last dot. It reads a mod project's files and a `.fantome`'s entries so. A file of that
shape inside a folder, whose path its bins name, lands at the hash its stem spells, where those
bins do not look.

Other tools read a name at a WAD folder's root so too, some with no check of its length.
TopazModFixer's folder packer and cslol-go's fixer take any root name whose stem reads as a hex
number for that number: `face.dds` is packed at `0x000000000000face`, `1.png` at `0x1`.

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
- LTK Manager's reading, from the source of `ltk_overlay` 0.14.2 and `ltk_wad` 0.5.8 (read
  2026-10-05): `resolve_chunk_hash` and the `.fantome` reader take the hash from
  `hex_chunk_hash`, which tests the path's file stem only.
- The tools' extractors and readers, from the source of LtMAO, TopazModFixerCLI and cslol-go
  (read 2026-10-05 and 2026-10-06). The baseline repair run (2026-10-04) holds one packed WAD
  with a chunk at `0x000000000000face`, which no name table spells: the hash such a reader makes
  of a file named `face`.
- Seen in game (2026-10-05, build 16.19): a port that shipped four textures as
  `textures/<16 hex>.tex` under its namespace, installed with LTK Manager, showed no texture on
  the model, no loading screen picture and no icons. Its mesh and skeleton, under names, loaded.
