//! A sound bank as the stages after `load` ask for one: an events bank's header and hierarchy,
//! a media bank's clips by id and key, and the game's path of a bank a repath tool moved.
//!
//! The census history records the same of each bank, so the game answers these from an
//! install's bytes or from the history alike (#ADR-0043).

use reforge_wwise::{Bank, BankKind, Object};
use serde::Serialize;

use super::hash::ContentId;
use super::keys::ClipKey;

/// An events bank's header and hierarchy, as the bank stores them: what `reforge_wwise::Bank`
/// holds of one besides its clips.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventsBank {
    /// The body of the bank's `BKHD` section.
    pub header: Vec<u8>,
    /// The objects of its `HIRC` section, in stored order.
    pub objects: Vec<Object>,
}

/// The bytes of a `BKHD` body before which no bank reads: the version and the bank id.
const HEADER_MIN: usize = 8;

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let four = bytes.get(at..).and_then(|rest| rest.first_chunk());
    four.copied().map_or(0, u32::from_le_bytes)
}

impl EventsBank {
    /// The bank of a header and objects. None when the header is too short to state a version
    /// and a bank id: `reforge_wwise::Bank::read` refuses such a bank too.
    pub fn new(header: Vec<u8>, objects: Vec<Object>) -> Option<Self> {
        (header.len() >= HEADER_MIN).then_some(Self { header, objects })
    }

    /// The header and hierarchy of a bank's file. None for bytes that read as no sound bank:
    /// a package has no header.
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let bank = Bank::read(bytes).ok()?;
        Self::new(bank.header, bank.objects)
    }

    /// The bank's version, from its header.
    pub fn version(&self) -> u32 {
        u32_at(&self.header, 0)
    }

    /// The bank's id, from its header.
    pub fn bank_id(&self) -> u32 {
        u32_at(&self.header, 4)
    }

    /// The bank as `reforge-wwise` holds one, to cut and to write: the header, the objects and
    /// no clip.
    pub fn bank(&self) -> Bank {
        let mut bank = Bank::new_bnk(self.header.clone(), true);
        bank.objects.clone_from(&self.objects);
        bank
    }
}

/// One clip of a bank: its id, its size and the key of its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankClip {
    /// The id sounds name the clip by.
    pub id: u32,
    /// The size of its bytes.
    pub size: u64,
    /// The key of its bytes.
    pub key: ClipKey,
}

impl BankClip {
    /// The clip of an id with these bytes.
    pub fn of(id: u32, bytes: &[u8]) -> Self {
        Self {
            id,
            size: bytes.len() as u64,
            key: ClipKey::from_sha256(&ContentId::of(bytes).0),
        }
    }

    /// The clips of a bank's or a package's file, in stored order. None for bytes that read
    /// as neither.
    pub fn read(bytes: &[u8]) -> Option<Vec<Self>> {
        let bank = Bank::read(bytes).ok()?;
        let clips = bank.media.iter().map(|wem| Self::of(wem.id, &wem.bytes));
        Some(clips.collect())
    }
}

/// What a bank's file holds, by the role it has in a bank unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BankRole {
    /// Clips in `DIDX` and `DATA`.
    Media,
    /// Objects in `HIRC`.
    Events,
    /// A header alone, beside a package.
    Stub,
    /// A `.wpk` of voice clips.
    Package,
}

impl BankRole {
    /// The role of a bank as read.
    pub fn of(bank: &Bank) -> Self {
        match bank.kind {
            BankKind::Wpk => BankRole::Package,
            BankKind::Bnk if !bank.media.is_empty() => BankRole::Media,
            BankKind::Bnk if bank.has_hirc() => BankRole::Events,
            BankKind::Bnk => BankRole::Stub,
        }
    }

    /// The role's name in a dump.
    pub fn as_str(self) -> &'static str {
        match self {
            BankRole::Media => "media",
            BankRole::Events => "events",
            BankRole::Stub => "stub",
            BankRole::Package => "package",
        }
    }
}

/// The folder every bank of the game sits under.
const BANKS_FOLDER: &str = "sounds/wwise2016/";

/// The game's init bank: the buses and the rest of the mixing graph every bank routes into.
pub const INIT_BANK: &str = "assets/sounds/wwise2016/sfx/shared/init.bnk";

/// The game's path of a bank, lowercase. Whatever a path holds before `sounds/wwise2016/` is a
/// repath tool's (`assets/<tag>/sounds/wwise2016/...`): the rest is where the game has the bank.
pub fn game_bank_path(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    let own = format!("assets/{BANKS_FOLDER}");
    match lower.find(BANKS_FOLDER) {
        Some(at) if !lower.starts_with(&own) => format!("assets/{}", &lower[at..]),
        _ => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repathed_bank_counts_at_the_games_path() {
        let game = "assets/sounds/wwise2016/sfx/characters/test/skins/base/test_base_sfx_audio.bnk";
        assert_eq!(game_bank_path(&game.to_ascii_uppercase()), game);
        let moved = game.replace("assets/", "assets/shulk/");
        assert_eq!(game_bank_path(&moved), game);
        assert_eq!(game_bank_path("a/b.bnk"), "a/b.bnk");
    }

    #[test]
    fn an_events_bank_needs_a_version_and_an_id() {
        let bank = EventsBank::new(vec![145, 0, 0, 0, 7, 0, 0, 0], Vec::new()).expect("a bank");
        assert_eq!((bank.version(), bank.bank_id()), (145, 7));
        assert_eq!(EventsBank::new(vec![145, 0, 0, 0], Vec::new()), None);
        let written = bank.bank().write();
        assert_eq!(EventsBank::read(&written), Some(bank));
        assert_eq!(EventsBank::read(b"r3d2\x01\0\0\0\0\0\0\0"), None);
    }
}
