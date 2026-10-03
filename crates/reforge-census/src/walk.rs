//! The walk over the history: every commit's changed files, oldest build first, folded into one
//! life per key.
//!
//! The walk reads each commit's diff against its parent, never whole trees: a build changes a
//! few thousand entries out of a million, and the union of every record ever added is the set.
//!
//! Entry texts are most of the bytes a build changes: 3.5 and 3.9 GB, both sides, for the first
//! builds of 12.20 and 12.21, against 63 and 73 MB of entries' files (2026-10-03). What a text
//! names is a function of its blob, so it is read and parsed once, when the blob first appears,
//! and kept while the tree holds it: the text a build removes or replaces is not read again.

use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;
use reforge::history::tables::{
    At, BankEvent, Clips, Contents, EventWem, EventWems, Events, FileFacts, Files, Held, Meshes,
    Naming, ObjectFacts, Objects, Paths, Refs, Row, Skeletons, Table, Tag, Tags, Textures, Unit,
    UnitBank, UnitBanks, UnitEvent, UnitEvents, Units,
};
use reforge::history::{BuildRow, Families, Family, FileKind, Life, Position};
use reforge::model::{BinHash, WadHash};

use crate::error::CensusError;
use crate::format::{self, BuildFile, Entry, ObjectText};

/// A blob's id in the history: the same id is the same bytes.
pub type BlobId = [u8; 20];

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
}

impl Tally {
    fn new(at: Position) -> Self {
        Self {
            first: at,
            last: at,
            alive: 0,
        }
    }

    fn apply(&mut self, delta: i64, at: Position) {
        let before = self.alive;
        self.alive += delta;
        if before > 0 && self.alive <= 0 {
            self.last = Position(at.0.saturating_sub(1)).max(self.first);
        }
    }

    fn life(&self, head: Position) -> Life {
        Life {
            first: self.first,
            last: if self.alive > 0 { head } else { self.last },
        }
    }
}

