//! Files in a format the game dropped, made the file it reads at the same stem
//! (#GAME-CHANGE-0007, #GAME-CHANGE-0008).
//!
//! The game's bins name the twin, so a mod's file at the dropped path is never read. The twin
//! is made beside it where the game ships the twin and the mod does not. The original stays,
//! for what the mod's own bins name.
//!
//! - A DDS is made the `.tex` with its bytes as they are (#GAME-BEHAVIOR-0003).
//! - An SCO is read and written as SCB through `ltk_mesh`.
//! - A file already of the kind the twin holds, under the dropped extension, is made the twin
//!   with its bytes as they are.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use ltk_mesh::StaticMesh;
use serde::{Serialize, Serializer};

use super::files::{FileFixCx, FileFixup, NewFile};
use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Subject};
use crate::game::Game;
use crate::history::HistoryError;
use crate::model::entity::{FileId, FileKind, MadeBy, WadId};
use crate::model::{ChunkPath, panic_text};

/// A format the game dropped for another at the same stem.
#[derive(Debug, PartialEq, Eq)]
pub struct DroppedFormat {
    /// The extension the game dropped.
    pub dropped: &'static str,
    /// The extension the game reads at the same stem.
    pub read: &'static str,
    /// The kind of file the game holds under the extension it reads.
    pub kind: FileKind,
    /// The game change that dropped the format.
    pub record: Record,
}

/// The formats the game dropped.
pub static FORMATS: [DroppedFormat; 2] = [
    DroppedFormat {
        dropped: "dds",
        read: "tex",
        kind: FileKind::Tex,
        record: Record::game_change(7),
    },
    DroppedFormat {
        dropped: "sco",
        read: "scb",
        kind: FileKind::Scb,
        record: Record::game_change(8),
    },
];

/// The path the game reads the stem of `path` at, and the dropped format `path` is in by its
/// extension. `None` for a path in no dropped format.
pub fn twin(path: &ChunkPath) -> Option<(ChunkPath, &'static DroppedFormat)> {
    let extension = path.extension()?;
    let format = FORMATS.iter().find(|f| f.dropped == extension)?;
    let stem = path.as_str().strip_suffix(format.dropped)?;
    let twin = ChunkPath::parse(&format!("{stem}{}", format.read)).ok()?;
    Some((twin, format))
}

/// How a file was made from another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Conversion {
    /// A DDS at a `.tex` path, its bytes as they are (#GAME-BEHAVIOR-0003).
    DdsAsTex,
    /// An SCO read and written as SCB.
    ScoAsScb,
    /// The bytes as they are, of the kind the path holds or under the extension they had.
    AsIs,
}

impl Conversion {
    /// Every conversion, in the order a message counts them.
    pub const ALL: [Conversion; 3] = [Conversion::DdsAsTex, Conversion::ScoAsScb, Conversion::AsIs];
}

/// Why a file does not convert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "why", rename_all = "kebab-case")]
pub enum ConvertError {
    /// The SCO declares more vertices or faces than its bytes can list.
    #[error("{declared} {what} declared, and the file's {bytes} bytes can list {fit}")]
    CountLargerThanData {
        /// What is counted: `vertices` or `faces`.
        what: &'static str,
        /// The count the text declares.
        declared: usize,
        /// The bytes of the file.
        bytes: usize,
        /// The most of them those bytes can list.
        fit: usize,
    },
    /// `ltk_mesh` refused the SCO.
    #[error("not a readable SCO: {reason}")]
    Read {
        /// What failed.
        reason: String,
    },
    /// `ltk_mesh` did not write the mesh as SCB.
    #[error("the SCB does not write: {reason}")]
    Write {
        /// What failed.
        reason: String,
    },
    /// `ltk_mesh` panicked on the bytes.
    #[error("the reader panicked: {reason}")]
    Panicked {
        /// The panic's message.
        reason: String,
    },
}

/// The least bytes a vertex of an SCO takes: three numbers of one digit, two spaces and the
/// line end.
const VERTEX_LINE_BYTES: usize = 6;

/// The least bytes a face of an SCO takes: eleven fields of one character and ten spaces.
const FACE_LINE_BYTES: usize = 21;

