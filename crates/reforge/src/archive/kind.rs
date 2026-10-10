//! The kind of a file, for the formats a run reads.

use reforge_magic::{HeadKind, Kind};

use crate::model::entity::FileKind;

/// The kind of a file by its first bytes: the formats a run reads, and `Other` for every other
/// one. `reforge_magic` says what those are.
pub fn file_kind(bytes: &[u8]) -> FileKind {
    kind_of(HeadKind::of(bytes))
}

/// The kind of a file by what its first bytes say, the way [`file_kind`] reads them.
pub fn kind_of(head: HeadKind) -> FileKind {
    match head.identify(None).kind {
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
