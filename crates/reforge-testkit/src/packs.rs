//! One archive taken through `load`, `fix` and the build of a repair's package, for tests of
//! the package, and DDS textures written by hand.

use reforge::diag::{Diag, Stage};
use reforge::env::Env;
use reforge::fixups::{FixupSet, ObjectSet, TargetTypes};
use reforge::load::LoadOptions;
use reforge::model::path_hash;
use reforge::package::{BuildError, CheckCx, CheckSet, Chunk, InPlaceBuilder, Package};

use crate::fixes::FixRun;
use crate::mods::ModArchive;

/// A mod loaded, fixed and built in place, with what the stages raised.
pub struct PackRun<'a> {
    env: &'a Env,
    /// The mod after the fixups, with the run's log and names.
    pub fix: FixRun<'a>,
    /// The package, or why there is none.
    pub package: Result<Package, BuildError>,
}

impl<'a> PackRun<'a> {
    /// Loads an archive, applies a set of fixups and builds the repair's package.
    ///
    /// # Panics
    ///
    /// As [`FixRun::of`].
    pub fn of(env: &'a Env, set: &FixupSet, archive: &ModArchive) -> Self {
        Self::under(env, set, archive, LoadOptions::default())
    }

    /// As [`PackRun::of`], with `load` under options of the caller's.
    ///
    /// # Panics
    ///
    /// As [`FixRun::of`].
    pub fn under(env: &'a Env, set: &FixupSet, archive: &ModArchive, options: LoadOptions) -> Self {
        let judged = options.clone();
        let mut fix = FixRun::under(env, set, archive, (options, &|_| ObjectSet::every()));
        let builder = InPlaceBuilder::new(&fix.fixed, &fix.changes)
            .names(&fix.names, env.names())
            .judged(env, &judged);
        let package = builder.build(&mut fix.log.at(Stage::Build));
        Self { env, fix, package }
    }

    /// The package.
    ///
    /// # Panics
    ///
    /// When the build gave none, with what the run raised.
    pub fn package(&self) -> &Package {
        match &self.package {
            Ok(package) => package,
            Err(error) => panic!("no package: {error}\n{:#?}", self.messages()),
        }
    }

    /// Runs a set of checks on the package, against the types the fixups fixed the mod to.
    ///
    /// # Panics
    ///
    /// When there is no package, or the game does not read.
    pub fn check(&mut self, checks: &CheckSet) {
        let types = TargetTypes::of(&self.fix.fixed, self.env)
            .unwrap_or_else(|error| panic!("the game reads: {error}"));
        let Ok(package) = &self.package else {
            panic!("no package to check");
        };
        let cx = CheckCx::new(package, &types);
        let ran = checks.run(&cx, &mut self.fix.log.at(Stage::Check));
        ran.unwrap_or_else(|cancelled| panic!("a log with no cancel ended: {cancelled}"));
    }

    /// The chunk of the base layer at a path, in any WAD.
    ///
    /// # Panics
    ///
    /// When the package holds none there.
    pub fn chunk(&self, path: &str) -> &Chunk {
        let hash = path_hash(path);
        let mut chunks = self.package().chunks();
        let found = chunks.find(|(_, key, _)| key.hash() == hash);
        let (_, _, chunk) = found.unwrap_or_else(|| panic!("no chunk at {path}"));
        chunk
    }

    /// Whether the package holds a chunk at a path.
    pub fn ships(&self, path: &str) -> bool {
        let hash = path_hash(path);
        self.package()
            .chunks()
            .any(|(_, key, _)| key.hash() == hash)
    }

    /// The package as an archive: every chunk loose at its path, to take through a run again.
    pub fn archive(&self) -> ModArchive {
        let chunks = self.package().chunks();
        chunks.fold(ModArchive::new(), |archive, (_, key, chunk)| {
            archive.file(key.wad.as_str(), key.path.as_str(), chunk.bytes.to_vec())
        })
    }

