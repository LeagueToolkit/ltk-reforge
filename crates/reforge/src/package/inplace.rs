//! A repair's package (#ADR-0011): the mod as it is, each file the client reaches at its own
//! WAD and path (#ADR-0039). A bin an object of which a fixup changed is written again, and
//! every other file keeps its bytes.

use std::collections::BTreeSet;

use super::config::{Made, ProjectConfig};
use super::findings::{Empty, InPlace};
use super::shaders;
use super::texture::TexContainer;
use super::wired::LeftOut;
use super::wired::{LeftBehind, Wired};
use super::{BinWrite, Chunk, ChunkKey, Layer, Origin, Package, WrittenBin};
use crate::diag::{Halted, Sink};
use crate::env::Env;
use crate::fixups::{Changes, FixedMod};
use crate::game::{GameSource, NameBook, Names, Speller};
use crate::load::{Fault, LoadOptions, Staleness};
use crate::model::entity::{BinId, FileKind, FileNode, FileSource};
use crate::model::value::{self, FileRef};
use crate::model::{ChunkPath, Mod, Spell};

/// Why a build gives no package.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// A fatal diagnostic: the package would hold nothing.
    #[error(transparent)]
    Halted(#[from] Halted),
    /// An input of the run did not answer: a fault of the machine, and no diagnostic.
    #[error("an input of the run did not answer while the package was built")]
    Fault(#[from] Fault),
}

/// Builds a repair's package from a fixed mod and what the fixups changed in it.
pub struct InPlaceBuilder<'a> {
    fixed: &'a FixedMod,
    changes: &'a Changes,
    names: Option<(&'a NameBook, &'a Names)>,
    stale: Option<Staleness<'a>>,
    game: Option<&'a dyn GameSource>,
    keep_dds: bool,
}

impl<'a> InPlaceBuilder<'a> {
    /// A builder for a fixed mod. Without names a file whose path the archive does not spell
    /// goes under its hash, and the package ships no name.
    pub fn new(fixed: &'a FixedMod, changes: &'a Changes) -> Self {
        Self {
            fixed,
            changes,
            names: None,
            stale: None,
            game: None,
            keep_dds: false,
        }
    }

    /// Ships each DDS as it is (#ADR-0055).
    #[must_use]
    pub fn keep_dds(mut self, keep: bool) -> Self {
        self.keep_dds = keep;
        self
    }

    /// What the mod was loaded against, and under which options. A texture put in TEX's
    /// container is judged there as `load` judged the file: one that is a stale copy in
    /// that container is left out (#ADR-0023, #ADR-0039). A material's shader is judged
    /// against the game's shader bin (#ADR-0069), and the variant it asks for against the
    /// game's shader cache (#GAME-BEHAVIOR-0029). Without it none is judged.
    #[must_use]
    pub fn judged(mut self, env: &'a Env, options: &LoadOptions) -> Self {
        self.stale = Some(Staleness::new(env, options));
        self.game = Some(env.game());
        self
    }

    /// The names of the run: what it learnt, and the tables. They spell a path the archive
    /// does not, and what the run learnt goes into the package's hash tables.
    #[must_use]
    pub fn names(mut self, learnt: &'a NameBook, tables: &'a Names) -> Self {
        self.names = Some((learnt, tables));
        self
    }

