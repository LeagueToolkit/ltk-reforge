//! The writer of the index: a whole build from the folded history, and an append of the builds
//! after an index's head. Both put a row into a table through `TableWriter`, which reads the
//! table's columns off its definition in `reforge::history::tables`.
//!
//! An index is built beside its directory and renamed into place, the stamp written last, so a
//! directory with a stamp is a whole index.
//!
//! An append applies each new commit's net record counts to the rows they touch. A row keeps
//! its record count at the head (`alive`). A key a commit only removes was held by its parent,
//! so its row is there. A count that falls to zero ends the row at the build before. Rows still
//! held at the end last to the new head. An index appended to holds the rows a build of the
//! same commits writes.

use std::collections::{BTreeMap, HashMap};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::tables::{self, Cell, Table, TableDef};
use reforge::history::{BuildRow, DB_FILE, FORMAT, Families, Life, Position, Stamp, version_order};
use rusqlite::types::{Null, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension, Statement, ToSql, Transaction, params};

use crate::error::CensusError;
use crate::walk::{ChangeSource, Fold, Progress, Visit, Walked, Walker, build_row, position};

/// A cell as SQLite takes it: a name is its id.
enum Bound {
    Integer(i64),
    Key([u8; 16]),
    Null,
}

impl ToSql for Bound {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Bound::Integer(n) => ToSqlOutput::from(*n),
            Bound::Key(key) => ToSqlOutput::Borrowed(ValueRef::Blob(key)),
            Bound::Null => ToSqlOutput::from(Null),
        })
    }
}

/// The ids of the names interned so far.
struct Names<'tx> {
    tx: &'tx Transaction<'tx>,
    ids: HashMap<String, i64>,
}

impl<'tx> Names<'tx> {
    fn new(tx: &'tx Transaction<'tx>) -> Self {
        Self {
            tx,
            ids: HashMap::new(),
        }
    }

    /// The id of a name, interned on first use. A new name takes the next id, so the names of
    /// a whole build are numbered by first appearance.
    fn id(&mut self, name: &str) -> Result<i64, CensusError> {
        if let Some(id) = self.ids.get(name) {
            return Ok(*id);
        }
        let mut find = self
            .tx
            .prepare_cached("SELECT id FROM names WHERE name = ?1")?;
        let found: Option<i64> = find.query_row(params![name], |r| r.get(0)).optional()?;
        let id = match found {
            Some(id) => id,
            None => {
                let mut insert = self.tx.prepare_cached(
                    "INSERT INTO names (id, name) \
                     VALUES ((SELECT IFNULL(MAX(id), 0) + 1 FROM names), ?1)",
                )?;
                insert.execute(params![name])?;
                self.tx.last_insert_rowid()
            }
        };
        self.ids.insert(name.to_string(), id);
        Ok(id)
    }

    fn bind(&mut self, cell: Cell<'_>) -> Result<Bound, CensusError> {
        Ok(match cell {
            Cell::Integer(n) => Bound::Integer(n),
            Cell::Key(key) => Bound::Key(key),
            Cell::Name(name) => Bound::Integer(self.id(name)?),
            Cell::NoName => Bound::Null,
        })
    }
}

/// The condition that finds a row by its key, the key's cells bound from slot `first` on.
fn where_key(def: &TableDef, first: usize) -> String {
    let columns = def.key.iter().zip(first..);
    let equal: Vec<String> = columns
        .map(|(column, slot)| format!("{} = ?{slot}", column.name))
        .collect();
    equal.join(" AND ")
}

fn slots(count: usize) -> String {
    let slots: Vec<String> = (1..=count).map(|n| format!("?{n}")).collect();
    slots.join(", ")
}

/// The statements that write one table's rows, made from the table's definition.
struct TableWriter<'tx, 'n> {
    def: TableDef,
    tx: &'tx Transaction<'tx>,
    names: &'n mut Names<'tx>,
    insert: Statement<'tx>,
    find: Statement<'tx>,
    update: Statement<'tx>,
    part: Option<Statement<'tx>>,
    cells: Vec<Bound>,
}