/// One table's lives so far: per key, its builds, its record count, and the facts of the
/// record that started it.
pub struct Fold<T: Table> {
    lives: HashMap<T::Key, (Tally, T::Facts)>,
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
        for (key, (net, facts)) in delta {
            if net != 0 {
                let life = self.lives.entry(key).or_insert((Tally::new(at), facts));
                life.0.apply(net, at);
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

    /// A key's life, for a history whose head is `head`.
    pub fn life(&self, key: &T::Key, head: Position) -> Option<Life> {
        self.lives.get(key).map(|(tally, _)| tally.life(head))
    }

    /// The table's rows for a history whose head is `head`, in key order.
    pub fn rows(self, head: Position) -> Vec<Row<T>> {
        let rows = self.lives.into_iter().map(|(key, (tally, facts))| Row {
            key,
            facts,
            life: tally.life(head),
            alive: u32::try_from(tally.alive.max(0)).unwrap_or(u32::MAX),
        });
        let mut rows: Vec<Row<T>> = rows.collect();
        rows.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        rows
    }

    /// Each key whose record count one commit changes, with the net change, in key order so
    /// that the file does not depend on hashing.
    pub(crate) fn net(self) -> Vec<(T::Key, T::Facts, i64)> {
        let changed = self.lives.into_iter().filter(|(_, (t, _))| t.alive != 0);
        let mut net: Vec<_> = changed.map(|(k, (t, f))| (k, f, t.alive)).collect();
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
    /// Every path.
    pub paths: Fold<Paths>,
    /// Every content at every path.
    pub contents: Fold<Contents>,
    /// Every object.
    pub objects: Fold<Objects>,
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
    /// Every skeleton at a path.
    pub skeletons: Fold<Skeletons>,
    /// Every mesh at a path.
    pub meshes: Fold<Meshes>,
    /// Every texture at a path.
    pub textures: Fold<Textures>,
}

impl Folds {
    /// Hands each table's fold to `visit`, in the order of `reforge::history::tables::TABLES`.
    pub(crate) fn each(self, visit: &mut impl Visit) -> Result<(), CensusError> {
        visit.table(self.files)?;
        visit.table(self.paths)?;
        visit.table(self.contents)?;
        visit.table(self.objects)?;
        visit.table(self.refs)?;
        visit.table(self.tags)?;
        visit.table(self.units)?;
        visit.table(self.unit_banks)?;
        visit.table(self.unit_events)?;
        visit.table(self.clips)?;
        visit.table(self.events)?;
        visit.table(self.event_wems)?;
        visit.table(self.skeletons)?;
        visit.table(self.meshes)?;
        visit.table(self.textures)
    }
}

/// What an entry text names. None when it does not parse.
type TextFacts = Arc<Option<ObjectText>>;

/// One side of a text change: the blob and what it names.
type TextSide = Option<(BlobId, TextFacts)>;

/// A change read: an entry's file before and after, or a bin entry's text.
enum Parsed {
    Entry {
        path: WadHash,
        old: Option<Box<Entry>>,
        new: Option<Box<Entry>>,
    },
    Text {
        bin: WadHash,
        entry: BinHash,
        old: TextSide,
        new: TextSide,
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
    /// What each entry text the tree holds names, and how many places hold it.
    texts: HashMap<BlobId, (u32, TextFacts)>,
}

impl Walker {
    pub(crate) fn new(families: Families) -> Self {
        Self {
            families,
            folds: Folds::default(),
            unparsed_texts: 0,
            texts: HashMap::new(),
        }
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

    /// Reads and parses one change, a text from the texts held when the tree already holds it.
    fn parse(
        &self,
        source: &dyn ChangeSource,
        change: &Change,
    ) -> Result<Option<Parsed>, CensusError> {
        let read = |id: &BlobId| -> Result<String, CensusError> {
            Ok(String::from_utf8_lossy(&source.read(id)?).into_owned())
        };
        if let Some(path) = format::entry_file(&change.path) {
            let entry = |id: &Option<BlobId>| -> Result<Option<Box<Entry>>, CensusError> {
                let text = id.as_ref().map(read).transpose()?;
                Ok(text.and_then(|t| format::parse_entry(&t)).map(Box::new))
            };
            return Ok(Some(Parsed::Entry {
                path,
                old: entry(&change.old)?,
                new: entry(&change.new)?,
            }));
        }
        let Some((bin, entry)) = format::object_file(&change.path) else {
            return Ok(None);
        };
        let text = |id: &Option<BlobId>| -> Result<TextSide, CensusError> {
            let Some(id) = id else { return Ok(None) };
            if let Some((_, facts)) = self.texts.get(id) {
                return Ok(Some((*id, facts.clone())));
            }
            let facts = format::parse_object_text(&read(id)?);
            Ok(Some((*id, Arc::new(facts))))
        };
        Ok(Some(Parsed::Text {
            bin,
            entry,
            old: text(&change.old)?,
            new: text(&change.new)?,
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
        let parsed = changes.par_iter().map(|c| this.parse(source, c));
        let parsed: Vec<Option<Parsed>> = parsed.collect::<Result<_, _>>()?;
        for change in parsed.iter().flatten() {
            match change {
                Parsed::Entry { path, old, new } => {
                    self.entry(at, *path, old.as_deref(), new.as_deref());
                }
                Parsed::Text {
                    bin,
                    entry,
                    old,
                    new,
                } => {
                    self.hold(old, new);
                    self.text(at, *bin, *entry, named(old), named(new));
                }
            }
        }
        Ok(())
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

    fn entry(&mut self, at: Position, path: WadHash, old: Option<&Entry>, new: Option<&Entry>) {
        let folds = &mut self.folds;
        if self.families.has(Family::Files) {
            let kind = |e: &Entry| FileKind::of(&e.kind);
            folds.files.change(at, old, new, |e| {
                let facts = FileFacts {
                    kind: kind(e),
                    path,
                };
                [(e.content, facts)]
            });
            folds.paths.change(at, old, new, |e| [(path, kind(e))]);
            folds.contents.change_keys(at, old, new, |e| {
                let key = e.content;
                [Held { path, key }]
            });
        }
        if self.families.has(Family::Objects) {
            folds.objects.change(at, old, new, |e| {
                e.objects.iter().map(|o| {
                    let facts = ObjectFacts {
                        entry: o.entry,
                        class: o.class,
                        bin: path,
                        norm: o.norm,
                    };
                    (o.object, facts)
                })
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
        tally.apply(2, Position(3));
        tally.apply(-1, Position(5));
        assert_eq!(tally.life(Position(9)), Life::new(3, 9));
        tally.apply(-1, Position(7));
        assert_eq!(tally.life(Position(9)), Life::new(3, 6));
        tally.apply(1, Position(8));
        assert_eq!(tally.life(Position(9)), Life::new(3, 9));
    }
}
