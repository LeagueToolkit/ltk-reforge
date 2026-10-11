# ADR-0102: A package embeds its report compressed

- **Status:** Accepted (2026-10-12)
- **Related:** #ADR-0041, #ADR-0096; `docs/design/ARCHITECTURE.md`, "Package" and "Report and
  dump"; `docs/design/ROADMAP-UX.md`, "The app's store"

## Context

A package carries the evidence of the run that made it in its readme (#ADR-0041): a few KB a
person can read. The run's report, 0.4 to 12 MB of JSON, is written beside the package as a
file. The file is lost when the package is copied, shared or installed alone, and the app's
"open package" (#ADR-0096) then shows the evidence alone: no findings, no decisions with their
candidates, no changes.

`ltk_modpkg` stores the readme chunk as it is given, uncompressed, and its builder has no API
for a meta chunk of the caller's. A chunk of a WAD is the client's.

Measured on three ports (2026-10-11): the report's JSON under zstd is 50 to 340 KB at level 19
and about a third more at level 3; base64 adds a third.

The options were:

- the report beside the package only, as it is;
- a meta chunk of its own, compressed by the container, which needs a change to `ltk_modpkg`
  upstream;
- a second block in the readme, compressed and encoded as text.

## Decision

The readme holds the run's report as a second fenced block after the evidence's (the user's
call, 2026-10-12).

- The block's info string is `reforge.report+zstd/1`. Its body is the report's JSON, as
  `Report::to_json` writes it and the writer writes it beside the package, compressed with
  zstd at level 9 and encoded as standard base64 in lines of 96 characters.
- A line of text for a person stands between the two blocks.
- The block is left out when the encoded text would pass a bound, or when the readme with it
  would pass the readme's read cap. The evidence document says whether the report is
  embedded, and how many bytes its JSON holds.
- A reader decodes the block under a cap of its own on the decoded bytes. A report block in a
  readme with no evidence block is no report: the report alone is not trusted to say what
  made the package.
- The report file beside the package stays (#ADR-0041).

## Consequences

- A package opened in the app gives its run's whole result, the report beside it or not.
- A readme a person opens in a mod manager shows a block of base64 under the text.
- The readme of a large port is several hundred KB, under the read cap of 1 MiB.
- The zstd level, the line length and the bound are the writer's constants. A reader takes
  any level and any line length.
- A meta chunk of the caller's is the better place once `ltk_modpkg` gains one. The reader
  keeps reading the readme's block after that, for the packages written before.
