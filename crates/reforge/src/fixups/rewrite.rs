//! The bins a fixup changed, written again as bytes: what the snapshot after `fix` names by
//! content id (#ADR-0094), and what a repair's package ships. A fixed mod reopened from a
//! snapshot reads its objects back from those bytes.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ltk_meta::{Bin, BinDelta, BinObject, BinStream};
use serde::{Deserialize, Serialize};

use super::{Changes, FixedMod};
use crate::model::entity::{BinId, FileId, FileKind};
use crate::model::{ByteView, Mod, bin};

/// How a bin was written again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BinWrite {
    /// Through `ltk_meta`'s delta writer: every object no fixup changed is copied byte for
    /// byte.
    Delta,
    /// Every object encoded again: the delta writer refuses a bin of the old numbering.
    Whole,
}

/// A bin a fixup changed, written again.
#[derive(Debug, Clone)]
pub struct Rewritten {
    /// The bin's bytes.
    pub bytes: Vec<u8>,
    /// How they were written.
    pub how: BinWrite,
    /// The objects a fixup changed, as they were written.
    pub edited: Vec<BinObject>,
}

impl FixedMod {
    /// The loaded bin of a file, when a fixup changed an object of it.
    pub fn edited_bin(&self, changes: &Changes, file: FileId) -> Option<BinId> {
        if self.files[file].kind != FileKind::Bin {
            return None;
        }
        let bin = self.bin_of(file)?;
        let objects = &self.bins[bin].objects;
        let edited = objects.iter().any(|&object| changes.edits(object));
        edited.then_some(bin)
    }

    /// A bin's bytes with its changed objects written again: through the delta writer where
    /// it takes the bytes the bin was read from, else the whole bin from the mod's objects.
    pub fn rewrite(&self, changes: &Changes, bin: BinId) -> Rewritten {
        let m: &Mod = self;
        let node = &m.bins[bin];
        let objects = || node.objects.iter().map(|&object| &m.objects[object]);
        let edited: Vec<BinObject> = node
            .objects
            .iter()
            .filter(|&&object| changes.edits(object))
            .map(|&object| m.objects[object].value.clone())
            .collect();
        let (bytes, how) = match patched(&m.bytes(node.file), &edited) {
            Some(bytes) => (bytes, BinWrite::Delta),
            None => {
                let links = node.links.iter().map(|link| link.path.raw().to_string());
                let whole = Bin::new(
                    objects().map(|object| object.value.clone()),
                    links.collect::<Vec<_>>(),
                );
                (bin::write(&whole), BinWrite::Whole)
            }
        };
        Rewritten { bytes, how, edited }
    }

    /// Every bin a fixup changed, written again, by the file it was read from, in file order.
    pub fn rewritten(&self, changes: &Changes) -> Vec<(FileId, Rewritten)> {
        let files = self.files.ids();
        let bins = files.filter_map(|file| Some((file, self.edited_bin(changes, file)?)));
        bins.map(|(file, bin)| (file, self.rewrite(changes, bin)))
            .collect()
    }

    /// The mod as `fix` left it, from the mod as `load` left it and the bytes of each bin
    /// `fix` wrote again, by the file it was read from. Each object of such a bin takes the
    /// value read from the new bytes; its digests stay those of the bytes the archive ships.
    ///
    /// Refused, with what does not hold, when a file names no loaded bin, or the bytes do not
    /// read as the same objects under the same entry hashes in the same order.
    pub fn reopen(
        mut m: Mod,
        bins: impl IntoIterator<Item = (FileId, ByteView)>,
    ) -> Result<Self, String> {
        for (file, bytes) in bins {
            let bin = m
                .bin_of(file)
                .ok_or_else(|| format!("{file} is no loaded bin"))?;
            let parsed = bin::parse(&bytes).map_err(|why| format!("{bin} reads no more: {why}"))?;
            let ids = &m.bins[bin].objects;
            if ids.len() != parsed.objects.len() {
                return Err(format!(
                    "{bin} holds {} objects, and its bytes {}",
                    ids.len(),
                    parsed.objects.len()
                ));
            }
            for (&id, object) in ids.iter().zip(parsed.objects) {
                let node = &mut m.objects[id];
                if node.entry != object.value.path_hash {
                    return Err(format!("{id} reads under another entry hash"));
                }
                node.value = object.value;
            }
        }
        Ok(Self(m))
    }
}

/// The base bytes with these objects in place of the ones under their entry hashes, every
/// other object copied byte for byte. None where the delta writer refuses the base: a bin of
/// the old numbering. The base is a mod's, so the writer runs where its panic is caught.
fn patched(base: &[u8], edited: &[BinObject]) -> Option<Vec<u8>> {
    let mut delta = BinDelta::new();
    for object in edited {
        delta.replace(object.clone());
    }
    let write = || {
        let mut stream = BinStream::mount(Cursor::new(base)).ok()?;
        let mut out = Vec::new();
        stream.write_patched(&delta, &mut out).ok()?;
        Some(out)
    };
    catch_unwind(AssertUnwindSafe(write)).ok().flatten()
}
