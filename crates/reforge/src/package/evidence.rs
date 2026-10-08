//! The evidence of a run, embedded in the package the run built (#ADR-0041): the document, the
//! README that carries it, and the reader of that README.
//!
//! A `.modpkg` holds one meta chunk of free text that a packer copies byte for byte, the
//! readme, and a packer takes it from the project's `README.md`. So the document sits in that
//! file: a few lines for a person, then one fenced block whose info string is the document's
//! format and whose body is the document as JSON.
//!
//! The document holds no id of a run's mod: a file is named by its path hash and an object by
//! its entry hash, as a later run finds them. Nothing in it is a clock or a path of the
//! machine.
//!
//! `docs/design/ARCHITECTURE.md`, "Package".

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::config::Made;
use crate::env::Env;
use crate::game::Build;
use crate::load::Dating;
use crate::load::dating::Piece;
use crate::model::{Mod, Spelled, clean, clean_json};
use crate::port::{AnimReuse, Port, TargetPolicy, Targets};

/// The format an evidence document states. A field is added under the same format; a field
/// renamed or retyped takes the next number.
pub const EVIDENCE_FORMAT: &str = "reforge.evidence/1";

/// What the format of every number starts with. A block of another number is refused, where a
/// block of anything else is no evidence.
const FORMAT_FAMILY: &str = "reforge.evidence/";

/// The file at a project's root that holds the document.
pub const README_FILE: &str = "README.md";

/// The largest README the reader takes. A document holds a row per fixup, per restoring WAD
/// and per unit that ships, which is far below it.
pub const MAX_README_BYTES: usize = 1 << 20;

/// The line that opens and closes a fenced block.
const FENCE: &str = "```";

/// How much of a format's name an error keeps.
const SHOWN_FORMAT: usize = 64;

/// Why a README gives no evidence document though it holds a block of one, or is too large to
/// look in.
#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    /// The README is larger than the reader takes.
    #[error("the README is {bytes} bytes, and at most {max} are read", max = MAX_README_BYTES)]
    TooLarge {
        /// Its size.
        bytes: usize,
    },
    /// The block of the evidence opens and no line closes it.
    #[error("the block of the evidence is not closed")]
    Unclosed,
    /// The README holds more than one block of evidence.
    #[error("the README holds more than one block of evidence")]
    Repeated,
    /// The block, or the document in it, states another format.
    #[error("the evidence is of the format {found}, and {read} is read", read = EVIDENCE_FORMAT)]
    Format {
        /// The format stated, up to its first characters.
        found: Spelled,
    },
    /// The block's body is not the JSON of a document.
    #[error("the block is not the JSON of an evidence document")]
    Json(#[from] serde_json::Error),
    /// A field holds a value that is not what the field holds: a digest that is no digest, a
    /// build that is no build, a count above its total.
    #[error("the evidence's {0} is not what the field holds")]
    Field(&'static str),
}

/// The program that made a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
}

/// The archive a package was made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveRef {
    /// The SHA-256 of the archive file, as 64 hex digits.
    pub sha256: String,
}

/// The history a run was judged against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceHistory {
    /// The commit of the build the history took in last. A position of the dating counts in
    /// this history.
    pub head: String,
    /// The highest version among its builds.
    pub newest_build: String,
    /// How many builds it holds.
    pub builds: usize,
}

/// What a run was judged against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceInputs {
    /// The target build: the game's, when it states one.
    pub build: Option<String>,
    /// The history. None when the run was judged against the game alone.
    pub history: Option<EvidenceHistory>,
}

impl EvidenceInputs {
    fn of(env: &Env) -> Self {
        Self {
            build: env.game().build().map(|build| build.as_str().to_string()),
            history: env.history().stamp().map(|stamp| EvidenceHistory {
                head: stamp.head.clone(),
                newest_build: stamp.newest_build.as_str().to_string(),
                builds: stamp.builds,
            }),
        }
    }
}

/// The build a mod was made at or after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBound {
    /// The build's position in the history.
    pub at: u16,
    /// The build.
    pub build: String,
}

/// What kind of piece of a mod dates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PieceKind {
    /// A file, by its bytes.
    File,
    /// An object of a loaded bin, by its bytes from its entry hash to its end.
    Object,
}

/// The piece that sets a mod's dating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePiece {
    /// A file or an object.
    pub piece: PieceKind,
    /// A file's path hash as sixteen hex digits, an object's entry hash as eight.
    pub hash: String,
    /// The SHA-256 of the piece's bytes as the archive ships them, as 64 hex digits: what the
    /// history is asked by.
    pub key: String,
}

/// The dating of a mod: what [`Dating`] holds, with the piece named by its hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceDating {
    /// The build the mod was made at or after. None when the history dates no piece of it.
    pub not_before: Option<EvidenceBound>,
    /// The first piece that first shipped at the bound.
    pub evidence: Option<EvidencePiece>,
    /// How many pieces the history dates.
    pub dated: usize,
    /// How many of them first shipped at the bound.
    pub at_bound: usize,
}

