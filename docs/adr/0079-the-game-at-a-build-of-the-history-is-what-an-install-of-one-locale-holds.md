# ADR-0079: The game at a build of the history is what an install of one locale holds

- **Status:** Withdrawn (2026-10-08): the install holds one locale, and the install is required
  (#ADR-0080)
- **Related:** #ADR-0043, #ADR-0077, #ADR-0078

## Context

A run with no install is judged against the game a build of the history holds (#ADR-0043). The
typed questions an install answers from its chunks are answered from the index: whether a path
holds a file, which WADs hold it, whether it holds a file's bytes, a bin, a skeleton, a clip's
joints, an events bank.

The index holds what an install holds, and more, and in another form:

- It holds every locale's WADs. An install holds the WADs of the locale it was installed in. At
  16.19.8207193 every champion's voice bank is at one path in 21 locale WADs, each with its own
  bytes.
- It keys a path's files by WAD and content, and orders WADs by `wad_order`. An install reads a
  path from its champion WADs first.
- It holds a bin's links as path hashes, in the order the census lists them, and its objects as
  `ltk_meta` writes them, in entry hash order. It holds no bin's bytes.
- `placements` leads its key with the path. A question by path is one indexed query, which can
  fail at any call. An install reads its WADs' tables of contents once, when it opens.

The options were: to ask the index at each question, or to read the build's files once; to hold
every locale, or one; to order a path's WADs by `wad_order`, or as an install does; to fail a
bin with a link no table spells, or to leave the link out; to give a bin's bytes as written from
its objects, or none.

## Decision

The game at a build of the history is what an install of one locale holds, read from the index
when it opens.

- **Read once.** The files the build's WADs held are read at open, a scan of `placements`:
  948,356 files at 16.19.8207193, in 0.6 s. Whether the game holds a path, which WADs hold it
  and whether it holds a file's bytes are answered from them, by the key of the bytes
  (#ADR-0037), and no such question fails.
- **One locale.** A locale WAD of another locale than the caller's is left out. The command
  takes `en_US`, the locale of the install the baselines are measured against.
- **The install's order.** A path's WADs are the champion WADs first, each group in
  `wad_order`, as an install orders its own.
- **Links spelled by the names.** A bin's links are spelled by the run's name tables. A link no
  table spells is left out: 1 of the 14,766 links of the bins of 16.19.8207193 by the
  CommunityDragon tables.
- **Bytes written.** A bin's bytes, which the overlay applies a patch to and a binless mod takes
  as its own, are its links and objects as `ltk_meta` writes them.
- **The record that came last.** Of a path's records in a table whose lives are hulls
  (skeletons, meshes, events banks), the one whose life holds the build and started last.

## Consequences

- A run with no install reads the build's files at open, as one with an install reads its WADs.
  A port of the reduced corpus takes 65 s against the history and 17 s against the install.
- What the index holds in another form than the install's file shows in the output: a bin's
  objects in entry hash order, its links sorted and in lower case, a WAD's name in lower case, a
  bin taken from the game with other bytes than the game's file.
- An events bank that every locale's WAD holds at one path is several records at the build, and
  the index does not say which is which WAD's. The record taken is the one that came last, which
  may be another locale's. 707 paths hold such banks at 16.19.8207193.
- A run for another locale than `en_US` needs the command to take one.
