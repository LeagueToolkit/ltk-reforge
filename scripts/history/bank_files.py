"""Keys each events bank of a history index by the file that held it (#ADR-0081), in place.

bank_files.py fill  --db FILE --builds FILE --repo CLONE [--jobs N] [--dry-run]
bank_files.py check --db FILE --game GAME --at POSITION [--locale LOCALE]

An index written before #ADR-0081 keys a row of `event_banks` by its path and its record, so the
voice banks every locale's WAD holds at one path are rows that do not say whose they are. `fill`
rewrites the table as the census writes it now: one row per path and file key. Of each file
`placements` holds at a bank's path it reads one entry file from the census history the index
was built from: in the first WAD that held it, at the first build it did. The bank's record is
computed as the census computes it, the head of a SHA-256 over its header and its objects'
records, and must be one of the path's rows. That row is copied under the file's key, with the
builds and the count the file's placements give. Nothing else in the database changes.

`check` reads the table back. Every row is keyed by a file the placements hold at its path. Of
the paths that hold banks of more than one file at the build POSITION, each bank is a locale
WAD's, one per locale. The bank in LOCALE's WAD is the one the install GAME ships there: the
same WAD checksum, the key of its bytes and the record of its header and objects.

Inputs: --db, the index's database; --builds, its builds list, for the commit of each position;
--repo, a clone of the census history that holds the index's head; --game, an install's `Game`
folder. `fill` writes the database in place, then vacuums it, unless --dry-run. Both print
counts and exit non-zero on a fault.
"""

import argparse
import hashlib
import multiprocessing
import re
import sqlite3
import struct
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

import zstandard

MASK = (1 << 64) - 1

# What `reforge::history::tables` creates for the table, word for word.
CREATE = (
    "CREATE TABLE event_banks (id INTEGER PRIMARY KEY, path INTEGER NOT NULL, key BLOB NOT NULL, "
    "record INTEGER NOT NULL, version INTEGER NOT NULL, bank_id INTEGER NOT NULL, header BLOB NOT "
    "NULL, objects BLOB NOT NULL, first INTEGER NOT NULL, last INTEGER NOT NULL, alive INTEGER "
    "NOT NULL)"
)
INDEX = "CREATE UNIQUE INDEX event_banks_key ON event_banks (path, key)"

# The shortest header the bank reader takes: a version and a bank id.
HEADER_MIN = 8


def stored(n):
    """A u64 as SQLite holds it: the 64 bits as a signed integer."""
    return n - (1 << 64) if n >= 1 << 63 else n


class Digester:
    """`reforge::history::tables::Digester`: SHA-256 over little-endian lengths and integers."""

    def __init__(self, tag):
        self.h = hashlib.sha256()
        self.text(tag)

    def text(self, text):
        data = text.encode()
        self.int(len(data))
        self.h.update(data)

    def int(self, n):
        self.h.update(struct.pack("<Q", n))

    def bytes(self, data):
        self.int(len(data))
        self.h.update(data)

    def finish(self):
        return int.from_bytes(self.h.digest()[:8], "big")


def bank_record(header, objects):
    """The record of a bank of this header and these objects (kind, id, body), as stored."""
    records = []
    for kind, oid, body in objects:
        d = Digester("bank-object")
        d.int(kind)
        d.int(oid)
        d.bytes(body)
        records.append(d.finish())
    d = Digester("events-bank")
    d.bytes(header)
    for record in records:
        d.int(record)
    return stored(d.finish())


# The census's entry file, as `reforge_census::format::parse_entry` reads its digest and its
# bank: an indented key and value per line, a list item after a dash.
HEX = re.compile(r"[0-9a-fA-F]*")
HEX_NUMBER = re.compile(r"\+?[0-9a-fA-F]+")
NUMBER = re.compile(r"\+?[0-9]+")


def unquoted(value):
    if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
        return value[1:-1]
    return None