/// Refuses an SCO that declares more vertices or faces than its bytes can list.
fn check_counts(bytes: &[u8]) -> Result<(), ConvertError> {
    // `StaticMesh::from_ascii` (`ltk_mesh` 0.4.6) reserves memory for the counts the text
    // declares before it reads one vertex or face. An allocation that fails aborts the process,
    // which `catch_unwind` does not catch. The counts are looked for on the lines the reader
    // takes them from. A text the reader refuses before it takes a count is left to the reader.
    fn field(line: &str, at: usize) -> Option<&str> {
        line.split_whitespace().nth(at)
    }
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split('\n');
    let fits = |what: &'static str, declared: usize, line_bytes: usize| {
        let fit = bytes.len() / line_bytes;
        if declared <= fit {
            return Ok(());
        }
        Err(ConvertError::CountLargerThanData {
            what,
            declared,
            bytes: bytes.len(),
            fit,
        })
    };

    // The header, the name and the central point come first, then two optional lines.
    let Some(mut line) = lines.nth(3) else {
        return Ok(());
    };
    if field(line, 0) == Some("PivotPoint=") {
        let Some(next) = lines.next() else {
            return Ok(());
        };
        line = next;
    }
    let mut colors = false;
    if field(line, 0) == Some("VertexColors=") {
        colors = field(line, 1)
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0)
            != 0;
        let Some(next) = lines.next() else {
            return Ok(());
        };
        line = next;
    }
    if field(line, 0) != Some("Verts=") {
        return Ok(());
    }
    let Some(vertices) = field(line, 1).and_then(|n| n.parse::<usize>().ok()) else {
        return Ok(());
    };
    fits("vertices", vertices, VERTEX_LINE_BYTES)?;

    // The face count is on the line after the vertices and their colors.
    let before_faces = vertices * if colors { 2 } else { 1 };
    let Some(line) = lines.nth(before_faces) else {
        return Ok(());
    };
    let Some(faces) = field(line, 1).and_then(|n| n.parse::<usize>().ok()) else {
        return Ok(());
    };
    fits("faces", faces, FACE_LINE_BYTES)
}

/// An SCO as the SCB the game reads. The bytes may be a mod's, so `ltk_mesh` runs where its
/// panic is caught.
pub fn sco_to_scb(bytes: &[u8]) -> Result<Vec<u8>, ConvertError> {
    check_counts(bytes)?;
    let convert = || {
        let mesh =
            StaticMesh::from_ascii(&mut Cursor::new(bytes)).map_err(|e| ConvertError::Read {
                reason: e.to_string(),
            })?;
        let mut out = Vec::new();
        mesh.to_writer(&mut out).map_err(|e| ConvertError::Write {
            reason: e.to_string(),
        })?;
        Ok(out)
    };
    catch_unwind(AssertUnwindSafe(convert)).unwrap_or_else(|payload| {
        Err(ConvertError::Panicked {
            reason: panic_text(&*payload),
        })
    })
}

/// A file a fixup made from another, before it is added to the mod.
pub(super) struct Made {
    /// Its bytes.
    pub(super) bytes: Arc<[u8]>,
    /// The kind the bytes are.
    pub(super) kind: FileKind,
    /// How it was made.
    pub(super) how: Conversion,
}

impl Made {
    /// The file to add at `to`, made from `from` by a rule.
    pub(super) fn at(self, from: FileId, to: ChunkPath, by: MadeBy) -> NewFile {
        NewFile {
            from,
            by,
            to,
            bytes: self.bytes,
            kind: self.kind,
        }
    }
}

/// The bytes of a file of kind `from` as the file at a path holding `to`: the same bytes when
/// the kinds agree or the client reads either, an SCB written from an SCO. `None` when no
/// conversion exists.
pub(super) fn make(
    bytes: &Arc<[u8]>,
    from: FileKind,
    to: FileKind,
) -> Result<Option<Made>, ConvertError> {
    let as_it_is = |how| Made {
        bytes: Arc::clone(bytes),
        kind: from,
        how,
    };
    Ok(match (from, to) {
        (from, to) if from == to => Some(as_it_is(Conversion::AsIs)),
        // #GAME-BEHAVIOR-0003
        (FileKind::Dds, FileKind::Tex) => Some(as_it_is(Conversion::DdsAsTex)),
        (FileKind::Sco, FileKind::Scb) => Some(Made {
            bytes: sco_to_scb(bytes)?.into(),
            kind: FileKind::Scb,
            how: Conversion::ScoAsScb,
        }),
        _ => None,
    })
}

/// A record as a report holds it: its code, `#GAME-CHANGE-0007`.
fn record_code<S: Serializer>(record: &Record, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(record)
}

/// Records as a report holds them: their codes.
pub(super) fn record_codes<S: Serializer>(records: &[Record], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(records.iter().map(ToString::to_string))
}

/// Writes how many files were made each way: `2 .dds as .tex, unchanged; 1 .sco as .scb`, then
/// the count of those made as they are with `as_is`.
pub(super) fn count_by_conversion(f: &mut Message<'_>, made: &[Conversion], as_is: &str) {
    let mut first = true;
    for how in Conversion::ALL {
        let n = made.iter().filter(|h| **h == how).count();
        if n == 0 {
            continue;
        }
        if !first {
            f.text("; ");
        }
        first = false;
        let text = match how {
            Conversion::DdsAsTex => ".dds as .tex, unchanged",
            Conversion::ScoAsScb => ".sco as .scb",
            Conversion::AsIs => as_is,
        };
        let _ = write!(f, "{n} {text}");
    }
}

