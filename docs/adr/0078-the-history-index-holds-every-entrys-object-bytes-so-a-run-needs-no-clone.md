# ADR-0078: The history index holds every entry's object bytes, so a run needs no clone

- **Status:** Accepted (2026-10-08)
- **Related:** #ADR-0005, #ADR-0036, #ADR-0047, #ADR-0057, #ADR-0060, #ADR-0077;
  `docs/design/HISTORY.md`, "Object bytes"

## Context

#ADR-0077 keeps the census clone as the only store of entry content: the index names each entry
text's blob, and `reforge-census` reads the text from the clone by it. The clone `Fetcher` fills
holds only the texts of the builds after the index's head (#ADR-0057), so an object at an older
build needs a whole clone, which #ADR-0060 has the app make from a released archive: 1.77 GB
downloaded, 5.4 GB on disk. A text read in a run is also parsed as ritobin, which #ADR-0005
keeps out of a run.

The census history's 757 builds hold 3,165,814 distinct entry texts. The objects they describe,
as `ltk_meta` writes them, are 22.4 GB. Kept apart from the index on 2026-10-08, they took
(`docs/design/RUNS.md`, "Entry objects stored apart from the index"):

| layout | bytes | a read |
|---|---:|---|
| every object in one file, the file compressed whole | 360 MB | none by object |
| each object alone, a dictionary per class | 2.07 GB | one frame, 34 µs |
| each version of an entry against the one before, a keyframe every 16 | 545 MB | up to 16 frames, 125 µs |
| each version against its block's keyframe, a keyframe every 8 | 866 MB | two frames, 58 µs |

Dictionaries trained per group of similar objects, by a MinHash of their bytes, did no better
than a dictionary per class.

## Decision

The index holds every entry's object bytes at every build, so a run needs no clone (the user,
2026-10-08). The clone is what an append reads, and nothing else.

- An object is stored as `ltk_meta` writes it from its entry hash on: the bytes its object key
  digests. A run reads an object as bytes, never as text.
- Each distinct object is stored once. An entry's versions are its distinct objects in the order
  they appeared, in blocks of eight behind a keyframe. A keyframe is compressed with the
  dictionary of its class, of 1 MiB at most, or a general one; every other version with its
  keyframe's bytes. A read decodes two frames at most.
- A whole build trains the dictionaries. An append uses them as they are.
- The zstd level, the keyframe interval, the dictionary cap and the zstd version are in the
  stamp.
- Format 1 is not released, so it is defined with these tables and no format is added
  (#ADR-0036).

## Consequences

At 757 builds (`docs/design/RUNS.md`, "The history index with every object's bytes"):

- The index grows by 1.10 GB, from 3.67 GB to 4.77 GB, and its download under zstd from 1.01 GB to
  1.73 GB: the frames are zstd already.
- An object at a build reads in 82 µs at the median, from the index alone, where its text through
  the clone took 128 to 162 µs.
- A whole build takes 3,837 s at 26.0 GiB, against 3,231 s at 20.9 GiB without the object bytes.
  An append of the last five builds takes 32.5 s, against 14.1 s.
- The clone #ADR-0057 fetches is the only clone a user's machine holds. A whole clone serves no
  run, so the archive #ADR-0060 bootstraps one from is superseded. The app's code for it stays
  until it is removed.
- A run that reads an object links no ritobin parser: the writer parses each text once.
- A text that does not parse, or holds no object or several, has no bytes. The stamp counts them,
  and a run has no other source for them.
- An object first held at a build whose bins number property kinds the old way (before 10.8) is
  stored in the current numbering. When it holds a kind that numbering writes as another byte,
  its digest is not the object key the census recorded from the game's bytes.
