//! What a run is judged against: the game, the history, the names and the schema, built once.
//! A stage reads it and changes nothing in it. A stage's options are its own and are not here.
//!
//! `docs/design/ARCHITECTURE.md`, "The environment".

use std::sync::Arc;

use crate::game::{Game, GameView, Install, Names, Schema, SizeIndex};
use crate::history::{History, NoHistory};

/// Why an environment is not built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EnvError {
    /// No game was given: a run has nothing to judge a mod against.
    #[error("no game was given to the environment")]
    NoGame,
}

/// Where the game's chunks by size come from.
enum Sizes {
    /// An index a caller built from the game's chunks, or an empty one.
    Given(SizeIndex),
    /// The install's own.
    Install(Arc<Install>),
}

/// What a run is judged against.
pub struct Env {
    game: GameView,
    sizes: Sizes,
    history: Box<dyn History>,
    names: Names,
    schema: Option<Schema>,
}

impl Env {
    /// A builder with nothing given. The game is required. Without a history every question
    /// to it is answered with nothing, and without names every hash prints as hex.
    pub fn builder() -> EnvBuilder {
        EnvBuilder {
            game: None,
            sizes: Sizes::Given(SizeIndex::new()),
            history: Box::new(NoHistory),
            names: Names::default(),
            schema: None,
        }
    }

    /// The target game.
    pub fn game(&self) -> &GameView {
        &self.game
    }

    /// The chunks of each champion WAD of the game by their size. Empty when none was given: a
    /// copy of a game file under another path is then not found.
    pub fn sizes(&self) -> &SizeIndex {
        match &self.sizes {
            Sizes::Given(sizes) => sizes,
            Sizes::Install(install) => install.sizes(),
        }
    }

    /// The build history.
    pub fn history(&self) -> &dyn History {
        &*self.history
    }

    /// Whether the run has a history. Without one nothing is known of earlier builds, which a
    /// finding says where it counts files by their past.
    pub fn has_history(&self) -> bool {
        self.history.stamp().is_some()
    }

    /// The name tables.
    pub fn names(&self) -> &Names {
        &self.names
    }

    /// The schema of the target build, when a database was given.
    pub fn schema(&self) -> Option<&Schema> {
        self.schema.as_ref()
    }
}

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("game", &self.game)
            .field("history", &self.has_history())
            .field("schema", &self.schema.is_some())
            .finish_non_exhaustive()
    }
}

/// Builds an [`Env`] from the inputs a caller opened.
pub struct EnvBuilder {
    game: Option<Box<dyn Game>>,
    sizes: Sizes,
    history: Box<dyn History>,
    names: Names,
    schema: Option<Schema>,
}

impl EnvBuilder {
    /// The target game: an install, with its own size index.
    #[must_use]
    pub fn install(mut self, install: Install) -> Self {
        let install = Arc::new(install);
        self.game = Some(Box::new(Arc::clone(&install)));
        self.sizes = Sizes::Install(install);
        self
    }

    /// The target game, when it is no install. Its size index is given apart.
    #[must_use]
    pub fn game(mut self, game: impl Game + 'static) -> Self {
        self.game = Some(Box::new(game));
        self
    }

    /// The chunks of the game's champion WADs by size, for a game that is no install.
    #[must_use]
    pub fn sizes(mut self, sizes: SizeIndex) -> Self {
        self.sizes = Sizes::Given(sizes);
        self
    }

    /// The build history, as `history::open` gives it.
    #[must_use]
    pub fn history(mut self, history: Box<dyn History>) -> Self {
        self.history = history;
        self
    }

    /// The name tables.
    #[must_use]
    pub fn names(mut self, names: Names) -> Self {
        self.names = names;
        self
    }

    /// The schema of the target build, or none.
    #[must_use]
    pub fn schema(mut self, schema: impl Into<Option<Schema>>) -> Self {
        self.schema = schema.into();
        self
    }

    /// The environment.
    pub fn build(self) -> Result<Env, EnvError> {
        Ok(Env {
            game: GameView::new(self.game.ok_or(EnvError::NoGame)?),
            sizes: self.sizes,
            history: self.history,
            names: self.names,
            schema: self.schema,
        })
    }
}
