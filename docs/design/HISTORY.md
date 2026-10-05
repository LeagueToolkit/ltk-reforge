# History

The install is one build. A mod is made against another, often a year older, and carries game
files that build had and this one does not. Against the install's bytes those files look like
the author's own. A run asks the build history: has the game ever shipped these bytes, this
object, this clip, and when.

## The source

The source is the census history in format 2 (#ADR-0035): one commit per live build on its
`history-v2` branch, from 8.20, and one file per WAD entry. That branch is format 2 with two
additions (#ADR-0042): the joints an animation clip animates (`clip.joints`), and an events
bank's header and objects (`bank.header`, `bank.objects`). The branch has them from its first
build. The reader takes them as optional keys.

| file | holds |
|---|---|
| `census.yaml` | the format number; the walk stops at a commit of another format |
| `build.yaml` | version, patch, manifest, date, realms |
| `files/<manifest path>/_wad.yaml` | the WAD's version, file id and tags (not read) |
| `files/<manifest path>/xx/<path hash>.yaml` | the entry's SHA-256 and kind, and what its bytes say: a bin's objects (class, object key, norm key), a skeleton (joints and parents), a mesh (submeshes), a texture (format, size, mips, the top mip's hash), an animation clip (the hash of each joint it animates, in stored order), a bank's clips (id, size, key), and an events bank's events, its header (the body of its `BKHD` section, as hex) and its objects (each object of its `HIRC` section in stored order: type, id, and the bytes after the id as hex) |
| `files/<manifest path>/xx/<path hash>.bin/yy/<entry>.rito` | one bin entry as ritobin text |

Builds land in the order they arrive. A hotfix of a patch that ships after the next patch's
first build is a commit after it. A build's **position** is its number in that order. The
**head** is the build appended last. The stamp's newest build is the one of the highest
version, which is not always the head.

A version is its numbers between dots, `16.19.8207193`. Two builds of the history are a second
manifest of a version another build has, and the census writes them `14.8.5756478__2`. Each is a
build of its own at its own position, and it orders with the version it repeats.

`reforge-census` reads the history through gix. It reads no database of the census.

## The index

`reforge history index --repo <clone> [--until <commit or build>]`:

1. Walks the `history-v2` branch's first-parent chain oldest first, up to `--until`.
2. Diffs each commit's `files/` tree against its parent by object id. It descends only into
   subtrees that changed, and into a bin's entry texts only for the objects family.
3. Reads and parses each changed file before and after on every core, and applies the
   differences in record counts to one **life** per key. A life is the key's first position
   and, once its count falls to zero, its last. A key that comes back keeps the first position
   of its first life.

Entry texts are most of the bytes a build changes. What a text names is a function of its blob,
so the walk reads and parses a text once, when the blob first appears, and keeps the result
while the tree holds the blob.

An entry's text gives what its object names: every chunk link and every string shaped like a
path, the tags of a `tagEventList`, and each bank unit with its bank paths, its events (by
Wwise id) and whether it is voice. A text that does not parse names nothing, and the stamp
counts it.

The index is three files:

| file | holds |
|---|---|
| `history.sqlite` | a table per kind of key (below) |
| `builds.tsv` | position, commit, version, manifest, date |
| `stamp.json` | format, repository, head commit, newest build, build count, families, row count per table, unparsed texts, the writer |

| table | key | facts |
|---|---|---|
| `files` | file key | kind, the first path the content was seen under |
| `paths` | path hash | the first file's kind |
| `contents` | path hash, file key | none: the game held this content at this path |
| `objects` | object key | entry, class, the first bin's path hash, norm key |
| `refs` | path hash, bin path hash, entry | none: that entry of that bin named the path |
| `tags` | bin, entry, tag | none: the object held that audio tag |
| `units` | bin, entry, unit, voice | none: the object held that bank unit |
| `unit_banks` | bin, entry, unit, bank path hash | none: the unit loaded that bank |
| `unit_events` | bin, entry, unit, event id | none: the unit listed that event |
| `clips` | clip key | none |
| `events` | bank path hash, event id | none: an events bank defined that event |
| `event_wems` | bank path hash, event id, clip id | none: the event played that clip |
| `bank_objects` | record | type, id, body: one object of an events bank's hierarchy |
| `event_banks` | bank path hash, record | version, bank id, header, and its objects in order, each as its record in `bank_objects` |
| `skeletons` | path hash, record | influences, and its joints in `joints` (name and parent) |
| `meshes` | path hash, record | vertex type, vertex and index counts, and its submeshes in `submeshes` |
| `joint_lists` | record | joint count, and its joints in `list_joints` (joint hash) |
| `animations` | path hash, file key | the record of the clip's joint list |
| `textures` | path hash, record | format, width, height, mips, the top mip's hash |
| `names` | id | each joint, material, format, vertex-type, tag and unit name once |

Every table of lives also has `first`, `last` and `alive`. `alive` is how many records hold the
key at the head, none once it is gone; an append starts from it. A `record` is the head of a
SHA-256 over the record's fields, by which an append finds the row of a rig, a texture or an
events bank at a path.

A joint list and a bank object are found by their record alone and held once. The clips of a
skin animate the same joints. A bank changes few objects from one build to the next, and the
voice banks of two languages share most. `event_banks.objects` is one value, eight bytes for
each object in stored order: the object's record, little endian. A bank's record is over its
header and those records.

A table is defined once, in `reforge::history::tables`: its name, family, key columns, fact
columns and lookup indexes. The schema text, the writer's statements and the list of tables in
the stamp come from that definition. The reader's queries are checked against it by a test.

`--families` limits the index to some of: files (with `paths` and `contents`), objects (with
`refs`, the tags and the units), clips (with the events, the events banks and their objects),
rigs (with the animation clips and their joint lists), textures. Every table exists in every
index; one of a family not built is empty.

The index is built beside its place and renamed into it.

## Keys

#ADR-0037. Path hashes, clip keys, norm keys and records are SQLite integers, the 64 bits as
a signed integer. File keys and object keys are 16-byte blobs. A lookup is one indexed query,
and one more for each joint list and each object of an events bank. Nothing is loaded whole,
and a script reads the same file with `sqlite3`.

## Rules

- **Exact.** A key is compared whole. There is no filter that could call an own file the
  game's.
- **Stamped.**
  - `history.index-stale` warns when the repository's head is not the index's.
  - `history.install-newer` warns when the install is newer than the newest build, because
    bytes shipped since then pass as the mod's own.
  - `history.absent` notes a run with no index, and `history.index-missing` an index that
    does not open. Both runs are judged against the install alone.
- **Disposable.** Deleting the index loses nothing: it is rebuilt from the history. An index
  behind the clone is appended to or rebuilt. One whose builds the history does not have is
  rebuilt. A change of the index's format is a rebuild too (#ADR-0036).
- **Format 0** (#ADR-0005). An index of another format is refused by its stamp before anything
  else of it is read.

## Appending

`reforge history index --repo <clone> --append [--until <commit or build>]` adds the builds
after the index's head. It walks each new commit's diff alone and applies each key's net record
count to its row:

- a key new to the index gets a row from that build;
- a count that falls to zero ends the row at the build before;
- a key that comes back keeps its row's first position.

Every build the index holds must be the history's at the same position, or the append stops and
asks for a rebuild. The database is copied beside the index, appended to, and renamed into
place. An index appended to holds the rows a whole build of the same commits writes.

On Windows the rename fails while a process holds the index open. `Indexer::append_to` writes
the appended index into another folder and leaves the index as it is, which is how the app
takes forward the index its runs hold open.

## Fetching

#ADR-0057. `Fetcher` (`reforge-census`, the feature `fetch`) fetches the census history into a
bare clone as far as an append after an index's head reads it:

1. the tip of the builds' branch on the server;
2. every commit of the branch, without its tree;
3. of each build after the head, the commit's tree, its `census.yaml` and `build.yaml`, and the
   trees under `files/` that its diff descends into, on both sides, a level of the tree at a
   time for every build at once;
4. both sides of each entry file and entry text the walk reads.

A request names its objects by id, at most 10,000 of them, with the filter `tree:0`: the server
sends what is named and nothing it reaches. The trees fetched are decided by the walk's own
comparison of two trees, so the clone holds every object the append reads. It holds every
commit, so an append from it checks each build of the index as one from a full clone does. A
whole build from it stops at the first object it lacks.

A clone can instead start whole (#ADR-0060): `Seed` makes a bare clone, the caller writes whole
packs into it, and the builds' branch is set where `Fetcher` keeps it, at a commit the packs
hold. A fetch into it then asks only for the commits after that one and for what the append
reads that the packs lack.

## The questions

`History` is a trait with one method per question. `SqliteHistory` reads the index,
`NoHistory` answers nothing, and `reforge-testkit` has `MemoryHistory`.

| question | method | table |
|---|---|---|
| were these bytes ever a game file, and when? | `file` | `files` |
| did the game ever hold a file at this path? | `path` | `paths` |
| did the game ship these bytes at this path? | `held` | `contents` |
| where else did the game hold these bytes? | `paths_of` | `contents`, by file key |
| is this object a game object, under which entry, in which bin? | `object` | `objects` |
| is this object a game object under another entry name? | `objects_by_norm` | `objects`, by norm key |
| was this clip ever the game's? | `clip` | `clips` |
| which game objects named this path, and until when? | `named_by` | `refs` |
| which game units loaded this bank? | `units_loading` | `unit_banks` |
| which clips did this event play? | `event_wems` | `event_wems` |
| which events played this clip? | `events_playing` | `event_wems` |
| what skeleton and mesh did the game hold at this path, per build? | `skeletons`, `meshes` | `skeletons`, `meshes` |
| which joints does the animation clip at this path animate, per build? | `clip_joints` | `animations`, `joint_lists` |
| which joints does the animation clip of these bytes animate? | `joints_of` | `animations`, by file key |
| what header and objects did the events bank at this path hold, per build? | `events_banks` | `event_banks`, `bank_objects` |
| which version is the build at this position? | `version` | the builds |
| which is the first build of this patch? | `first_of_patch` | the builds |
| does this life reach the head? | `reaches_head` | the builds |

An object found by its norm key is a copy only when its class is the same: the key leaves the
class out.

An events bank is answered as `reforge-wwise` holds one: the header's bytes and the objects, each
with its type, its id and its body. `Bank::new_bnk` takes the header and the objects go in
`Bank::objects`, so the bank is cut (`Bank::cut_to`) and written as one read from an install.
A bank the bank reader refuses (a header too short for a version and a bank id, an object too
short for an id) is not in the index.

`GitRepo::entry_text` reads what an object looked like at a build: the bin entry's text at a
commit, from the history itself.

## The commands

| command | does |
|---|---|
| `reforge history index` | builds the index, or appends to it |
| `reforge history status` | prints the stamp, and raises the stamp's diagnostics against a repository and an install |
| `reforge history probe <path> --game <install>` | hashes a game file and looks it up, with a bin's objects, a bank's clips, an events bank's header and objects, or an animation clip's joints; `--strict` fails when one is missing |
| `reforge history show-entry <entry> --bin <path> --repo <clone>` | prints an entry's text at a build |

## Limits

- The history covers live builds only. Bytes that existed only on the test realm pass as the
  mod's own.
- It carries no file bytes. What a file held at a build is known only by what the census
  recorded of it.
- Of an events bank it holds the header and the objects. A bank that holds clips or another
  section beside them is not rebuilt whole from the history.
- A life is a hull. Two events banks, or two clips, whose lives at one path overlap are told
  apart by which of them reaches the head, and not at a build between.
- It starts at 8.20. A file the game dropped before then passes as the mod's own.
