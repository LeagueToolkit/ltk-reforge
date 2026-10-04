//! The index, open for questions. A question is one indexed query: nothing is loaded whole,
//! and a script reads the same file with `sqlite3`.

use std::sync::{Mutex, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use reforge_wwise::Object;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};

use super::stamp::{BuildRow, DB_FILE, Stamp};
use super::tables::{
    BankFacts, EventsBank, Joint, Life, Mesh, Position, Skeleton, Submesh, loaded, stored,
};
use super::{
    ClipJoints, EventsBankHit, FileHit, FileKind, HeldAt, History, HistoryError, MeshHit, NamedBy,
    ObjectHit, PathHit, PlayedBy, SkeletonHit, UnitHit, WemPlayed,
};
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

fn path_hash(row: &Row<'_>, at: usize) -> rusqlite::Result<WadHash> {
    Ok(WadHash(loaded(row.get(at)?)))
}

fn bin_hash(row: &Row<'_>, at: usize) -> rusqlite::Result<BinHash> {
    Ok(BinHash(row.get(at)?))
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
        self.one(
            "SELECT first, last, kind FROM paths WHERE path = ?1",
            params![stored(path.0)],
            |r| {
                Ok(PathHit {
                    life: life(r, 0)?,
                    kind: FileKind(r.get(2)?),
                })
            },
        )
    }

    fn held(&self, path: WadHash, key: FileKey) -> Result<Option<Life>, HistoryError> {
        self.one(
            "SELECT first, last FROM contents WHERE path = ?1 AND key = ?2",
            params![stored(path.0), &key.0[..]],
            |r| life(r, 0),
        )
    }

    fn paths_of(&self, key: FileKey) -> Result<Vec<HeldAt>, HistoryError> {
        self.all(
            "SELECT path, first, last FROM contents WHERE key = ?1 ORDER BY path",
            params![&key.0[..]],
            |r| {
                Ok(HeldAt {
                    path: path_hash(r, 0)?,
                    life: life(r, 1)?,
                })
            },
        )
    }

    fn object(&self, key: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        self.one(
            "SELECT entry, class, bin, norm, first, last FROM objects WHERE key = ?1",
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
