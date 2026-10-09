//! A mod as the window shows it when it is picked, before any run: the library's read of the
//! archive alone (`reforge::peek`), every string through `clean` as the views of a run are.

use camino::Utf8Path;
use reforge::archive::Caps;
use reforge::diag::Render;
use reforge::model::clean;
use reforge::peek::{NotRead, peek_file};
use serde::{Deserialize, Serialize};

use super::drawn::data_url;
use super::views::ModView;
use crate::error::AppError;
use crate::view::through_clean;

/// A character the archive ships skin bins for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CharacterPeek {
    /// Its name, as the paths spell it.
    pub name: String,
    /// The slots its skin bins ship, in order.
    pub slots: Vec<u16>,
}

/// An archive read alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ModPeek {
    /// The mod, as its archive and `META/info.json` name it.
    pub meta: ModView,
    /// The characters its paths name, each with its slots.
    pub characters: Vec<CharacterPeek>,
    /// Its thumbnail as a `data:` URL: the WebP the library encoded from the mod's image
    /// (#ADR-0051), never the archive's own bytes.
    pub thumbnail: Option<String>,
}

/// Reads the archive at `path` alone, under the standard caps.
pub fn peek(path: &Utf8Path) -> Result<ModPeek, AppError> {
    let peek = peek_file(path, &Caps::default()).map_err(|not_read| unreadable(path, &not_read))?;
    let characters = peek
        .characters
        .into_iter()
        .map(|character| CharacterPeek {
            name: clean(&character.name).into_owned(),
            slots: character.slots.into_iter().map(|slot| slot.0).collect(),
        })
        .collect();
    Ok(ModPeek {
        meta: through_clean(&peek.meta),
        characters,
        thumbnail: peek.thumbnail.as_ref().map(data_url),
    })
}

/// The halting diagnostic as the error the window shows, its text cleaned.
fn unreadable(path: &Utf8Path, not_read: &NotRead) -> AppError {
    let halting = not_read.log.get(not_read.halted.0);
    let detail = halting.map_or_else(String::new, |d| d.message(&Render::plain()));
    AppError::ModUnreadable {
        path: path.to_string(),
        detail: clean(&detail).into_owned(),
    }
}
