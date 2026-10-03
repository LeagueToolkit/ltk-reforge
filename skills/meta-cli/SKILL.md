---
name: meta-cli
description: Look up League of Legends bin/meta facts with the rito-meta CLI (npm @leaguetoolkit/meta-cli, backed by meta-api.leaguetoolkit.dev) - a property's type at a patch or build, hash to name and name to hash resolution, a class's fields and inheritance, its subclasses, every property whose type changed between two patches, and the wiki's written docs. Use this whenever a question or task involves LoL bin property types, meta classes (names like VfxEmitterDefinitionData or SkinCharacterDataProperties), 0x-prefixed hashes, "what type is X", "which patch added/changed/removed Y", or code that reads or writes .bin files (ltk, ltk-manager, ritobin, bin parsers and generators) and needs a field's type or hash - even when the user never mentions the CLI, the API or the wiki. Do not guess types or hashes from memory when this skill applies; the dataset changes every patch.
---

# Meta lookups with rito-meta

`rito-meta` answers questions about the *structure* of League of Legends bin data:
which classes exist, what properties they have, what type each property is, and
how that changed across patches. It does not know game values (damage numbers,
skin prices); for those look elsewhere.

Run it with npx, no install needed (Node 22+):

```
npm install -g @leaguetoolkit/meta-cli   # rito-meta on your PATH
npx -y @leaguetoolkit/meta-cli@1 <command> [args] --json # or run it without installing
```

Assume `rito-meta` is already on PATH, that works too. Pass `--json` even though a
pipe already gets JSON: it makes the intent obvious in the transcript.

## Pick the command from the question

| The user wants to know                                   | Run                                                  |
|----------------------------------------------------------|------------------------------------------------------|
| what type a property is (now, or at a patch/build)       | `property <Class>.<field> [--at 16.17]`              |
| whether/when a property's type changed                   | `property <Class>.<field>` and read `history`        |
| a class: hash, bases, lifetime, its own fields           | `class <name-or-hash>`                               |
| every field a class carries, including inherited ones    | `class <name-or-hash> --inherited`                   |
| which classes derive from one                            | `class <name-or-hash> --tree`                        |
| the name for a hash, or the hash for a name, plus URL    | `hash <0x... or Name>`                               |
| a class name they half-remember                          | `search <substring-or-glob>` (case-insensitive)      |
| every property whose type changed between two points    | `diff <from> <to>` (patches or builds, from < to)    |
| what a patch added, removed, or changed                  | `changelog 16.17` (or `changelog` for the index)     |
| which patches and builds are covered at all              | `versions`                                           |
| the human-written explanation of a class or field        | `docs <name-or-hash>`                                |
| anything else the API serves                             | `raw /v1/<path>` (verbatim response)                 |

Names and hashes are interchangeable everywhere a class or property is named:
`property 0x9a4b299d.0x0329f1d7` works. Patches (`16.17`) and build numbers
(`8104348`) are interchangeable wherever a point in time is named.

## Read the answer, not the exit code

Every answer is one JSON object with `outcome`, `api`, `generation` (when the
dataset was captured) and the command's fields. Exit codes 2 to 5 are answers,
not failures, and the JSON still prints:

| exit | outcome           | meaning                                                   |
|------|-------------------|-----------------------------------------------------------|
| 0    | `ok`              | answered                                                  |
| 2    | `no-such-class`   | nothing by that name or hash; try `search`                |
| 3    | `no-such-property`| class exists, property does not; check `didYouMean` (names are case-sensitive) |
| 4    | `not-at-build`    | it exists, just not at `--at`; the payload carries `since`, `removedIn` and `history` |
| 5    | `outside-window`  | the build or patch is before tracking began (13.15) or after the newest build |
| 1    | error             | network, bad arguments, unreadable `--db`; message on stderr |

So a non-zero exit is a finding to report, not a reason to retry or to fall
back to guessing.

## How types are written

Every type in every payload has the same shape:

```
{ "name": "Map", "tag": 134, "key": {...}, "value": {...} }
```

