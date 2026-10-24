//! The census history fetched as far as an append reads it (#ADR-0057): every commit of the
//! builds' branch without its tree, and of the builds after an index's head only the trees a diff
//! descends into and the files the walk reads. The clone is bare and holds nothing else, so it
//! serves an append and not a whole build.
//!
//! A request names the objects it wants by id, with the filter `tree:0`: the server sends what is
//! named and nothing it reaches. The trees are asked for a level at a time, every build's at
//! once, and the files last.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use camino::Utf8PathBuf;
use gix::ObjectId;
use gix::progress::{Count, Id, MessageLevel, NestedProgress, Progress, Step, StepShared, Unit};
use gix::protocol::fetch::{Arguments, Negotiate, Response, negotiate};
use gix::protocol::transport::client::blocking_io::{Transport, connect};
use gix::protocol::{Handshake, LsRefsCommand, ls_refs};
use reforge::history::{Families, Family};

use crate::error::CensusError;
use crate::format;
use crate::git::{GitRepo, Gx, TreeEntry, commit_id, differs, entries, tree_of};
use crate::seed::branch_ref;

/// What the server is asked to leave out: everything a named object reaches.
const FILTER: &str = "tree:0";

/// The most objects one request names. A major patch changes some 50,000 files.
const WANTS_PER_REQUEST: usize = 10_000;

/// The id `gix` gives the count of a pack's bytes as they are read.
const PACK_BYTES: Id = *b"BWRB";

/// What a fetch has received so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Received {
    /// The objects asked for.
    pub objects: usize,
    /// The bytes of the packs read.
    pub bytes: u64,
}

/// What hears a fetch as it goes.
type Heard = Arc<dyn Fn(Received) + Send + Sync>;

/// Fetches the census history into a clone of its own.
pub struct Fetcher<'a> {
    dir: Utf8PathBuf,
    url: String,
    cancel: &'a AtomicBool,
    heard: Heard,
    objects: usize,
    bytes: Arc<AtomicU64>,
}

