//! What the archive carries: every file of its WADs and every entry outside them, told by its
//! first bytes and its name (`reforge_magic`), and what of it is no game file: executables,
//! archives, the author's sources, a bin as text where the client reads a binary one, leftovers
//! of file browsers, bytes no table knows (#MOD-ARTIFACT-0016).
//!
//! The findings are facts for the report. None changes what a run carries.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use reforge_magic::{Categories, Confidence, Kind, identify};
use serde::{Deserialize, Serialize};

use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Sink, Subject};
use crate::model::entity::{FileId, FileSource, FileUse, Mod};
use crate::model::{Spell, Spelled};

/// Every diagnostic code the inventory raises, in the order raised.
pub const CODES: &[Code] = &[
    Executables::CODE,
    TextBins::CODE,
    NestedArchives::CODE,
    Sources::CODE,
    OsArtifacts::CODE,
    UnknownFiles::CODE,
];

/// The place of an entry outside the WADs, where a file has its WAD's name.
const ARCHIVE: &str = "archive";

/// Why a file is worth a reader's look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Finding {
    /// A program. Never run, never examined.
    Executable,
    /// A bin as text or JSON at a path the client reads a binary bin from: what it holds never
    /// showed.
    BinText,
    /// An archive inside the mod. Never opened (#ADR-0024).
    NestedArchive,
    /// An authoring file: a scene, a layered image, a bin as text. No use to the client, a help
    /// to recovering the skin's sources.
    Source,
    /// A leftover of an operating system or a file browser.
    OsArtifact,
    /// Bytes no table knows.
    Unknown,
}

impl Finding {
    /// Every finding, in the order they are raised.
    pub const ALL: [Finding; 6] = [
        Finding::Executable,
        Finding::BinText,
        Finding::NestedArchive,
        Finding::Source,
        Finding::OsArtifact,
        Finding::Unknown,
    ];

    /// What a file of a kind is found to be. None for a game file and for what a mod carries
    /// with no harm and no use: a readme, a cover image.
    ///
    /// `at_bin_path` says the file sits where the client reads a binary bin.
    pub fn of(kind: Kind, at_bin_path: bool) -> Option<Self> {
        let categories = kind.info().categories;
        if matches!(kind, Kind::RitobinText | Kind::RitobinJson) && at_bin_path {
            Some(Finding::BinText)
        } else if categories.contains(Categories::EXECUTABLE) {
            Some(Finding::Executable)
        } else if categories.contains(Categories::OS_ARTIFACT) {
            Some(Finding::OsArtifact)
        } else if categories.contains(Categories::ARCHIVE) && !categories.contains(Categories::GAME)
        {
            // A WAD is an archive and a game file.
            Some(Finding::NestedArchive)
        } else if categories.contains(Categories::SOURCE) {
            Some(Finding::Source)
        } else if matches!(kind, Kind::Unknown | Kind::Ole) {
            Some(Finding::Unknown)
        } else {
            None
        }
    }
}

/// One file or entry that is no game file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// The file, for a file of a WAD. None for an entry outside the WADs.
    pub file: Option<FileId>,
    /// The WAD it is in, as the archive spells it, or `archive` for an entry outside the WADs.
    pub place: Spelled,
    /// A file's path, or its hash as hex where neither the archive nor a table spells one. An
    /// entry's name.
    pub name: Spelled,
    /// Its size in bytes. An entry's is the size the archive declares.
    pub size: u64,
    /// What it is.
    pub kind: Kind,
    /// How its kind was known.
    pub confidence: Confidence,
    /// Why it is listed.
    pub finding: Finding,
}

/// The files of one kind: how many, and their bytes together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// How many files.
    pub files: usize,
    /// Their sizes together.
    pub bytes: u64,
}

impl Tally {
    fn add(&mut self, size: u64) {
        self.files += 1;
        // An entry's size is the one the archive declares.
        self.bytes = self.bytes.saturating_add(size);
    }
}

/// What the archive carries, by kind, and what of it is no game file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    /// Every file and entry by kind.
    pub by_kind: BTreeMap<Kind, Tally>,
    /// What is no game file: the files of the WADs in file order, then the entries outside them
    /// in archive order.
    pub items: Vec<Item>,
}

/// Whether an entry is the archive's own metadata and not the mod's content.
fn is_meta(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "meta/info.json" || lower.starts_with("meta/image.")
}

