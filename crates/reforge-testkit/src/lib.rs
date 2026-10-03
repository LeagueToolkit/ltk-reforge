//! Fixtures for tests: a game and a history in memory, builders of bins and of archives, and a
//! synthetic game with its skins. A test builds what it reads; nothing here reads a file of the
//! machine.

pub mod archive;
pub mod bins;
pub mod game;
pub mod history;
pub mod mods;
pub mod skins;

pub use self::game::MemoryGame;
pub use self::history::{BrokenHistory, MemoryHistory};
pub use self::mods::{LoadRun, ModArchive};