impl<'a> Fetcher<'a> {
    /// A fetcher from the history at `url` into the clone at `dir`, made there when absent. It
    /// stops when `cancel` is set.
    pub fn new(
        dir: impl Into<Utf8PathBuf>,
        url: impl Into<String>,
        cancel: &'a AtomicBool,
    ) -> Self {
        Self {
            dir: dir.into(),
            url: url.into(),
            cancel,
            heard: Arc::new(|_| {}),
            objects: 0,
            bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Sets what hears the objects asked for and the bytes read, as they come.
    #[must_use]
    pub fn on_progress(mut self, heard: impl Fn(Received) + Send + Sync + 'static) -> Self {
        self.heard = Arc::new(heard);
        self
    }

    /// The commit the builds' branch is at on the server.
    pub fn tip(&self) -> Result<String, CensusError> {
        let mut session = Session::open(&self.url)?;
        let tip = session.tip(&self.url)?;
        session.close();
        Ok(tip.to_string())
    }

    /// Fetches every commit of the builds' branch, and what an append of the builds after `head`
    /// reads for an index of `families`, and opens the clone.
    pub fn fetch(&mut self, head: &str, families: Families) -> Result<GitRepo, CensusError> {
        let mut repo = self.repository()?;
        let mut session = Session::open(&self.url)?;
        let tip = session.tip(&self.url)?;
        let branch = branch_ref();
        let held = repo
            .try_find_reference(branch.as_str())
            .gx("reading the clone's branch")?
            .map(|mut r| r.peel_to_id().map(gix::Id::detach))
            .transpose()
            .gx("peeling the clone's branch")?;
        if held != Some(tip) {
            // The commits alone: the filter leaves out every tree.
            let haves: Vec<ObjectId> = held.into_iter().collect();
            self.get(&mut session, &mut repo, &[tip], &haves)?;
            repo.reference(
                branch.as_str(),
                tip,
                gix::refs::transaction::PreviousValue::Any,
                "fetch",
            )
            .gx("moving the clone's branch")?;
        }
        let chain = chain(&repo, tip, commit_id(head)?)?;
        self.prefetch(
            &mut session,
            &mut repo,
            &chain,
            families.has(Family::Objects),
        )?;
        session.close();
        GitRepo::open(&self.dir)
    }

    /// The clone, made bare when absent.
    fn repository(&self) -> Result<gix::Repository, CensusError> {
        if self.dir.join("HEAD").is_file() {
            return gix::open(self.dir.as_std_path()).gx("opening the clone");
        }
        fs_err::create_dir_all(&self.dir)?;
        gix::init_bare(self.dir.as_std_path()).gx("making the clone")
    }

    /// Fetches what the walk reads of each build of `chain` after its first: the commit's tree,
    /// its `census.yaml` and `build.yaml`, the trees of `files/` the diff descends into, both
    /// sides of each, and both sides of each file it reads.
    fn prefetch(
        &mut self,
        session: &mut Session,
        repo: &mut gix::Repository,
        chain: &[ObjectId],
        texts: bool,
    ) -> Result<(), CensusError> {
        if chain.len() < 2 {
            return Ok(());
        }
        let commits: Vec<String> = chain.iter().map(ToString::to_string).collect();
        let roots = commits
            .iter()
            .map(|commit| tree_of(repo, commit))
            .collect::<Result<Vec<_>, _>>()?;
        self.get_missing(session, repo, roots.iter().copied())?;

        let mut files = Vec::new();
        let mut level = Vec::new();
        for pair in roots.windows(2) {
            let (old, new) = (entries(repo, pair[0])?, entries(repo, pair[1])?);
            let named = |list: &[TreeEntry], name: &str, tree: bool| {
                list.iter()
                    .find(|e| e.name == name && e.tree == tree)
                    .map(|e| e.id)
            };
            files.extend(["census.yaml", "build.yaml"].map(|name| named(&new, name, false)));
            level.push((
                named(&old, "files", true),
                named(&new, "files", true),
                "files/".to_string(),
            ));
        }

        // A build that changes no file has the same `files/` as the build before, and the diff
        // reads neither side.
        level.retain(|(old, new, _)| old != new);
        while !level.is_empty() {
            let sides = level.iter().flat_map(|(old, new, _)| [*old, *new]);
            self.get_missing(session, repo, sides.flatten())?;
            let mut next = Vec::new();
            for (old, new, prefix) in level.drain(..) {
                let list = |tree: Option<ObjectId>| tree.map(|t| entries(repo, t)).transpose();
                let (old, new) = (
                    list(old)?.unwrap_or_default(),
                    list(new)?.unwrap_or_default(),
                );
                for name in differs(&old, &new, &prefix, texts) {
                    if let Some((old, new)) = name.trees {
                        next.push((old, new, format!("{}/", name.path)));
                    }
                    let read = format::entry_file(&name.path).is_some()
                        || format::object_file(&name.path).is_some()
                        || format::is_wad_file(&name.path);
                    if let Some((old, new)) = name.blobs.filter(|_| read) {
                        files.extend([old, new]);
                    }
                }
            }
            level = next;
        }
        self.get_missing(session, repo, files.into_iter().flatten())
    }

    /// Fetches the objects of `ids` the clone lacks, each once.
    fn get_missing(
        &mut self,
        session: &mut Session,
        repo: &mut gix::Repository,
        ids: impl Iterator<Item = ObjectId>,
    ) -> Result<(), CensusError> {
        let mut seen = HashSet::new();
        let missing: Vec<ObjectId> = ids
            .filter(|id| seen.insert(*id) && !repo.has_object(id))
            .collect();
        for wants in missing.chunks(WANTS_PER_REQUEST) {
            self.get(session, repo, wants, &[])?;
        }
        Ok(())
    }

    /// Fetches `wants` and what they reach that the filter keeps, into a pack of the clone, and
    /// opens the clone again so `repo` sees the pack. `haves` are commits the clone holds, so
    /// their ancestors are not sent.
    fn get(
        &mut self,
        session: &mut Session,
        repo: &mut gix::Repository,
        wants: &[ObjectId],
        haves: &[ObjectId],
    ) -> Result<(), CensusError> {
        self.objects += wants.len();
        self.hear();
        let progress = Bytes {
            id: gix::progress::UNKNOWN,
            bytes: Arc::clone(&self.bytes),
            objects: self.objects,
            heard: Arc::clone(&self.heard),
            counter: StepShared::default(),
        };
        let mut ask = Ask {
            wants,
            haves,
            filtered: false,
        };
        let pack_dir = repo.objects.store_ref().path().join("pack");
        let lookup = repo.objects.clone();
        let mut written = None;
        let fetched = gix::protocol::fetch(
            &mut ask,
            |pack, progress, interrupt| {
                let options = gix::odb::pack::bundle::write::Options::default();
                let bundle = gix::odb::pack::Bundle::write_to_directory(
                    pack,
                    Some(&pack_dir),
                    progress,
                    interrupt,
                    Some(lookup),
                    repo.object_hash(),
                    options,
                )?;
                written = Some(bundle);
                Ok(true)
            },
            progress,
            self.cancel,
            gix::protocol::fetch::Context {
                handshake: &mut session.handshake,
                transport: &mut session.transport,
                user_agent: agent(),
                trace_packetlines: false,
            },
            gix::protocol::fetch::Options {
                shallow_file: repo.shallow_file(),
                shallow: &gix::protocol::fetch::Shallow::NoChange,
                tags: gix::protocol::fetch::Tags::None,
                reject_shallow_remote: true,
            },
        );
        if self.cancel.load(Ordering::Relaxed) {
            return Err(CensusError::Cancelled);
        }
        fetched
            .map_err(gix::Exn::into_error)
            .gx("fetching from the census history")?;
        if !ask.filtered {
            return Err(CensusError::NoFilter {
                url: session.url.clone(),
            });
        }
        // gix keeps a new pack from `git gc` with a `.keep` until the fetch is done with it.
        if let Some(keep) = written.and_then(|bundle| bundle.keep_path) {
            fs_err::remove_file(keep)?;
        }
        // A handle opened before the pack was written does not always find the pack's objects
        // on its own (a fresh install's first update, 2026-10-25); one opened after it does.
        *repo = self.repository()?;
        self.hear();
        Ok(())
    }

    fn hear(&self) {
        (self.heard)(Received {
            objects: self.objects,
            bytes: self.bytes.load(Ordering::Relaxed),
        });
    }
}

/// The commits from `head` to `tip`, oldest first, on the tip's first-parent chain.
fn chain(
    repo: &gix::Repository,
    tip: ObjectId,
    head: ObjectId,
) -> Result<Vec<ObjectId>, CensusError> {
    let mut out = vec![tip];
    let mut at = tip;
    while at != head {
        let commit = repo.find_commit(at).gx("reading a commit")?;
        let Some(parent) = commit.parent_ids().next() else {
            return Err(CensusError::NoCommit(head.to_string()));
        };
        at = parent.detach();
        out.push(at);
    }
    out.reverse();
    Ok(out)
}

/// How the app names itself to the server.
fn agent() -> gix::protocol::command::Feature {
    (
        "agent",
        Some(concat!("reforge/", env!("CARGO_PKG_VERSION")).into()),
    )
}

/// One connection to the census history's server.
struct Session {
    url: String,
    transport: Box<dyn Transport + Send>,
    handshake: Handshake,
}

impl Session {
    fn open(url: &str) -> Result<Self, CensusError> {
        let options = connect::Options {
            version: gix::protocol::transport::Protocol::V2,
            ..Default::default()
        };
        let mut transport = connect::connect(url, options)
            .map_err(gix::Exn::into_error)
            .gx(&format!("connecting to {url}"))?;
        // A public repository: a request for credentials is answered with none.
        let handshake = gix::protocol::handshake(
            &mut transport,
            gix::protocol::transport::Service::UploadPack,
            |_| Ok(None),
            Vec::new(),
            &mut gix::progress::Discard,
        )
        .map_err(gix::Exn::into_error)
        .gx(&format!("greeting {url}"))?;
        Ok(Self {
            url: url.to_string(),
            transport,
            handshake,
        })
    }

    /// The commit the builds' branch is at.
    fn tip(&mut self, url: &str) -> Result<ObjectId, CensusError> {
        let branch = format!("refs/heads/{}", format::BRANCH);
        let mut prefixes = ls_refs::RefPrefixes::new();
        prefixes.extend([branch.clone().into()]);
        let refs = LsRefsCommand::new(Some(prefixes), &self.handshake.capabilities, agent())
            .invoke_blocking(&mut self.transport, &mut gix::progress::Discard, false)
            .map_err(gix::Exn::into_error)
            .gx(&format!("listing the branches of {url}"))?;
        refs.iter()
            .find_map(|r| match r.unpack() {
                (name, Some(object), _) if name == branch.as_str() => Some(object.to_owned()),
                _ => None,
            })
            .ok_or_else(|| CensusError::NoRemoteBranch {
                url: url.to_string(),
                branch,
            })
    }

    fn close(mut self) {
        // The server ends the session when the connection drops too.
        let _ = gix::protocol::indicate_end_of_interaction(&mut self.transport, false);
    }
}

/// A request for objects by id: no negotiation, one round.
struct Ask<'a> {
    wants: &'a [ObjectId],
    haves: &'a [ObjectId],
    /// Whether the server took the filter. A server that does not would send all the history.
    filtered: bool,
}

impl Negotiate for Ask<'_> {
    fn mark_complete_and_common_ref(&mut self) -> gix::ExnMessageResult<negotiate::Action> {
        Ok(negotiate::Action::MustNegotiate {
            remote_ref_target_known: Vec::new(),
        })
    }

