//! What `load` finds: one type per code of `docs/design/CODES.md` for WADs, characters, files
//! and bins, and `archive.no-skin`. The findings of the file fixups, the inventory and the
//! files' provenance are declared beside what raises them.

use std::fmt::Write as _;

use serde::{Serialize, Serializer};

use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Subject};
use crate::model::bin::BinError;
use crate::model::entity::{BinId, CharId, FileId, UnitId, WadId};
use crate::model::{BinHash, Slot, Spelled, WadHash};

const fn code(name: &'static str, severity: Severity) -> Code {
    Code::new(name, Channel::Input, severity)
}

fn paths_hex<S: Serializer>(paths: &[WadHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(paths.iter().map(|path| format!("{:016x}", path.0)))
}

fn entries_hex<S: Serializer>(entries: &[BinHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(entries.iter().map(|entry| format!("{:08x}", entry.0)))
}

fn error_text<S: Serializer>(error: &BinError, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&error.to_string())
}

/// Writes ` (1.5 MiB)`.
fn mib(f: &mut Message<'_>, bytes: u64) {
    let size = bytes as f64 / (1024.0 * 1024.0);
    let _ = write!(f, " ({size:.1} MiB)");
}

/// No WAD of the archive is a champion's with a skin in it: the archive is no skin mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoSkin {
    /// The WADs the archive holds.
    pub wads: usize,
}

impl Diag for NoSkin {
    const CODE: Code = code("archive.no-skin", Severity::Fatal);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("no WAD ships a champion skin bin or a file at a skin's path: not a skin mod (")
            .count(self.wads, "WAD")
            .text(")");
    }
}

/// A locale WAD beside its champion's. Its voice goes to a voice layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocaleWad {
    /// The WAD.
    pub wad: WadId,
    /// The champion its name spells, lowercase.
    pub champion: String,
    /// The locale, lowercase: `en_us`.
    pub locale: String,
}

impl Diag for LocaleWad {
    const CODE: Code = code("wad.locale", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("a locale WAD of ")
            .spelled(&self.champion)
            .text("; its voice goes to a voice layer");
    }
}

/// A locale WAD holding its champion's skin bins, with no champion WAD beside it
/// (#MOD-ARTIFACT-0010). It is taken as the champion's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalePacked {
    /// The WAD.
    pub wad: WadId,
    /// The champion, lowercase.
    pub champion: String,
    /// The locale, lowercase, which the files the game localizes keep.
    pub locale: String,
}

impl Diag for LocalePacked {
    const CODE: Code = code("wad.locale-packed", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(10));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("a locale WAD holding ")
            .spelled(&self.champion)
            .text("'s skin bins, with no champion WAD beside it: taken as the champion's");
    }
}

/// A WAD named for no champion that ships the skin bins of one (#MOD-ARTIFACT-0011). It is
/// taken as that champion's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChampionGuessed {
    /// The WAD.
    pub wad: WadId,
    /// The champion whose skin bins it ships, lowercase.
    pub champion: String,
}

impl Diag for ChampionGuessed {
    const CODE: Code = code("wad.champion-guessed", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(11));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("a WAD named for no champion ships ")
            .spelled(&self.champion)
            .text("'s skin bins and no other champion's: taken as ")
            .spelled(&self.champion)
            .text("'s");
    }
}

/// A WAD named for no champion that ships the skin bins of several (#MOD-ARTIFACT-0011). It is
/// taken as none's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChampionAmbiguous {
    /// The WAD.
    pub wad: WadId,
    /// The champions whose skin bins it ships, in name order.
    pub champions: Vec<String>,
}

impl Diag for ChampionAmbiguous {
    const CODE: Code = code("wad.champion-ambiguous", Severity::Warning);
    const RECORD: Option<Record> = Some(Record::mod_artifact(11));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("a WAD named for no champion ships the skin bins of ")
            .count(self.champions.len(), "champion")
            .text(": ");
        f.list(&self.champions, |f, champion| {
            f.spelled(champion);
        })
        .text("; taken as none's");
    }
}

/// A second WAD for a champion that holds no file the carried one lacks (#MOD-ARTIFACT-0012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WadDuplicate {
    /// The WAD that is not carried.
    pub wad: WadId,
    /// The champion, lowercase.
    pub champion: String,
    /// The WAD that is carried.
    pub carried: WadId,
}

impl Diag for WadDuplicate {
    const CODE: Code = code("wad.duplicate", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(12));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("a second WAD for ")
            .spelled(&self.champion)
            .text(" beside ")
            .wad(self.carried)
            .text("; that one is carried, and this one adds nothing");
    }
}

/// A second WAD for a champion that holds files the carried one lacks (#MOD-ARTIFACT-0012).
/// They are not carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WadDuplicateDiffers {
    /// The WAD that is not carried.
    pub wad: WadId,
    /// The champion, lowercase.
    pub champion: String,
    /// The WAD that is carried.
    pub carried: WadId,
    /// The files at paths the carried WAD has no file at.
    pub files: Vec<FileId>,
}

