//! Fixtures for tests: a game and a history in memory, builders of bins, of archives and of a
//! schema, a synthetic game with its skins, rig files, shader cache files, and a mod taken
//! through `load`, `fix`, `facts`, the plan of its port and the build of its package, and test
//! mods for checks in game made from a game's bins. A test builds what it reads; only `ingame`,
//! for the examples that write those mods, opens an install and writes files.

pub mod archive;
pub mod bins;
pub mod facts;
pub mod fixes;
pub mod game;
pub mod history;
pub mod ingame;
pub mod mods;
pub mod packs;
pub mod plans;
pub mod ports;
pub mod rigs;
pub mod schema;
pub mod shaders;
pub mod skins;
pub mod sounds;

pub use self::facts::FactRun;
pub use self::fixes::FixRun;
pub use self::game::{BrokenGame, MemoryGame};
pub use self::history::{BrokenHistory, MemoryHistory};
pub use self::mods::{LoadRun, ModArchive};
pub use self::packs::PackRun;
pub use self::plans::PlanRun;
pub use self::ports::PortRun;
pub use self::schema::SchemaRows;
