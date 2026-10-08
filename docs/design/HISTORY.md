# History

The install is one build. A mod is made against another, often a year older, and carries game
files that build had and this one does not. Against the install's bytes those files look like
the author's own. A run asks the build history: has the game ever shipped these bytes, this
object, this clip, and when. The porter asks it what the game held at one build: the file at a
path, the bin there and its objects, an object's bytes, what a skin entry reaches by its links,
and the kind of a field.

## The source

The source is the census history in format 2 (#ADR-0035): one commit per live build on its
`history-v2` branch, from 8.20, and one file per WAD entry. That branch is format 2 with two
additions (#ADR-0042): the joints an animation clip animates (`clip.joints`), and an events
bank's header and objects (`bank.header`, `bank.objects`). The branch has them from its first
build. The reader takes them as optional keys.

| file | holds |
|---|---|
| `census.yaml` | the format number; the walk stops at a commit of another format |
| `build.yaml` | version, patch, manifest, source, a `rads` build's solution, release and executable, date, legacy bins, realms |
| `files/<manifest path>/_wad.yaml` | the WAD's version, file id and tags |
| `files/<manifest path>/xx/<path hash>.yaml` | the entry's SHA-256, checksum and kind, a bin's links, and what its bytes say: a bin's objects (class, object key, norm key), a skeleton (joints and parents), a mesh (submeshes), a texture (format, size, mips, the top mip's hash), an animation clip (the hash of each joint it animates, in stored order), a bank's clips (id, size, key), and an events bank's events, its header (the body of its `BKHD` section, as hex) and its objects (each object of its `HIRC` section in stored order: type, id, and the bytes after the id as hex) |
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

#ADR-0077, #ADR-0078. The index is one SQLite file. It names every entry text's blob and holds
the bytes of the text's object, so a run reads an object at any build from the index alone, as
bytes and never as text. The census clone is what an append reads, and nothing else.

`reforge history index --repo <clone> [--until <commit or build>]`:

1. Walks the `history-v2` branch's first-parent chain oldest first, up to `--until`.
2. Diffs each commit's `files/` tree against its parent by object id. It descends only into
   subtrees that changed, and into a bin's entry texts only for the objects family.
3. Reads and parses each changed file before and after on every core, and applies the
   differences in record counts to the **lives** of each key. A life is the key's first
   position and, once its count falls to zero, its last.

A table keeps a key's lives in one of two ways:

- A **table of spans** keeps one row per unbroken life: its primary key is the key's columns
  and `first`, and a key that comes back after a build without it gets a row of its own. The
  rows alive at a build are the build's. `placements`, `wad_lives`, `entry_texts`, `schema` and
  `links` are tables of spans.
- Every other table keeps one row per key, and its life is the hull of the key's lives: a key
  that comes back keeps the first position of its first life.

A key that goes and comes back within one build keeps its life: the build's net count is what
an append sees.

Entry texts are most of the bytes a build changes. What a text names is a function of its blob,
so the walk reads and parses a text once, when the blob first appears, and keeps the result
while the tree holds the blob. The text's object, as `ltk_meta` writes it, is kept from then
until the object bytes are written ("Object bytes").

An entry's text gives what its object names: every chunk link and every string shaped like a
path, every object it links by a `link`, the type of each field of each struct in it, the tags
of a `tagEventList`, and each bank unit with its bank paths, its events (by Wwise id) and whether
it is voice. A text that does not parse names nothing, and the stamp counts it. Its blob is in
`entry_texts` all the same.

The index is three files:

| file | holds |
|---|---|
| `history.sqlite` | a table per kind of key (below) |
| `builds.tsv` | per build: position, commit, version, manifest, date, patch, source, solution, release, exe, legacy bins (0 or 1), and the realms joined by spaces |
| `stamp.json` | format, repository, head commit, newest build, build count, families, the row count of every table (the tables of lives, their parts, `names`, `wads` and the tables of the object bytes), unparsed texts, texts that hold no object or several, how the object bytes were compressed, the writer |

| table | key | facts |
|---|---|---|
| `files` | file key | kind, the first path the content was seen under |
| `placements` | path hash, WAD, file key, checksum | none: the WAD held this content at this path |
| `wad_lives` | WAD, version, file id, tags | none: the WAD's file as its manifest described it |
| `objects` | object key, class | entry, the first bin's path hash, norm key; an id of its own |
| `entry_texts` | entry hash, bin path hash, WAD, blob | none: the WAD's bin held this text of the entry |
| `bins` | file key | the bins it links, and its objects in `bin_entries` (entry hash, the object's id in `objects`) |
| `schema` | class, field, kind, key kind, item kind, value class | none: a value of the field had this type |
| `links` | bin path hash, entry hash, linked entry hash | none: the object held a `link` to that entry |
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
| `names` | id | each joint, material, format, vertex-type, tag, unit, WAD version and WAD tag set once |
| `wads` | id | each WAD's path in the manifest once |
| `object_bytes` | id | each distinct object of an entry text once: the keyframe it was compressed against (itself for a keyframe), a keyframe's dictionary, the zstd frame |
| `text_bytes` | blob | the text's object: its class and its row of `object_bytes` |
| `object_dicts` | id | a dictionary keyframes were compressed with: its class, none for the general one, and its bytes |

Every table of lives also has `first`, `last` and `alive`. `alive` is how many records hold the
key at the head, none once it is gone; an append starts from it. A `record` is the head of a
SHA-256 over the record's fields, by which an append finds the row of a rig, a texture or an
events bank at a path.

A joint list and a bank object are found by their record alone and held once. The clips of a
skin animate the same joints. A bank changes few objects from one build to the next, and the
voice banks of two languages share most. `event_banks.objects` is one value, eight bytes for
each object in stored order: the object's record, little endian. A bank's record is over its
header and those records.

`placements` holds every file a WAD held at a path. A path leads its key, so the files at a path
are one query, and the file key is indexed, so the paths of a content are another. A checksum is
0 in a WAD whose table holds none.

`objects` keys an object by its key and its class: the key covers the object's bytes from its
entry hash on, and a bin stores the class apart, so the same bytes under two classes are two
objects. A row has an id of its own, found by the key and the class through a unique index, and
`bin_entries` stores the id.

`entry_texts` leads its key with the entry hash, so an object's text at a build (by bin and
entry) and the bins that held an entry at a build (by entry) are both reads of its key, with no
index beside the table. An index of a table without row ids repeats the whole key.

`bins` holds a bin by its content. `bins.links` is one value: the path hash of each bin the bin
links, eight bytes each, little endian, in the order the census history lists them. A bin links a
few bins and its links are read whole. Its objects are rows of `bin_entries`, keyed by the bin's
row and the entry hash. Their order is the entry hashes' order, which is the census's, so an
object's position in the bin is its rank. An object there is the id of its row in `objects`,
which holds its key, its class and its norm key: a changed bin repeats every object it holds, so
each byte of an object's row in `bin_entries` is paid by every version of every bin. A whole
build indexes each table once its rows are in and before the next is written, so the writer
finds an object's id by the index.

A type in `schema` is the four slots of `FieldType`: the value's kind, a map's key kind, an
item's kind (a list's, an option's, a map's value), and a class (an embed's or a pointer's, or
of the structs a list, an option or a map holds). A kind slot that does not apply is 255, a class
that does not apply 0. A list, an option or a map of structs has one row per class its items
had. A field whose values had two kinds at one build has two rows alive there.

