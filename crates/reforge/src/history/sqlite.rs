//! The index, open for questions. A question is one indexed query: nothing is loaded whole,
//! and a script reads the same file with `sqlite3`.

use std::sync::{Mutex, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_meta::PropertyKind as Kind;
use reforge_wwise::Object;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};

use super::stamp::{BuildRow, DB_FILE, Stamp};
use super::tables::{
    BankFacts, BinEntry, BinFacts, EventsBank, Joint, Life, Mesh, NO_KIND, Position, Skeleton,
    Submesh, loaded, stored,
};
use super::{
    BinHit, ClipJoints, EventsBankHit, FileHit, FileKind, HeldAt, History, HistoryError,
    MAX_REACHED, MeshHit, NamedBy, ObjectHit, PathHit, Placed, PlayedBy, Reached, SkeletonHit,
    TextBlob, UnitHit, WemPlayed, wad_order,
};
use crate::game::FieldType;
use crate::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash};

/// The index on disk, read through SQLite.
pub struct SqliteHistory {
    dir: Utf8PathBuf,
    stamp: Stamp,
    builds: Vec<BuildRow>,
    db: Mutex<Connection>,
}

/// The life in two columns of a row, `first` then `last`.
fn life(row: &Row<'_>, first: usize) -> rusqlite::Result<Life> {
    Ok(Life {
        first: Position(row.get(first)?),
        last: Position(row.get(first + 1)?),
    })
}

/// The hull of a key's lives, in two aggregate columns that are null when no row holds the key.
fn hull(row: &Row<'_>, at: usize) -> rusqlite::Result<Option<Life>> {
    let first: Option<u16> = row.get(at)?;
    let last: Option<u16> = row.get(at + 1)?;
    Ok(first.zip(last).map(|(first, last)| Life::new(first, last)))
}

fn path_hash(row: &Row<'_>, at: usize) -> rusqlite::Result<WadHash> {
    Ok(WadHash(loaded(row.get(at)?)))
}

fn bin_hash(row: &Row<'_>, at: usize) -> rusqlite::Result<BinHash> {
    Ok(BinHash(row.get(at)?))
}

/// A kind as `schema` stores it, none for `NO_KIND`.
fn stored_kind(row: &Row<'_>, at: usize) -> rusqlite::Result<Option<Kind>> {
    let stored: u8 = row.get(at)?;
    if stored == NO_KIND {
        return Ok(None);
    }
    let unknown = |_| rusqlite::Error::IntegralValueOutOfRange(at, i64::from(stored));
    Kind::try_from(stored).map(Some).map_err(unknown)
}

/// A row of `entry, class, bin, norm, first, last`.
fn object_hit(row: &Row<'_>) -> rusqlite::Result<ObjectHit> {
    Ok(ObjectHit {
        entry: bin_hash(row, 0)?,
        class: bin_hash(row, 1)?,
        bin: path_hash(row, 2)?,
        norm: NormKey(loaded(row.get(3)?)),
        life: life(row, 4)?,
    })
}

impl SqliteHistory {
    /// Opens the index in `dir`: its stamp, its builds and its tables, read only.
    pub fn open(dir: &Utf8Path) -> Result<Self, HistoryError> {
        let stamp = Stamp::read(dir)?;
        let builds = BuildRow::read_all(dir)?;
        if builds.len() != stamp.builds {
            return Err(HistoryError::BuildCount {
                dir: dir.to_owned(),
                stamped: stamp.builds,
                listed: builds.len(),
            });
        }
        let path = dir.join(DB_FILE);
        // The connection is used by one thread at a time, behind the lock below.
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let db = Connection::open_with_flags(&path, flags)
            .map_err(|source| HistoryError::Open { path, source })?;
        Ok(Self {
            dir: dir.to_owned(),
            stamp,
            builds,
            db: Mutex::new(db),
        })
    }