impl EvidenceDating {
    /// The dating of `m`. The pieces of `dating` are ids of `m`.
    fn of(dating: &Dating, m: &Mod) -> Self {
        let evidence = dating.evidence.map(|piece| match piece {
            Piece::File(file) => EvidencePiece {
                piece: PieceKind::File,
                hash: format!("{:016x}", m.files[file].hash.0),
                key: m.files[file].content.hex(),
            },
            Piece::Object(object) => EvidencePiece {
                piece: PieceKind::Object,
                hash: format!("{:08x}", m.objects[object].entry.0),
                key: m.objects[object].raw.hex(),
            },
        });
        Self {
            not_before: dating.not_before.as_ref().map(|bound| EvidenceBound {
                at: bound.at.0,
                build: bound.build.as_str().to_string(),
            }),
            evidence,
            dated: dating.dated,
            at_bound: dating.at_bound,
        }
    }
}

/// The policy a port was decided under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePolicy {
    /// How the target is chosen: `auto`, `base`, `origin`, `slot` or a skin number.
    pub target: String,
    /// The level of animation reuse.
    pub anim_reuse: String,
}

/// A unit a port ships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceUnit {
    /// Its character's directory name, lowercase.
    pub character: String,
    /// The slot its bin sits in, in the archive.
    pub source: u16,
    /// The slot it is ported to.
    pub target: u16,
    /// Why that slot, as the decision names its reason.
    pub reason: String,
}

/// What a port decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePort {
    /// The policy.
    pub policy: EvidencePolicy,
    /// The units that ship, in unit order.
    pub units: Vec<EvidenceUnit>,
}

/// What a run established of a mod and its package does not show again (#ADR-0041), with what
/// made the package and against what.
///
/// A reader ignores a field it does not know, so a field added under the same format reads in
/// an earlier program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceDoc {
    /// The document's format, [`EVIDENCE_FORMAT`].
    pub format: String,
    /// The program that made the package.
    pub program: Program,
    /// The kind of run: a repair or a port.
    pub run: Made,
    /// The archive the package was made from.
    pub archive: ArchiveRef,
    /// What the run was judged against.
    pub inputs: EvidenceInputs,
    /// The dating of the mod.
    pub dating: EvidenceDating,
    /// The WADs that restore a model from before its rework, by name, in lowercase, sorted.
    pub restored: Vec<String>,
    /// The fixups the run ran under, by id, in the order they run.
    pub fixups: Vec<String>,
    /// What a port decided. None for a repair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<EvidencePort>,
}

