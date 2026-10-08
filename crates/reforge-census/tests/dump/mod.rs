//! An index as lines of text per table, names, WADs and ids resolved, so that two indexes
//! compare by what they say and not by the order they were written in. The object bytes are
//! compared decoded: two dictionaries write other frames of the same bytes.

use std::collections::BTreeMap;

use camino::Utf8Path;
use reforge::history::tables::{TABLES, table_names};
use reforge::history::{BlobId, SqliteHistory};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

/// The query that reads each table as its lines.
const QUERIES: &[(&str, &str)] = &[
    (
        "files",
        "SELECT hex(key), first, last, kind, path, alive FROM files",
    ),
    (
        "placements",
        "SELECT p.path, w.path, hex(p.key), p.checksum, p.first, p.last, p.alive \
         FROM placements p JOIN wads w ON w.id = p.wad",
    ),
    (
        "wad_lives",
        "SELECT w.path, v.name, l.file_id, t.name, l.first, l.last, l.alive FROM wad_lives l \
         JOIN wads w ON w.id = l.wad JOIN names v ON v.id = l.version \
         JOIN names t ON t.id = l.tags",
    ),
    (
        "objects",
        "SELECT hex(key), entry, class, bin, norm, first, last, alive FROM objects",
    ),
    (
        "bins",
        "SELECT hex(b.key), hex(b.links), b.first, b.last, b.alive, \
         (SELECT group_concat(e.entry || ':' || o.class || ':' || hex(o.key), ',' \
          ORDER BY e.entry) FROM bin_entries e JOIN objects o ON o.id = e.object \
          WHERE e.bin = b.id) FROM bins b",
    ),
    (
        "links",
        "SELECT bin, entry, linked, first, last, alive FROM links",
    ),
    (
        "schema",
        "SELECT class, field, kind, key_kind, item_kind, value_class, first, last, alive \
         FROM schema",
    ),
    (
        "entry_texts",
        "SELECT t.bin, t.entry, w.path, hex(t.blob), t.first, t.last, t.alive \
         FROM entry_texts t JOIN wads w ON w.id = t.wad",
    ),
    (
        "refs",
        "SELECT path, bin, entry, first, last, alive FROM refs",
    ),
    ("clips", "SELECT key, first, last, alive FROM clips"),
    (
        "events",
        "SELECT bank, event, first, last, alive FROM events",
    ),
    (
        "event_wems",
        "SELECT bank, event, wem, first, last, alive FROM event_wems",
    ),
    (
        "tags",
        "SELECT t.bin, t.entry, n.name, t.first, t.last, t.alive FROM tags t \
         JOIN names n ON n.id = t.tag",
    ),
    (
        "units",
        "SELECT u.bin, u.entry, n.name, u.voice, u.first, u.last, u.alive FROM units u \
         JOIN names n ON n.id = u.unit",
    ),
    (
        "unit_banks",
        "SELECT u.bin, u.entry, n.name, u.bank, u.first, u.last, u.alive FROM unit_banks u \
         JOIN names n ON n.id = u.unit",
    ),
    (
        "unit_events",
        "SELECT u.bin, u.entry, n.name, u.event, u.first, u.last, u.alive \
         FROM unit_events u JOIN names n ON n.id = u.unit",
    ),
    (
        "skeletons",
        "SELECT s.path, s.record, s.first, s.last, s.influences, s.alive, \
         (SELECT group_concat(n.name || '>' || IFNULL(p.name, '-'), ',' ORDER BY j.ordinal) \
          FROM joints j JOIN names n ON n.id = j.name LEFT JOIN names p ON p.id = j.parent \
          WHERE j.skeleton = s.id) FROM skeletons s",
    ),
    (
        "meshes",
        "SELECT m.path, m.record, m.first, m.last, v.name, m.vertices, m.indices, m.alive, \
         (SELECT group_concat(n.name, ',' ORDER BY s.ordinal) FROM submeshes s \
          JOIN names n ON n.id = s.material WHERE s.mesh = m.id) \
         FROM meshes m JOIN names v ON v.id = m.vertex_type",
    ),
    (
        "textures",
        "SELECT t.path, t.record, t.first, t.last, n.name, t.width, t.height, t.mips, t.top, \
         t.alive FROM textures t JOIN names n ON n.id = t.format",
    ),
    (
        "joint_lists",
        "SELECT l.record, l.count, l.first, l.last, l.alive, \
         (SELECT group_concat(j.joint, ',' ORDER BY j.ordinal) FROM list_joints j \
          WHERE j.list = l.record) FROM joint_lists l",
    ),
    (
        "animations",
        "SELECT path, hex(key), joints, first, last, alive FROM animations",
    ),
    (
        "bank_objects",
        "SELECT record, kind, id, hex(body), first, last, alive FROM bank_objects",
    ),
    (
        "event_banks",
        "SELECT path, hex(key), record, version, bank_id, hex(header), hex(objects), first, \
         last, alive FROM event_banks",
    ),
    ("names", "SELECT name FROM names"),
    ("wads", "SELECT path FROM wads"),
];

