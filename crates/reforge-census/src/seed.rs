//! A clone of the census history made from whole packs, as a release's archive holds them
//! (#ADR-0060): a bare repository, the packs' files written into it by the caller, and the
//! builds' branch set where a fetch reads and moves it.

use camino::Utf8PathBuf;
use gix::refs::transaction::PreviousValue;

use crate::error::CensusError;
use crate::format;
use crate::git::{GitRepo, Gx, commit_id};

/// The reference a clone keeps the builds' branch under, as a fetch keeps it.
pub(crate) fn branch_ref() -> String {
    format!("refs/remotes/origin/{}", format::BRANCH)
}

/// A bare clone being made from whole packs.
pub struct Seed {
    dir: Utf8PathBuf,
}

impl Seed {
    /// An empty bare clone at `dir`.
    pub fn create(dir: impl Into<Utf8PathBuf>) -> Result<Self, CensusError> {
        let dir = dir.into();
        fs_err::create_dir_all(&dir)?;
        gix::init_bare(dir.as_std_path()).gx("making the clone")?;
        let seed = Self { dir };
        fs_err::create_dir_all(seed.pack_dir())?;
        Ok(seed)
    }

    /// The folder a pack's files are written in.
    pub fn pack_dir(&self) -> Utf8PathBuf {
        self.dir.join("objects").join("pack")
    }

    /// Sets the builds' branch at `head`, a commit the packs written hold, and opens the clone.
    pub fn finish(self, head: &str) -> Result<GitRepo, CensusError> {
        let repo = gix::open(self.dir.as_std_path()).gx("opening the clone")?;
        let id = commit_id(head)?;
        let object = repo.try_find_object(id).gx("reading the branch's commit")?;
        if object.is_none_or(|object| object.kind != gix::object::Kind::Commit) {
            return Err(CensusError::NoCommit(head.to_string()));
        }
        repo.reference(branch_ref().as_str(), id, PreviousValue::Any, "seed")
            .gx("setting the clone's branch")?;
        GitRepo::open(&self.dir)
    }
}
