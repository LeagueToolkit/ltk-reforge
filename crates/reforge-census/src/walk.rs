//! The walk over the history: every commit's changed files, oldest build first, folded into the
//! lives of each key: one hull per key, or of a table of spans one row per unbroken life.
//!
//! The walk reads each commit's diff against its parent, never whole trees: a build changes a
//! few thousand entries out of a million, and the union of every record ever added is the set.
//!
//! Entry texts are most of the bytes a build changes: 3.5 and 3.9 GB, both sides, for the first
//! builds of 12.20 and 12.21, against 63 and 73 MB of entries' files (2026-10-03). What a text
//! names is a function of its blob, so it is read and parsed once, when the blob first appears,
//! and kept while the tree holds it: the text a build removes or replaces is not read again. Its
//! object is kept until the object bytes are written (`objects.rs`).

use std::collections::hash_map::Entry as MapEntry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rayon::prelude::*;
use reforge::history::tables::{
    Animation, Animations, At, BankAt, BankEvent, BankFacts, BankObjects, BinEntry, BinFacts, Bins,
    ClassedObject, Clips, EntryText, EntryTexts, EventBanks, EventWem, EventWems, Events,
    FileFacts, Files, JointLists, Linking, Links, Meshes, Naming, ObjectFacts, Objects, Placement,
    Placements, Recorded, Refs, Row, Shape, Skeletons, Table, Tag, Tags, TextSchema, Textures,
    Unit, UnitBank, UnitBanks, UnitEvent, UnitEvents, Units, WadLife, WadLives,
};
use reforge::history::{BuildRow, EventsBank, Families, Family, FileKind, Life, Position};
use reforge::model::{BinHash, FileKey, WadHash};
use reforge_wwise::Object;

use crate::error::CensusError;
use crate::format::{self, BuildFile, Entry, ObjectText, WadFile};
use crate::objects::{Fresh, Store};

pub use reforge::history::BlobId;

/// One changed file between a commit and its parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The file's path, from the tree's root.
    pub path: String,
    /// Its blob in the parent.
    pub old: Option<BlobId>,
    /// Its blob in the commit.
    pub new: Option<BlobId>,
}

/// Where the walk reads the history from: the repository through gix, or memory in tests.
pub trait ChangeSource: Sync {
    /// Commit ids on the first-parent chain, oldest first.
    fn commits(&self) -> Result<Vec<String>, CensusError>;

    /// The commit's `build.yaml`.
    fn build(&self, commit: &str) -> Result<BuildFile, CensusError>;

    /// Every file under `files/` that differs between `parent` (none for the first commit) and
    /// `commit`, in path order. A bin's entry texts only when `texts`.
    fn changes(
        &self,
        commit: &str,
        parent: Option<&str>,
        texts: bool,
    ) -> Result<Vec<Change>, CensusError>;

    /// A blob's bytes. The walk reads from every core at once.
    fn read(&self, blob: &BlobId) -> Result<Vec<u8>, CensusError>;
}

/// A key's builds so far and its record count. In an append the count starts at zero and ends
/// as the commit's net change, which is negative for a key only removed.
#[derive(Debug, Clone, Copy)]
struct Tally {
    first: Position,
    last: Position,
    alive: i64,
    /// The build at which the count fell to zero, while it is zero.
    gone: Option<Position>,
}

impl Tally {
    fn new(at: Position) -> Self {
        Self {
            first: at,
            last: at,
            alive: 0,
            gone: None,
        }
    }

    /// Applies a change of the record count at a build. Of a table of spans, a key that comes
    /// back at a later build than it went starts a new life, and the life it ended is returned.
    /// A key that goes and comes back within one build keeps its life: an append sees only the
    /// build's net change.
    fn apply(&mut self, delta: i64, at: Position, spans: bool) -> Option<Life> {
        let before = self.alive;
        self.alive += delta;
        if before > 0 && self.alive <= 0 {
            self.last = Position(at.0.saturating_sub(1)).max(self.first);
            self.gone = Some(at);
        } else if before <= 0
            && self.alive > 0
            && self.gone.take().is_some_and(|gone| spans && gone != at)
        {
            let ended = Life {
                first: self.first,
                last: self.last,
            };
            self.first = at;
            return Some(ended);
        }
        None
    }

