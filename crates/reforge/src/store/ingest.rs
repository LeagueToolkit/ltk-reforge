//! The ingest tier (#ADR-0093): blobs in a directory, one file per blob, which any number of
//! processes read and write at once.
//!
//! `cacache` keeps the files. A put writes a temporary file and renames it to the blob's hash,
//! so a blob is whole or absent; a get checks the bytes against the hash before it answers,
//! and a get unchecked, a run's read of what it put or found whole, reads the blob's file.
//! The store uses it by hash alone, with SHA-256 as the algorithm, so a blob's address is its
//! content id. `cacache`'s index, a key to a hash, is never written.
//!
//! A spool is a scratch file in a directory of the store's own, never a blob (#ADR-0100): its
//! bytes are read once and kept as what is read of them. The file's name is the id of the
//! process that writes it and a random part. The file is removed when its reader is dropped,
//! on an error too, and a file whose process is gone is removed when the store is next opened.

use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::time::{Duration, SystemTime};

use cacache::{Algorithm, Integrity};
use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use tempfile::NamedTempFile;

use super::{BlobStore, ReadSeek, Spool, empty_blob};
use crate::model::{ByteView, ContentId};

/// The directory of the spools' scratch files, beside the blobs.
const SCRATCH: &str = "scratch";

/// The age past which a scratch file is removed whatever its name says: a process id the
/// system gave to a new process keeps a dead process's file no longer than this.
const SCRATCH_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// A store of blobs in a directory, by content id.
///
/// A put that does not write (a full disk, a directory that does not open) is logged and still
/// gives the id: a get of it then answers nothing, as for any blob the store lacks. A blob whose
/// bytes no longer match its hash answers nothing and is removed, so the next put of it writes
/// it again.
#[derive(Debug, Clone)]
pub struct IngestStore {
    dir: Utf8PathBuf,
}

impl IngestStore {
    /// The store in a directory, made at the first put when absent. The scratch files a process
    /// that is gone left are removed.
    pub fn new(dir: impl Into<Utf8PathBuf>) -> Self {
        let store = Self { dir: dir.into() };
        store.clear_scratch();
        store
    }

    /// Removes the scratch files no running process reads ([`left`]). One that does not
    /// remove stays.
    fn clear_scratch(&self) {
        let Ok(files) = fs::read_dir(self.dir.join(SCRATCH)) else {
            return;
        };
        for file in files.flatten() {
            if left(&file) {
                let _ = fs::remove_file(file.path());
            }
        }
    }

    /// The directory it keeps its blobs in.
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// Whether it holds a blob under an id. The bytes are not read, so not checked.
    pub fn contains(&self, id: ContentId) -> bool {
        id == ContentId::EMPTY || cacache::exists_sync(&self.dir, &integrity(id))
    }

    /// The file `cacache` 13.0 keeps a blob in: `content-v2/sha256/` and the id's hex digits,
    /// split after the second and the fourth. `cacache` reads a blob only through a check of
    /// its hash, so an unchecked read names the file itself.
    fn blob_file(&self, id: ContentId) -> Utf8PathBuf {
        let hex = id.hex();
        let mut path = self.dir.join("content-v2").join("sha256");
        path.push(&hex[..2]);
        path.push(&hex[2..4]);
        path.push(&hex[4..]);
        path
    }
}

/// The start of the name of a scratch file this process writes: its id.
fn scratch_prefix() -> String {
    format!("{}-", std::process::id())
}

/// Whether a scratch file is one no running process reads: its name gives no process, the
/// process it gives is gone, or the file is older than [`SCRATCH_AGE`].
fn left(file: &fs::DirEntry) -> bool {
    let name = file.file_name();
    let owner = name.to_str().and_then(|name| name.split_once('-'));
    let Some(pid) = owner.and_then(|(pid, _)| pid.parse::<u32>().ok()) else {
        return true;
    };
    if !runs(pid) {
        return true;
    }
    // The one clock the store reads. It decides what is removed, and reaches no report.
    let modified = file.metadata().and_then(|metadata| metadata.modified());
    let age = modified.map(|at| SystemTime::now().duration_since(at).unwrap_or_default());
    age.is_ok_and(|age| age > SCRATCH_AGE)
}

/// Whether the process with this id runs. A process that does not let this one ask of it
/// runs; one whose handle does not say counts as running.
#[cfg(windows)]
fn runs(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, GetLastError, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle is used only while open and closed once; the exit code is written
    // into a local.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let read = GetExitCodeProcess(handle, &raw mut code);
        CloseHandle(handle);
        read == 0 || code == STILL_ACTIVE.cast_unsigned()
    }
}

/// Whether the process with this id runs: `kill(pid, 0)`, which a process of another user
/// answers with a refusal.
#[cfg(unix)]
fn runs(pid: u32) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    test_kill_process(pid) != Err(Errno::SRCH)
}

/// Whether the process with this id runs, on a system this store cannot ask: always.
#[cfg(not(any(windows, unix)))]
fn runs(_pid: u32) -> bool {
    true
}

/// The address `cacache` keeps a blob of this id at.
fn integrity(id: ContentId) -> Integrity {
    Integrity::from_hex(id.hex(), Algorithm::Sha256)
        .unwrap_or_else(|error| panic!("the content id {id:?} is no SHA-256 to cacache: {error}"))
}

/// Bytes being written into a scratch file of the store's, removed when dropped.
struct DiskSpool {
    file: BufWriter<NamedTempFile>,
}

impl Write for DiskSpool {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl Spool for DiskSpool {
    fn finish(self: Box<Self>) -> io::Result<Box<dyn ReadSeek>> {
        let mut scratch = self
            .file
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        scratch.seek(SeekFrom::Start(0))?;
        Ok(Box::new(scratch))
    }
}

impl BlobStore for IngestStore {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        if let Some(empty) = empty_blob(id) {
            return Some(empty);
        }
        let address = integrity(id);
        match cacache::read_hash_sync(&self.dir, &address) {
            Ok(bytes) => Some(ByteView::from(bytes)),
            Err(cacache::Error::IntegrityError(error)) => {
                tracing::warn!(?id, %error, "a blob of the ingest store fails its hash");
                // Another process may hold the file open; the next put writes it again.
                let _ = cacache::remove_hash_sync(&self.dir, &address);
                None
            }
            Err(_) => None,
        }
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        let read = || fs::read(self.blob_file(id)).ok().map(ByteView::from);
        empty_blob(id).or_else(read)
    }

    fn contains(&self, id: ContentId) -> bool {
        IngestStore::contains(self, id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        if bytes.is_empty() {
            return ContentId::EMPTY;
        }
        let id = ContentId::of(&bytes);
        if self.contains(id) {
            return id;
        }
        match cacache::write_hash_sync_with_algo(Algorithm::Sha256, &self.dir, &bytes) {
            Ok(written) => assert!(
                written.matches(&integrity(id)).is_some(),
                "cacache keeps the blob {id:?} at {written}"
            ),
            Err(error) => tracing::warn!(?id, %error, "a blob is not written to the ingest store"),
        }
        id
    }

    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        let scratch = self.dir.join(SCRATCH);
        fs::create_dir_all(&scratch)?;
        let file = tempfile::Builder::new()
            .prefix(&scratch_prefix())
            .tempfile_in(&scratch)?;
        Ok(Box::new(DiskSpool {
            file: BufWriter::new(file),
        }))
    }
}