impl Diag for WadDuplicateDiffers {
    const CODE: Code = code("wad.duplicate-differs", Severity::Warning);
    const RECORD: Option<Record> = Some(Record::mod_artifact(12));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("a second WAD for ")
            .spelled(&self.champion)
            .text(" beside ")
            .wad(self.carried)
            .text("; that one is carried, and ")
            .count(self.files.len(), "file")
            .text(" of this one that it lacks are not carried: ");
        f.list(&self.files, |f, file| {
            f.file(*file);
        });
    }
}

/// The archive holds more than one champion WAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeveralChampions {
    /// The champions, in the order of their WADs.
    pub champions: Vec<CharId>,
}

impl Diag for SeveralChampions {
    const CODE: Code = code("wad.several-champions", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.champions.len(), "champion WAD").text(": ");
        f.list(&self.champions, |f, champion| {
            f.character(*champion);
        });
    }
}

/// A WAD with no skin bin of the champion its name spells. It is not carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WadNotCarried {
    /// The WAD.
    pub wad: WadId,
}

impl Diag for WadNotCarried {
    const CODE: Code = code("wad.not-carried", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("ships no champion skin bin; not carried");
    }
}

/// A sub-character whose every skin bin is the game's byte for byte. It is left to the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EqualToGame {
    /// The sub-character.
    pub character: CharId,
    /// Its units.
    pub units: Vec<UnitId>,
}

impl Diag for EqualToGame {
    const CODE: Code = code("character.equal-to-game", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Character(self.character)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.units.len(), "skin bin")
            .text(", each the game's byte for byte; left to the game");
    }
}

/// Files of a WAD that nothing loads. They are not judged or carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnusedFiles {
    /// The WAD.
    pub wad: WadId,
    /// The files, in the WAD's order.
    pub files: Vec<FileId>,
    /// Their size together.
    pub bytes: u64,
    /// Those of them that are bins: stale bins and working copies.
    pub bins: Vec<FileId>,
}

impl Diag for UnusedFiles {
    const CODE: Code = code("file.unused", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file");
        mib(f, self.bytes);
        f.text(" that nothing loads; not judged or carried");
        if !self.bins.is_empty() {
            let _ = write!(f, ", {} of them bins: ", self.bins.len());
            f.list(&self.bins, |f, file| {
                f.file(*file);
            });
        }
    }
}

/// Files of a WAD at paths the game ships, holding the bytes the game shipped there at earlier
/// builds (#MOD-ARTIFACT-0004). They are no override, and the game's own bytes stand
/// (#ADR-0023).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaleFiles {
    /// The WAD.
    pub wad: WadId,
    /// The files, in the WAD's order.
    pub files: Vec<FileId>,
    /// Their size together.
    pub bytes: u64,
}

impl Diag for StaleFiles {
    const CODE: Code = code("file.stale", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(4));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file");
        mib(f, self.bytes);
        f.text(
            " at the game's paths, holding the bytes the game shipped there at earlier builds; \
             not carried",
        );
    }
}

/// Files of a WAD at paths the game shipped at earlier builds and does not ship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormerPathFiles {
    /// The WAD.
    pub wad: WadId,
    /// The files, in the WAD's order.
    pub files: Vec<FileId>,
    /// Their size together.
    pub bytes: u64,
}

impl Diag for FormerPathFiles {
    const CODE: Code = code("file.former-path", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file");
        mib(f, self.bytes);
        f.text(" at paths the game shipped at earlier builds and does not ship");
    }
}

/// A bin the client loads that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BinUnreadable {
    /// The bin.
    pub bin: BinId,
    /// Why it does not read.
    #[serde(serialize_with = "error_text")]
    pub why: BinError,
}

impl Diag for BinUnreadable {
    const CODE: Code = code("bin.unreadable", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Bin(self.bin)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("does not read as a bin: ")
            .spelled(&self.why.to_string());
    }
}

/// A bin that lists entry hashes more than once. The last object under each is kept. Which one
/// the client keeps is not established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryDuplicates {
    /// The bin.
    pub bin: BinId,
    /// The entry hashes listed again, once for each further listing, in file order.
    #[serde(serialize_with = "entries_hex")]
    pub entries: Vec<BinHash>,
}

impl Diag for EntryDuplicates {
    const CODE: Code = code("bin.entry-duplicate", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Bin(self.bin)
    }

    fn message(&self, f: &mut Message<'_>) {
        let n = self.entries.len();
        let noun = if n == 1 { "entry hash" } else { "entry hashes" };
        let _ = write!(
            f,
            "{n} {noun} listed more than once, the last object kept: "
        );
        f.list(&self.entries, |f, entry| {
            f.entry(*entry);
        });
    }
}

/// A linked path neither the mod nor the game holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissingLink {
    /// The path as the bin spells it.
    pub path: Spelled,
    /// Whether the game shipped a file at the path at earlier builds.
    pub former: bool,
}