    fn life(&self, head: Position) -> Life {
        Life {
            first: self.first,
            last: if self.alive > 0 { head } else { self.last },
        }
    }
}

/// A key's lives in one table: the one it is in, and of a table of spans the ones before.
struct Lives<F> {
    tally: Tally,
    /// The facts of the record that started the life the key is in.
    facts: F,
    /// The lives that ended, each with the facts it started with.
    past: Vec<(Life, F)>,
}

/// One table's lives so far: per key, its builds, its record count, and the facts of the
/// record that started it.
pub struct Fold<T: Table> {
    lives: HashMap<T::Key, Lives<T::Facts>>,
}

impl<T: Table> Default for Fold<T> {
    fn default() -> Self {
        Self {
            lives: HashMap::new(),
        }
    }
}

impl<T: Table> Fold<T> {
    /// Applies the difference between the records the old and the new side of a change hold.
    /// A key seen first keeps the facts of its first record.
    fn change<'x, X: ?Sized, I>(
        &mut self,
        at: Position,
        old: Option<&'x X>,
        new: Option<&'x X>,
        records: impl Fn(&'x X) -> I,
    ) where
        I: IntoIterator<Item = (T::Key, T::Facts)>,
    {
        let mut delta: HashMap<T::Key, (i64, T::Facts)> = HashMap::new();
        for (sign, side) in [(-1, old), (1, new)] {
            for (key, facts) in side.into_iter().flat_map(&records) {
                delta.entry(key).or_insert((0, facts)).0 += sign;
            }
        }
        let spans = T::DEF.shape == Shape::Spans;
        for (key, (net, facts)) in delta {
            if net == 0 {
                continue;
            }
            match self.lives.entry(key) {
                MapEntry::Vacant(slot) => {
                    let mut tally = Tally::new(at);
                    tally.apply(net, at, spans);
                    slot.insert(Lives {
                        tally,
                        facts,
                        past: Vec::new(),
                    });
                }
                MapEntry::Occupied(slot) => {
                    let lives = slot.into_mut();
                    if let Some(ended) = lives.tally.apply(net, at, spans) {
                        let started = std::mem::replace(&mut lives.facts, facts);
                        lives.past.push((ended, started));
                    }
                }
            }
        }
    }

    /// How many keys the fold holds.
    pub fn len(&self) -> usize {
        self.lives.len()
    }

    /// Whether the fold holds no key.
    pub fn is_empty(&self) -> bool {
        self.lives.is_empty()
    }

    /// A key's life, the last one of a table of spans, for a history whose head is `head`.
    pub fn life(&self, key: &T::Key, head: Position) -> Option<Life> {
        self.lives.get(key).map(|lives| lives.tally.life(head))
    }

    /// The table's rows for a history whose head is `head`, in key order, a key's lives oldest
    /// first.
    pub fn rows(self, head: Position) -> Vec<Row<T>> {
        let mut rows: Vec<Row<T>> = Vec::with_capacity(self.lives.len());
        for (key, lives) in self.lives {
            for (life, facts) in lives.past {
                let key = key.clone();
                rows.push(Row {
                    key,
                    facts,
                    life,
                    alive: 0,
                });
            }
            let tally = lives.tally;
            rows.push(Row {
                key,
                facts: lives.facts,
                life: tally.life(head),
                alive: u32::try_from(tally.alive.max(0)).unwrap_or(u32::MAX),
            });
        }
        rows.sort_unstable_by(|a, b| a.key.cmp(&b.key).then(a.life.first.cmp(&b.life.first)));
        rows
    }

