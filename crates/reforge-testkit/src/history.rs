//! A history in memory, for tests: built from facts (a path, a content, an object or a clip at
//! builds a to b), with no SQLite file behind it. It answers the questions about files, objects
//! and clips. The families no stage reads yet answer nothing.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use reforge::game::Build;
use reforge::history::{
    BuildRow, FORMAT, Families, FileHit, FileKind, HeldAt, History, HistoryError, Life, MeshHit,
    NamedBy, ObjectHit, PathHit, PlayedBy, Position, SkeletonHit, Stamp, UnitHit, WemPlayed,
    version_order,
};
use reforge::model::{
    ClipKey, ContentId, FileKey, NormKey, ObjectKey, WadHash, name_hash, path_hash,
};

/// A history with no file behind it.
pub struct MemoryHistory {
    stamp: Stamp,
    builds: Vec<BuildRow>,
    paths: BTreeMap<WadHash, Life>,
    contents: BTreeMap<(WadHash, FileKey), Life>,
    objects: BTreeMap<ObjectKey, ObjectHit>,
    clips: BTreeMap<ClipKey, Life>,
}

fn life(builds: RangeInclusive<u16>) -> Life {
    Life::new(*builds.start(), *builds.end())
}

/// The hull of two lives.
fn hull(a: Life, b: Life) -> Life {
    Life {
        first: a.first.min(b.first),
        last: a.last.max(b.last),
    }
}

/// Widens the life at a key to hold another, as a key that came back keeps the hull.
fn widen<K: Ord>(lives: &mut BTreeMap<K, Life>, key: K, with: Life) {
    let held = lives.entry(key).or_insert(with);
    *held = hull(*held, with);
}

/// The key of a file with these bytes.
pub fn file_key(bytes: &[u8]) -> FileKey {
    FileKey::from_sha256(&ContentId::of(bytes).0)
}

impl MemoryHistory {
    /// A history of these builds, in the order they arrived. The commit of build `n` is `c<n>`.
    pub fn new(versions: &[&str]) -> Self {
        let build = |version: &str| {
            Build::parse(version).unwrap_or_else(|| panic!("{version:?} is no build of the game"))
        };
        let rows = (0u16..).zip(versions).map(|(at, version)| BuildRow {
            at: Position(at),
            commit: format!("c{at}"),
            build: build(version),
            manifest: "0000000000000000".into(),
            date: "2026-01-01".into(),
        });
        let builds: Vec<BuildRow> = rows.collect();
        let newest = builds.iter().map(|row| &row.build);
        let newest = newest.max_by(|a, b| version_order(a, b));
        let stamp = Stamp {
            format: FORMAT,
            repo: "memory".into(),
            head: builds
                .last()
                .map(|row| row.commit.clone())
                .unwrap_or_default(),
            newest_build: newest.cloned().unwrap_or_else(|| build("0.0.0")),
            builds: builds.len(),
            families: Families::ALL,
            writer: "memory".into(),
            rows: BTreeMap::new(),
            unparsed_texts: 0,
        };
        Self {
            stamp,
            builds,
            paths: BTreeMap::new(),
            contents: BTreeMap::new(),
            objects: BTreeMap::new(),
            clips: BTreeMap::new(),
        }
    }

    /// The game held some file at `path` at these builds.
    #[must_use]
    pub fn with_path(mut self, path: &str, builds: RangeInclusive<u16>) -> Self {
        widen(&mut self.paths, path_hash(path), life(builds));
        self
    }

    /// The game held `bytes` at `path` at these builds.
    #[must_use]
    pub fn with_content(mut self, path: &str, bytes: &[u8], builds: RangeInclusive<u16>) -> Self {
        let (path, key, life) = (path_hash(path), file_key(bytes), life(builds));
        widen(&mut self.paths, path, life);
        widen(&mut self.contents, (path, key), life);
        self
    }

    /// The game held the object `key` at these builds, first as `entry` of class `class` in
    /// the bin at `bin`.
    #[must_use]
    pub fn with_object(
        mut self,
        key: ObjectKey,
        norm: NormKey,
        (entry, class, bin): (&str, &str, &str),
        builds: RangeInclusive<u16>,
    ) -> Self {
        let hit = ObjectHit {
            life: life(builds),
            entry: name_hash(entry),
            class: name_hash(class),
            bin: path_hash(bin),
            norm,
        };
        let held = self.objects.entry(key).or_insert(hit);
        held.life = hull(held.life, hit.life);
        self
    }

    /// The game held the clip `key` at these builds.
    #[must_use]
    pub fn with_clip(mut self, key: ClipKey, builds: RangeInclusive<u16>) -> Self {
        widen(&mut self.clips, key, life(builds));
        self
    }
}

impl History for MemoryHistory {
    fn stamp(&self) -> Option<&Stamp> {
        Some(&self.stamp)
    }

    fn builds(&self) -> &[BuildRow] {
        &self.builds
    }

    fn file(&self, key: FileKey) -> Result<Option<FileHit>, HistoryError> {
        let held = self.paths_of(key)?;
        // The first path seen: the oldest, and among those the first in path order.
        let Some(first) = held.iter().min_by_key(|at| (at.life.first, at.path)) else {
            return Ok(None);
        };
        Ok(Some(FileHit {
            life: held.iter().fold(first.life, |all, at| hull(all, at.life)),
            path: first.path,
            kind: FileKind::of(""),
        }))
    }

    fn path(&self, path: WadHash) -> Result<Option<PathHit>, HistoryError> {
        let hit = |life: &Life| PathHit {
            life: *life,
            kind: FileKind::of(""),
        };
        Ok(self.paths.get(&path).map(hit))
    }

    fn held(&self, path: WadHash, key: FileKey) -> Result<Option<Life>, HistoryError> {
        Ok(self.contents.get(&(path, key)).copied())
    }

    fn paths_of(&self, key: FileKey) -> Result<Vec<HeldAt>, HistoryError> {
        let of_key = self.contents.iter().filter(|((_, k), _)| *k == key);
        Ok(of_key
            .map(|((path, _), life)| HeldAt {
                path: *path,
                life: *life,
            })
            .collect())
    }

    fn object(&self, key: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        Ok(self.objects.get(&key).copied())
    }

    fn objects_by_norm(&self, norm: NormKey) -> Result<Vec<ObjectHit>, HistoryError> {
        let of_norm = self.objects.values().filter(|hit| hit.norm == norm);
        let mut hits: Vec<ObjectHit> = of_norm.copied().collect();
        hits.sort_by_key(|hit| (hit.life.first, hit.bin, hit.entry));
        Ok(hits)
    }

    fn clip(&self, key: ClipKey) -> Result<Option<Life>, HistoryError> {
        Ok(self.clips.get(&key).copied())
    }

    fn named_by(&self, _: WadHash) -> Result<Vec<NamedBy>, HistoryError> {
        Ok(Vec::new())
    }

    fn units_loading(&self, _: WadHash) -> Result<Vec<UnitHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn event_wems(&self, _: WadHash, _: u32) -> Result<Vec<WemPlayed>, HistoryError> {
        Ok(Vec::new())
    }

    fn events_playing(&self, _: u32) -> Result<Vec<PlayedBy>, HistoryError> {
        Ok(Vec::new())
    }

    fn skeletons(&self, _: WadHash) -> Result<Vec<SkeletonHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn meshes(&self, _: WadHash) -> Result<Vec<MeshHit>, HistoryError> {
        Ok(Vec::new())
    }
}