A table is defined once, in `reforge::history::tables`: its name, family, key columns, fact
columns, shape (keyed, recorded or spans), parts and lookup indexes. The schema text, the
writer's statements and the list of tables in the stamp come from that definition. The reader's
queries are checked against it by a test.

`--families` limits the index to some of: files (with `placements`, `wad_lives` and `wads`),
objects (with `entry_texts`, the object bytes, `bins`, `schema`, `links`, `refs`, the tags and
the units), clips
(with the events, the events banks and their objects), rigs (with the animation clips and their
joint lists), textures. Every table exists in every index; one of a family not built is empty.

The index is built beside its place and renamed into it.

## Object bytes

#ADR-0078. An entry text's object is stored as `ltk_meta` writes it from its entry hash on, with
no size before it: the bytes its object key digests. A bin stores an object's class apart, and
so does `text_bytes`.

- **Once.** Each distinct object is one row of `object_bytes`. Two texts whose objects write the
  same bytes under two classes share the row, and `text_bytes` holds each text's class.
- **Versions.** An entry's versions are its distinct objects, ordered by the build each first
  appeared at, then by the SHA-256 of its bytes. Every eighth from the first is a keyframe, and
  each other version is in the block of the keyframe before it.
- **Frames.** A keyframe is a zstd frame compressed with its class's dictionary, or the general
  one when its class has none, or with none. Every other version is a frame compressed with its
  keyframe's bytes as raw content. A keyframe's own keyframe is itself, so a read decodes two
  frames at most.