    /// Each key whose record count one commit changes, with the net change, in key order so
    /// that the file does not depend on hashing.
    pub(crate) fn net(self) -> Vec<(T::Key, T::Facts, i64)> {
        let changed = self.lives.into_iter().filter(|(_, l)| l.tally.alive != 0);
        let mut net: Vec<_> = changed.map(|(k, l)| (k, l.facts, l.tally.alive)).collect();
        net.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        net
    }
}

impl<T: Table<Facts = ()>> Fold<T> {
    /// `change` for a table whose rows keep nothing but their key.
    fn change_keys<'x, X: ?Sized, I>(
        &mut self,
        at: Position,
        old: Option<&'x X>,
        new: Option<&'x X>,
        keys: impl Fn(&'x X) -> I,
    ) where
        I: IntoIterator<Item = T::Key>,
    {
        self.change(at, old, new, |side| {
            keys(side).into_iter().map(|key| (key, ()))
        });
    }
}

/// What is done with each table's fold once the commits are folded.
pub(crate) trait Visit {
    /// Takes one table's fold.
    fn table<T: Table>(&mut self, fold: Fold<T>) -> Result<(), CensusError>;
}

/// One fold per table of the index.
#[derive(Default)]
pub struct Folds {
    /// Every content.
    pub files: Fold<Files>,
    /// Every content at every path of every WAD.
    pub placements: Fold<Placements>,
    /// Every WAD.
    pub wad_lives: Fold<WadLives>,
    /// Every object.
    pub objects: Fold<Objects>,
    /// The blob of every bin entry's text in every WAD.
    pub entry_texts: Fold<EntryTexts>,
    /// Every bin, with its links and its objects.
    pub bins: Fold<Bins>,
    /// Every type of every field of every class.
    pub schema: Fold<TextSchema>,
    /// Every object an object links.
    pub links: Fold<Links>,
    /// Every path an object names.
    pub refs: Fold<Refs>,
    /// Every audio tag of an object.
    pub tags: Fold<Tags>,
    /// Every bank unit of an object.
    pub units: Fold<Units>,
    /// Every bank a unit loads.
    pub unit_banks: Fold<UnitBanks>,
    /// Every event a unit lists.
    pub unit_events: Fold<UnitEvents>,
    /// Every clip.
    pub clips: Fold<Clips>,
    /// Every event of an events bank.
    pub events: Fold<Events>,
    /// Every wem an event plays.
    pub event_wems: Fold<EventWems>,
    /// Every object of an events bank.
    pub bank_objects: Fold<BankObjects>,
    /// Every events bank at a path.
    pub event_banks: Fold<EventBanks>,
    /// Every skeleton at a path.
    pub skeletons: Fold<Skeletons>,
    /// Every mesh at a path.
    pub meshes: Fold<Meshes>,
    /// Every list of joints an animation clip animates.
    pub joint_lists: Fold<JointLists>,
    /// Every animation clip at a path.
    pub animations: Fold<Animations>,
    /// Every texture at a path.
    pub textures: Fold<Textures>,
}

impl Folds {
    /// Hands each table's fold to `visit`, in the order of `reforge::history::tables::TABLES`.
    pub(crate) fn each(self, visit: &mut impl Visit) -> Result<(), CensusError> {
        visit.table(self.files)?;
        visit.table(self.placements)?;
        visit.table(self.wad_lives)?;
        visit.table(self.objects)?;
        visit.table(self.entry_texts)?;
        visit.table(self.bins)?;
        visit.table(self.schema)?;
        visit.table(self.links)?;
        visit.table(self.refs)?;
        visit.table(self.tags)?;
        visit.table(self.units)?;
        visit.table(self.unit_banks)?;
        visit.table(self.unit_events)?;
        visit.table(self.clips)?;
        visit.table(self.events)?;
        visit.table(self.event_wems)?;
        visit.table(self.bank_objects)?;
        visit.table(self.event_banks)?;
        visit.table(self.skeletons)?;
        visit.table(self.meshes)?;
        visit.table(self.joint_lists)?;
        visit.table(self.animations)?;
        visit.table(self.textures)
    }
}