    /// The package: every file the client reaches, at its own WAD and path, in the base
    /// layer. A package that would hold nothing, or a material the client crashes on, ends the
    /// run.
    pub fn build(self, sink: &mut Sink<'_>) -> Result<Package, BuildError> {
        let m: &Mod = self.fixed;
        let _span = tracing::info_span!("build", files = m.files.len()).entered();
        if let Some(game) = self.game {
            let objects = || m.objects.values().map(|object| &object.value);
            if let Some(missing) = shaders::missing(objects(), game).map_err(Fault::from)? {
                return Err(sink.halt(missing).into());
            }
            let variants = shaders::variants_missing(objects(), game);
            if let Some(missing) = variants.map_err(Fault::from)? {
                return Err(sink.halt(missing).into());
            }
        }
        let mut package = Package::new(ProjectConfig::of(m, Made::Repaired));
        let wired = Wired::by(m.objects.values().map(|object| &object.value));
        let mut found = InPlace {
            kept: 0,
            rewritten: 0,
            whole: 0,
            made: 0,
            left: LeftBehind::default(),
            unnamed: 0,
        };
        let mut container = TexContainer::new().keep_dds(self.keep_dds);
        for (id, file) in m.files.iter() {
            if let Err(reason) = wired.judge(file) {
                found.left.count(reason);
                continue;
            }
            let named = self.path_of(file);
            let path = named.clone().unwrap_or_else(|| hash_path(file));
            let contained = container.contain(&path, &file.bytes);
            if let Some(tex) = &contained
                && let Some(stale) = &self.stale
                && stale.of_bytes(m, id, tex)?
            {
                found.left.count(LeftOut::Stale);
                continue;
            }
            found.unnamed += usize::from(named.is_none());
            let chunk = match self.fixed.edited_bin(self.changes, id) {
                Some(bin) => {
                    let (bytes, written) = self.rewrite(bin);
                    found.rewritten += 1;
                    found.whole += usize::from(written.how == BinWrite::Whole);
                    Chunk {
                        bytes: bytes.into(),
                        origin: Origin::Rewritten { file: id, written },
                    }
                }
                None => match contained {
                    Some(tex) => {
                        container.shipped();
                        Chunk {
                            bytes: tex.into(),
                            origin: Origin::Contained(id),
                        }
                    }
                    None => Chunk::kept(id, file.bytes.clone()),
                },
            };
            found.kept += 1;
            found.made += usize::from(matches!(file.source, FileSource::Made { .. }));
            let key = ChunkKey::new(m.wads[file.wad].name.clone(), path);
            package.insert(Layer::Base, key, chunk);
        }
        if package.is_empty() {
            let empty = Empty {
                files: m.files.len(),
            };
            return Err(sink.halt(empty).into());
        }
        container.report(sink);
        self.fill_tables(&mut package);
        sink.raise(found);
        Ok(package)
    }

    /// A file's path: as the archive or `load` spells it, else as the run's names spell its
    /// hash now. A name that is no path a writer may use spells nothing.
    fn path_of(&self, file: &FileNode) -> Option<ChunkPath> {
        if let Some(path) = &file.path {
            return Some(path.clone());
        }
        let (learnt, tables) = self.names?;
        let spelled = Speller::new(learnt, tables).path(file.hash)?;
        let path = ChunkPath::parse(&spelled).ok()?;
        (path.hash() == file.hash).then_some(path)
    }

    /// A bin's bytes with its changed objects written again, and what they were built to hold.
    fn rewrite(&self, bin: BinId) -> (Vec<u8>, WrittenBin) {
        let m: &Mod = self.fixed;
        let objects = m.bins[bin].objects.iter();
        let entries: BTreeSet<_> = objects.map(|&object| m.objects[object].entry).collect();
        let rewritten = self.fixed.rewrite(self.changes, bin);
        let written = WrittenBin {
            how: rewritten.how,
            entries,
            edited: rewritten.edited,
        };
        (rewritten.bytes, written)
    }

    /// The names the package ships: the path of each file an object names by its hash, where
    /// the run learnt the path.
    fn fill_tables(&self, package: &mut Package) {
        let Some((learnt, _)) = self.names else {
            return;
        };
        let m: &Mod = self.fixed;
        for object in m.objects.values() {
            for file in value::file_refs(&object.value) {
                if let FileRef::Hash(hash) = file
                    && let Some(path) = learnt.path(hash)
                {
                    package.tables.add_path(&path.to_ascii_lowercase());
                }
            }
        }
    }
}

/// The path of a file no table names: its hash at its WAD's root, which a packer reads back
/// as the hash (#MOD-ARTIFACT-0007).
///
/// # Panics
///
/// When sixteen hex digits and a kind's name make no path, which none of them does.
fn hash_path(file: &FileNode) -> ChunkPath {
    let extension = match file.kind {
        FileKind::Other => "dat",
        kind => kind.as_str(),
    };
    ChunkPath::of_hash(file.hash, extension).unwrap_or_else(|error| {
        panic!(
            "the hash {:016x} with the extension {extension} is no path: {error}",
            file.hash.0
        )
    })
}