impl Inventory {
    /// The inventory of a mod. `names` spells the path of a chunk the archive names by its
    /// hash: the only way to know a file with no magic, such as a WAD's `.subchunktoc`.
    pub fn take(m: &Mod, names: &dyn Spell) -> Self {
        let mut out = Self::default();
        for (id, file) in m.files.iter() {
            // A file a fixup made is the archive's file again under another name.
            if matches!(file.source, FileSource::Made { .. }) {
                continue;
            }
            let path = file.path.as_ref().map(ToString::to_string);
            let path = path.or_else(|| names.path(file.hash));
            // What the store knows of the file's bytes: `load` read their head once.
            let facts = m.blobs().facts_of(file.content);
            let identity = facts.kind.identify(path.as_deref());
            out.by_kind.entry(identity.kind).or_default().add(file.size);
            let named_bin = path
                .as_deref()
                .is_some_and(|p| p.to_ascii_lowercase().ends_with(".bin"));
            // A path the game ships or shipped: the file replaces a game file, whatever its name.
            let at_bin_path =
                named_bin || matches!(file.used, FileUse::Override | FileUse::FormerPath);
            if let Some(finding) = Finding::of(identity.kind, at_bin_path) {
                out.items.push(Item {
                    file: Some(id),
                    place: Spelled::new(m.wads[file.wad].name.as_str()),
                    name: Spelled::new(path.unwrap_or_else(|| format!("{:016x}", file.hash.0))),
                    size: file.size,
                    kind: identity.kind,
                    confidence: identity.confidence,
                    finding,
                });
            }
        }
        for extra in m.extras.iter().filter(|x| !is_meta(x.name.raw())) {
            let identity = identify(&extra.head, Some(extra.name.raw()));
            out.by_kind
                .entry(identity.kind)
                .or_default()
                .add(extra.size);
            if let Some(finding) = Finding::of(identity.kind, false) {
                out.items.push(Item {
                    file: None,
                    place: Spelled::new(ARCHIVE),
                    name: extra.name.clone(),
                    size: extra.size,
                    kind: identity.kind,
                    confidence: identity.confidence,
                    finding,
                });
            }
        }
        out
    }

    /// The files of a kind.
    pub fn tally(&self, kind: Kind) -> Tally {
        self.by_kind.get(&kind).copied().unwrap_or_default()
    }

    /// The items of a finding.
    pub fn found(&self, finding: Finding) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .filter(move |item| item.finding == finding)
    }

    /// Raises one diagnostic per finding the archive has items of.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for finding in Finding::ALL {
            let items: Vec<Item> = self.found(finding).cloned().collect();
            if items.is_empty() {
                continue;
            }
            match finding {
                Finding::Executable => sink.raise(Executables { items }),
                Finding::BinText => sink.raise(TextBins { items }),
                Finding::NestedArchive => sink.raise(NestedArchives { items }),
                Finding::Source => sink.raise(Sources { items }),
                Finding::OsArtifact => sink.raise(OsArtifacts { items }),
                Finding::Unknown => sink.raise(UnknownFiles { items }),
            };
        }
    }
}

/// Writes `3 executables, not examined (0.4 MiB; <mime> 3): <place>/<name>, ...`.
fn write_items(f: &mut Message, items: &[Item], noun: &str, what: &str) {
    let bytes = items
        .iter()
        .fold(0u64, |sum, item| sum.saturating_add(item.size));
    let mut kinds: BTreeMap<Kind, usize> = BTreeMap::new();
    for item in items {
        *kinds.entry(item.kind).or_default() += 1;
    }
    f.count(items.len(), noun).text(what);
    let _ = write!(f, " ({:.1} MiB", bytes as f64 / f64::from(1u32 << 20));
    for (i, (kind, n)) in kinds.iter().enumerate() {
        let _ = write!(
            f,
            "{} {} {n}",
            if i == 0 { ";" } else { "," },
            kind.info().mime
        );
    }
    f.text("): ");
    f.list(items, |f, item| {
        f.spelled(item.place.raw())
            .text("/")
            .spelled(item.name.raw());
    });
}

/// Declares the finding type of one code: the items of one `Finding`, about the whole mod.
macro_rules! finding {
    (
        $(#[$doc:meta])*
        $name:ident, $code:literal, $severity:ident, $record:expr, $noun:literal, $what:literal
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
        pub struct $name {
            /// The items: the files of the WADs in file order, then the entries outside them
            /// in archive order.
            pub items: Vec<Item>,
        }

        impl Diag for $name {
            const CODE: Code = Code::new($code, Channel::Input, Severity::$severity);
            const RECORD: Option<Record> = $record;

            fn subject(&self) -> Subject {
                Subject::Run
            }

            fn message(&self, f: &mut Message) {
                write_items(f, &self.items, $noun, $what);
            }
        }
    };
}

finding! {
    /// Programs in the archive, never run or examined (#MOD-ARTIFACT-0016).
    Executables, "inventory.executable", Warning, Some(Record::mod_artifact(16)),
    "executable", ", not examined"
}

finding! {
    /// Bins as text or JSON where the client reads a binary bin, so what they hold never
    /// showed (#MOD-ARTIFACT-0016).
    TextBins, "inventory.bin-text", Warning, Some(Record::mod_artifact(16)),
    "bin", " as text or JSON where the client reads a binary bin, never shown"
}

finding! {
    /// Archives inside the mod, not opened (#ADR-0024).
    NestedArchives, "inventory.nested-archive", Note, Some(Record::mod_artifact(16)),
    "archive", " inside the mod, not opened"
}

finding! {
    /// Authoring files: scenes, layered images, bins as text (#MOD-ARTIFACT-0016).
    Sources, "inventory.source", Note, Some(Record::mod_artifact(16)),
    "authoring file", ", a help to recovering the skin's sources"
}

finding! {
    /// Leftovers of operating systems and file browsers (#MOD-ARTIFACT-0016).
    OsArtifacts, "inventory.os-artifact", Note, Some(Record::mod_artifact(16)),
    "leftover", " of file browsers"
}

finding! {
    /// Files of no known kind.
    UnknownFiles, "inventory.unknown", Note, None,
    "file", " of no known kind"
}