    /// The one finding of a type.
    ///
    /// # Panics
    ///
    /// When the run raised none or several of the type.
    pub fn only<T: Diag>(&self) -> &T {
        let mut found = self.fix.log.of::<T>();
        match (found.next(), found.next()) {
            (Some(finding), None) => finding,
            _ => panic!("not one {}:\n{:#?}", T::CODE, self.messages()),
        }
    }

    /// Every diagnostic of the run as its code and its line for a person.
    pub fn messages(&self) -> Vec<String> {
        let speller = self.fix.speller();
        let cx = reforge::diag::Render {
            names: &speller,
            module: Some(&self.fix.fixed),
        };
        let log = self.fix.log.iter();
        log.map(|d| format!("{} {}", d.code, d.message(&cx)))
            .collect()
    }
}

/// The pixels of a DDS a test writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdsPixels {
    /// BC1 blocks.
    Dxt1,
    /// BC2 blocks, which TEX's container does not hold.
    Dxt3,
    /// BC3 blocks.
    Dxt5,
    /// 32 bits in the order B, G, R, A.
    Bgra8,
    /// 32 bits in the order R, G, B, A.
    Rgba8,
}

impl DdsPixels {
    /// The bytes of one mip of a size.
    pub fn level_bytes(self, width: u32, height: u32) -> usize {
        let blocks = (width.div_ceil(4) * height.div_ceil(4)) as usize;
        match self {
            DdsPixels::Dxt1 => blocks * 8,
            DdsPixels::Dxt3 | DdsPixels::Dxt5 => blocks * 16,
            DdsPixels::Bgra8 | DdsPixels::Rgba8 => (width * height * 4) as usize,
        }
    }
}

/// A DDS of a size with this many mips. Mip `n`, from the largest at 0, holds the byte
/// `n + 1` throughout, but 32-bit pixels, which hold the bytes 0, 1, 2, 3 in each pixel.
pub fn dds(pixels: DdsPixels, (width, height): (u32, u32), mips: u32) -> Vec<u8> {
    const CAPS: u32 = 0x1;
    const HEIGHT: u32 = 0x2;
    const WIDTH: u32 = 0x4;
    const PIXEL_FORMAT: u32 = 0x1000;
    const MIP_COUNT: u32 = 0x2_0000;
    let four_cc = |name: &[u8; 4]| (0x4, u32::from_le_bytes(*name), 0, [0; 4]);
    let (format_flags, name, bits, masks) = match pixels {
        DdsPixels::Dxt1 => four_cc(b"DXT1"),
        DdsPixels::Dxt3 => four_cc(b"DXT3"),
        DdsPixels::Dxt5 => four_cc(b"DXT5"),
        // Uncompressed with alpha: the masks of R, G, B and A.
        DdsPixels::Bgra8 => (0x41, 0, 32, [0xff_0000, 0xff00, 0xff, 0xff00_0000]),
        DdsPixels::Rgba8 => (0x41, 0, 32, [0xff, 0xff00, 0xff_0000, 0xff00_0000]),
    };
    let mut words = vec![
        124,
        CAPS | HEIGHT | WIDTH | PIXEL_FORMAT | MIP_COUNT,
        height,
        width,
        0,
        0,
        mips,
    ];
    words.extend([0; 11]);
    words.extend([32, format_flags, name, bits]);
    words.extend(masks);
    // A texture, with nothing of a cube or a volume.
    words.extend([0x1000, 0, 0, 0, 0]);
    let mut out = b"DDS ".to_vec();
    out.extend(words.iter().flat_map(|word| word.to_le_bytes()));
    assert_eq!(out.len(), 128, "a DDS header is 128 bytes");
    for level in 0..mips {
        let size = ((width >> level).max(1), (height >> level).max(1));
        let bytes = pixels.level_bytes(size.0, size.1);
        match pixels {
            DdsPixels::Bgra8 | DdsPixels::Rgba8 => out.extend((0..bytes).map(|i| (i % 4) as u8)),
            _ => out.extend(std::iter::repeat_n(level as u8 + 1, bytes)),
        }
    }
    out
}