/// Whether a text is this many lowercase hex digits.
fn is_hex(text: &str, digits: usize) -> bool {
    text.len() == digits
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_build(text: &str) -> bool {
    Build::parse(text).is_some()
}

/// Names the field of a document read whose value is not what the field holds.
fn held(holds: bool, field: &'static str) -> Result<(), EvidenceError> {
    if holds {
        Ok(())
    } else {
        Err(EvidenceError::Field(field))
    }
}

impl EvidenceDoc {
    /// The evidence of a run on a mod: the archive, the inputs, the dating and the
    /// restorations. `dating` is the dating of `m`, found on it.
    pub fn of(made: Made, m: &Mod, dating: &Dating, env: &Env) -> Self {
        let restored = m.restorations.iter();
        let restored: BTreeSet<String> = restored
            .map(|&wad| m.wads[wad].name.lower().to_string())
            .collect();
        Self {
            format: EVIDENCE_FORMAT.to_string(),
            program: Program {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            run: made,
            archive: ArchiveRef {
                sha256: m.meta.sha256.hex(),
            },
            inputs: EvidenceInputs::of(env),
            dating: EvidenceDating::of(dating, m),
            restored: restored.into_iter().collect(),
            fixups: Vec::new(),
            port: None,
        }
    }

    /// The fixups the run ran under, in the order they run.
    #[must_use]
    pub fn fixups<'a>(mut self, ids: impl IntoIterator<Item = &'a str>) -> Self {
        self.fixups = ids.into_iter().map(str::to_string).collect();
        self
    }

    /// What a port of `m` decided: the policy, and each unit that ships with its target.
    #[must_use]
    pub fn ported(mut self, targets: &Targets, port: &Port, m: &Mod) -> Self {
        // A unit ships only with a decision that chose its target (`Shipping`).
        let units = port.units.iter().filter_map(|unit| {
            let decision = targets.decisions.get(&unit.unit)?;
            Some(EvidenceUnit {
                character: m.characters[unit.character].name.clone(),
                source: m.units[unit.unit].source_slot.0,
                target: unit.target.0,
                reason: decision.reason.as_str().to_string(),
            })
        });
        self.port = Some(EvidencePort {
            policy: EvidencePolicy {
                target: targets.policy.target.to_string(),
                anim_reuse: targets.policy.anim_reuse.as_str().to_string(),
            },
            units: units.collect(),
        });
        self
    }

    /// The document as JSON, its keys in the order of the fields: every character a terminal
    /// acts on is written as a JSON escape, so a parser reads back the same text.
    ///
    /// # Panics
    ///
    /// When serde fails to write a document into memory, which no value of one makes it do.
    pub fn to_json(&self) -> String {
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|error| {
            panic!("a document of format {EVIDENCE_FORMAT} does not write: {error}")
        });
        clean_json(&json).into_owned()
    }

    /// The README of a package: what made it, from which archive and for which build, then
    /// the block that holds the document. No text of the mod is outside the block.
    pub fn readme(&self) -> String {
        let (name, version) = (clean(&self.program.name), clean(&self.program.version));
        let run = match self.run {
            Made::Repaired => "a repair",
            Made::Ported => "a port",
        };
        let build = match &self.inputs.build {
            Some(build) => format!("the game build {}", clean(build)),
            None => "a game that does not state its build".to_string(),
        };
        format!(
            "# Package made by {name}\n\
             \n\
             {name} {version} made this package by {run} of a mod archive, for {build}.\n\
             The SHA-256 of the archive is `{sha256}`.\n\
             The block below holds what the run established of the mod, in the format\n\
             `{EVIDENCE_FORMAT}`. A program reads it, so it stays as written.\n\
             \n\
             {FENCE}{EVIDENCE_FORMAT}\n\
             {json}\n\
             {FENCE}\n",
            sha256 = clean(&self.archive.sha256),
            json = self.to_json(),
        )
    }

    /// The document a README holds. None for a README with no block of evidence: a package
    /// without one is judged as an archive is.
    ///
    /// The block is a line of three backticks and the format, the JSON, and a line of three
    /// backticks. The README is untrusted: it is read up to [`MAX_README_BYTES`], a block of
    /// another format, a second block and a body that is no document are refused, and each
    /// field is held to what it holds.
    pub fn parse(readme: &str) -> Result<Option<Self>, EvidenceError> {
        if readme.len() > MAX_README_BYTES {
            return Err(EvidenceError::TooLarge {
                bytes: readme.len(),
            });
        }
        let mut found: Option<String> = None;
        let mut lines = readme.lines();
        while let Some(line) = lines.next() {
            let Some(info) = line.trim_end().strip_prefix(FENCE) else {
                continue;
            };
            let ours = info.starts_with(FORMAT_FAMILY);
            // The body of any block runs to its closing line: a fence inside the block of
            // another language opens nothing.
            let mut body = String::new();
            let mut closed = false;
            for inner in lines.by_ref() {
                if inner.trim_end() == FENCE {
                    closed = true;
                    break;
                }
                if ours {
                    body.push_str(inner);
                    body.push('\n');
                }
            }
            if !ours {
                continue;
            }
            if info != EVIDENCE_FORMAT {
                return Err(Self::other_format(info));
            }
            if !closed {
                return Err(EvidenceError::Unclosed);
            }
            if found.replace(body).is_some() {
                return Err(EvidenceError::Repeated);
            }
        }
        let Some(body) = found else {
            return Ok(None);
        };
        let doc: Self = serde_json::from_str(&body)?;
        doc.check()?;
        Ok(Some(doc))
    }

    fn other_format(found: &str) -> EvidenceError {
        let shown: String = found.chars().take(SHOWN_FORMAT).collect();
        EvidenceError::Format {
            found: Spelled::new(shown),
        }
    }

    /// Holds each field of a document read to what the field holds.
    fn check(&self) -> Result<(), EvidenceError> {
        if self.format != EVIDENCE_FORMAT {
            return Err(Self::other_format(&self.format));
        }
        held(is_hex(&self.archive.sha256, 64), "archive.sha256")?;
        if let Some(build) = &self.inputs.build {
            held(is_build(build), "inputs.build")?;
        }
        let history = self.inputs.history.as_ref();
        if let Some(history) = history {
            held(
                is_build(&history.newest_build),
                "inputs.history.newest_build",
            )?;
        }
        let dating = &self.dating;
        if let Some(bound) = &dating.not_before {
            held(is_build(&bound.build), "dating.not_before.build")?;
            // A bound is a build of the history the run was judged against.
            let within = history.is_some_and(|history| usize::from(bound.at) < history.builds);
            held(within, "dating.not_before.at")?;
        }
        if let Some(piece) = &dating.evidence {
            let digits = match piece.piece {
                PieceKind::File => 16,
                PieceKind::Object => 8,
            };
            held(is_hex(&piece.hash, digits), "dating.evidence.hash")?;
            held(is_hex(&piece.key, 64), "dating.evidence.key")?;
        }
        held(dating.at_bound <= dating.dated, "dating.at_bound")?;
        if let Some(port) = &self.port {
            let policy = &port.policy;
            let target = TargetPolicy::parse(&policy.target).is_ok();
            held(target, "port.policy.target")?;
            let anim_reuse = AnimReuse::parse(&policy.anim_reuse).is_ok();
            held(anim_reuse, "port.policy.anim_reuse")?;
        }
        Ok(())
    }
}
