# rito-meta command reference

Every payload starts with `outcome`, `api` (`"v1"`), `generation` (ISO date
the dataset was captured) and `source` (API URL, or the `--db` path). The
fields below are what each command adds.

## Type shape

```
{ name, tag, key?, value?, keyHash?, size? }
```

- `key`/`value`: nested type shapes for `Map` (both) and `List`/`Option`
  (`value` only).
- `keyHash`: `{ hash, name | null, kind: "class" | "external" | "unknown" }`
  for `Pointer`, `Embed`, `Link`.
- `size`: fixed length hint for `List` when the format carries one.

Tag numbers:

| tag | name  | tag | name   | tag | name    |
|-----|-------|-----|--------|-----|---------|
| 0   | None  | 10  | F32    | 128 | List    |
| 1   | Bool  | 11  | Vec2   | 129 | List2   |
| 2   | I8    | 12  | Vec3   | 130 | Pointer |
| 3   | U8    | 13  | Vec4   | 131 | Embed   |
| 4   | I16   | 14  | Mtx44  | 132 | Link    |
| 5   | U16   | 15  | Color  | 133 | Option  |
| 6   | I32   | 16  | String | 134 | Map     |
| 7   | U32   | 17  | Hash   | 135 | Flag    |
| 8   | I64   | 18  | File   |     |         |
| 9   | U64   |     |        |     |         |

## property <Class>.<field> [--at]

```
class: { name, hash }
property: { name, hash }
at: { build, patch, requestedBuild? }
type: <type shape>
since: "16.6" | null        null = present since tracking began (13.15)
until: "16.5" | null        null = still present
defaultValue?: string
history?: [ { since, until, type, defaultValue? } ]   only when the type changed
definedOn?: { name, hash }  when the field is inherited from a base class
```

Outcome `not-at-build` (exit 4) carries `class`, `property`, `at`, `since`,
`removedIn`, `history`. Outcome `no-such-property` (exit 3) carries
`didYouMean: string[]` with case-insensitive matches.

## class <nameOrHash> [--inherited] [--tree] [--at]

```
class: {
  name, hash, interface, value, bases: string[],
  since, removedIn, ancestorLevels: string[][],
  flattened?: true,               with --inherited
  properties: [ { name, hash, type, since?, removedIn?, defaultValue?, from? } ],
  descendants?: tree              with --tree
}
at?: { build, patch }             with --at; properties absent at that point are dropped
```

`from` names the declaring class when `--inherited` is on. Unnamed classes
and properties use their hash as `name`.

## hash <hashOrName>

```
query, hash, name | null, kind: "class" | "external" | "unknown", url | null
```

`url` is the wiki page for classes.

## search <pattern>

```
pattern, count, matches: [ { name, hash } ]
```

Substring match, or a glob when the pattern contains `*`/`?`. Case-insensitive;
matches names and hashes.

## diff <from> <to>

```
from: { build, patch }, to: { build, patch }, count,
changes: [ { class, property, build, patch, oldType, newType } ]
```

Sorted by build, then class, then property. One API request per patch in the
span.

## changelog [patch]

Index (no argument):

```
latestPatch, count, patches: [ { patch, slug, builds, counts: { added, readded, removed, changed } } ]
```

One patch:

```
patch, slug, builds, counts, buildGroups: [ { build, entries: [ { kind, name, ... } ] } ]
```

`kind` is `added`, `readded`, `removed` or `changed`; `changed` entries list
property additions, removals and type changes for that class.

## versions

```
first: { build, patch }, latest: { build, patch }, count, patches: [ { patch, builds } ]
```

## docs <nameOrHash> | docs all

```
license: "CC BY-SA 4.0"
docs: { name, class?: entry, properties: { <field>: entry } }
entry = { description?, notes?: string[], examples?: string[] }
```

`docs all` returns `count` and `docs` keyed by class name. Outcome
`not-documented` (exit 3) means the class exists but nobody wrote prose.

## db fetch <path> / db check <path>

`fetch` writes the raw database and reports `{ path, bytes, etag, generation,
latest }`. `check` compares the file's generation with the published one:
outcome `ok` (exit 0) or `stale` (exit 5) with `local` and `published`.

## raw <path>

Prints the API response verbatim, no envelope. Exit 0 on success, 2 on 404.
Useful paths: `/v1` (dataset meta), `/v1/openapi`, `/v1/classes`,
`/v1/hashes`, `/v1/index`.

## Global flags

`--at <build|patch>`, `--db <path>`, `--json`, `--no-color`, `--cache-dir`,
`--no-cache`, `--api <base-url>`. Environment: `RITO_META_API`,
`RITO_META_CACHE_DIR`, `NO_COLOR`.
