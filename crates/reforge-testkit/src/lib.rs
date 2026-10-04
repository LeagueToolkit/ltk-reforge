//! Fixtures for tests: a game and a history in memory, builders of bins, of archives and of a
//! schema, a synthetic game with its skins, and a mod taken through `load`, `fix` and the build
//! of its package. A test builds what it reads; nothing here reads a file of the machine.

pub mod archive;
pub mod bins;
pub mod fixes;
pub mod game;
pub mod history;
pub mod mods;
pub mod packs;
pub mod schema;
pub mod skins;

pub use self::fixes::FixRun;
pub use self::game::MemoryGame;
pub use self::history::{BrokenHistory, MemoryHistory};
pub use self::mods::{LoadRun, ModArchive};
pub use self::packs::PackRun;
pub use self::schema::SchemaRows;
