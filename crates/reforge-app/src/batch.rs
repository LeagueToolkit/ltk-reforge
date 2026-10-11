//! A batch (#ADR-0091): the archives of a folder, or files dropped, run as one job with one set
//! of settings.

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;

use crate::error::AppError;

/// The extensions a mod archive is listed by, compared without case.
const ARCHIVE_EXTENSIONS: [&str; 2] = ["fantome", "zip"];

/// Whether a path is a mod archive by its extension.
fn is_archive(path: &Utf8Path) -> bool {
    path.extension().is_some_and(|extension| {
        ARCHIVE_EXTENSIONS
            .iter()
            .any(|archive| extension.eq_ignore_ascii_case(archive))
    })
}

/// The mod archives of a folder, by extension, sorted by name. Its subfolders are not read, and
/// a name that is not UTF-8 is left out: a run takes its archive by a path as text.
pub fn list_archives(folder: &Utf8Path) -> Result<Vec<String>, AppError> {
    let unreadable = |error: std::io::Error| AppError::FolderUnreadable {
        path: folder.to_string(),
        detail: error.to_string(),
    };
    let mut archives = Vec::new();
    for entry in fs::read_dir(folder).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if is_archive(&path) && path.is_file() {
            archives.push(path.into_string());
        }
    }
    archives.sort();
    Ok(archives)
}