    fn add_wants(&mut self, arguments: &mut Arguments, _known: &[bool]) -> bool {
        self.filtered = arguments.can_use_filter();
        if !self.filtered || self.wants.is_empty() {
            return false;
        }
        for want in self.wants {
            arguments.want(want);
        }
        arguments.filter(FILTER);
        true
    }

    fn one_round(
        &mut self,
        _state: &mut negotiate::one_round::State,
        arguments: &mut Arguments,
        _previous: Option<&Response>,
    ) -> gix::ExnMessageResult<(negotiate::Round, bool)> {
        for have in self.haves {
            arguments.have(have);
        }
        let round = negotiate::Round {
            haves_sent: self.haves.len(),
            in_vain: 0,
            haves_to_send: 0,
            previous_response_had_at_least_one_in_common: false,
        };
        Ok((round, true))
    }
}

/// The progress `gix` reports, of which only the bytes of a pack are heard.
struct Bytes {
    id: Id,
    bytes: Arc<AtomicU64>,
    objects: usize,
    heard: Heard,
    counter: StepShared,
}

impl Bytes {
    fn child(&self, id: Id) -> Self {
        Self {
            id,
            bytes: Arc::clone(&self.bytes),
            objects: self.objects,
            heard: Arc::clone(&self.heard),
            counter: StepShared::default(),
        }
    }
}

