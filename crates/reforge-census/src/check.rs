//! The checks of an index's object bytes against its other tables: which texts of `entry_texts`
//! have their object, and whether the digest of each object's bytes is an object key of
//! `objects` under its text's class, as the census digested the game's own bytes.

use std::collections::HashSet;
use std::io::Cursor;

use camino::Utf8Path;
use ltk_meta::BinObject;
use ltk_meta::PropertyKind as Kind;
use rayon::prelude::*;
use reforge::game::FieldType;
use reforge::history::tables::loaded;
use reforge::history::{BlobId, DB_FILE, History, Position, SqliteHistory};
use reforge::model::value::walk_object;
use reforge::model::{BinHash, ObjectKey, WadHash};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::error::CensusError;

/// Texts checked per task: enough to keep a core busy, few enough to spread over every core.
const CHUNK: usize = 1 << 14;

/// A text whose object's digest is no object key under its class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unkeyed {
    /// The text's blob.
    pub blob: BlobId,
    /// The text's class.
    pub class: BinHash,
    /// The digest of the object's bytes.
    pub key: ObjectKey,
    /// The path of the first bin that held the text.
    pub bin: WadHash,
    /// The entry the text is of.
    pub entry: BinHash,
    /// The first build that held the text.
    pub first: Position,
    /// Whether that build's bins number property kinds as the builds before 10.8 do, so that
    /// the census digested bytes `ltk_meta` does not write.
    pub legacy: bool,
    /// Whether the object holds a kind legacy numbering writes as another byte: of a value, an
    /// item or a map's key, `WadChunkLink` or a kind after it. An object that holds none writes
    /// the same bytes either way.
    pub renumbered: bool,
}

/// Whether an object holds a kind legacy numbering writes as another byte. None when the bytes
/// do not read back.
fn renumbered(class: BinHash, bytes: &[u8]) -> Option<bool> {
    let mut sized = u32::try_from(bytes.len()).ok()?.to_le_bytes().to_vec();
    sized.extend_from_slice(bytes);
    let object = BinObject::from_reader(&mut Cursor::new(sized), class, false).ok()?;
    let moved = |kind: Kind| u8::from(kind) >= u8::from(Kind::WadChunkLink);
    let mut any = false;
    walk_object(&object, &mut |value| {
        let ty = FieldType::of(value);
        any |= moved(ty.kind) || ty.key.is_some_and(moved) || ty.item.is_some_and(moved);
    });
    Some(any)
}

/// What an index's object bytes hold against its other tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectCheck {
    /// Texts of `entry_texts`, each blob once.
    pub texts: usize,
    /// Texts that have their object.
    pub stored: usize,
    /// Rows of `text_bytes` whose blob no row of `entry_texts` holds.
    pub orphans: usize,
    /// Rows of `object_bytes`.
    pub objects: usize,
    /// Texts whose object's digest is an object key under the text's class.
    pub keyed: usize,
    /// Texts whose object's digest is no object key under the text's class, in blob order.
    pub unkeyed: Vec<Unkeyed>,
}

fn count(db: &Connection, sql: &str) -> Result<usize, CensusError> {
    let n: i64 = db.query_row(sql, [], |r| r.get(0))?;
    Ok(usize::try_from(n).unwrap_or(0))
}

/// Checks the texts of one chunk: answers how many are keyed, and the others.
fn check_chunk(
    dir: &Utf8Path,
    texts: &[(BlobId, BinHash)],
) -> Result<(usize, Vec<Unkeyed>), CensusError> {
    let history = SqliteHistory::open(dir)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
    let db = Connection::open_with_flags(dir.join(DB_FILE), flags)?;
    let mut keyed_by = db.prepare("SELECT 1 FROM objects WHERE key = ?1 AND class = ?2")?;
    let mut first_of = db.prepare(
        "SELECT bin, entry, first FROM entry_texts WHERE entry = ?1 AND blob = ?2 \
         ORDER BY first LIMIT 1",
    )?;
    let (mut keyed, mut unkeyed) = (0, Vec::new());
    for (blob, class) in texts {
        let Some(stored) = history.stored_object(blob)? else {
            continue;
        };
        let digest: [u8; 32] = Sha256::digest(&stored.bytes).into();
        let key = ObjectKey::from_sha256(&digest);
        let found = keyed_by.query_row(params![&key.0[..], class.0], |_| Ok(()));
        if found.optional()?.is_some() {
            keyed += 1;
            continue;
        }
        let entry = stored
            .bytes
            .first_chunk()
            .map_or(0, |b| u32::from_le_bytes(*b));
        let place = first_of.query_row(params![entry, &blob[..]], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, u16>(2)?,
            ))
        });
        // No row: an orphan, counted apart.
        let Some((bin, entry, first)) = place.optional()? else {
            continue;
        };
        let first = Position(first);
        let legacy = history.build(first).is_some_and(|b| b.details.legacy_bins);
        unkeyed.push(Unkeyed {
            blob: *blob,
            class: *class,
            key,
            bin: WadHash(loaded(bin)),
            entry: BinHash(entry),
            first,
            legacy,
            renumbered: renumbered(*class, &stored.bytes).unwrap_or(false),
        });
    }
    Ok((keyed, unkeyed))
}

/// Checks the object bytes of the index in `dir`, on every core.
pub fn check_objects(dir: &Utf8Path) -> Result<ObjectCheck, CensusError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
    let db = Connection::open_with_flags(dir.join(DB_FILE), flags)?;
    let mut texts: HashSet<BlobId> = HashSet::new();
    {
        let mut read = db.prepare("SELECT DISTINCT blob FROM entry_texts")?;
        let mut rows = read.query([])?;
        while let Some(row) = rows.next()? {
            texts.insert(row.get(0)?);
        }
    }
    let mut stored: Vec<(BlobId, BinHash)> = Vec::new();
    {
        let mut read = db.prepare("SELECT blob, class FROM text_bytes ORDER BY blob")?;
        let mut rows = read.query([])?;
        while let Some(row) = rows.next()? {
            stored.push((row.get(0)?, BinHash(row.get(1)?)));
        }
    }
    let orphans = stored.iter().filter(|(b, _)| !texts.contains(b)).count();
    stored.retain(|(b, _)| texts.contains(b));
    let checked: Vec<(usize, Vec<Unkeyed>)> = stored
        .par_chunks(CHUNK)
        .map(|chunk| check_chunk(dir, chunk))
        .collect::<Result<_, _>>()?;
    let mut check = ObjectCheck {
        texts: texts.len(),
        stored: stored.len(),
        orphans,
        objects: count(&db, "SELECT COUNT(*) FROM object_bytes")?,
        ..ObjectCheck::default()
    };
    for (keyed, unkeyed) in checked {
        check.keyed += keyed;
        check.unkeyed.extend(unkeyed);
    }
    Ok(check)
}
