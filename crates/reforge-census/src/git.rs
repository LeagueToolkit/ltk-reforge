//! The history repository read through gix: the builds' branch (`format::BRANCH`), its
//! first-parent chain, tree diffs by object id, blobs, and an entry's text at a commit. Nothing here reads what a file
//! holds beyond `census.yaml`'s format and `build.yaml`.
//!
//! A `gix::Repository` is not shared between threads, so each thread keeps its own handle on
//! the repository: the walk reads blobs from every core at once.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;

use camino::{Utf8Path, Utf8PathBuf};
use gix::ObjectId;
use reforge::model::{BinHash, WadHash};

use crate::error::CensusError;
use crate::format::{self, BuildFile};
use crate::walk::{BlobId, Change, ChangeSource};

/// Carries a gix error into this crate's with what was being read.
trait Gx<T> {
    fn gx(self, what: &str) -> Result<T, CensusError>;
}

// Some of gix's errors are no `Error` themselves and convert into a boxed one.
impl<T, E: Into<Box<dyn Error + Send + Sync>>> Gx<T> for Result<T, E> {
    fn gx(self, what: &str) -> Result<T, CensusError> {
        self.map_err(|source| CensusError::Git {
            what: what.to_string(),
            source: source.into(),
        })
    }
}

/// Each thread's object cache: enough for the trees of a diff and the delta bases of a blob.
const CACHE: usize = 64 * 1024 * 1024;

thread_local! {
    /// This thread's handle on each repository opened, by path.
    static LOCAL: RefCell<HashMap<Utf8PathBuf, gix::Repository>> = RefCell::new(HashMap::new());
}

/// A clone of the census history, read up to one commit.
pub struct GitRepo {
    repo: gix::ThreadSafeRepository,
    path: Utf8PathBuf,
    tip: ObjectId,
}

struct TreeEntry {
    name: String,
    id: ObjectId,
    tree: bool,
}

fn commit_id(commit: &str) -> Result<ObjectId, CensusError> {
    ObjectId::from_hex(commit.as_bytes()).gx("reading a commit id")
}

impl GitRepo {
    /// Opens a clone of the history, bare or not. The builds are on the branch `format::BRANCH`, or
    /// in a clone that never checked it out, on `origin/history`.
    pub fn open(path: &Utf8Path) -> Result<Self, CensusError> {
        let repo =
            gix::open(path.as_std_path()).gx(&format!("opening the history repository {path}"))?;
        let local = format!("refs/heads/{}", format::BRANCH);
        let remote = format!("refs/remotes/origin/{}", format::BRANCH);
        let found = [&local, &remote]
            .into_iter()
            .find_map(|name| repo.try_find_reference(name.as_str()).ok().flatten());
        let Some(reference) = found else {
            return Err(CensusError::NoBranch {
                repo: path.to_owned(),
                local,
                remote,
            });
        };
        let tip = reference
            .into_fully_peeled_id()
            .gx("peeling the history branch")?
            .detach();
        Ok(Self {
            repo: repo.into_sync(),
            path: path.to_owned(),
            tip,
        })
    }

    /// Runs `f` with this thread's handle on the repository. `f` does not call back into it.
    fn local<T>(&self, f: impl FnOnce(&gix::Repository) -> T) -> T {
        LOCAL.with(|local| {
            let mut local = local.borrow_mut();
            let repo = local.entry(self.path.clone()).or_insert_with(|| {
                let mut repo = self.repo.to_thread_local();
                repo.object_cache_size_if_unset(CACHE);
                repo
            });
            f(repo)
        })
    }

    /// The clone's directory.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Reads the history only up to `at`, a commit or a build as `resolve` takes them. The
    /// commits after it are not this repository's any more.
    pub fn stop_at(&mut self, at: &str) -> Result<(), CensusError> {
        self.tip = commit_id(&self.resolve(at)?)?;
        Ok(())
    }

    /// The tip of the builds' branch, or the commit `stop_at` set.
    pub fn head(&self) -> String {
        self.tip.to_string()
    }