impl Count for Bytes {
    fn set(&self, step: Step) {
        self.counter.store(step, Ordering::Relaxed);
    }

    fn step(&self) -> Step {
        self.counter.load(Ordering::Relaxed)
    }

    fn inc_by(&self, step: Step) {
        self.counter.fetch_add(step, Ordering::Relaxed);
        if self.id == PACK_BYTES {
            let bytes = self.bytes.fetch_add(step as u64, Ordering::Relaxed) + step as u64;
            (self.heard)(Received {
                objects: self.objects,
                bytes,
            });
        }
    }

    fn counter(&self) -> StepShared {
        Arc::clone(&self.counter)
    }
}

impl Progress for Bytes {
    fn init(&mut self, _max: Option<Step>, _unit: Option<Unit>) {}

    fn set_name(&mut self, _name: String) {}

    fn name(&self) -> Option<String> {
        None
    }

    fn id(&self) -> Id {
        self.id
    }

    fn message(&self, _level: MessageLevel, _message: String) {}
}

impl NestedProgress for Bytes {
    type SubProgress = Self;

    fn add_child(&mut self, _name: impl Into<String>) -> Self {
        self.child(gix::progress::UNKNOWN)
    }

    fn add_child_with_id(&mut self, _name: impl Into<String>, id: Id) -> Self {
        self.child(id)
    }
}
