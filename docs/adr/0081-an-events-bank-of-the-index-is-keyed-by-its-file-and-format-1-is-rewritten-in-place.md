# ADR-0081: An events bank of the index is keyed by its file, and format 1 is rewritten in place

- **Status:** Accepted (2026-10-08)
- **Related:** #ADR-0036, #ADR-0042, #ADR-0047, #ADR-0077, #ADR-0080; `docs/design/HISTORY.md`,
  "The index"

## Context

`event_banks` keyed an events bank by its path and its record, the digest of its header and its
objects. Every locale's WAD holds its own voice bank at one path, with bytes and objects of its
own. At 16.19.8207193, 707 paths hold the banks of one file per locale. Their rows did not say
whose each was, so no reader could take the bank of the install's locale.

The user had a bank keyed by its file (2026-10-08). Either of two columns says which file: the
WAD of the placement, or the key of the file, on which `placements` joins with the path. Over the
history, 3,208 files at a bank's path were held by more than one WAD. A WAD column copies their
header and objects into a row per WAD. A file key holds each once, as `bins` and `animations`
key by content.

The index of format 1 at the head `f68de01e049c` (757 builds) is published. A change of the
format is a rebuild and a release under new names (#ADR-0036, #ADR-0047). No released program
reads format 1: release 0.1.3 reads format 0, and the reader and the writer of format 1 are not
released.

## Decision

`event_banks` keys a bank by its path and the key of its file. The record stays a fact of the
row. `events_banks` answers one bank per file, with each placement of the file at the path: the
WAD, and the builds it held the file there. The census writes the table so from now on.

`FORMAT` stays 1 (the user, 2026-10-08). The published file is replaced in place by the same
file with the table rewritten. `scripts/history/bank_files.py` rewrites it: of each file
`placements` holds at a bank's path, it reads one entry file of the census history the index was
built from, computes the bank's record as the census does, and copies the row of that record
under the file's key, with the builds and the count of the file's placements.

## Consequences

- A reader of one locale's WAD takes the bank that WAD held at its build. `reforge history
  probe` says which WADs held the bank of the install's bytes.
- `units_loading` reads `unit_banks`, which the units of a bin write: a bank's path, not a file.
  It is unchanged.
- The rewrite of the published file (2026-10-08): 162,930 rows at 16,291 paths became 162,970,
  one per file that holds a bank, because 35 records were held by more than one file. Of the
  163,950 files at those paths, 980 hold no bank the census records. Every row's record is the
  one its file's entry gives, and every row's builds and count are its files' placements'. The
  file grew by 5.2 MB. The rewrite took 63 s, the vacuum included.
- The check of the rewritten file at 16.19.8207193 against the install found no fault. Every row
  is keyed by a file a WAD held at its path. At each of the 707 paths, each file is one locale
  WAD's, one per locale. 710 en_US files are the row's bank, by the WAD's checksum, the key of
  their bytes, the header and the record. One more, Zaahen's voice audio bank, has no `HIRC`
  section in the install and no row. The census's own check against the trees of builds 378 and
  756 (`reforge-census`, `tests/history.rs`, ignored by default) found the 12,873 and 17,774
  events banks of the trees to be the rows a placement at the build joins, by path, file key
  and record.
- A format 1 file written before this keys the table by record. `events_banks` fails on it with a
  SQLite error, and so does an append. The app keeps an installed index folder that opens and
  does not compare it with the release's digest, so a copy fetched before the swap, and an
  index the app took forward from one, stay such files. `bank_files.py fill` rewrites one in
  place, given its builds list and a clone that holds its head.
- A writer of before this, appending to a rewritten file, inserts a bank row with no key and
  fails. None is released.