def hex32(value):
    text = unquoted(value)
    if text is None or not HEX_NUMBER.fullmatch(text):
        return None
    n = int(text, 16)
    return n if n <= 0xFFFFFFFF else None


def u8(value):
    if not NUMBER.fullmatch(value):
        return None
    n = int(value)
    return n if n <= 0xFF else None


def hex_bytes(value):
    text = unquoted(value)
    if text is None or len(text) % 2 or not HEX.fullmatch(text):
        return None
    return bytes.fromhex(text)


def line_of(raw):
    body = raw.lstrip(" ")
    if not body:
        return None
    indent = len(raw) - len(body)
    item = body.startswith("- ")
    if item:
        body = body[2:]
    if body.startswith('"'):
        end = body.find('"', 1)
        if end >= 0 and body[end + 1 : end + 2] == ":":
            return indent, item, body[1:end], body[end + 2 :].lstrip()
        return indent, item, "", body
    colon = body.find(":")
    if colon >= 0:
        return indent, item, body[:colon], body[colon + 1 :].lstrip()
    return indent, item, "", body


def read_entry(text):
    """The entry's digest, and its bank's record: None for an entry with no bank the census
    reads."""
    digest, header, objects = None, None, []
    section, listed = "", ""
    for raw in text.split("\n"):
        line = line_of(raw[:-1] if raw.endswith("\r") else raw)
        if line is None:
            continue
        indent, item, key, value = line
        if indent == 0:
            section, listed = key, ""
            if key == "sha256":
                digest = unquoted(value)
            continue
        if section != "bank":
            continue
        if indent == 1:
            if key == "header":
                header = hex_bytes(value)
            elif value in ("", "[]"):
                listed = key
        elif indent in (2, 4) and listed == "objects":
            if item:
                objects.append([None, None, None])
            if objects:
                field = {"id": 0, "type": 1, "body": 2}.get(key)
                if field is not None:
                    objects[-1][field] = (hex32, u8, hex_bytes)[field](value)
    if digest is None or len(digest) != 64 or not HEX.fullmatch(digest):
        return None, None
    whole = [(kind, oid, body) for oid, kind, body in objects]
    if header is None or len(header) < HEADER_MIN or any(None in object for object in whole):
        return digest.lower(), None
    return digest.lower(), bank_record(header, whole)


GIT = None