/// Linked paths of a bin that neither the mod nor the game holds. The client skips them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinksMissing {
    /// The bin.
    pub bin: BinId,
    /// The links, in the bin's order.
    pub links: Vec<MissingLink>,
}

impl Diag for LinksMissing {
    const CODE: Code = code("bin.link-missing", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Bin(self.bin)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.links.len(), "link")
            .text(" neither the mod nor the game holds");
        let former = self.links.iter().filter(|link| link.former).count();
        if former > 0 {
            let _ = write!(f, " ({former} of them the game shipped at earlier builds)");
        }
        f.text("; the client skips them: ");
        f.list(&self.links, |f, link| {
            f.spelled(link.path.raw());
        });
    }
}

/// A skin of the game whose bins were taken as a mod's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TakenSkin {
    /// The character, lowercase.
    pub character: String,
    /// The skin's slot.
    pub slot: Slot,
}

/// A WAD with no skin bin, only files at the game's paths (#MOD-ARTIFACT-0013). The game's bins
/// of the skins that name the files it overrides are taken as its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TakenFromGame {
    /// The WAD.
    pub wad: WadId,
    /// The skins whose bins were taken, a champion's before its sub-characters', by slot.
    pub skins: Vec<TakenSkin>,
    /// The bins taken, in the order added.
    pub bins: Vec<FileId>,
    /// How many files of the WAD override a file of the game.
    pub overridden: usize,
    /// The overridden paths no skin's bin claims: named by none, or only by skins other than
    /// the one whose folder they are in. In path-hash order.
    #[serde(serialize_with = "paths_hex")]
    pub unclaimed: Vec<WadHash>,
    /// The paths of files where the game ships none, not counted in the three below. With a
    /// history these are paths the game shipped at earlier builds. In path-hash order.
    #[serde(serialize_with = "paths_hex")]
    pub former: Vec<WadHash>,
    /// Files at paths the game never shipped. Counted with a history only.
    pub never: usize,
    /// Files at paths the game does not ship whose asset has another file of the mod.
    pub covered: usize,
    /// Stale copies: no override.
    pub stale: usize,
    /// Whether a history told former paths from paths never the game's.
    pub history: bool,
}

impl Diag for TakenFromGame {
    const CODE: Code = code("bin.taken-from-game", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(13));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("no skin bin: the game's bins of ");
        f.list(&self.skins, |f, skin| {
            f.spelled(&skin.character);
            let _ = write!(f, " skin{}", skin.slot);
        });
        let _ = write!(
            f,
            " taken as the mod's, for the {} files it overrides at the game's paths ({} of them \
             claimed by no skin's bin); ",
            self.overridden,
            self.unclaimed.len()
        );
        if self.history {
            let _ = write!(
                f,
                "{} files at paths the game shipped at earlier builds, not mapped; {} at paths \
                 never the game's; {} whose asset has another file of the mod; {} stale copies",
                self.former.len(),
                self.never,
                self.covered,
                self.stale
            );
        } else {
            let _ = write!(
                f,
                "{} files at paths the game does not ship, not mapped (no history tells former \
                 paths); {} whose asset has another file of the mod",
                self.former.len(),
                self.covered
            );
        }
    }
}

/// A WAD that restores a champion's model from before its rework (#GAME-CHANGE-0001): files at
/// paths the rework dropped beside files at paths only the rework has. Its copies of the game's
/// older bytes are its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Restoration {
    /// The WAD.
    pub wad: WadId,
    /// The champion, lowercase.
    pub champion: String,
    /// The patch the new model shipped in.
    pub patch: String,
    /// The mod's files at the champion's paths the game dropped before the rework.
    pub older: u32,
    /// The mod's files at the champion's paths the game shipped only from the rework on.
    pub newer: u32,
}

impl Diag for Restoration {
    const CODE: Code = code("rework.restoration", Severity::Note);
    const RECORD: Option<Record> = Some(Record::game_change(1));

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("restores ")
            .spelled(&self.champion)
            .text("'s model from before its rework of ")
            .spelled(&self.patch);
        let _ = write!(
            f,
            " ({} files at paths the rework dropped, {} at paths since): its copies of the \
             game's older bytes are its own",
            self.older, self.newer
        );
    }
}

/// The codes the core of `load` raises: what a WAD and a character are, what the client loads,
/// and `archive.no-skin`.
pub const CODES: &[Code] = &[
    NoSkin::CODE,
    LocaleWad::CODE,
    LocalePacked::CODE,
    ChampionGuessed::CODE,
    ChampionAmbiguous::CODE,
    WadDuplicate::CODE,
    WadDuplicateDiffers::CODE,
    SeveralChampions::CODE,
    WadNotCarried::CODE,
    EqualToGame::CODE,
    UnusedFiles::CODE,
    StaleFiles::CODE,
    FormerPathFiles::CODE,
    BinUnreadable::CODE,
    EntryDuplicates::CODE,
    LinksMissing::CODE,
    TakenFromGame::CODE,
    Restoration::CODE,
];