/// The tables read decoded, as one line per row of `object_bytes`.
const OBJECT_TABLES: [&str; 2] = ["object_bytes", "text_bytes"];

/// The table not compared: a whole build trains its dictionaries over its own builds, and an
/// append keeps those it was given.
const DICTS: &str = "object_dicts";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Hands every line of every table to `line`, a table at a time, in no order within a table.
pub fn each_line(dir: &Utf8Path, mut line: impl FnMut(&str, String)) {
    // Every table is read: its own rows, its parts with its owner's, or its objects decoded.
    let mut read: Vec<&str> = QUERIES.iter().map(|(table, _)| *table).collect();
    read.extend(TABLES.iter().filter_map(|t| t.parts).map(|p| p.name));
    read.extend(OBJECT_TABLES);
    read.push(DICTS);
    read.sort_unstable();
    let mut tables = table_names();
    tables.sort_unstable();
    assert_eq!(read, tables, "the dump leaves a table out");

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
    let db = Connection::open_with_flags(dir.join("history.sqlite"), flags).unwrap();
    for (table, sql) in QUERIES {
        let mut statement = db.prepare(sql).unwrap();
        let columns = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let values = (0..columns).map(|i| row.get::<_, Value>(i).unwrap());
            line(table, format!("{:?}", values.collect::<Vec<Value>>()));
        }
    }

    // A row of the object bytes: the digest of its bytes, the digest of its keyframe's, and
    // every text that holds it.
    let history = SqliteHistory::open(dir).unwrap();
    let mut statement = db
        .prepare("SELECT object, blob, class FROM text_bytes ORDER BY object, blob")
        .unwrap();
    let mut rows = statement.query([]).unwrap();
    let mut texts: Vec<(BlobId, u32)> = Vec::new();
    let mut held: Option<i64> = None;
    let mut flush = |texts: &mut Vec<(BlobId, u32)>| {
        let Some((blob, _)) = texts.first() else {
            return;
        };
        let stored = history.stored_object(blob).unwrap().unwrap();
        let keyframe = stored.keyframe.as_deref().unwrap_or(&stored.bytes);
        let held: Vec<String> = texts
            .iter()
            .map(|(blob, class)| format!("{}:{class:08x}", hex(blob)))
            .collect();
        let text = format!(
            "{} {} {}",
            sha(&stored.bytes),
            sha(keyframe),
            held.join(",")
        );
        line("object_bytes", text);
        texts.clear();
    };
    while let Some(row) = rows.next().unwrap() {
        let object: i64 = row.get(0).unwrap();
        if held != Some(object) {
            flush(&mut texts);
            held = Some(object);
        }
        texts.push((row.get(1).unwrap(), row.get(2).unwrap()));
    }
    flush(&mut texts);
    let rows: i64 = db
        .query_row("SELECT COUNT(*) FROM object_bytes", [], |r| r.get(0))
        .unwrap();
    line("object_bytes", format!("{rows} rows"));
}

/// Every table as its sorted lines.
pub fn dump(dir: &Utf8Path) -> Vec<(String, Vec<String>)> {
    let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
    each_line(dir, |table, line| {
        tables.entry(table.to_string()).or_default().push(line);
    });
    for lines in tables.values_mut() {
        lines.sort_unstable();
    }
    tables.into_iter().collect()
}
