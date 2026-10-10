//! A bank whose header id is zero, given the id its file name hashes to (#MOD-ARTIFACT-0023).
//!
//! The id is the hash of the file name, so the fixup needs the bank's path: it runs once the
//! bins the client loads are read, when a file has the path a bin of the mod spells for it.
//! It rewrites the banks the client uses and leaves a file nothing loads as it is.

use reforge_wwise::{Bank, BankKind, id_of};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::files::{FileFixCx, FileFixup, FileStep};
use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Subject};
use crate::history::HistoryError;
use crate::model::entity::{FileId, FileKind, FileUse, MadeBy, WadId};
use crate::model::{ChunkPath, FixupInfo, Phase};

/// A bank given the id of its file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Named {
    /// The bank.
    pub file: FileId,
    /// The id its header holds after the fixup.
    pub id: u32,
}

/// Banks of a WAD whose header id was zero, each given the id its file name hashes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BankIdSet {
    /// The WAD.
    pub wad: WadId,
    /// The banks, in file order.
    pub banks: Vec<Named>,
}

impl Diag for BankIdSet {
    const CODE: Code = Code::new("audio.bank-id-set", Channel::Output, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.banks.len(), "bank")
            .text(" with header id 0, given the id of the file name: ");
        f.list(&self.banks, |f, bank| {
            f.file(bank.file);
            let _ = write!(f, " ({})", bank.id);
        });
    }
}

/// Banks of a WAD whose header id is zero and whose path nothing spells. The id of a name
/// that is not known is not known either: they stay as they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BankUnnamed {
    /// The WAD.
    pub wad: WadId,
    /// The banks, in file order.
    pub banks: Vec<FileId>,
}

impl Diag for BankUnnamed {
    const CODE: Code = Code::new("audio.bank-unnamed", Channel::Output, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.banks.len(), "bank")
            .text(" with header id 0 at a path nothing spells, left as they are: ");
        f.list(&self.banks, |f, bank| {
            f.file(*bank);
        });
    }
}

/// Every diagnostic code this file raises.
pub const CODES: &[Code] = &[BankIdSet::CODE, BankUnnamed::CODE];

/// Where the body of a bank's first section starts: after the section's tag and its length.
const FIRST_BODY_AT: usize = 8;

/// A sound bank whose header id is zero, with the bytes it was read from.
struct ZeroId<'a> {
    bytes: &'a [u8],
    bank: Bank,
}

impl<'a> ZeroId<'a> {
    /// The bank the bytes hold, when it reads and its header id is zero. A package has no
    /// header, and a bank that does not read is not judged.
    fn read(bytes: &'a [u8]) -> Option<Self> {
        let bank = Bank::read(bytes).ok()?;
        (bank.kind == BankKind::Bnk && bank.id() == 0).then_some(Self { bytes, bank })
    }

    /// The bank's bytes with an id in its header and every other byte as it was.
    fn with_id(mut self, id: u32) -> Option<Vec<u8>> {
        self.bank.set_id(id);
        // The writer of a whole bank lays its clips out again. A bank a tool wrote keeps the
        // layout it has: only the header's body is written, where the reader took it from,
        // which is the first section of a bank it reads.
        let mut out = self.bytes.to_vec();
        let header = &self.bank.header;
        let body = out.get_mut(FIRST_BODY_AT..FIRST_BODY_AT + header.len())?;
        body.copy_from_slice(header);
        Some(out)
    }
}

/// The name a bank's id is the hash of: its file name up to the last dot.
fn stem(path: &ChunkPath) -> &str {
    let name = path.parts().last().unwrap_or(path.as_str());
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// What the fixup found in one WAD.
#[derive(Default)]
struct Found {
    named: Vec<Named>,
    unnamed: Vec<FileId>,
}

/// The file fixup `bank-id`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BankId;

impl BankId {
    /// The id a bank at a path has in the game: the hash of its file name's stem.
    pub fn of_path(path: &ChunkPath) -> u32 {
        id_of(stem(path))
    }
}

impl FileFixup for BankId {
    fn info(&self) -> &FixupInfo {
        static INFO: FixupInfo = FixupInfo::built_in(
            "bank-id",
            "A bank whose header id is zero, given the id of its file name",
            &[Record::mod_artifact(23)],
            Phase::Files,
        );
        &INFO
    }

    fn step(&self) -> FileStep {
        FileStep::AfterResolve
    }

    fn fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError> {
        let mut found: BTreeMap<WadId, Found> = BTreeMap::new();
        let files: Vec<FileId> = cx.module.files.ids().collect();
        for file in files {
            let node = &cx.module.files[file];
            if node.kind != FileKind::Bnk || node.used == FileUse::Unused {
                continue;
            }
            let wad = node.wad;
            let bytes = node.bytes.clone();
            let Some(bank) = ZeroId::read(&bytes) else {
                continue;
            };
            let Some(path) = cx.path_of(file) else {
                found.entry(wad).or_default().unnamed.push(file);
                continue;
            };
            let id = Self::of_path(&path);
            let Some(rewritten) = bank.with_id(id) else {
                continue;
            };
            cx.rewrite(file, rewritten.into(), MadeBy::BankId);
            found.entry(wad).or_default().named.push(Named { file, id });
        }
        for (wad, found) in found {
            if !found.named.is_empty() {
                let banks = found.named;
                cx.sink.raise(BankIdSet { wad, banks });
            }
            if !found.unnamed.is_empty() {
                let banks = found.unnamed;
                cx.sink.raise(BankUnnamed { wad, banks });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_banks_name_is_its_file_name_up_to_the_last_dot() {
        let path = |text: &str| ChunkPath::parse(text).expect("a path");
        let bank = path(
            "assets/sounds/wwise2016/sfx/characters/braum/skins/base/braum_base_sfx_audio.bnk",
        );
        assert_eq!(stem(&bank), "braum_base_sfx_audio");
        // The id the game's own bank of that name carries at 16.19.
        assert_eq!(BankId::of_path(&bank), 3_580_963_463);
        assert_eq!(stem(&path("a/b.c.bnk")), "b.c");
        assert_eq!(stem(&path("plain")), "plain");
    }
}