/// What an entry text names. None when it does not parse.
type TextFacts = Arc<Option<ObjectText>>;

/// One side of a text change: the blob and what it names.
type TextSide = Option<(BlobId, TextFacts)>;

/// The rows an events bank makes: its own, and one per object. An object is shared with the
/// fold that holds it, so a bank the walk has seen costs its records only.
struct BankRows {
    record: u64,
    facts: BankFacts,
    objects: Vec<(u64, Arc<Object>)>,
}

impl BankRows {
    fn of(bank: EventsBank) -> Self {
        let objects = bank.objects.into_iter();
        let objects: Vec<(u64, Arc<Object>)> = objects.map(|o| (o.digest(), Arc::new(o))).collect();
        let records: Vec<u64> = objects.iter().map(|(record, _)| *record).collect();
        let (record, facts) = BankFacts::of(&bank.header, &records);
        Self {
            record,
            facts,
            objects,
        }
    }
}

/// One side of an entry's change: the file read, its events bank taken out as rows.
struct EntrySide {
    entry: Entry,
    bank: Option<BankRows>,
    /// A bin's row: its links and its objects.
    bin: Option<Arc<BinFacts>>,
}

impl EntrySide {
    /// The key and the facts of the side's row of `bins`, when it is a bin.
    fn bin(&self) -> Option<(FileKey, &Arc<BinFacts>)> {
        let facts = self.bin.as_ref()?;
        Some((self.entry.content, facts))
    }
}

/// A bin's row of `bins`, of an entry the census read as a bin.
fn bin_facts(entry: &Entry) -> Option<Arc<BinFacts>> {
    let links = entry.links.as_ref()?;
    let objects = entry.objects.iter().map(|o| BinEntry {
        entry: o.entry,
        class: o.class,
        object: o.object,
        norm: o.norm,
    });
    let objects: Vec<BinEntry> = objects.collect();
    Some(Arc::new(BinFacts::of(links, &objects)))
}

/// A change read, in the WAD whose directory holds it: an entry's file before and after, a bin
/// entry's text, or the WAD's own file.
enum Parsed<'c> {
    Entry {
        wad: &'c str,
        path: WadHash,
        old: Option<Box<EntrySide>>,
        new: Option<Box<EntrySide>>,
    },
    Text {
        wad: &'c str,
        bin: WadHash,
        entry: BinHash,
        old: TextSide,
        new: TextSide,
        /// What the new side holds, when it was read here for the first time.
        fresh: Option<Fresh>,
    },
    Wad {
        wad: &'c str,
        old: Option<WadFile>,
        new: Option<WadFile>,
    },
}

/// What one side of a text change names. None for a side absent or a text that does not parse.
fn named(side: &TextSide) -> Option<&ObjectText> {
    side.as_ref().and_then(|(_, facts)| facts.as_ref().as_ref())
}

/// Changes read and folded at a time: enough to keep every core parsing, few enough that a
/// build's first commit (every file added) is not held whole.
const BATCH: usize = 8192;

/// The commits folded so far.
pub(crate) struct Walker {
    families: Families,
    pub(crate) folds: Folds,
    /// Entry texts that appeared and did not parse, so name nothing.
    pub(crate) unparsed_texts: usize,
    /// The object of each text read, kept until it is written.
    pub(crate) objects: Store,
    /// What each entry text the tree holds names, and how many places hold it.
    texts: HashMap<BlobId, (u32, TextFacts)>,
    /// Each WAD's path, held once for every row that names the WAD.
    wads: HashSet<Arc<str>>,
}

impl Walker {
    pub(crate) fn new(families: Families) -> Self {
        Self {
            families,
            folds: Folds::default(),
            unparsed_texts: 0,
            objects: Store::default(),
            texts: HashMap::new(),
            wads: HashSet::new(),
        }
    }

    /// The WAD's path, held once.
    fn wad(&mut self, path: &str) -> Arc<str> {
        if let Some(held) = self.wads.get(path) {
            return held.clone();
        }
        let held: Arc<str> = path.into();
        self.wads.insert(held.clone());
        held
    }