    /// The commit a commit id, a prefix of one, or a build version names. A version
    /// (`16.19.8207193`) is matched against each commit's `build.yaml`, the newest commit
    /// first.
    pub fn resolve(&self, at: &str) -> Result<String, CensusError> {
        if at.contains('.') {
            for commit in self.commits()?.into_iter().rev() {
                if self.build(&commit)?.build.as_str() == at {
                    return Ok(commit);
                }
            }
            return Err(CensusError::NoBuild(at.to_string()));
        }
        let prefix = at.to_ascii_lowercase();
        let commits = self.commits()?;
        let mut hits = commits.into_iter().filter(|c| c.starts_with(&prefix));
        match (hits.next(), hits.count()) {
            (Some(one), 0) => Ok(one),
            (None, _) => Err(CensusError::NoCommit(prefix)),
            (Some(_), more) => Err(CensusError::AmbiguousCommit {
                prefix,
                count: more + 1,
            }),
        }
    }

    /// A bin entry's text at a commit, from the first WAD in path order whose bin at `bin`
    /// holds it.
    pub fn entry_text(
        &self,
        bin: WadHash,
        entry: BinHash,
        commit: &str,
    ) -> Result<Option<String>, CensusError> {
        self.local(|repo| {
            let Some(files) = files_tree(repo, commit)? else {
                return Ok(None);
            };
            find_text(repo, files, &format::object_path(bin, entry))
        })
    }
}

fn tree_of(repo: &gix::Repository, commit: &str) -> Result<ObjectId, CensusError> {
    let commit = repo
        .find_commit(commit_id(commit)?)
        .gx("reading a commit")?;
    Ok(commit.tree_id().gx("reading a commit's tree")?.detach())
}

/// The tree of `files/` at a commit, if it has one.
fn files_tree(repo: &gix::Repository, commit: &str) -> Result<Option<ObjectId>, CensusError> {
    let files = entry_at(repo, tree_of(repo, commit)?, "files")?;
    Ok(files.filter(|e| e.tree).map(|e| e.id))
}

/// A tree's entries, sorted by name.
fn entries(repo: &gix::Repository, tree: ObjectId) -> Result<Vec<TreeEntry>, CensusError> {
    let tree = repo.find_tree(tree).gx("reading a tree")?;
    let decoded = tree.decode().gx("decoding a tree")?;
    let entries = decoded.entries.iter().map(|e| TreeEntry {
        name: e.filename.to_string(),
        id: e.oid.to_owned(),
        tree: e.mode.is_tree(),
    });
    let mut entries: Vec<TreeEntry> = entries.collect();
    // git orders a tree as if a subtree's name ended in `/`. A merge of two lists needs plain
    // order.
    entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn blob(repo: &gix::Repository, id: ObjectId) -> Result<Vec<u8>, CensusError> {
    let mut blob = repo.find_blob(id).gx("reading a blob")?;
    Ok(blob.take_data())
}

/// The entry at `path` under `tree`, if any.
fn entry_at(
    repo: &gix::Repository,
    tree: ObjectId,
    path: &str,
) -> Result<Option<TreeEntry>, CensusError> {
    let mut current = tree;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        let found = entries(repo, current)?.into_iter().find(|e| e.name == part);
        let Some(entry) = found else { return Ok(None) };
        if parts.peek().is_none() {
            return Ok(Some(entry));
        }
        if !entry.tree {
            return Ok(None);
        }
        current = entry.id;
    }
    Ok(None)
}

/// The blob at `path` under `tree`, if any.
fn blob_at(
    repo: &gix::Repository,
    tree: ObjectId,
    path: &str,
) -> Result<Option<Vec<u8>>, CensusError> {
    match entry_at(repo, tree, path)? {
        Some(e) if !e.tree => blob(repo, e.id).map(Some),
        _ => Ok(None),
    }
}

/// `path` under the first WAD directory (the one holding `_wad.yaml`) below `tree` that has it.
fn find_text(
    repo: &gix::Repository,
    tree: ObjectId,
    path: &str,
) -> Result<Option<String>, CensusError> {
    let entries = entries(repo, tree)?;
    if entries.iter().any(|e| e.name == "_wad.yaml" && !e.tree) {
        let bytes = blob_at(repo, tree, path)?;
        return Ok(bytes.map(|b| String::from_utf8_lossy(&b).into_owned()));
    }
    for e in entries.iter().filter(|e| e.tree) {
        if let Some(text) = find_text(repo, e.id, path)? {
            return Ok(Some(text));
        }
    }
    Ok(None)
}

fn blob_id(id: &ObjectId) -> BlobId {
    let bytes = id.as_bytes();
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("object id {id} is {} bytes, not SHA-1's 20", bytes.len()))
}

