//! How a failure reaches the front end: [`AppError`], tagged on a `code` the front end matches
//! on and carrying the fields it translates over, and the [`IpcResult`] every command returns.
//! The front end owns every sentence a person reads, as in LTK Manager, so no variant carries
//! one. A `detail` is text from outside the app, such as an OS error, shown as data.

use serde::Serialize;
use specta::datatype::{DataType, Enum, Field, Variant};

/// What went wrong, as the fields the front end translates over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, thiserror::Error)]
#[serde(
    tag = "code",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum AppError {
    /// The settings file did not write.
    #[error("the settings did not save: {detail}")]
    SettingsUnwritable { detail: String },
    /// The app could not name its data directory.
    #[error("no data directory: {detail}")]
    NoDataDir { detail: String },
}

/// What a command answers: `{ ok: true, value }` or `{ ok: false, error }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcResult<T> {
    /// The command's value.
    Ok { value: T },
    /// Why the command has no value.
    Err { error: AppError },
}

impl<T> IpcResult<T> {
    /// A command's value.
    pub fn ok(value: T) -> Self {
        Self::Ok { value }
    }

    /// A command's failure.
    pub fn err(error: AppError) -> Self {
        tracing::warn!(%error, "a command failed");
        Self::Err { error }
    }
}

impl<T, E: Into<AppError>> From<Result<T, E>> for IpcResult<T> {
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self::ok(value),
            Err(error) => Self::err(error.into()),
        }
    }
}

// `ok` is the literal `true` or `false`, which no derive writes.
impl<T: Serialize> Serialize for IpcResult<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("IpcResult", 2)?;
        match self {
            Self::Ok { value } => {
                state.serialize_field("ok", &true)?;
                state.serialize_field("value", value)?;
            }
            Self::Err { error } => {
                state.serialize_field("ok", &false)?;
                state.serialize_field("error", error)?;
            }
        }
        state.end()
    }
}

// The two objects the `Serialize` above writes, as the exporter reads them. The pair is one
// edit.
impl<T: specta::Type> specta::Type for IpcResult<T> {
    fn definition(types: &mut specta::Types) -> DataType {
        let mut shape = Enum::default();
        shape.variants = vec![
            (
                "Ok".into(),
                Variant::named()
                    .field("ok", literal("true"))
                    .field("value", Field::new(T::definition(types)))
                    .build(),
            ),
            (
                "Err".into(),
                Variant::named()
                    .field("ok", literal("false"))
                    .field("error", Field::new(AppError::definition(types)))
                    .build(),
            ),
        ];
        // A union of the two objects, not one object per variant name. The exporter reads the
        // representation from `specta_serde`'s key.
        shape.attributes.insert("serde:container:untagged", true);
        DataType::Enum(shape)
    }
}

/// A field holding one TypeScript literal, which specta has no datatype for.
fn literal(value: &'static str) -> Field {
    Field::new(DataType::Reference(specta_typescript::define(value)))
}
