# ADR-0093: A mod's bytes live in a blob store, by content id

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0041, #ADR-0044, #ADR-0051, #ADR-0078, #ADR-0090, #ADR-0091, #ADR-0094,
  #ADR-0095, #ADR-0096; `docs/design/ARCHITECTURE.md`, "Untrusted input";
  `docs/design/ROADMAP-UX.md`, "The second version"

## Context

A run holds a mod's bytes once, in memory, and nothing of them outlives the run. `AGENTS.md`
forbids extracting an archive to disk: the concern is a mod's names reaching the filesystem
as paths and extensions, since a mod is untrusted input. The rule's wording, "entries are read
in memory and only derived facts are written", also forbids a store of the bytes under the
program's own keys, which the concern does not cover.

The pages #ADR-0091 decides and the guided view of #ADR-0090 need the bytes after the run: a
mod browsed at any stage, a run reopened, two runs compared, a batch of mods. Held in memory,
each is a run of its own from the archive. Over the datasets (2026-10-10, the 4,909 reports of
`baseline-port-5`; `docs/design/RUNS.md`, "What a blob store holds"):

| | total | median mod | largest mod |
|---|---:|---:|---:|
| archive bytes, the `.fantome` on disk | 69.7 GB | 8.7 MB | 440 MB |
| raw bytes, the files inflated and unpacked | 235.1 GB | 28.0 MB | 948 MB |
| files | 1,491,566 | 178 | 6,550 |

The raw bytes are 3.4 times the archive's, since most mods are deflated zips of zstd WADs. A
file's content is already identified: every file row carries a SHA-256 of its bytes, computed
for dating and the clip keys. Kept once per distinct content, the raw bytes are 206.0 GB
within each mod alone and 130.4 GB across the datasets: 417,503 distinct contents, 45 % of the
raw bytes shared between mods. 246 mods, in 119 groups, name the same content set as another
mod: one mod repacked. The reduced corpus shows none of this, since it is chosen to cover
cases, not copies.

The options were to keep holding everything in memory and rerun; to write a project file per
mod with the bytes under the mod's names, which the rule forbids with reason; or to keep bytes
by their content id, under the program's keys only.

## Decision

A mod's bytes are kept in a blob store, keyed by content id, behind a trait (the user's call,
2026-10-10). Nothing above the store holds bytes: a run, a snapshot (#ADR-0094), a kept run
and a project (#ADR-0096) name bytes by content id only. A path is a key in a map of the
run's, never a location in a store or on disk.

- The content id is the SHA-256 the file rows carry. The store answers a get by id with the
  bytes or nothing, and a put with the id.
- Two stores: one in memory, which the command, the tests and the corpus use, and one on disk,
  which the app owns in its data folder. The library ships the trait and the memory store; the
  app crate ships the disk store.
- A store may be layered: a store that answers from its own blobs and falls through to the one
  under it, by the same id. A run writes into a memory layer of its own over the app's store,
  and the layer's blobs reach the store under it when the stage's snapshot is written
  (#ADR-0094), so a stage that ends early leaves nothing in the store (#ADR-0097); a project
  opens as a layer over the store without being imported; the corpus cache (#ADR-0095) is a
  read-only layer under the runs.
- The store is invisible in the output. A run over the memory store and over the disk store
  gives one report and one package, byte for byte. The corpus comparison with a baseline
  (#ADR-0044) is the test.
- The rule in `AGENTS.md` is restated: no archive is extracted to disk under its own names;
  bytes are written only into the blob store, under their content id.

The engine of the disk store is chosen by measurement once the trait exists, not here. SQLite
is the first candidate, since `rusqlite` is in the workspace and the history index keeps blob
bytes in it (#ADR-0078); its one writer is the point to measure against the corpus populating
a cache at 24 in parallel, where an LSM store would do better.

## Consequences

- The bytes of a stage's output are written to the store once per distinct content. A fixup
  that rewrites a bin adds one blob; the files it does not touch add none.
- A big mod becomes a disk cost, not a memory one. The caps on what is inflated (#ADR-0051)
  stay, since they bound what one run allocates.
- A mod's text still reaches a person through `clean` only. A store holds bytes, and no viewer
  of them hands a mod's bytes to the webview as anything but text or a decoded image.
- Two archives whose raw snapshots name the same content ids are one mod repacked: 246 of the
  datasets' 4,909.
- What is swept, and when, is #ADR-0096's.