    /// Folds one commit's changes against its parent in at position `at`, and returns its
    /// build.
    pub(crate) fn commit(
        &mut self,
        source: &dyn ChangeSource,
        commit: &str,
        parent: Option<&str>,
        at: Position,
    ) -> Result<BuildFile, CensusError> {
        let build = source.build(commit)?;
        let texts = self.families.has(Family::Objects);
        let changes = source.changes(commit, parent, texts)?;
        tracing::debug!(
            at = at.0,
            commit,
            changes = changes.len(),
            "folding a build"
        );
        for batch in changes.chunks(BATCH) {
            self.apply(source, at, batch)?;
        }
        Ok(build)
    }

    /// Reads and parses one change of the build at `at`, a text from the texts held when the
    /// tree already holds it.
    fn parse<'c>(
        &self,
        source: &dyn ChangeSource,
        at: Position,
        change: &'c Change,
    ) -> Result<Option<Parsed<'c>>, CensusError> {
        let read = |id: &BlobId| -> Result<String, CensusError> {
            Ok(String::from_utf8_lossy(&source.read(id)?).into_owned())
        };
        let Some(wad) = format::wad_of(&change.path) else {
            return Ok(None);
        };
        if format::is_wad_file(&change.path) {
            if !self.families.has(Family::Files) {
                return Ok(None);
            }
            let side = |id: &Option<BlobId>| -> Result<Option<WadFile>, CensusError> {
                let text = id.as_ref().map(read).transpose()?;
                Ok(text.map(|text| format::parse_wad(&text)))
            };
            return Ok(Some(Parsed::Wad {
                wad,
                old: side(&change.old)?,
                new: side(&change.new)?,
            }));
        }
        if let Some(path) = format::entry_file(&change.path) {
            let banks = self.families.has(Family::Clips);
            let bins = self.families.has(Family::Objects);
            let entry = |id: &Option<BlobId>| -> Result<Option<Box<EntrySide>>, CensusError> {
                let text = id.as_ref().map(read).transpose()?;
                let side = text.and_then(|t| format::parse_entry(&t)).map(|mut entry| {
                    // Digested here, on every core, and not where the folds take the rows.
                    let bank = entry.bank.take().filter(|_| banks).map(BankRows::of);
                    let bin = bin_facts(&entry).filter(|_| bins);
                    Box::new(EntrySide { entry, bank, bin })
                });
                Ok(side)
            };
            return Ok(Some(Parsed::Entry {
                wad,
                path,
                old: entry(&change.old)?,
                new: entry(&change.new)?,
            }));
        }
        let Some((bin, entry)) = format::object_file(&change.path) else {
            return Ok(None);
        };
        let old = match &change.old {
            Some(id) => match self.texts.get(id) {
                Some((_, facts)) => Some((*id, facts.clone())),
                None => Some((*id, Arc::new(format::parse_object_text(&read(id)?)))),
            },
            None => None,
        };
        // A new side the tree does not hold is read whole: what it names, and its object.
        let (new, fresh) = match &change.new {
            Some(id) => match self.texts.get(id) {
                Some((_, facts)) => (Some((*id, facts.clone())), None),
                None => {
                    let read = format::read_object_text(&read(id)?);
                    let (facts, fresh) = match read {
                        Some((names, objects)) => {
                            (Some(names), Some(Fresh::of(entry, at, objects)))
                        }
                        None => (None, None),
                    };
                    (Some((*id, Arc::new(facts))), fresh)
                }
            },
            None => (None, None),
        };
        Ok(Some(Parsed::Text {
            wad,
            bin,
            entry,
            old,
            new,
            fresh,
        }))
    }

    /// Reads a batch of one commit's changes in parallel and folds them in path order.
    fn apply(
        &mut self,
        source: &dyn ChangeSource,
        at: Position,
        changes: &[Change],
    ) -> Result<(), CensusError> {
        let this = &*self;
        let parsed = changes.par_iter().map(|c| this.parse(source, at, c));
        let parsed: Vec<Option<Parsed>> = parsed.collect::<Result<_, _>>()?;
        for change in parsed.into_iter().flatten() {
            match change {
                Parsed::Entry {
                    wad,
                    path,
                    old,
                    new,
                } => {
                    let wad = self.wad(wad);
                    self.entry(at, &wad, path, old.as_deref(), new.as_deref());
                }
                Parsed::Text {
                    wad,
                    bin,
                    entry,
                    old,
                    new,
                    fresh,
                } => {
                    let (old, new) = (&old, &new);
                    self.hold(old, new);
                    if let (Some((blob, _)), Some(fresh)) = (new, fresh) {
                        self.objects.add(*blob, entry, fresh);
                    }
                    let wad = self.wad(wad);
                    let texts = &mut self.folds.entry_texts;
                    texts.change_keys(at, old.as_ref(), new.as_ref(), |(blob, _)| {
                        let wad = wad.clone();
                        let blob = *blob;
                        [EntryText {
                            bin,
                            entry,
                            wad,
                            blob,
                        }]
                    });
                    self.text(at, bin, entry, named(old), named(new));
                }
                Parsed::Wad { wad, old, new } => {
                    let wad = self.wad(wad);
                    self.wad_file(at, &wad, old.as_ref(), new.as_ref());
                }
            }
        }
        Ok(())
    }

    fn wad_file(
        &mut self,
        at: Position,
        wad: &Arc<str>,
        old: Option<&WadFile>,
        new: Option<&WadFile>,
    ) {
        self.folds.wad_lives.change_keys(at, old, new, |file| {
            [WadLife {
                wad: wad.clone(),
                version: file.version.clone(),
                file_id: file.file_id,
                tags: file.tags.join(","),
            }]
        });
    }

    /// Keeps what a text that appears names, and drops what a text no place holds any more
    /// names.
    fn hold(&mut self, old: &TextSide, new: &TextSide) {
        if let Some((id, facts)) = new {
            // A text that does not parse names nothing. It is counted where it appears.
            self.unparsed_texts += usize::from(facts.is_none());
            let held = self.texts.entry(*id).or_insert_with(|| (0, facts.clone()));
            held.0 += 1;
        }
        if let Some((id, _)) = old
            && let Some(held) = self.texts.get_mut(id)
        {
            held.0 -= 1;
            if held.0 == 0 {
                self.texts.remove(id);
            }
        }
    }

    fn entry(
        &mut self,
        at: Position,
        wad: &Arc<str>,
        path: WadHash,
        old: Option<&EntrySide>,
        new: Option<&EntrySide>,
    ) {
        let folds = &mut self.folds;
        let old_bank = old.and_then(|side| side.bank.as_ref());
        let new_bank = new.and_then(|side| side.bank.as_ref());
        let (old_bin, new_bin) = (old.and_then(EntrySide::bin), new.and_then(EntrySide::bin));
        let (old, new) = (old.map(|side| &side.entry), new.map(|side| &side.entry));
        if self.families.has(Family::Files) {
            folds.files.change(at, old, new, |e| {
                let facts = FileFacts {
                    kind: FileKind::of(&e.kind),
                    path,
                };
                [(e.content, facts)]
            });
            folds.placements.change_keys(at, old, new, |e| {
                [Placement {
                    path,
                    wad: wad.clone(),
                    key: e.content,
                    checksum: e.checksum,
                }]
            });
        }
        if self.families.has(Family::Objects) {
            folds.objects.change(at, old, new, |e| {
                e.objects.iter().map(|o| {
                    let key = ClassedObject {
                        key: o.object,
                        class: o.class,
                    };
                    let facts = ObjectFacts {
                        entry: o.entry,
                        bin: path,
                        norm: o.norm,
                    };
                    (key, facts)
                })
            });
            let (old_bin, new_bin) = (old_bin.as_ref(), new_bin.as_ref());
            folds.bins.change(at, old_bin, new_bin, |(key, facts)| {
                [(*key, Arc::clone(facts))]
            });
        }
        if self.families.has(Family::Clips) {
            let clips = |e: &Entry| e.clips.iter().map(|c| c.clip).collect::<Vec<_>>();
            folds.clips.change_keys(at, old, new, clips);
            folds.events.change_keys(at, old, new, |e| {
                let events = e.events.iter();
                events.map(|event| BankEvent {
                    bank: path,
                    event: event.id,
                })
            });
            folds.event_wems.change_keys(at, old, new, |e| {
                e.events.iter().flat_map(|event| {
                    event.wems.iter().map(|wem| EventWem {
                        bank: path,
                        event: event.id,
                        wem: *wem,
                    })
                })
            });
            folds.bank_objects.change(at, old_bank, new_bank, |b| {
                let objects = b.objects.iter();
                objects.map(|(record, object)| (*record, object.clone()))
            });
            folds.event_banks.change(at, old_bank, new_bank, |b| {
                let record = b.record;
                [(BankAt { path, record }, b.facts.clone())]
            });
        }
        if self.families.has(Family::Rigs) {
            // A rig is keyed by its path and its whole record, so a changed rig is a new life.
            folds.skeletons.change_keys(at, old, new, |e| {
                let record = e.skeleton.clone();
                record.map(|record| At { path, record })
            });
            folds.meshes.change_keys(at, old, new, |e| {
                let record = e.mesh.clone();
                record.map(|record| At { path, record })
            });
            folds
                .joint_lists
                .change_keys(at, old, new, |e| e.joints.clone());
            folds.animations.change(at, old, new, |e| {
                let key = e.content;
                let joints = e.joints.as_ref();
                joints.map(|joints| (Animation { path, key }, joints.digest()))
            });
        }
        if self.families.has(Family::Textures) {
            folds.textures.change_keys(at, old, new, |e| {
                let record = e.texture.clone();
                record.map(|record| At { path, record })
            });
        }
    }

    fn text(
        &mut self,
        at: Position,
        bin: WadHash,
        entry: BinHash,
        old: Option<&ObjectText>,
        new: Option<&ObjectText>,
    ) {
        let folds = &mut self.folds;
        folds
            .schema
            .change_keys(at, old, new, |t| t.schema.iter().copied());
        folds.links.change_keys(at, old, new, |t| {
            let links = t.links.iter();
            links.map(|linked| Linking {
                bin,
                entry,
                linked: *linked,
            })
        });
        folds.refs.change_keys(at, old, new, |t| {
            let paths = t.paths.iter();
            paths.map(|path| Naming {
                path: *path,
                bin,
                entry,
            })
        });
        folds.tags.change_keys(at, old, new, |t| {
            let tags = t.tags.iter().cloned();
            tags.map(|tag| Tag { bin, entry, tag })
        });
        folds.units.change_keys(at, old, new, |t| {
            t.units.iter().map(|u| Unit {
                bin,
                entry,
                unit: u.name.clone(),
                voice: u.voice,
            })
        });
        folds.unit_banks.change_keys(at, old, new, |t| {
            t.units.iter().flat_map(|u| {
                u.banks.iter().map(|bank| UnitBank {
                    bin,
                    entry,
                    unit: u.name.clone(),
                    bank: *bank,
                })
            })
        });
        folds.unit_events.change_keys(at, old, new, |t| {
            t.units.iter().flat_map(|u| {
                u.events.iter().map(|event| UnitEvent {
                    bin,
                    entry,
                    unit: u.name.clone(),
                    event: *event,
                })
            })
        });
    }
}