def start_git(repo):
    global GIT
    GIT = subprocess.Popen(
        ["git", "-C", repo, "cat-file", "--batch"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
    )


def read_task(task):
    """Reads one file's entry: (path, key, the entry's digest, the bank's record)."""
    path, key, commit, wad = task
    u = path & MASK
    spec = f"{commit}:files/{wad}/{u >> 56:02x}/{u:016x}.yaml\n"
    GIT.stdin.write(spec.encode())
    GIT.stdin.flush()
    head = GIT.stdout.readline().split()
    if head[-1] == b"missing":
        return path, key, None, None
    data = GIT.stdout.read(int(head[2]))
    GIT.stdout.read(1)
    digest, record = read_entry(data.decode("utf-8", "replace"))
    return path, key, digest, record


def builds_of(path):
    commits = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            cells = line.rstrip("\n").split("\t")
            commits[int(cells[0])] = cells[1]
    return commits


def columns(db):
    return [row[1] for row in db.execute("PRAGMA table_info(event_banks)")]


def fill(args):
    mode = "ro" if args.dry_run else "rw"
    db = sqlite3.connect(f"file:{args.db}?mode={mode}", uri=True)
    if "key" in columns(db):
        sys.exit("event_banks already keys a bank by its file: nothing to fill")
    commits = builds_of(args.builds)
    (last,) = db.execute("SELECT max(last) FROM placements").fetchone()
    if last is None or last >= len(commits):
        sys.exit(f"the builds list holds {len(commits)} builds, the index reaches {last}")

    rows = {}
    for path, record, first, last, alive in db.execute(
        "SELECT path, record, first, last, alive FROM event_banks"
    ):
        rows[(path, record)] = (first, last, alive)
    paths = sorted({path for path, _ in rows})
    db.execute("CREATE TEMP TABLE bank_paths (path INTEGER PRIMARY KEY)")
    db.executemany("INSERT INTO bank_paths VALUES (?)", [(p,) for p in paths])
    spans = defaultdict(list)
    for path, wad, key, first, last, alive in db.execute(
        "SELECT p.path, w.path, p.key, p.first, p.last, p.alive FROM placements p "
        "JOIN wads w ON w.id = p.wad JOIN temp.bank_paths b ON b.path = p.path"
    ):
        spans[(path, bytes(key))].append((first, wad, last, alive))
    print(f"{len(rows)} rows at {len(paths)} paths, {len(spans)} files at those paths")

    tasks = []
    for (path, key), held in spans.items():
        first, wad, _, _ = min(held)
        tasks.append((path, key, commits[first], wad))
    tasks.sort(key=lambda t: (t[2], t[3], t[0]))
    with multiprocessing.Pool(args.jobs, start_git, (args.repo,)) as pool:
        read = list(pool.imap_unordered(read_task, tasks, chunksize=64))

    faults = []
    of_file = {}
    banked = Counter()
    for path, key, digest, record in read:
        if digest is None or digest[:32] != key.hex():
            faults.append(f"{path & MASK:016x} {key.hex()}: the entry's digest is {digest}")
        elif record is None:
            banked["no bank"] += 1
        elif (path, record) not in rows:
            faults.append(f"{path & MASK:016x} {key.hex()}: record {record} is no row of the path")
        else:
            of_file[(path, key)] = record
            banked["bank"] += 1
    print(f"files read: {dict(banked)}")

    # Each row's builds and count are its files', over every WAD that held one.
    new = []
    by_row = defaultdict(list)
    for (path, key), record in of_file.items():
        held = spans[(path, key)]
        life = (min(s[0] for s in held), max(s[2] for s in held), sum(s[3] for s in held))
        new.append((path & MASK, key, path, record, life))
        by_row[(path, record)].append(life)
    for at, (first, last, alive) in rows.items():
        lives = by_row.get(at)
        if not lives:
            faults.append(f"{at[0] & MASK:016x} record {at[1]}: no file holds the row")
            continue
        hull = (min(l[0] for l in lives), max(l[1] for l in lives), sum(l[2] for l in lives))
        if hull != (first, last, alive):
            faults.append(
                f"{at[0] & MASK:016x} record {at[1]}: its files give {hull}, the row "
                f"{(first, last, alive)}"
            )
    for fault in faults[:20]:
        print("fault:", fault)
    if faults:
        sys.exit(f"{len(faults)} faults: the table is left as it was")
    split = sum(1 for lives in by_row.values() if len(lives) > 1)
    print(f"{len(new)} rows by file; {split} rows whose record several files hold are one each")
    if args.dry_run:
        return

    new.sort(key=lambda row: (row[0], row[1]))
    with db:
        db.execute("ALTER TABLE event_banks RENAME TO event_banks_old")
        db.execute("DROP INDEX event_banks_key")
        db.execute("CREATE INDEX event_banks_old_key ON event_banks_old (path, record)")
        db.execute(CREATE)
        for _, key, path, record, (first, last, alive) in new:
            db.execute(
                "INSERT INTO event_banks (path, key, record, version, bank_id, header, objects, "
                "first, last, alive) SELECT path, ?3, record, version, bank_id, header, objects, "
                "?4, ?5, ?6 FROM event_banks_old WHERE path = ?1 AND record = ?2",
                (path, record, key, first, last, alive),
            )
        db.execute("DROP TABLE event_banks_old")
        db.execute(INDEX)
    db.execute("VACUUM")
    (count,) = db.execute("SELECT count(*) FROM event_banks").fetchone()
    print(f"written: {count} rows")


def wad_toc(path):
    """A WAD's table of contents, by path hash: offset, compressed size, size, type, checksum."""
    with open(path, "rb") as f:
        head = f.read(4)
        if head[:2] != b"RW" or head[2] != 3:
            raise ValueError(f"{path} is no WAD of version 3")
        f.read(256 + 8)
        (count,) = struct.unpack("<I", f.read(4))
        toc = {}
        for _ in range(count):
            at, offset, packed, size, kind, _, _, checksum = struct.unpack(
                "<QIIIBBHQ", f.read(32)
            )
            toc[at] = (offset, packed, size, kind & 0x0F, checksum)
    return toc


ZSTD_MAGIC = b"\x28\xb5\x2f\xfd"


def chunk_bytes(path, entry):
    offset, packed, size, kind, _ = entry
    with open(path, "rb") as f:
        f.seek(offset)
        data = f.read(packed)
    if kind == 0:
        out = data
    elif kind == 3:
        out = zstandard.ZstdDecompressor().decompress(data, max_output_size=size)
    elif kind == 4:
        # Subchunks: zstd frames, and raw runs where a subchunk did not compress.
        out, at = bytearray(), 0
        while at < len(data):
            if data[at : at + 4] == ZSTD_MAGIC:
                frame = zstandard.ZstdDecompressor().decompressobj()
                out += frame.decompress(data[at:])
                at = len(data) - len(frame.unused_data)
            else:
                next_frame = data.find(ZSTD_MAGIC, at)
                end = len(data) if next_frame < 0 else next_frame
                out += data[at:end]
                at = end
        out = bytes(out)
    else:
        raise ValueError(f"chunk type {kind} in {path}")
    if len(out) != size:
        raise ValueError(f"{len(out)} bytes read of {size} in {path}")
    return out


def bank_of(data):
    """The header and the objects (kind, id, body) of a bank's bytes. None for objects of a bank
    with no `HIRC` section, which the census records no bank of."""
    header, objects, at = None, None, 0
    while at + 8 <= len(data):
        tag = data[at : at + 4]
        (size,) = struct.unpack_from("<I", data, at + 4)
        body = data[at + 8 : at + 8 + size]
        at += 8 + size
        if tag == b"BKHD":
            header = body
        elif tag == b"HIRC":
            (count,) = struct.unpack_from("<I", body, 0)
            objects, where = [], 4
            for _ in range(count):
                kind = body[where]
                (length,) = struct.unpack_from("<I", body, where + 1)
                held = body[where + 5 : where + 5 + length]
                where += 5 + length
                objects.append((kind, struct.unpack_from("<I", held, 0)[0], held[4:]))
    return header, objects


LOCALE_WAD = re.compile(r"\.([a-z]{2}_[a-z]{2})\.wad\.client$")


def check(args):
    db = sqlite3.connect(f"file:{args.db}?mode=ro", uri=True)
    if "key" not in columns(db):
        sys.exit("event_banks keys no bank by its file")
    faults = []
    (rows,) = db.execute("SELECT count(*) FROM event_banks").fetchone()
    (unkeyed,) = db.execute(
        "SELECT count(*) FROM event_banks WHERE typeof(key) != 'blob' OR length(key) != 16"
    ).fetchone()
    (unplaced,) = db.execute(
        "SELECT count(*) FROM event_banks e WHERE NOT EXISTS (SELECT 1 FROM placements p "
        "WHERE p.path = e.path AND p.key = e.key)"
    ).fetchone()
    index = db.execute("SELECT sql FROM sqlite_master WHERE name = 'event_banks_key'").fetchone()
    print(f"{rows} rows: {unkeyed} with no file key, {unplaced} whose file no WAD held there")
    if unkeyed or unplaced:
        faults.append("a row is keyed by no file the placements hold")
    if index is None or index[0] != INDEX:
        faults.append(f"the table's unique index is {index}")

    held = defaultdict(lambda: defaultdict(list))
    facts = {}
    for path, key, record, header, wad, checksum in db.execute(
        "SELECT e.path, e.key, e.record, e.header, w.path, p.checksum FROM event_banks e "
        "JOIN placements p ON p.path = e.path AND p.key = e.key JOIN wads w ON w.id = p.wad "
        "WHERE p.first <= ?1 AND p.last >= ?1",
        (args.at,),
    ):
        held[path][bytes(key)].append((wad, checksum & MASK))
        facts[(path, bytes(key))] = (record, bytes(header))
    several = {path: files for path, files in held.items() if len(files) > 1}
    print(f"at {args.at}: {len(held)} paths hold banks, {len(several)} of more than one file")

    game = Path(args.game)
    files = {
        str(p.relative_to(game)).replace("\\", "/").lower(): p
        for p in game.rglob("*.wad.client")
    }
    tocs = {}
    locale = args.locale.lower()
    matched = Counter()
    def locale_of(wad):
        found = LOCALE_WAD.search(wad)
        return found.group(1) if found else None

    for path, banks in sorted(several.items()):
        seen = Counter()
        for key, wads in banks.items():
            names = {locale_of(wad) for wad, _ in wads}
            if None in names or len(names) != 1:
                faults.append(f"{path & MASK:016x} {key.hex()}: held in {sorted(wads)}")
                continue
            seen[names.pop()] += 1
        if any(n > 1 for n in seen.values()):
            faults.append(f"{path & MASK:016x}: a locale with two banks: {dict(seen)}")
        # Every file LOCALE's WADs hold at the path, a bank of no `HIRC` among them.
        mine = [
            (bytes(key), wad, checksum & MASK)
            for wad, key, checksum in db.execute(
                "SELECT w.path, p.key, p.checksum FROM placements p JOIN wads w "
                "ON w.id = p.wad WHERE p.path = ?1 AND p.first <= ?2 AND p.last >= ?2",
                (path, args.at),
            )
            if locale_of(wad) == locale
        ]
        if not mine:
            faults.append(f"{path & MASK:016x}: no WAD of {locale} holds a file there")
        for key, wad, checksum in mine:
            file = files.get(wad)
            if file is None:
                faults.append(f"{path & MASK:016x}: the install has no {wad}")
                continue
            toc = tocs.setdefault(file, wad_toc(file))
            entry = toc.get(path & MASK)
            if entry is None:
                faults.append(f"{path & MASK:016x}: {wad} of the install holds no file there")
                continue
            data = chunk_bytes(file, entry)
            got_header, objects = bank_of(data)
            same = [entry[4] == checksum, hashlib.sha256(data).digest()[:16] == key]
            if (path, key) in facts:
                record, header = facts[(path, key)]
                same += [got_header == header, bank_record(got_header, objects) == record]
                kind = "a bank, the row's"
            else:
                same.append(objects is None)
                kind = "no HIRC, and no row"
            if all(same):
                matched[kind] += 1
            else:
                faults.append(f"{path & MASK:016x} {wad}: {kind}: {same}")
    print(f"files of {locale} checked against the install: {dict(matched)}")
    for fault in faults[:20]:
        print("fault:", fault)
    if faults:
        sys.exit(f"{len(faults)} faults")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    filling = commands.add_parser("fill")
    filling.add_argument("--db", required=True)
    filling.add_argument("--builds", required=True)
    filling.add_argument("--repo", required=True)
    filling.add_argument("--jobs", type=int, default=24)
    filling.add_argument("--dry-run", action="store_true")
    checking = commands.add_parser("check")
    checking.add_argument("--db", required=True)
    checking.add_argument("--game", required=True)
    checking.add_argument("--at", type=int, required=True)
    checking.add_argument("--locale", default="en_US")
    args = parser.parse_args()
    {"fill": fill, "check": check}[args.command](args)


if __name__ == "__main__":
    main()
