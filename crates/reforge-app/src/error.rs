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
    /// No game install is chosen.
    #[error("no game install is chosen")]
    NoGame,
    /// The game install chosen does not open.
    #[error("the game at {path} does not open: {detail}")]
    GameUnreadable { path: String, detail: String },
    /// The name tables do not open.
    #[error("the name tables do not open: {detail}")]
    NamesUnreadable { detail: String },
    /// A fixup id the library does not have.
    #[error("{id} is no fixup")]
    UnknownFixup { id: String },
    /// A check id the library does not have.
    #[error("{id} is no check")]
    UnknownCheck { id: String },
    /// A filter of the findings that block the write does not parse.
    #[error("the filter {filter} does not parse: {detail}")]
    BadFilter { filter: String, detail: String },
    /// A rule does not read as the library's.
    #[error("the rule {id} does not read: {detail}")]
    BadRule { id: String, detail: String },
    /// A selection file does not read or write.
    #[error("the selection {path} does not read or write: {detail}")]
    SelectionUnreadable { path: String, detail: String },
    /// A target policy or a level of animation reuse the library does not have.
    #[error("{detail}")]
    BadPolicy { detail: String },
    /// A write was asked for with no folder to write in.
    #[error("no folder to write the package in")]
    NoOutput,
    /// A mod archive does not read: the halting diagnostic of `read`, its text cleaned.
    #[error("the mod at {path} does not read: {detail}")]
    ModUnreadable { path: String, detail: String },
    /// The history index's release did not download.
    #[error("the history index did not download: {detail}")]
    IndexFetch { detail: String },
    /// A file of the release downloaded with other bytes than the release lists.
    #[error("{file} is not the file the release lists")]
    IndexDigest { file: String },
    /// The index downloaded does not open.
    #[error("the index downloaded does not open: {detail}")]
    IndexUnreadable { detail: String },
    /// The index could not be written to disk.
    #[error("the index could not be written: {detail}")]
    IndexUnwritable { detail: String },
    /// A download or an update of the index is under way already.
    #[error("the index is downloading or updating already")]
    IndexBusy,
    /// No release of the census history holds an index of the app's format.
    #[error("no release holds an index of the app's format")]
    NoIndexRelease,
    /// No history index is chosen to update.
    #[error("no history index is chosen")]
    NoIndex,
    /// The census history did not fetch.
    #[error("the census history did not fetch: {detail}")]
    CensusFetch { detail: String },
    /// The index's head is no commit of the census history: it was built from another.
    #[error("the index's head {head} is not in the census history")]
    IndexForeign { head: String },
    /// The builds after the index's head did not append.
    #[error("the builds did not append to the index: {detail}")]
    IndexAppend { detail: String },
    /// The newest release holds no archive of the census history.
    #[error("no release holds the census history")]
    NoArchive,
    /// The census history's archive did not download.
    #[error("the census history did not download: {detail}")]
    CensusDownload { detail: String },
    /// The census history's archive does not read as a repository.
    #[error("the census history's archive does not read: {detail}")]
    ArchiveUnreadable { detail: String },
    /// The clone of the census history could not be written to disk.
    #[error("the census history could not be written: {detail}")]
    CensusUnwritable { detail: String },
    /// The hash tables did not download.
    #[error("the hash tables did not download: {detail}")]
    HashesFetch { detail: String },
    /// Another program, or this one, is updating the hash tables.
    #[error("the hash tables are being updated already")]
    HashesBusy,
    /// The schema database did not download.
    #[error("the schema database did not download: {detail}")]
    SchemaFetch { detail: String },
    /// The schema database served is not one the app reads.
    #[error("the schema database served does not read: {detail}")]
    SchemaUnusable { detail: String },
    /// The schema database could not be written to disk.
    #[error("the schema database could not be written: {detail}")]
    SchemaUnwritable { detail: String },
    /// A download of the schema database is under way already.
    #[error("the schema database is downloading already")]
    SchemaBusy,
    /// A run of a mod is under way already.
    #[error("a run is under way already")]
    RunBusy,
    /// The app's store of runs did not open, read or write.
    #[error("the store of runs failed: {detail}")]
    StoreFailed { detail: String },
    /// A folder of the app's did not go when the user asked for it removed.
    #[error("{path} could not be removed: {detail}")]
    NotRemoved { path: String, detail: String },
    /// A folder did not open in the system's file manager.
    #[error("{path} could not be shown: {detail}")]
    NotShown { path: String, detail: String },
    /// The user stopped the work.
    #[error("cancelled")]
    Cancelled,
    /// A fault of the app's own, with no remedy a person can take.
    #[error("{detail}")]
    Internal { detail: String },
}

/// An error and what caused it, on one line.
pub(crate) fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
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