    /// The index's directory.
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// The one row `sql` gives for `key`, mapped by `row`.
    fn one<T>(
        &self,
        sql: &str,
        key: impl rusqlite::Params,
        row: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, HistoryError> {
        // A reader that panicked left the connection as it was: it holds no state of a query.
        let db = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let mut statement = db.prepare_cached(sql)?;
        Ok(statement.query_row(key, row).optional()?)
    }

    /// Every row `sql` gives for `key`, mapped by `row`.
    fn all<T>(
        &self,
        sql: &str,
        key: impl rusqlite::Params,
        row: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, HistoryError> {
        let db = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let mut statement = db.prepare_cached(sql)?;
        let rows = statement.query_map(key, row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
    }

    /// The joints of the list of this record, in stored order.
    fn joint_list(&self, record: i64) -> Result<Vec<u32>, HistoryError> {
        self.all(
            "SELECT joint FROM list_joints WHERE list = ?1 ORDER BY ordinal",
            params![record],
            |r| r.get(0),
        )
    }
}

impl History for SqliteHistory {
    fn stamp(&self) -> Option<&Stamp> {
        Some(&self.stamp)
    }

    fn builds(&self) -> &[BuildRow] {
        &self.builds
    }

    fn file(&self, key: FileKey) -> Result<Option<FileHit>, HistoryError> {
        self.one(
            "SELECT first, last, kind, path FROM files WHERE key = ?1",
            params![&key.0[..]],
            |r| {
                Ok(FileHit {
                    life: life(r, 0)?,
                    kind: FileKind(r.get(2)?),
                    path: path_hash(r, 3)?,
                })
            },
        )
    }

    fn path(&self, path: WadHash) -> Result<Option<PathHit>, HistoryError> {
        let hit = self.one(
            "SELECT MIN(p.first), MAX(p.last), (SELECT f.kind FROM placements q \
             JOIN wads w ON w.id = q.wad JOIN files f ON f.key = q.key WHERE q.path = ?1 \
             ORDER BY q.first, w.path LIMIT 1) FROM placements p WHERE p.path = ?1",
            params![stored(path.0)],
            |r| {
                let hull = hull(r, 0)?;
                let kind: Option<u8> = r.get(2)?;
                Ok(hull.map(|life| PathHit {
                    life,
                    kind: kind.map_or(FileKind::OTHER, FileKind),
                }))
            },
        )?;
        Ok(hit.flatten())
    }

    fn held(&self, path: WadHash, key: FileKey) -> Result<Option<Life>, HistoryError> {
        let hull = self.one(
            "SELECT MIN(first), MAX(last) FROM placements WHERE path = ?1 AND key = ?2",
            params![stored(path.0), &key.0[..]],
            |r| hull(r, 0),
        )?;
        Ok(hull.flatten())
    }

    fn paths_of(&self, key: FileKey) -> Result<Vec<HeldAt>, HistoryError> {
        self.all(
            "SELECT path, MIN(first), MAX(last) FROM placements WHERE key = ?1 GROUP BY path \
             ORDER BY path",
            params![&key.0[..]],
            |r| {
                Ok(HeldAt {
                    path: path_hash(r, 0)?,
                    life: life(r, 1)?,
                })
            },
        )
    }

    fn placed(&self, path: WadHash, at: Position) -> Result<Vec<Placed>, HistoryError> {
        let mut placed = self.all(
            "SELECT w.path, p.key, p.checksum, p.first, p.last FROM placements p \
             JOIN wads w ON w.id = p.wad WHERE p.path = ?1 AND p.first <= ?2 AND p.last >= ?2",
            params![stored(path.0), at.0],
            |r| {
                Ok(Placed {
                    wad: r.get(0)?,
                    key: FileKey(r.get(1)?),
                    checksum: loaded(r.get(2)?),
                    life: life(r, 3)?,
                })
            },
        )?;
        placed.sort_by(|a, b| wad_order(&a.wad, &b.wad));
        Ok(placed)
    }

    fn bin(&self, path: WadHash, at: Position) -> Result<Option<BinHit>, HistoryError> {
        for placed in self.placed(path, at)? {
            let row: Option<(i64, Vec<u8>)> = self.one(
                "SELECT id, links FROM bins WHERE key = ?1",
                params![&placed.key.0[..]],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let Some((id, links)) = row else { continue };
            // The same records write a bin's objects and their rows of `objects`.
            let entries = self.all(
                "SELECT e.entry, o.class, o.key, o.norm FROM bin_entries e \
                 JOIN objects o ON o.id = e.object WHERE e.bin = ?1 ORDER BY e.entry",
                params![id],
                |r| {
                    Ok(BinEntry {
                        entry: bin_hash(r, 0)?,
                        class: bin_hash(r, 1)?,
                        object: ObjectKey(r.get(2)?),
                        norm: NormKey(loaded(r.get(3)?)),
                    })
                },
            )?;
            return Ok(Some(BinHit {
                wad: placed.wad,
                key: placed.key,
                life: placed.life,
                links: BinFacts::links(&links).collect(),
                entries,
            }));
        }
        Ok(None)
    }

    fn object(&self, key: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        self.one(
            "SELECT entry, class, bin, norm, first, last FROM objects WHERE key = ?1 \
             ORDER BY last DESC, first, class LIMIT 1",
            params![&key.0[..]],
            object_hit,
        )
    }

    fn object_lives(&self, key: ObjectKey) -> Result<Vec<ObjectHit>, HistoryError> {
        self.all(
            "SELECT entry, class, bin, norm, first, last FROM objects WHERE key = ?1 \
             ORDER BY first, class",
            params![&key.0[..]],
            object_hit,
        )
    }

    fn objects_by_norm(&self, norm: NormKey) -> Result<Vec<ObjectHit>, HistoryError> {
        self.all(
            "SELECT entry, class, bin, norm, first, last FROM objects WHERE norm = ?1 \
             ORDER BY first, bin, entry",
            params![stored(norm.0)],
            object_hit,
        )
    }

    fn text_blob(
        &self,
        bin: WadHash,
        entry: BinHash,
        at: Position,
    ) -> Result<Option<TextBlob>, HistoryError> {
        let held = self.all(
            "SELECT w.path, t.blob, t.first, t.last FROM entry_texts t JOIN wads w ON w.id = t.wad \
             WHERE t.bin = ?1 AND t.entry = ?2 AND t.first <= ?3 AND t.last >= ?3",
            params![stored(bin.0), entry.0, at.0],
            |r| {
                Ok(TextBlob {
                    wad: r.get(0)?,
                    blob: r.get(1)?,
                    life: life(r, 2)?,
                })
            },
        )?;
        Ok(held.into_iter().min_by(|a, b| wad_order(&a.wad, &b.wad)))
    }

    fn links(
        &self,
        bin: WadHash,
        entry: BinHash,
        at: Position,
    ) -> Result<Vec<BinHash>, HistoryError> {
        self.all(
            "SELECT linked FROM links WHERE bin = ?1 AND entry = ?2 AND first <= ?3 \
             AND last >= ?3 ORDER BY linked",
            params![stored(bin.0), entry.0, at.0],
            |r| bin_hash(r, 0),
        )
    }

    fn reached(&self, bin: WadHash, entry: BinHash, at: Position) -> Result<Reached, HistoryError> {
        // One more than the bound, so that a search the bound stops is told from one that ends
        // at it.
        let limit = i64::try_from(MAX_REACHED + 1).unwrap_or(i64::MAX);
        let mut objects = self.all(
            "WITH RECURSIVE reach(bin, entry) AS ( \
               SELECT bin, entry FROM entry_texts \
                 WHERE bin = ?1 AND entry = ?2 AND first <= ?3 AND last >= ?3 \
               UNION \
               SELECT t.bin, t.entry FROM reach r \
                 JOIN links l ON l.bin = r.bin AND l.entry = r.entry \
                   AND l.first <= ?3 AND l.last >= ?3 \
                 JOIN entry_texts t ON t.entry = l.linked AND t.first <= ?3 AND t.last >= ?3 \
               LIMIT ?4) \
             SELECT bin, entry FROM reach",
            params![stored(bin.0), entry.0, at.0, limit],
            |r| Ok((path_hash(r, 0)?, bin_hash(r, 1)?)),
        )?;
        let capped = objects.len() > MAX_REACHED;
        objects.truncate(MAX_REACHED);
        objects.sort_unstable();
        Ok(Reached { objects, capped })
    }

    fn field_types(
        &self,
        class: BinHash,
        field: BinHash,
        at: Position,
    ) -> Result<Vec<FieldType>, HistoryError> {
        self.all(
            "SELECT kind, key_kind, item_kind, value_class FROM schema WHERE class = ?1 \
             AND field = ?2 AND first <= ?3 AND last >= ?3 \
             ORDER BY kind, key_kind, item_kind, value_class",
            params![class.0, field.0, at.0],
            |r| {
                // A value always has a kind: only the slots of a map and an item may be empty.
                let none = rusqlite::Error::IntegralValueOutOfRange(0, i64::from(NO_KIND));
                Ok(FieldType {
                    kind: stored_kind(r, 0)?.ok_or(none)?,
                    key: stored_kind(r, 1)?,
                    item: stored_kind(r, 2)?,
                    class: Some(bin_hash(r, 3)?).filter(|class| class.0 != 0),
                })
            },
        )
    }

    fn clip(&self, key: ClipKey) -> Result<Option<Life>, HistoryError> {
        self.one(
            "SELECT first, last FROM clips WHERE key = ?1",
            params![stored(key.0)],
            |r| life(r, 0),
        )
    }

    fn named_by(&self, path: WadHash) -> Result<Vec<NamedBy>, HistoryError> {
        self.all(
            "SELECT bin, entry, first, last FROM refs WHERE path = ?1 ORDER BY first, bin, entry",
            params![stored(path.0)],
            |r| {
                Ok(NamedBy {
                    bin: path_hash(r, 0)?,
                    entry: bin_hash(r, 1)?,
                    life: life(r, 2)?,
                })
            },
        )
    }

    fn units_loading(&self, bank: WadHash) -> Result<Vec<UnitHit>, HistoryError> {
        self.all(
            "SELECT b.bin, b.entry, n.name, u.voice, b.first, b.last FROM unit_banks b \
             JOIN names n ON n.id = b.unit \
             JOIN units u ON u.bin = b.bin AND u.entry = b.entry AND u.unit = b.unit \
             WHERE b.bank = ?1 ORDER BY b.first, b.bin, b.entry, n.name",
            params![stored(bank.0)],
            |r| {
                Ok(UnitHit {
                    bin: path_hash(r, 0)?,
                    entry: bin_hash(r, 1)?,
                    unit: r.get(2)?,
                    voice: r.get(3)?,
                    life: life(r, 4)?,
                })
            },
        )
    }

    fn event_wems(&self, bank: WadHash, event: u32) -> Result<Vec<WemPlayed>, HistoryError> {
        self.all(
            "SELECT wem, first, last FROM event_wems WHERE bank = ?1 AND event = ?2 \
             ORDER BY wem, first",
            params![stored(bank.0), event],
            |r| {
                Ok(WemPlayed {
                    wem: r.get(0)?,
                    life: life(r, 1)?,
                })
            },
        )
    }

    fn events_playing(&self, wem: u32) -> Result<Vec<PlayedBy>, HistoryError> {
        self.all(
            "SELECT bank, event, first, last FROM event_wems WHERE wem = ?1 \
             ORDER BY bank, event, first",
            params![wem],
            |r| {
                Ok(PlayedBy {
                    bank: path_hash(r, 0)?,
                    event: r.get(1)?,
                    life: life(r, 2)?,
                })
            },
        )
    }

    fn skeletons(&self, path: WadHash) -> Result<Vec<SkeletonHit>, HistoryError> {
        let heads: Vec<(i64, Life, u32)> = self.all(
            "SELECT id, first, last, influences FROM skeletons WHERE path = ?1 ORDER BY first, id",
            params![stored(path.0)],
            |r| Ok((r.get(0)?, life(r, 1)?, r.get(3)?)),
        )?;
        let mut out = Vec::with_capacity(heads.len());
        for (id, life, influences) in heads {
            let joints = self.all(
                "SELECT n.name, p.name FROM joints j JOIN names n ON n.id = j.name \
                 LEFT JOIN names p ON p.id = j.parent WHERE j.skeleton = ?1 ORDER BY j.ordinal",
                params![id],
                |r| {
                    Ok(Joint {
                        name: r.get(0)?,
                        parent: r.get(1)?,
                    })
                },
            )?;
            out.push(SkeletonHit {
                life,
                skeleton: Skeleton { influences, joints },
            });
        }
        Ok(out)
    }

    fn meshes(&self, path: WadHash) -> Result<Vec<MeshHit>, HistoryError> {
        let heads: Vec<(i64, Life, Mesh)> = self.all(
            "SELECT m.id, m.first, m.last, n.name, m.vertices, m.indices FROM meshes m \
             JOIN names n ON n.id = m.vertex_type WHERE m.path = ?1 ORDER BY m.first, m.id",
            params![stored(path.0)],
            |r| {
                let mesh = Mesh {
                    vertex_type: r.get(3)?,
                    vertices: r.get(4)?,
                    indices: r.get(5)?,
                    submeshes: Vec::new(),
                };
                Ok((r.get(0)?, life(r, 1)?, mesh))
            },
        )?;
        let mut out = Vec::with_capacity(heads.len());
        for (id, life, mut mesh) in heads {
            mesh.submeshes = self.all(
                "SELECT s.vertices, s.indices, s.positions, s.rounded, n.name FROM submeshes s \
                 JOIN names n ON n.id = s.material WHERE s.mesh = ?1 ORDER BY s.ordinal",
                params![id],
                |r| {
                    Ok(Submesh {
                        vertices: r.get(0)?,
                        indices: r.get(1)?,
                        positions: loaded(r.get(2)?),
                        rounded: loaded(r.get(3)?),
                        material: r.get(4)?,
                    })
                },
            )?;
            out.push(MeshHit { life, mesh });
        }
        Ok(out)
    }

    fn clip_joints(&self, path: WadHash) -> Result<Vec<ClipJoints>, HistoryError> {
        let heads: Vec<(Life, [u8; 16], i64)> = self.all(
            "SELECT first, last, key, joints FROM animations WHERE path = ?1 ORDER BY first, key",
            params![stored(path.0)],
            |r| Ok((life(r, 0)?, r.get(2)?, r.get(3)?)),
        )?;
        let mut out = Vec::with_capacity(heads.len());
        for (life, key, list) in heads {
            out.push(ClipJoints {
                life,
                key: FileKey(key),
                joints: self.joint_list(list)?,
            });
        }
        Ok(out)
    }

    fn joints_of(&self, key: FileKey) -> Result<Option<Vec<u32>>, HistoryError> {
        // The joints are read off the bytes, so every row of the key names the same list.
        let list: Option<i64> = self.one(
            "SELECT joints FROM animations WHERE key = ?1 LIMIT 1",
            params![&key.0[..]],
            |r| r.get(0),
        )?;
        list.map(|list| self.joint_list(list)).transpose()
    }

    fn events_banks(&self, path: WadHash) -> Result<Vec<EventsBankHit>, HistoryError> {
        let heads: Vec<(Life, Vec<u8>, Vec<u8>)> = self.all(
            "SELECT first, last, header, objects FROM event_banks WHERE path = ?1 \
             ORDER BY first, id",
            params![stored(path.0)],
            |r| Ok((life(r, 0)?, r.get(2)?, r.get(3)?)),
        )?;
        let mut out = Vec::with_capacity(heads.len());
        for (life, header, records) in heads {
            let mut objects = Vec::with_capacity(records.len() / 8);
            for record in BankFacts::records(&records) {
                let object = self.one(
                    "SELECT kind, id, body FROM bank_objects WHERE record = ?1",
                    params![stored(record)],
                    |r| {
                        Ok(Object {
                            kind: r.get(0)?,
                            id: r.get(1)?,
                            body: r.get(2)?,
                        })
                    },
                )?;
                // A bank names only objects its own build wrote beside it.
                objects.push(object.ok_or(HistoryError::BankObject { path, record })?);
            }
            let bank = EventsBank { header, objects };
            out.push(EventsBankHit { life, bank });
        }
        Ok(out)
    }
}
