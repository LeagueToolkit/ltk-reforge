//! The categories of a kind, as a set of flags.

use serde::Serialize;

/// What kind of thing a file is, as flags: a kind may be several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Categories(u16);

impl Categories {
    /// The empty set.
    pub const NONE: Categories = Categories(0);
    /// A format the League client reads.
    pub const GAME: Categories = Categories(1);
    /// Text a person reads in an editor.
    pub const TEXT: Categories = Categories(1 << 1);
    /// An image, flat or layered.
    pub const IMAGE: Categories = Categories(1 << 2);
    /// Sound, or a container of sounds.
    pub const AUDIO: Categories = Categories(1 << 3);
    /// A video.
    pub const VIDEO: Categories = Categories(1 << 4);
    /// A font, or a collection of fonts.
    pub const FONT: Categories = Categories(1 << 5);
    /// Geometry, a skeleton, an animation or a scene of them.
    pub const MODEL: Categories = Categories(1 << 6);
    /// An authoring file: a scene, a layered image, a bin as text. Useful to recover a skin's
    /// sources, never read by the client.
    pub const SOURCE: Categories = Categories(1 << 7);
    /// A program or a script: something that runs.
    pub const EXECUTABLE: Categories = Categories(1 << 8);
    /// A container of other files, or a compressed stream.
    pub const ARCHIVE: Categories = Categories(1 << 9);
    /// Left by an operating system or a file browser.
    pub const OS_ARTIFACT: Categories = Categories(1 << 10);
    /// A document written for a person to read.
    pub const DOCUMENT: Categories = Categories(1 << 11);

    const NAMES: [(Categories, &'static str); 12] = [
        (Self::GAME, "game"),
        (Self::TEXT, "text"),
        (Self::IMAGE, "image"),
        (Self::AUDIO, "audio"),
        (Self::VIDEO, "video"),
        (Self::FONT, "font"),
        (Self::MODEL, "model"),
        (Self::SOURCE, "source"),
        (Self::EXECUTABLE, "executable"),
        (Self::ARCHIVE, "archive"),
        (Self::OS_ARTIFACT, "os-artifact"),
        (Self::DOCUMENT, "document"),
    ];

    /// The categories of either set.
    pub const fn union(self, other: Categories) -> Categories {
        Categories(self.0 | other.0)
    }

    /// Whether every category of `other` is in this set.
    ///
    /// The empty set is contained in none, itself included.
    pub const fn contains(self, other: Categories) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    /// The names of the set's categories, in the order of their flags. The set serialises as
    /// this list.
    pub fn names(self) -> Vec<&'static str> {
        Self::NAMES
            .iter()
            .filter(|(c, _)| self.contains(*c))
            .map(|(_, n)| *n)
            .collect()
    }
}

impl std::ops::BitOr for Categories {
    type Output = Categories;
    fn bitor(self, rhs: Categories) -> Categories {
        self.union(rhs)
    }
}

impl Serialize for Categories {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.names())
    }
}