- **Dictionaries.** A whole build trains them once every text is in. A class whose keyframes
  hold 1.6 MB or more gets one of its own, and the other classes share the general one. A
  dictionary is a hundredth of its keyframes' bytes, between 16 KiB and the cap, trained on
  samples taken at even steps through the keyframes, a hundred times its size. A group zstd does
  not train on has none. An append uses the dictionaries as they are, and a class with none of
  its own uses the general one.
- **Parameters.** The stamp records the zstd level (19), the keyframe interval (8), the
  dictionary cap (1 MiB) and the zstd library's version: what makes the frames again. Another
  version of zstd may write other frames of the same bytes.

The walk keeps each new text's object packed at zstd's fastest level until every text is in. A
whole build then orders the versions, trains the dictionaries and compresses on every core. An
append writes each text a build adds that the index lacks: to the row of the entry that holds
the same bytes, or as a version against the entry's current keyframe, or as a keyframe once the
keyframe's block holds eight. It reads the entry's rows through `entry_texts`, so the count comes
from the tables. Both give the same order and the same blocks, so an appended index holds the
bytes a whole build of the same commits holds. Their frames differ where their dictionaries do.

A text that does not parse, or holds no object or several, has no row of `text_bytes`. The stamp
counts the texts that hold no object or several apart from the unparsed ones.

