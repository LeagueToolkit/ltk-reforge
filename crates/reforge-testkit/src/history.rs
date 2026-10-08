//! A history in memory, for tests: built from facts (a path, a content in a WAD, an object, a
//! clip, an animation clip's joints, an events bank or a bank unit at builds a to b), with no
//! SQLite file behind it. It answers the questions about those. The families no stage reads yet
//! answer nothing.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use reforge::game::{Build, FieldType};
use reforge::history::{
    BinHit, BuildDetails, BuildRow, ClipJoints, EventsBank, EventsBankHit, FORMAT, Families,
    FileHit, FileKind, HeldAt, History, HistoryError, Life, MeshHit, NamedBy, ObjectBytes,
    ObjectCodec, ObjectHit, PathHit, Placed, PlayedBy, Position, Reached, SkeletonHit, Stamp,
    TextBlob, UnitHit, WemPlayed, version_order, wad_order,
};
use reforge::model::{
    BinHash, ClipKey, ContentId, FileKey, NormKey, ObjectKey, WadHash, name_hash, path_hash,
};

/// The WAD a history in memory holds a content in when no WAD is named.
pub const MEMORY_WAD: &str = "data/memory.wad.client";

/// A history with no file behind it.
pub struct MemoryHistory {
    stamp: Stamp,
    builds: Vec<BuildRow>,
    paths: BTreeMap<WadHash, Life>,
    contents: BTreeMap<(WadHash, FileKey), Life>,
    placements: BTreeMap<(WadHash, String, FileKey), Life>,
    objects: BTreeMap<(ObjectKey, BinHash), ObjectHit>,
    clips: BTreeMap<ClipKey, Life>,
    animations: BTreeMap<(WadHash, FileKey), (Life, Vec<u32>)>,
    banks: BTreeMap<(WadHash, FileKey), EventsBank>,
    units: BTreeMap<WadHash, Vec<UnitHit>>,
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
            details: BuildDetails::default(),
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
            objectless_texts: 0,
            objects: ObjectCodec::current(),
        };
        Self {
            stamp,
            builds,
            paths: BTreeMap::new(),
            contents: BTreeMap::new(),
            placements: BTreeMap::new(),
            objects: BTreeMap::new(),
            clips: BTreeMap::new(),
            animations: BTreeMap::new(),
            banks: BTreeMap::new(),
            units: BTreeMap::new(),
        }
    }

    /// The game held some file at `path` at these builds.
    #[must_use]
    pub fn with_path(mut self, path: &str, builds: RangeInclusive<u16>) -> Self {
        widen(&mut self.paths, path_hash(path), life(builds));
        self
    }

    /// The game held `bytes` at `path` at these builds, in [`MEMORY_WAD`].
    #[must_use]
    pub fn with_content(self, path: &str, bytes: &[u8], builds: RangeInclusive<u16>) -> Self {
        self.with_content_in(MEMORY_WAD, path, bytes, builds)
    }

    /// The WAD at `wad`, its path in the manifest, held `bytes` at `path` at these builds.
    #[must_use]
    pub fn with_content_in(
        mut self,
        wad: &str,
        path: &str,
        bytes: &[u8],
        builds: RangeInclusive<u16>,
    ) -> Self {
        let (path, key, life) = (path_hash(path), file_key(bytes), life(builds));
        widen(&mut self.paths, path, life);
        widen(&mut self.contents, (path, key), life);
        widen(&mut self.placements, (path, wad.to_string(), key), life);
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
        let held = self.objects.entry((key, hit.class)).or_insert(hit);
        held.life = hull(held.life, hit.life);
        self
    }

    /// The game held the clip `key` at these builds.
    #[must_use]
    pub fn with_clip(mut self, key: ClipKey, builds: RangeInclusive<u16>) -> Self {
        widen(&mut self.clips, key, life(builds));
        self
    }

    /// The game held the animation clip `bytes` at `path` at these builds, and it animates
    /// `joints`.
    #[must_use]
    pub fn with_animation(
        mut self,
        path: &str,
        bytes: &[u8],
        joints: &[u32],
        builds: RangeInclusive<u16>,
    ) -> Self {
        let (at, with) = ((path_hash(path), file_key(bytes)), life(builds.clone()));
        let held = self.animations.entry(at).or_insert((with, joints.to_vec()));
        held.0 = hull(held.0, with);
        self.with_content(path, bytes, builds)
    }

    /// The WAD at `wad` held the events bank `bank`, a file of `bytes`, at `path` at these
    /// builds.
    #[must_use]
    pub fn with_events_bank(
        mut self,
        wad: &str,
        path: &str,
        bytes: &[u8],
        bank: EventsBank,
        builds: RangeInclusive<u16>,
    ) -> Self {
        self.banks.insert((path_hash(path), file_key(bytes)), bank);
        self.with_content_in(wad, path, bytes, builds)
    }

    /// The game's bank unit `unit`, of the skin entry `entry` in the bin at `bin`, loaded the
    /// bank at `bank` at these builds.
    #[must_use]
    pub fn with_unit(
        mut self,
        bank: &str,
        (unit, entry, bin): (&str, &str, &str),
        voice: bool,
        builds: RangeInclusive<u16>,
    ) -> Self {
        let hit = UnitHit {
            bin: path_hash(bin),
            entry: name_hash(entry),
            unit: unit.to_string(),
            voice,
            life: life(builds),
        };
        let held = self.units.entry(path_hash(bank)).or_default();
        held.push(hit);
        held.sort_by_key(|hit| hit.life.first);
        self
    }

    /// Every content a WAD held at the path, in `wad_order` and then oldest first.
    fn placed_at(&self, path: WadHash) -> Vec<Placed> {
        let held = self.placements.iter().filter(|((p, ..), _)| *p == path);
        let mut placed: Vec<Placed> = held
            .map(|((_, wad, key), life)| Placed {
                wad: wad.clone(),
                key: *key,
                checksum: 0,
                life: *life,
            })
            .collect();
        placed.sort_by(|a, b| wad_order(&a.wad, &b.wad).then(a.life.first.cmp(&b.life.first)));
        placed
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

    fn placed(&self, path: WadHash, at: Position) -> Result<Vec<Placed>, HistoryError> {
        let mut placed = self.placed_at(path);
        placed.retain(|held| held.life.holds(at));
        Ok(placed)
    }

    fn bin(&self, _: WadHash, _: Position) -> Result<Option<BinHit>, HistoryError> {
        Ok(None)
    }

    fn object(&self, key: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        let lives = self.object_lives(key)?;
        Ok(lives.into_iter().max_by_key(|hit| hit.life.last))
    }

    fn object_lives(&self, key: ObjectKey) -> Result<Vec<ObjectHit>, HistoryError> {
        let of_key = self
            .objects
            .range((key, BinHash(0))..=(key, BinHash(u32::MAX)));
        let mut hits: Vec<ObjectHit> = of_key.map(|(_, hit)| *hit).collect();
        hits.sort_by_key(|hit| (hit.life.first, hit.class));
        Ok(hits)
    }

    fn objects_by_norm(&self, norm: NormKey) -> Result<Vec<ObjectHit>, HistoryError> {
        let of_norm = self.objects.values().filter(|hit| hit.norm == norm);
        let mut hits: Vec<ObjectHit> = of_norm.copied().collect();
        hits.sort_by_key(|hit| (hit.life.first, hit.bin, hit.entry));
        Ok(hits)
    }

    fn text_blob(
        &self,
        _: WadHash,
        _: BinHash,
        _: Position,
    ) -> Result<Option<TextBlob>, HistoryError> {
        Ok(None)
    }

    fn object_bytes(
        &self,
        _: WadHash,
        _: BinHash,
        _: Position,
    ) -> Result<Option<ObjectBytes>, HistoryError> {
        Ok(None)
    }

    fn links(&self, _: WadHash, _: BinHash, _: Position) -> Result<Vec<BinHash>, HistoryError> {
        Ok(Vec::new())
    }

    fn reached(&self, _: WadHash, _: BinHash, _: Position) -> Result<Reached, HistoryError> {
        Ok(Reached::default())
    }

    fn field_types(
        &self,
        _: BinHash,
        _: BinHash,
        _: Position,
    ) -> Result<Vec<FieldType>, HistoryError> {
        Ok(Vec::new())
    }

    fn clip(&self, key: ClipKey) -> Result<Option<Life>, HistoryError> {
        Ok(self.clips.get(&key).copied())
    }

    fn named_by(&self, _: WadHash) -> Result<Vec<NamedBy>, HistoryError> {
        Ok(Vec::new())
    }

    fn units_loading(&self, bank: WadHash) -> Result<Vec<UnitHit>, HistoryError> {
        Ok(self.units.get(&bank).cloned().unwrap_or_default())
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

    fn clip_joints(&self, path: WadHash) -> Result<Vec<ClipJoints>, HistoryError> {
        let at_path = self.animations.iter().filter(|((p, _), _)| *p == path);
        let hits = at_path.map(|((_, key), (life, joints))| ClipJoints {
            life: *life,
            key: *key,
            joints: joints.clone(),
        });
        let mut hits: Vec<ClipJoints> = hits.collect();
        hits.sort_by_key(|hit| (hit.life.first, hit.key));
        Ok(hits)
    }

    fn joints_of(&self, key: FileKey) -> Result<Option<Vec<u32>>, HistoryError> {
        let mut of_key = self.animations.iter().filter(|((_, k), _)| *k == key);
        Ok(of_key.next().map(|(_, (_, joints))| joints.clone()))
    }

    fn events_banks(&self, path: WadHash) -> Result<Vec<EventsBankHit>, HistoryError> {
        let placed = self.placed_at(path);
        let at_path = self.banks.iter().filter(|((p, _), _)| *p == path);
        let mut hits: Vec<EventsBankHit> = at_path
            .map(|((_, key), bank)| {
                let held = placed.iter().filter(|held| held.key == *key);
                EventsBankHit {
                    key: *key,
                    life: self.contents[&(path, *key)],
                    placed: held.cloned().collect(),
                    bank: bank.clone(),
                }
            })
            .collect();
        hits.sort_by_key(|hit| (hit.life.first, hit.key));
        Ok(hits)
    }
}

/// A history whose index fails at every question: a fault of the machine, for the tests of how
/// a stage ends on one. It holds the builds of the history it wraps.
pub struct BrokenHistory(pub MemoryHistory);

impl BrokenHistory {
    fn fault<T>() -> Result<T, HistoryError> {
        Err(HistoryError::Io(std::io::Error::other(
            "the index does not answer",
        )))
    }
}

impl History for BrokenHistory {
    fn stamp(&self) -> Option<&Stamp> {
        self.0.stamp()
    }

    fn builds(&self) -> &[BuildRow] {
        self.0.builds()
    }

    fn file(&self, _: FileKey) -> Result<Option<FileHit>, HistoryError> {
        Self::fault()
    }

    fn path(&self, _: WadHash) -> Result<Option<PathHit>, HistoryError> {
        Self::fault()
    }

    fn held(&self, _: WadHash, _: FileKey) -> Result<Option<Life>, HistoryError> {
        Self::fault()
    }

    fn paths_of(&self, _: FileKey) -> Result<Vec<HeldAt>, HistoryError> {
        Self::fault()
    }

    fn placed(&self, _: WadHash, _: Position) -> Result<Vec<Placed>, HistoryError> {
        Self::fault()
    }

    fn bin(&self, _: WadHash, _: Position) -> Result<Option<BinHit>, HistoryError> {
        Self::fault()
    }

    fn object(&self, _: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        Self::fault()
    }

    fn objects_by_norm(&self, _: NormKey) -> Result<Vec<ObjectHit>, HistoryError> {
        Self::fault()
    }

    fn object_lives(&self, _: ObjectKey) -> Result<Vec<ObjectHit>, HistoryError> {
        Self::fault()
    }

    fn text_blob(
        &self,
        _: WadHash,
        _: BinHash,
        _: Position,
    ) -> Result<Option<TextBlob>, HistoryError> {
        Self::fault()
    }

    fn object_bytes(
        &self,
        _: WadHash,
        _: BinHash,
        _: Position,
    ) -> Result<Option<ObjectBytes>, HistoryError> {
        Self::fault()
    }

    fn links(&self, _: WadHash, _: BinHash, _: Position) -> Result<Vec<BinHash>, HistoryError> {
        Self::fault()
    }

    fn reached(&self, _: WadHash, _: BinHash, _: Position) -> Result<Reached, HistoryError> {
        Self::fault()
    }

    fn field_types(
        &self,
        _: BinHash,
        _: BinHash,
        _: Position,
    ) -> Result<Vec<FieldType>, HistoryError> {
        Self::fault()
    }

    fn clip(&self, _: ClipKey) -> Result<Option<Life>, HistoryError> {
        Self::fault()
    }

    fn named_by(&self, _: WadHash) -> Result<Vec<NamedBy>, HistoryError> {
        Self::fault()
    }

    fn units_loading(&self, _: WadHash) -> Result<Vec<UnitHit>, HistoryError> {
        Self::fault()
    }

    fn event_wems(&self, _: WadHash, _: u32) -> Result<Vec<WemPlayed>, HistoryError> {
        Self::fault()
    }

    fn events_playing(&self, _: u32) -> Result<Vec<PlayedBy>, HistoryError> {
        Self::fault()
    }

    fn skeletons(&self, _: WadHash) -> Result<Vec<SkeletonHit>, HistoryError> {
        Self::fault()
    }

    fn meshes(&self, _: WadHash) -> Result<Vec<MeshHit>, HistoryError> {
        Self::fault()
    }

    fn clip_joints(&self, _: WadHash) -> Result<Vec<ClipJoints>, HistoryError> {
        Self::fault()
    }

    fn joints_of(&self, _: FileKey) -> Result<Option<Vec<u32>>, HistoryError> {
        Self::fault()
    }

    fn events_banks(&self, _: WadHash) -> Result<Vec<EventsBankHit>, HistoryError> {
        Self::fault()
    }
}
