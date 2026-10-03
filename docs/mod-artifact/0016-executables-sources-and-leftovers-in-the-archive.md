# MOD-ARTIFACT-0016: Executables, sources and leftovers in the archive

- **Source:** authors' practice: a working folder packed whole
- **Affects:** the archive: files of kinds the client does not read, inside its WADs and beside
  them
- **Related:** #MOD-ARTIFACT-0015, #ADR-0024

## What is found

Files that are no game file, each known by its first bytes and by its name:

| finding | kinds | code |
|---|---|---|
| programs | Windows executables and libraries | `inventory.executable` |
| sources | bins as ritobin text or JSON, Maya scenes and their image caches, FBX, glTF, and GIMP, Paint.NET, Photoshop and EXR images | `inventory.source` |
| archives | zip, RAR, Zstandard | `inventory.nested-archive` |
| leftovers of file browsers | shortcuts, `desktop.ini` | `inventory.os-artifact` |
| files of no known kind | | `inventory.unknown` |

Every one found in the baseline run is a chunk of a WAD. Where no name table spells a chunk's
path, as for most of these in a WAD packed whole, its first bytes alone tell what it is.

A bin as text is a source when it sits under a name of its own: of the 593 with a known name,
589 end `.py` and 4 end `.json`. At a bin's own path it is another artifact
(#MOD-ARTIFACT-0015).

`META/info.json` and the archive's cover image are the archive's own and are not counted.

## Why it is there

The kinds are those of an author's working folder: a converter's text of a bin, the scenes and
layered images a skin is made from, a tool's program, a file browser's files. A folder packed
whole carries them. Which tool wrote which file is not established.

## What reads it, and how

No game object names these files, so the client does not load them. They add to the size of the
mod.

Nothing runs a program in a WAD. It reaches a disk when a person or a tool extracts the mod
(#ADR-0024).

## The repair

The files left out. A port ships what its objects name and a repair ships what the client
loads, and nothing names these.

- Each finding is reported, a program as a warning.
- No program is run or examined, and no archive inside a mod is opened (#ADR-0024).
- The sources are named in the report: they are what a skin's work can be recovered from.

## Evidence

The baseline run (2026-10-03), by mods of the 5,137 and by files:

| kind | mods | files |
|---|---:|---:|
| Windows executable or library | 66 | 106 |
| bin as ritobin text | 1,705 | 6,517 |
| bin as ritobin JSON | 625 | 2,091 |
| Maya image cache | 398 | 1,860 |
| GIMP image | 71 | 82 |
| FBX | 50 | 91 |
| Paint.NET image | 31 | 81 |
| EXR image | 19 | 34 |
| Photoshop image | 16 | 37 |
| Maya scene | 7 | 8 |
| glTF | 6 | 11 |
| Maya swatches | 2 | 2 |
| zip | 27 | 39 |
| RAR | 8 | 8 |
| Zstandard | 6 | 127 |
| shortcut | 23 | 126 |
| `desktop.ini` | 9 | 16 |
| no known kind | 16 | 86 |

- By finding: sources in 2,056 mods, programs in 66, archives in 40, leftovers of file browsers
  in 32, files of no known kind in 16. 2,102 mods hold one or more of these or a bin as text at
  a bin's path.
- No mod of the run holds such a file outside its WADs.
- ltk-forge's count over the 5,658 archives of the datasets (no date recorded): programs in 66
  mods and archives in 45.