`reforge history check-objects` reads every text's object and looks the digest of its bytes up
in `objects` under the text's class. An object first held at a build whose bins number property
kinds the old way (`legacy_bins`, the builds before 10.8), and that holds a kind this numbering
writes as another byte (`WadChunkLink` or a kind after it, of a value, an item or a map's key),
has no key there: the census digested the game's bytes, and `ltk_meta` writes the current
numbering only. The check names each such text with its bin, its entry and its first build.

## Keys

#ADR-0037. Path hashes, entry hashes, clip keys, norm keys and records are SQLite integers, the
64 bits as a signed integer. File keys and object keys are 16-byte blobs, and a text's blob is
its 20-byte git object id. A WAD is the id of its row in `wads`, as a name is in `names`, and an
object in `bin_entries` the id of its row in `objects`. A lookup is one indexed query, and one more for each joint list and each object of an events bank.
Nothing is loaded whole, and a script reads the same file with `sqlite3`.

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
- **Format 1** (#ADR-0005, #ADR-0077). An index of another format, format 0 among them, is
  refused by its stamp before anything else of it is read, and is rebuilt.
- **Untrusted.** The history is input like a mod: a value that would break a line of
  `builds.tsv` (a tab, a line break) is written as a space.

## Appending

`reforge history index --repo <clone> --append [--until <commit or build>]` adds the builds
after the index's head. It walks each new commit's diff alone and applies each key's net record
count to its row:

- a key new to the index gets a row from that build;
- a count that falls to zero ends the row at the build before;
- a key that comes back keeps its row's first position; of a table of spans, it gets a row from
  that build, and the row that ended stays as it is.

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
4. both sides of each entry file, entry text and `_wad.yaml` the walk reads.

A request names its objects by id, at most 10,000 of them, with the filter `tree:0`: the server
sends what is named and nothing it reaches. The trees fetched are decided by the walk's own
comparison of two trees, so the clone holds every object the append reads. It holds every
commit, so an append from it checks each build of the index as one from a full clone does. A
whole build from it stops at the first object it lacks.

This clone is the only one a user's machine needs (#ADR-0078): a run reads every object from the
index. `Seed` makes a bare clone from whole packs, as the app's bootstrap from a release's
archive (#ADR-0060, superseded) does, with the builds' branch set where `Fetcher` keeps it, at a
commit the packs hold. A fetch into it then asks only for the commits after that one and for
what the append reads that the packs lack.

## Distribution

#ADR-0047, "fetches the one made for it". The census history's repository has one GitHub release
for each head an index is published at, `history-v2-snapshot-<n>`. A release holds the three
files of an index once for each format published. A file takes its name in the index's
directory with `-v<format>` before its extension, and the database is compressed with zstd at
`--long=31`:

| file | in a release |
|---|---|
| `stamp.json` | `stamp-v<format>.json` |
| `builds.tsv` | `builds-v<format>.tsv` |
| `history.sqlite` | `history-v<format>.sqlite.zst` |

The files with no format in their names, `stamp.json`, `builds.tsv` and `history.sqlite.zst`,
are format 0's. `reforge_app::index::ReleaseNames` builds the names from `FORMAT`. The app takes
the newest release that holds the three files of its own format, each with the SHA-256 GitHub
lists for it. A release that holds other formats only holds no index for the app, which reads
none of their files.

The app installs an index in its data folder, in `history-indexes/<tag>-v<format>`, and an index
it takes forward in `history-indexes/head-<commit>-v<format>`. Two versions of the app that read
two formats write no folder in common. In the folder the files take their names in the index's
directory.

## The questions

`History` is a trait with one method per question. `SqliteHistory` reads the index,
`NoHistory` answers nothing, and `reforge-testkit` has `MemoryHistory`.

| question | method | table |
|---|---|---|
| were these bytes ever a game file, and when? | `file` | `files` |
| did the game ever hold a file at this path? | `path` | `placements`, the hull of the path's lives |
| did the game ship these bytes at this path? | `held` | `placements`, the hull of the lives |
| where else did the game hold these bytes? | `paths_of` | `placements`, by file key |
| which files did the game's WADs hold at this path at build B? | `placed` | `placements` |
| which bin was at this path at B, with its links and its objects? | `bin` | `placements`, `bins`, `bin_entries`, `objects` |
| is this object a game object, under which entry, in which bin? | `object` | `objects`, of two classes the life that reaches furthest |
| every life of this object, one per class? | `object_lives` | `objects` |
| is this object a game object under another entry name? | `objects_by_norm` | `objects`, by norm key |
| which blob holds this object's text at B? | `text_blob` | `entry_texts` |
| what were this object's bytes at B? | `object_bytes` | `entry_texts`, `text_bytes`, `object_bytes`, `object_dicts` |
| which objects does this object link at B? | `links` | `links` |
| which objects does this object reach by its links at B? | `reached` | `links` and `entry_texts`, one recursive query |
| which types did this field of this class have at B? | `field_types` | `schema` |
| was this clip ever the game's? | `clip` | `clips` |
| which game objects named this path, and until when? | `named_by` | `refs` |
| which game units loaded this bank? | `units_loading` | `unit_banks` |
| which clips did this event play? | `event_wems` | `event_wems` |
| which events played this clip? | `events_playing` | `event_wems` |
| what skeleton and mesh did the game hold at this path, per build? | `skeletons`, `meshes` | `skeletons`, `meshes` |
| which joints does the animation clip at this path animate, per build? | `clip_joints` | `animations`, `joint_lists` |
| which joints does the animation clip of these bytes animate? | `joints_of` | `animations`, by file key |
| what header and objects did the events bank at this path hold, per build? | `events_banks` | `event_banks`, `bank_objects` |
| which version is the build at this position, and what else did `build.yaml` say? | `version`, `build` | the builds |
| which is the first build of this patch? | `first_of_patch` | the builds |
| does this life reach the head? | `reaches_head` | the builds |

An object found by its norm key is a copy only when its class is the same: the key leaves the
class out.

Dating a mod by its objects is a batch of `object_lives`: the builds from the latest `first` to
the earliest `last`.

Where two WADs held a path at one build, the first in `wad_order` answers `bin`, `text_blob` and
`object_bytes`: the WADs' paths compared directory by directory, which is the order of the
census history's tree.

`reached` follows each `link` an object held at B into every bin that held the linked entry at
B, and on from there, and answers each object reached once, the object asked about among them.
It stops at `MAX_REACHED` objects (#ADR-0070) and says so.

An events bank is answered as `reforge-wwise` holds one: the header's bytes and the objects, each
with its type, its id and its body. `Bank::new_bnk` takes the header and the objects go in
`Bank::objects`, so the bank is cut (`Bank::cut_to`) and written as one read from an install.
A bank the bank reader refuses (a header too short for a version and a bank id, an object too
short for an id) is not in the index.

`object_bytes` is what a run reads of an object at a build: its bytes from the index, decoded from
two frames at most, with its class and its WAD. A run links no ritobin parser (#ADR-0005).
`GitRepo::entry_text_at`, in `reforge-census`, reads the object's text instead, by the blob
`text_blob` names, one object of a clone that holds it: what the writer reads, and the CLI's
`show-entry`. An index without the objects family names no blob, and `GitRepo::entry_text` finds
the text by a search of the build's tree, from the first WAD in path order whose bin holds it.

## The commands

| command | does |
|---|---|
| `reforge history index` | builds the index, or appends to it |
| `reforge history status` | prints the stamp, and raises the stamp's diagnostics against a repository and an install |
| `reforge history probe <path> --game <install>` | hashes a game file and looks it up, with a bin's objects, a bank's clips, an events bank's header and objects, or an animation clip's joints; `--strict` fails when one is missing |
| `reforge history show-entry <entry> --bin <path> --repo <clone> [--index <index>]` | prints an entry's text at a build, by the blob the index names when one is given |
| `reforge history check-objects [--index <index>] [--unkeyed <file>]` | counts the texts that have their object, checks each object's digest against `objects` under its text's class, and writes each text that has no key there, with its bin, entry and first build |

## Limits

- The history covers live builds only. Bytes that existed only on the test realm pass as the
  mod's own.
- It carries no file bytes but its entry texts' objects. What another file held at a build is
  known only by what the census recorded of it.
- An entry text that does not parse, or holds no object or several, has no object bytes, and a
  run has no other source for it. A clone that `Fetcher` filled for an append holds the texts of
  the builds after the index's head, and not those before.
- An object first held at a build of legacy bins is stored in the current numbering of property
  kinds. One that holds a kind this numbering writes as another byte is not the game's bytes of
  that build, and its digest is not the object key the census recorded.
- Of an events bank it holds the header and the objects. A bank that holds clips or another
  section beside them is not rebuilt whole from the history.
- A life of a table that is not a table of spans is a hull. Two events banks, or two clips,
  whose lives at one path overlap are told apart by which of them reaches the head, and not at
  a build between.
- It starts at 8.20. A file the game dropped before then passes as the mod's own.