/// How far a walk or an append has come.
#[derive(Debug, Clone, Copy)]
pub struct Progress<'a> {
    /// How many builds are folded.
    pub done: usize,
    /// How many builds the history holds.
    pub of: usize,
    /// The build folded last.
    pub build: &'a BuildRow,
}

/// The position of a commit of the history.
pub(crate) fn position(at: usize) -> Result<Position, CensusError> {
    let numbered = u16::try_from(at).ok().filter(|at| *at < u16::MAX);
    numbered
        .map(Position)
        .ok_or(CensusError::TooManyBuilds(at + 1))
}

/// A commit's build as a row of the index.
pub(crate) fn build_row(at: Position, commit: &str, file: BuildFile) -> BuildRow {
    BuildRow {
        at,
        commit: commit.to_string(),
        build: file.build,
        manifest: file.manifest,
        date: file.date,
        details: file.details,
    }
}

/// The folded history.
pub struct Walked {
    /// The builds, by position.
    pub builds: Vec<BuildRow>,
    /// One fold per table.
    pub folds: Folds,
    /// Entry texts read that did not parse, so name nothing.
    pub unparsed_texts: usize,
    /// The object of each text read.
    pub(crate) objects: Store,
}

impl Walked {
    /// The head: the position of the build walked last.
    pub fn head(&self) -> Position {
        self.builds.last().map_or(Position(0), |row| row.at)
    }
}

