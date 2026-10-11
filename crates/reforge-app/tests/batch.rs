//! A batch as the app runs it (#ADR-0091): the archives of a folder listed.

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge_app::batch;
use reforge_app::error::AppError;
use tempfile::TempDir;

/// A folder for archives, removed with it.
struct Folder {
    _dir: TempDir,
    root: Utf8PathBuf,
}

impl Folder {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        Self { _dir: dir, root }
    }

    fn path(&self, name: &str) -> Utf8PathBuf {
        self.root.join(name)
    }
}

#[test]
fn a_folder_lists_its_archives_by_extension_in_any_case_sorted_by_name() {
    let folder = Folder::new();
    for name in ["b.zip", "a.FANTOME", "readme.txt", "c.fantome.bak", "d.Zip"] {
        fs::write(folder.path(name), b"").unwrap();
    }
    fs::create_dir(folder.path("sub")).unwrap();
    fs::write(folder.path("sub").join("e.fantome"), b"").unwrap();
    // A folder named as an archive is no archive.
    fs::create_dir(folder.path("f.zip")).unwrap();

    let listed = batch::list_archives(&folder.root).unwrap();

    let expected: Vec<String> = ["a.FANTOME", "b.zip", "d.Zip"]
        .iter()
        .map(|name| folder.path(name).to_string())
        .collect();
    assert_eq!(listed, expected);
}

#[test]
fn a_folder_that_does_not_read_is_refused_with_its_path() {
    let folder = Folder::new();
    let missing = folder.path("missing");

    let Err(AppError::FolderUnreadable { path, .. }) = batch::list_archives(&missing) else {
        panic!("a missing folder listed");
    };
    assert_eq!(path, missing.to_string());
}