impl<'tx, 'n> TableWriter<'tx, 'n> {
    fn new(
        def: TableDef,
        tx: &'tx Transaction<'tx>,
        names: &'n mut Names<'tx>,
    ) -> Result<Self, CensusError> {
        let mut columns: Vec<&str> = def.columns().map(|c| c.name).collect();
        columns.extend(["first", "last", "alive"]);
        let insert = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            def.name,
            columns.join(", "),
            slots(columns.len())
        );
        let find = format!(
            "SELECT last, alive FROM {} WHERE {}",
            def.name,
            where_key(&def, 1)
        );
        // The update sets the life in its first two slots, so its key starts at the third.
        let update = format!(
            "UPDATE {} SET last = ?1, alive = ?2 WHERE {}",
            def.name,
            where_key(&def, 3)
        );
        let part = match &def.parts {
            Some(parts) => {
                let sql = format!(
                    "INSERT INTO {} VALUES ({})",
                    parts.name,
                    slots(parts.columns.len() + 2)
                );
                Some(tx.prepare(&sql)?)
            }
            None => None,
        };
        Ok(Self {
            def,
            tx,
            names,
            insert: tx.prepare(&insert)?,
            find: tx.prepare(&find)?,
            update: tx.prepare(&update)?,
            part,
            cells: Vec::new(),
        })
    }

    /// Fills `cells` with a row's key and facts.
    fn bind<T: Table>(&mut self, key: &T::Key, facts: &T::Facts) -> Result<(), CensusError> {
        self.cells.clear();
        for cell in T::cells(key, facts) {
            let bound = self.names.bind(cell)?;
            self.cells.push(bound);
        }
        Ok(())
    }

    /// Adds the row bound, with its parts.
    fn insert_bound<T: Table>(
        &mut self,
        key: &T::Key,
        life: Life,
        alive: i64,
    ) -> Result<(), CensusError> {
        let life = [life.first, life.last].map(|at| Bound::Integer(i64::from(at.0)));
        self.cells.extend(life);
        self.cells.push(Bound::Integer(alive));
        self.insert
            .execute(rusqlite::params_from_iter(self.cells.iter()))?;
        let Some(part) = &mut self.part else {
            return Ok(());
        };
        let owner = self.tx.last_insert_rowid();
        for (ordinal, cells) in (0i64..).zip(T::parts(key)) {
            let mut row = vec![Bound::Integer(owner), Bound::Integer(ordinal)];
            for cell in cells {
                row.push(self.names.bind(cell)?);
            }
            part.execute(rusqlite::params_from_iter(row.iter()))?;
        }
        Ok(())
    }

    /// Adds a row of a whole build.
    fn insert<T: Table>(&mut self, row: &tables::Row<T>) -> Result<(), CensusError> {
        self.bind::<T>(&row.key, &row.facts)?;
        self.insert_bound::<T>(&row.key, row.life, i64::from(row.alive))
    }

    /// Applies one commit's net record count of a key to its row. A key new to the index gets
    /// a row from that build, and a key that comes back keeps its row's first build.
    fn apply<T: Table>(
        &mut self,
        key: &T::Key,
        facts: &T::Facts,
        net: i64,
        at: Position,
    ) -> Result<(), CensusError> {
        self.bind::<T>(key, facts)?;
        let found = &self.cells[..self.def.key.len()];
        let held: Option<(i64, i64)> = self
            .find
            .query_row(rusqlite::params_from_iter(found.iter()), |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let Some((last, alive)) = held else {
            if net <= 0 {
                return Err(CensusError::NeverHeld {
                    table: self.def.name,
                    at,
                    key: format!("{key:?}"),
                });
            }
            let life = Life {
                first: at,
                last: at,
            };
            return self.insert_bound::<T>(key, life, net);
        };
        let now = alive + net;
        if now < 0 {
            return Err(CensusError::Overdrawn {
                table: self.def.name,
                at,
                key: format!("{key:?}"),
                removed: -net,
                held: alive,
            });
        }
        let last = if alive > 0 && now == 0 {
            i64::from(at.0) - 1
        } else {
            last
        };
        let life = [Bound::Integer(last), Bound::Integer(now)];
        self.update
            .execute(rusqlite::params_from_iter(life.iter().chain(found)))?;
        Ok(())
    }
}

/// Fills each table with the rows of a whole walk.
struct Whole<'tx> {
    tx: &'tx Transaction<'tx>,
    names: Names<'tx>,
    head: Position,
}

impl Visit for Whole<'_> {
    fn table<T: Table>(&mut self, fold: Fold<T>) -> Result<(), CensusError> {
        let mut writer = TableWriter::new(T::DEF, self.tx, &mut self.names)?;
        for row in fold.rows(self.head) {
            writer.insert(&row)?;
        }
        Ok(())
    }
}

/// Applies one commit's net record counts to each table.
struct Appended<'tx, 'n> {
    tx: &'tx Transaction<'tx>,
    names: &'n mut Names<'tx>,
    at: Position,
}

impl Visit for Appended<'_, '_> {
    fn table<T: Table>(&mut self, fold: Fold<T>) -> Result<(), CensusError> {
        let net = fold.net();
        if net.is_empty() {
            return Ok(());
        }
        let mut writer = TableWriter::new(T::DEF, self.tx, self.names)?;
        for (key, facts, net) in &net {
            writer.apply::<T>(key, facts, *net, self.at)?;
        }
        Ok(())
    }
}

/// How many rows each table of the index holds.
fn row_counts(db: &Connection) -> Result<BTreeMap<String, u64>, CensusError> {
    let mut rows = BTreeMap::new();
    for table in tables::table_names() {
        let count: i64 =
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
        rows.insert(table.to_string(), u64::try_from(count).unwrap_or(0));
    }
    Ok(rows)
}

/// The directory an index is built in, beside the one it is renamed to.
fn scratch(out: &Utf8Path) -> Result<Utf8PathBuf, CensusError> {
    let tmp = out.with_extension("tmp");
    if tmp.exists() {
        fs::remove_dir_all(&tmp)?;
    }
    fs::create_dir_all(&tmp)?;
    Ok(tmp)
}

