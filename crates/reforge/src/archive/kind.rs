//! The kind of a file, for the formats a run reads.

use reforge_magic::{HEAD_LEN, Kind, identify};

use crate::model::entity::FileKind;

/// The kind of a file by its first bytes: the formats a run reads, and `Other` for every other
/// one. `reforge_magic` says what those are.
pub fn file_kind(bytes: &[u8]) -> FileKind {
    match identify(&bytes[..bytes.len().min(HEAD_LEN)], None).kind {
        Kind::PropertyBin => FileKind::Bin,
        Kind::PropertyBinOverride => FileKind::Ptch,
        Kind::SimpleSkin => FileKind::Skn,
        Kind::Skeleton => FileKind::Skl,
        Kind::Animation => FileKind::Anm,
        Kind::StaticMeshBinary => FileKind::Scb,
        Kind::StaticMeshAscii => FileKind::Sco,
        Kind::Texture => FileKind::Tex,
        Kind::TextureDds => FileKind::Dds,
        Kind::WwiseBank => FileKind::Bnk,
        Kind::WwisePackage => FileKind::Wpk,
        _ => FileKind::Other,
    }
}