/// The diff of two trees by object id.
struct Diff<'a> {
    repo: &'a gix::Repository,
    /// Whether a bin's entry texts are read.
    texts: bool,
    out: Vec<Change>,
}

impl Diff<'_> {
    /// Adds every blob under two trees that differs, recursing only into subtrees whose ids
    /// differ, and into a bin's entry texts only when they are read.
    fn trees(
        &mut self,
        old: Option<ObjectId>,
        new: Option<ObjectId>,
        prefix: &str,
    ) -> Result<(), CensusError> {
        if old == new {
            return Ok(());
        }
        let list = |tree: Option<ObjectId>| tree.map(|t| entries(self.repo, t)).transpose();
        let old_entries = list(old)?.unwrap_or_default();
        let new_entries = list(new)?.unwrap_or_default();
        let (mut o, mut n) = (old_entries.iter().peekable(), new_entries.iter().peekable());
        loop {
            let (o, n) = match (o.peek(), n.peek()) {
                (None, None) => break,
                (Some(a), Some(b)) => match a.name.cmp(&b.name) {
                    Ordering::Less => (o.next(), None),
                    Ordering::Greater => (None, n.next()),
                    Ordering::Equal => (o.next(), n.next()),
                },
                (Some(_), None) => (o.next(), None),
                (None, Some(_)) => (None, n.next()),
            };
            if o.map(|e| (e.id, e.tree)) == n.map(|e| (e.id, e.tree)) {
                continue;
            }
            let Some(name) = o.or(n).map(|e| e.name.as_str()) else {
                continue;
            };
            let path = format!("{prefix}{name}");
            let tree = |e: Option<&TreeEntry>| e.filter(|e| e.tree).map(|e| e.id);
            let blob = |e: Option<&TreeEntry>| e.filter(|e| !e.tree).map(|e| blob_id(&e.id));
            let subtree = tree(o).is_some() || tree(n).is_some();
            if subtree && (self.texts || format::bin_dir(name).is_none()) {
                self.trees(tree(o), tree(n), &format!("{path}/"))?;
            }
            if blob(o).is_some() || blob(n).is_some() {
                self.out.push(Change {
                    path,
                    old: blob(o),
                    new: blob(n),
                });
            }
        }
        Ok(())
    }
}

impl ChangeSource for GitRepo {
    fn commits(&self) -> Result<Vec<String>, CensusError> {
        self.local(|repo| {
            let mut out = Vec::new();
            let mut id = Some(self.tip);
            while let Some(current) = id {
                let commit = repo.find_commit(current).gx("reading a commit")?;
                out.push(current.to_string());
                id = commit.parent_ids().next().map(gix::Id::detach);
            }
            out.reverse();
            Ok(out)
        })
    }

    /// The commit's build, once its `census.yaml` says it is the format this crate reads.
    fn build(&self, commit: &str) -> Result<BuildFile, CensusError> {
        self.local(|repo| {
            let tree = tree_of(repo, commit)?;
            let file = |file: &'static str| {
                let missing = || CensusError::MissingFile {
                    commit: commit.to_string(),
                    file,
                };
                let bytes = blob_at(repo, tree, file)?.ok_or_else(missing)?;
                Ok::<_, CensusError>(String::from_utf8_lossy(&bytes).into_owned())
            };
            let found = format::census_format(&file("census.yaml")?);
            if found != Some(format::FORMAT) {
                return Err(CensusError::Format {
                    commit: commit.to_string(),
                    found,
                });
            }
            format::parse_build(&file("build.yaml")?).ok_or_else(|| CensusError::NoVersion {
                commit: commit.to_string(),
            })
        })
    }

    fn changes(
        &self,
        commit: &str,
        parent: Option<&str>,
        texts: bool,
    ) -> Result<Vec<Change>, CensusError> {
        self.local(|repo| {
            let old = match parent {
                Some(parent) => files_tree(repo, parent)?,
                None => None,
            };
            let mut diff = Diff {
                repo,
                texts,
                out: Vec::new(),
            };
            diff.trees(old, files_tree(repo, commit)?, "files/")?;
            Ok(diff.out)
        })
    }

    fn read(&self, id: &BlobId) -> Result<Vec<u8>, CensusError> {
        self.local(|repo| blob(repo, ObjectId::from_bytes_or_panic(id)))
    }
}