/// Writes `builds.tsv` and the stamp beside the database in `tmp`, the stamp last, and moves
/// `tmp` into `out`.
fn install(
    tmp: &Utf8Path,
    out: &Utf8Path,
    builds: &[BuildRow],
    stamp: &Stamp,
) -> Result<(), CensusError> {
    BuildRow::write_all(tmp, builds)?;
    stamp.write(tmp)?;
    if out.exists() {
        fs::remove_dir_all(out)?;
    }
    Ok(fs::rename(tmp, out)?)
}

/// The version of this crate, which a stamp names as its writer.
const WRITER: &str = env!("CARGO_PKG_VERSION");

/// Writes the index of a whole walk into `out`: built beside it and renamed into place.
pub(crate) fn write(
    walked: Walked,
    out: &Utf8Path,
    repo: &str,
    families: Families,
) -> Result<Stamp, CensusError> {
    let newest = walked.builds.iter().map(|row| &row.build);
    let newest = newest.max_by(|a, b| version_order(a, b)).cloned();
    let (Some(newest_build), Some(head)) = (newest, walked.builds.last()) else {
        return Err(CensusError::Empty);
    };
    let tmp = scratch(out)?;
    let rows = {
        let mut db = Connection::open(tmp.join(DB_FILE))?;
        // A file written once and renamed into place: no journal, no syncing.
        db.execute_batch(
            "PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF; PRAGMA page_size = 8192;",
        )?;
        db.execute_batch(&tables::schema_sql())?;
        let tx = db.transaction()?;
        let mut whole = Whole {
            tx: &tx,
            names: Names::new(&tx),
            head: head.at,
        };
        walked.folds.each(&mut whole)?;
        drop(whole);
        tx.commit()?;
        db.execute_batch(&tables::index_sql())?;
        // Rewritten whole, the composite-key tables pack tighter: 1.41 GB to 1.29 GB in 6 s
        // (2026-10-01).
        db.execute_batch("VACUUM;")?;
        row_counts(&db)?
    };
    let stamp = Stamp {
        format: FORMAT,
        repo: repo.to_string(),
        head: head.commit.clone(),
        newest_build,
        builds: walked.builds.len(),
        families,
        writer: WRITER.to_string(),
        rows,
        unparsed_texts: walked.unparsed_texts,
    };
    install(&tmp, out, &walked.builds, &stamp)?;
    Ok(stamp)
}

/// Adds the builds of `source` after the head of the index at `out`. Every build the index
/// holds must be the history's at the same position. An index of a history that was rewritten
/// is rebuilt instead.
pub(crate) fn append(
    source: &dyn ChangeSource,
    repo: &str,
    out: &Utf8Path,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<Stamp, CensusError> {
    let stamp = Stamp::read(out)?;
    let mut builds = BuildRow::read_all(out)?;
    let commits = source.commits()?;
    for row in &builds {
        let at = usize::from(row.at.0);
        if commits.get(at) != Some(&row.commit) {
            return Err(CensusError::Diverged {
                at,
                commit: row.commit.clone(),
            });
        }
    }
    if commits.len() > usize::from(u16::MAX) {
        return Err(CensusError::TooManyBuilds(commits.len()));
    }
    if commits.len() == builds.len() {
        return Ok(stamp);
    }

    let tmp = scratch(out)?;
    fs::copy(out.join(DB_FILE), tmp.join(DB_FILE))?;
    let mut db = Connection::open(tmp.join(DB_FILE))?;
    // A copy renamed into place when done: no journal, no syncing.
    db.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    let tx = db.transaction()?;
    let mut names = Names::new(&tx);
    let (mut newest, mut unparsed) = (stamp.newest_build.clone(), stamp.unparsed_texts);
    for i in builds.len()..commits.len() {
        let at = position(i)?;
        let parent = i.checked_sub(1).map(|p| commits[p].as_str());
        // A walker of its own per commit: its counts are the commit's net changes.
        let mut walker = Walker::new(stamp.families);
        let build = walker.commit(source, &commits[i], parent, at)?;
        unparsed += walker.unparsed_texts;
        let mut appended = Appended {
            tx: &tx,
            names: &mut names,
            at,
        };
        walker.folds.each(&mut appended)?;
        if version_order(&build.build, &newest).is_gt() {
            newest = build.build.clone();
        }
        builds.push(build_row(at, &commits[i], build));
        progress(Progress {
            done: i + 1,
            of: commits.len(),
            build: &builds[i],
        });
    }
    let head = builds.last().ok_or(CensusError::Empty)?;
    for table in tables::TABLES {
        let sql = format!("UPDATE {} SET last = ?1 WHERE alive > 0", table.name);
        tx.execute(&sql, params![i64::from(head.at.0)])?;
    }
    drop(names);
    tx.commit()?;
    let stamp = Stamp {
        repo: repo.to_string(),
        head: head.commit.clone(),
        newest_build: newest,
        builds: builds.len(),
        writer: WRITER.to_string(),
        rows: row_counts(&db)?,
        unparsed_texts: unparsed,
        ..stamp
    };
    drop(db);
    install(&tmp, out, &builds, &stamp)?;
    Ok(stamp)
}
