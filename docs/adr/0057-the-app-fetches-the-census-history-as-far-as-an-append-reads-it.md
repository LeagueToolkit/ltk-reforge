# ADR-0057: The app fetches the census history as far as an append reads it

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0047, #ADR-0042; `docs/design/HISTORY.md`, "Appending" and "Fetching"

## Context

#ADR-0047 has the user's index take further builds on the user's machine from the census
history, and leaves open how the commits after the index's head are fetched with no full clone.

An append reads, for each build after the head: the commit, its tree, its `census.yaml` and
`build.yaml`, the trees under `files/` that its diff against the build before descends into,
on both sides, and both sides of each entry file and entry text that changed. It reads nothing
of a build before the head.

Three ways to get those objects, measured 2026-10-05 for the two builds after the released
index's head (16.19.8207193 to 16.19.8230722):

| fetch | objects | bytes | time |
|---|---|---|---|
| a full clone | 12.3 million | 5.25 GB | |
| a shallow clone of the head and the two builds | 1.33 million | 1.09 GiB | 8 minutes, most of it the server counting and compressing |
| the branch's commits without trees, and the objects the append reads, asked for by id | 757 commits and 70 objects | 616 KB | 4 to 9 seconds |

A shallow clone holds the whole tree of its oldest commit: 508,661 trees and 1,412,087 files
at 16.19.8207193. A minor build adds some hundred objects to the history, a patch some 88,000.

GitHub sends an object asked for by id with the filter `tree:0`, and nothing the object
reaches. It sends the branch's commits alone under the same filter. Both checked with git,
2026-10-05.

## Decision

The user, 2026-10-05: the app fetches the census history and adds the builds after the index's
head to the index, and fetches only what that needs. The fetch takes every commit of the
`history-v2` branch without its tree, then, of the builds after the head, the trees a diff
descends into, one level of the tree at a time for every build at once, and last the files the
walk reads.

- The clone is the app's, bare, in its data folder. Each update fetches the commits since the
  last and what the new builds read.
- The index the runs read is not written. The builds are appended to a copy in a folder of its
  own, and the settings move to it.
- The update starts by reading the branch's tip from the server, and fetches nothing when the
  tip is the index's head.

## Consequences

- The clone serves an append and nothing else. A whole build from it stops at the first object
  it lacks, so building an index from nothing still needs a full clone.
- An entry's text at a build before the head is not in the clone: reading one with no full
  clone stays open (#ADR-0047).
- The server has to send an object by id under a filter. A server that takes no filter is
  refused, since a fetch of one commit would bring the whole history.
- An index of another history is refused: its head is no commit of the published one. Its
  remedy is the download of the newest index.
- An index appended from this clone is the one appended from a full clone, byte for byte:
  `history.sqlite` had the same SHA-256 both ways for the two builds above (2026-10-05).
- An update writes a copy of the index, 2.75 GB at 16.19, beside the one the runs read. The
  older folder is removed at the next start.
- The fetch needs no git on the user's machine. The tests serve a repository with
  `git upload-pack`, so they need git.
- A request asks for at most 10,000 objects. A patch's update takes some ten requests.