/// Walks every commit of `source` and folds the files of `families` into lives.
pub fn walk(
    source: &dyn ChangeSource,
    families: Families,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<Walked, CensusError> {
    let commits = source.commits()?;
    if commits.is_empty() {
        return Err(CensusError::Empty);
    }
    if commits.len() > usize::from(u16::MAX) {
        return Err(CensusError::TooManyBuilds(commits.len()));
    }
    let mut walker = Walker::new(families);
    let mut builds = Vec::with_capacity(commits.len());
    for (i, commit) in commits.iter().enumerate() {
        let at = position(i)?;
        let parent = i.checked_sub(1).map(|p| commits[p].as_str());
        let build = walker.commit(source, commit, parent, at)?;
        builds.push(build_row(at, commit, build));
        progress(Progress {
            done: i + 1,
            of: commits.len(),
            build: &builds[i],
        });
    }
    Ok(Walked {
        builds,
        folds: walker.folds,
        unparsed_texts: walker.unparsed_texts,
        objects: walker.objects,
    })
}

#[cfg(test)]
mod tests {
    use reforge::history::tables::TABLES;

    use super::*;

    struct Names(Vec<&'static str>);

    impl Visit for Names {
        fn table<T: Table>(&mut self, _: Fold<T>) -> Result<(), CensusError> {
            self.0.push(T::DEF.name);
            Ok(())
        }
    }

    #[test]
    fn every_table_of_the_index_has_a_fold_in_the_index_order() {
        let mut names = Names(Vec::new());
        Folds::default().each(&mut names).unwrap();
        let tables: Vec<&str> = TABLES.iter().map(|table| table.name).collect();
        assert_eq!(names.0, tables);
    }

    #[test]
    fn a_life_ends_at_the_build_before_its_count_falls_to_zero() {
        let mut tally = Tally::new(Position(3));
        tally.apply(2, Position(3), false);
        tally.apply(-1, Position(5), false);
        assert_eq!(tally.life(Position(9)), Life::new(3, 9));
        tally.apply(-1, Position(7), false);
        assert_eq!(tally.life(Position(9)), Life::new(3, 6));
        assert_eq!(tally.apply(1, Position(8), false), None);
        assert_eq!(tally.life(Position(9)), Life::new(3, 9));
    }

    #[test]
    fn a_span_that_comes_back_at_a_later_build_is_a_new_life() {
        let mut tally = Tally::new(Position(3));
        tally.apply(1, Position(3), true);
        tally.apply(-1, Position(7), true);
        assert_eq!(tally.apply(1, Position(8), true), Some(Life::new(3, 6)));
        assert_eq!(tally.life(Position(9)), Life::new(8, 9));
        // Gone and back within one build: the build's net change is none.
        tally.apply(-1, Position(9), true);
        assert_eq!(tally.apply(1, Position(9), true), None);
        assert_eq!(tally.life(Position(9)), Life::new(8, 9));
    }
}
