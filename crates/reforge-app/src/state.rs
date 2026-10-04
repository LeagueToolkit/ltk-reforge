//! What the app holds while it runs, beside its settings: the inputs opened for the settings
//! as they stand.

use std::sync::Arc;

use camino::Utf8PathBuf;
use parking_lot::Mutex;

use crate::error::AppError;
use crate::inputs::{Choice, Inputs};

/// The inputs opened for one choice, opened again when the choice changes.
pub struct OpenInputs {
    names_index: Utf8PathBuf,
    current: Mutex<Option<(Choice, Arc<Inputs>)>>,
}

impl OpenInputs {
    /// Nothing opened yet. The name-hash index is kept in `names_index`.
    pub fn new(names_index: Utf8PathBuf) -> Self {
        Self {
            names_index,
            current: Mutex::new(None),
        }
    }

    /// The inputs a choice names: the ones open when the choice is the same, else opened now.
    ///
    /// The lock is held while they open, so two calls never open the same install twice.
    pub fn get(&self, choice: &Choice) -> Result<Arc<Inputs>, AppError> {
        let mut current = self.current.lock();
        if let Some((open, inputs)) = &*current
            && open == choice
        {
            return Ok(Arc::clone(inputs));
        }
        *current = None;
        let inputs = Arc::new(Inputs::open(choice, &self.names_index)?);
        *current = Some((choice.clone(), Arc::clone(&inputs)));
        Ok(inputs)
    }
}