`tag` is the bin format's own type tag number (see `references/commands.md`
for the table). Optional parts: `key` and `value` for containers, `keyHash` for
the class a Pointer/Embed/Link points at (`{hash, name, kind}`, name is null
when the class is unnamed), `size` for fixed-size lists. Render them the way
bin tooling writes them:

- `{name: "Pointer", keyHash: {name: "VfxLegacyPrimitiveBase"}}` -> `Pointer<VfxLegacyPrimitiveBase>`
- `{name: "Option", value: {name: "F32"}}` -> `Option<F32>`
- `{name: "Map", key: {name: "Hash"}, value: {name: "Embed", keyHash: {...}}}` -> `Map<Hash, Embed<GameModeConstants>>`
- `{name: "List", value: {name: "F32"}, size: 7}` -> `List<F32>[7]`

## Dates are patch-granular

The API records history per patch, so `since`, `until`, `removedIn` and
`history` entries are patches. A build inside a patch answers as that patch;
an unobserved build floors to the nearest earlier observed one and the payload
says so in `at.requestedBuild`. When the user asks about a specific build,
report both the patch and the build from `at`, and mention the floor if it
happened.

## Workflow that avoids wrong answers

1. **Unsure of the exact name?** `search` first. Names are case-sensitive and
   many classes are only known by hash, which `search` also matches.
2. **Listing a class's fields for a parser or generator?** Use `--inherited`.
   The plain `class` view lists only the fields declared on that class, and a
   generator that misses base-class fields produces broken files. Each
   inherited field carries `from` naming the class that declares it.
3. **"Did this change?"** Read `history` on `property`. It only appears when
   the type changed at least once; a single-revision property has just
   `since`/`until`.
4. **"What changed between A and B?"** `diff A B`. It walks every patch in the
   span and returns `changes[]` with `oldType`, `newType`, `build`, `patch`.
   Wide spans return hundreds of rows; summarize by kind of change (for
   example "String -> File on 300 path-like fields") rather than listing them.
5. **Reproducing `docs` text?** It is CC BY-SA 4.0; say where it came from.

## Reporting back

Answer the question that was asked, tersely. The user is usually mid-task in
an editor and wants the fact, not a briefing. One or two lines for a type or
hash question; a compact table for a field list; a grouped summary (by
old -> new type, with counts) for a wide diff. Leave out what was not asked:
why Riot restructured something, what an unnamed class "probably" is, which
tag numbers exist, how to reproduce the query. If the user wants the story,
they will ask, and the history is one command away.

A type question, answered:

> `VfxEmitterDefinitionData.primitive` (`0x007b14f6`) is `Pointer<VfxLegacyPrimitiveBase>`
> since 16.6. Before that, 13.15 to 16.5, it was `Pointer<VfxPrimitiveBase>`.
> As of 16.17 (build 8104348).

That last line is the provenance and it is enough: the patch or build the
answer is dated at, plus the dataset generation date only when the user is
likely on a newer game build than the dataset. Put hashes beside names,
because bin files carry hashes and unnamed classes have nothing else. Link the
wiki page (`https://meta-wiki.leaguetoolkit.dev/classes/<lowercase-name>`, or
the URL from `hash`) only when there is prose worth reading there. Never paste
raw JSON; for a long result, save the JSON beside your answer and say so.

## Pitfalls

- Windows Git Bash rewrites `/v1/...` into a Windows path before the CLI sees
  it. For `raw`, prefix `MSYS_NO_PATHCONV=1` or use `//v1/...`.
- `--at` is accepted only by `property` and `class`; `--inherited` and
  `--tree` only by `class`.
- `changelog 13.15` has no changelog: tracking starts there.
- `docs` needs the network even with `--db`; prose is not in the database.
- Responses are cached on disk with ETag validation, so repeated lookups are
  cheap. `--no-cache` forces a fresh fetch.
- Offline or pinned runs: `db fetch <path>` once, then `--db <path>` on any
  command except `docs` and `raw`. `db check <path>` says whether the pin is
  stale (exit 5).

Field-by-field payload shapes for every command are in
`references/commands.md`; read it when you need a field you have not seen in
an answer yet.