/// A file made in the format the game reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Converted {
    /// The file in the dropped format.
    pub from: FileId,
    /// The file made of it.
    pub made: FileId,
    /// How it was made.
    pub how: Conversion,
    /// The game change that dropped the format.
    #[serde(serialize_with = "record_code")]
    pub record: Record,
}

/// Files of a WAD in a format the game dropped, made the file it reads at the same stem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormatsConverted {
    /// The WAD.
    pub wad: WadId,
    /// The files made, in the order made.
    pub files: Vec<Converted>,
}

impl Diag for FormatsConverted {
    const CODE: Code = Code::new("file.format-converted", Channel::Output, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" at paths the game dropped, made the file it reads at the same stem: ");
        let made: Vec<Conversion> = self.files.iter().map(|c| c.how).collect();
        count_by_conversion(f, &made, "already in the format the game reads, copied");
    }
}

/// A file that does not convert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unconverted {
    /// The file.
    pub file: FileId,
    /// Why it does not convert.
    #[serde(flatten)]
    pub why: ConvertError,
}

/// Files of a WAD in a format the game dropped that do not read. They stay where they are, and
/// no twin is made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormatsUnreadable {
    /// The WAD.
    pub wad: WadId,
    /// The files, in file order.
    pub files: Vec<Unconverted>,
}

impl Diag for FormatsUnreadable {
    const CODE: Code = Code::new("file.format-unreadable", Channel::Output, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" at paths the game dropped that do not read, left where they are: ");
        f.list(&self.files, |f, e| {
            f.file(e.file)
                .text(" (")
                .spelled(&e.why.to_string())
                .text(")");
        });
    }
}

/// The codes the legacy formats raise.
pub const CODES: &[Code] = &[FormatsConverted::CODE, FormatsUnreadable::CODE];

/// What the fixup found in one WAD.
#[derive(Default)]
struct Found {
    converted: Vec<Converted>,
    unreadable: Vec<Unconverted>,
}

/// The file fixup `legacy-formats`. It implements two game changes, so each file made names
/// the one that dropped its format.
#[derive(Debug, Clone, Copy, Default)]
pub struct LegacyFormats;

impl LegacyFormats {
    /// The twin to make of a file, and the dropped format the file's path is in. `None` unless
    /// the file is a texture or a static mesh at a path the game does not ship, and the game
    /// ships the twin, and the mod does not.
    fn twin_to_make(
        cx: &FileFixCx<'_, '_>,
        file: FileId,
    ) -> Option<(ChunkPath, &'static DroppedFormat)> {
        let node = &cx.module.files[file];
        let of_a_dropped_format = matches!(
            node.kind,
            FileKind::Dds | FileKind::Sco | FileKind::Tex | FileKind::Scb
        );
        if !of_a_dropped_format || cx.game.contains(node.hash) {
            return None;
        }
        let (to, format) = twin(&cx.path_of(file)?)?;
        let wanted =
            cx.game.contains(to.hash()) && cx.module.file_at(node.wad, to.hash()).is_none();
        wanted.then_some((to, format))
    }
}

impl FileFixup for LegacyFormats {
    fn id(&self) -> &'static str {
        "legacy-formats"
    }

    fn records(&self) -> &'static [Record] {
        const RECORDS: &[Record] = &[Record::game_change(7), Record::game_change(8)];
        RECORDS
    }

    fn destinations(&self, path: &ChunkPath, _game: &dyn Game) -> Vec<ChunkPath> {
        twin(path).map(|(twin, _)| twin).into_iter().collect()
    }

    fn fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError> {
        let mut found: BTreeMap<WadId, Found> = BTreeMap::new();
        let files: Vec<FileId> = cx.module.files.ids().collect();
        for from in files {
            let Some((to, format)) = Self::twin_to_make(cx, from) else {
                continue;
            };
            let file = &cx.module.files[from];
            let wad = file.wad;
            match make(&file.bytes, file.kind, format.kind) {
                Ok(None) => {}
                Ok(Some(made)) => {
                    let how = made.how;
                    // The twin is made only where the mod ships none, and a WAD holds one
                    // chunk per path: a file there is the mod's twin.
                    let Ok(made) = cx.add(made.at(from, to, MadeBy::LegacyFormat)) else {
                        continue;
                    };
                    found.entry(wad).or_default().converted.push(Converted {
                        from,
                        made,
                        how,
                        record: format.record,
                    });
                }
                Err(why) => {
                    let unreadable = &mut found.entry(wad).or_default().unreadable;
                    unreadable.push(Unconverted { file: from, why });
                }
            }
        }
        for (wad, found) in found {
            if !found.converted.is_empty() {
                cx.sink.raise(FormatsConverted {
                    wad,
                    files: found.converted,
                });
            }
            if !found.unreadable.is_empty() {
                cx.sink.raise(FormatsUnreadable {
                    wad,
                    files: found.unreadable,
                });
            }
        }
        Ok(())
    }
}
