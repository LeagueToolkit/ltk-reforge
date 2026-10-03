//! Fixtures for tests: a game and a history in memory, and builders of archives. A test builds
//! what it reads; nothing here reads a file of the machine.

pub mod archive;
pub mod game;
pub mod history;

pub use self::game::MemoryGame;
pub use self::history::MemoryHistory;
