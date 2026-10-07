//! The census history fetched as far as an append reads it, from a server: every commit, and of
//! the builds after an index's head only what their diffs read. The server here is a bare
//! repository on disk served by `git upload-pack`, so the tests need git on the path.

#![cfg(feature = "fetch")]

mod common;
mod repo;

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{BUILDS_FILE, DB_FILE, Families, Stamp};
use reforge_census::format::BuildFile;
use reforge_census::{CensusError, ChangeSource, Fetcher, GitRepo, Indexer, PUBLISHED, Received};
use tempfile::TempDir;

use self::common::{B, Memory, build, memory, texture};

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
    (dir, root)
}

/// The fixture and a fourth build that drops path 3 and changes the object's text, so a diff
/// descends into a bin's entry texts.
fn four_builds() -> Memory {
    let mut history = memory();
    let mut fourth = history.commits[2].1.clone();
    fourth.retain(|path, _| !path.ends_with("/0000000000000003.yaml"));
    for (path, text) in &mut fourth {
        if path.ends_with(".rito") {
            *text = text.replace("\"Test\"", "\"Other\"");
        }
    }
    history.commits.push((build("1.4.1"), fourth));
    history
}

/// A bare repository holding builds on the census branch, served as the published history is:
/// an object asked for by id is sent, and so is a filter.
struct Server {
    dir: Utf8PathBuf,
    repo: gix::Repository,
    commits: Vec<String>,
}

impl Server {
    fn new(dir: Utf8PathBuf) -> Self {
        let repo = gix::init_bare(dir.as_std_path()).unwrap();
        let config = dir.join("config");
        let mut text = fs::read_to_string(&config).unwrap();
        text.push_str("[uploadpack]\n\tallowFilter = true\n\tallowAnySHA1InWant = true\n");
        fs::write(&config, text).unwrap();
        Self {
            dir,
            repo,
            commits: Vec::new(),
        }
    }

    /// A server holding every build of a history.
    fn of(history: &Memory, dir: Utf8PathBuf) -> Self {
        let mut server = Self::new(dir);
        for (build, files) in &history.commits {
            server.push(build, files);
        }
        server
    }

    /// Commits a build on top of the branch.
    fn push(&mut self, build: &BuildFile, files: &BTreeMap<String, String>) {
        let parent = self.commits.last().map(String::as_str);
        let commit = repo::commit(&self.repo, parent, build, files);
        self.commits.push(commit);
    }

    fn url(&self) -> &str {
        self.dir.as_str()
    }

    /// The history as the server holds it, up to the build at `at`.
    fn up_to(&self, at: usize) -> GitRepo {
        let mut repo = GitRepo::open(&self.dir).unwrap();
        repo.stop_at(&self.commits[at]).unwrap();
        repo
    }
}

/// The index of the server's builds up to the one at `at`.
fn index_of(server: &Server, at: usize, out: &Utf8Path) {
    Indexer::new(&server.up_to(at), "server")
        .build(out, Families::ALL)
        .unwrap();
}

fn fetch(server: &Server, clone: &Utf8Path, head: &str) -> Result<GitRepo, CensusError> {
    let cancel = AtomicBool::new(false);
    Fetcher::new(clone, server.url(), &cancel).fetch(head, Families::ALL)
}

#[test]
fn an_index_taken_forward_from_the_fetched_history_is_the_one_taken_from_the_whole() {
    let (_dir, root) = scratch();
    let server = Server::of(&four_builds(), root.join("server"));
    let whole = GitRepo::open(&server.dir).unwrap();
    for at in 0..server.commits.len() - 1 {
        let index = root.join(format!("index-{at}"));
        index_of(&server, at, &index);
        let expected = root.join(format!("expected-{at}"));
        Indexer::new(&whole, "server")
            .append_to(&index, &expected)
            .unwrap();

        let clone = fetch(
            &server,
            &root.join(format!("clone-{at}")),
            &server.commits[at],
        )
        .unwrap();
        let out = root.join(format!("out-{at}"));
        let stamp = Indexer::new(&clone, "server")
            .append_to(&index, &out)
            .unwrap();
        assert_eq!(stamp, Stamp::read(&expected).unwrap(), "from build {at}");
        for file in [DB_FILE, BUILDS_FILE] {
            let read = |dir: &Utf8Path| fs::read(dir.join(file)).unwrap();
            assert!(read(&out) == read(&expected), "{file} from build {at}");
        }
    }
}

#[test]
fn the_fetched_history_holds_nothing_of_the_builds_before_the_head() {
    let (_dir, root) = scratch();
    let server = Server::of(&four_builds(), root.join("server"));
    let clone = root.join("clone");
    let repo = fetch(&server, &clone, &server.commits[2]).unwrap();
    assert_eq!(repo.commits().unwrap(), server.commits, "every commit");

    let blob = |text: &str| server.repo.write_blob(text).unwrap().detach();
    let fetched = gix::open(clone.as_std_path()).unwrap();
    // Bytes B are path 1's in the second build alone.
    assert!(!fetched.has_object(blob(&texture(B))));
    let texts = four_builds().commits[3].1.clone();
    let changed = texts
        .iter()
        .find(|(path, _)| path.ends_with(".rito"))
        .unwrap()
        .1;
    assert!(
        fetched.has_object(blob(changed)),
        "the fourth build's entry text"
    );
}

#[test]
fn a_second_fetch_takes_the_builds_pushed_since() {
    let (_dir, root) = scratch();
    let history = four_builds();
    let mut server = Server::new(root.join("server"));
    for (build, files) in &history.commits[..2] {
        server.push(build, files);
    }
    let index = root.join("index");
    index_of(&server, 0, &index);
    let clone = root.join("clone");
    let repo = fetch(&server, &clone, &server.commits[0]).unwrap();
    let first = root.join("first");
    let stamp = Indexer::new(&repo, "server")
        .append_to(&index, &first)
        .unwrap();
    assert_eq!(stamp.builds, 2);

    for (build, files) in &history.commits[2..] {
        server.push(build, files);
    }
    let repo = fetch(&server, &clone, &stamp.head).unwrap();
    let second = root.join("second");
    let stamp = Indexer::new(&repo, "server")
        .append_to(&first, &second)
        .unwrap();

    let whole = root.join("whole");
    let expected = Indexer::new(&GitRepo::open(&server.dir).unwrap(), "server")
        .append_to(&index, &whole)
        .unwrap();
    assert_eq!(stamp, expected);
    let builds = |dir: &Utf8Path| fs::read_to_string(dir.join(BUILDS_FILE)).unwrap();
    assert_eq!(builds(&second), builds(&whole));
}

#[test]
fn the_tip_is_the_branch_on_the_server() {
    let (_dir, root) = scratch();
    let server = Server::of(&memory(), root.join("server"));
    let cancel = AtomicBool::new(false);
    let tip = Fetcher::new(root.join("clone"), server.url(), &cancel)
        .tip()
        .unwrap();
    assert_eq!(Some(&tip), server.commits.last());
}

#[test]
fn a_head_the_history_does_not_hold_is_refused() {
    let (_dir, root) = scratch();
    let server = Server::of(&memory(), root.join("server"));
    let elsewhere = "0123456789abcdef0123456789abcdef01234567";
    let Err(error) = fetch(&server, &root.join("clone"), elsewhere) else {
        panic!("a head of no build was fetched");
    };
    assert!(
        matches!(&error, CensusError::NoCommit(commit) if commit == elsewhere),
        "{error}"
    );
}

#[test]
fn a_fetch_stopped_before_it_starts_is_cancelled() {
    let (_dir, root) = scratch();
    let server = Server::of(&memory(), root.join("server"));
    let cancel = AtomicBool::new(true);
    let fetched = Fetcher::new(root.join("clone"), server.url(), &cancel)
        .fetch(&server.commits[0], Families::ALL);
    let Err(error) = fetched else {
        panic!("a stopped fetch went on");
    };
    assert!(matches!(error, CensusError::Cancelled), "{error}");
}

/// The published history fetched after the head of the index `REFORGE_HISTORY_INDEX` names, and
/// the index taken forward from it into `REFORGE_HISTORY_OUT`.
#[test]
#[ignore = "fetches the published census history into the index REFORGE_HISTORY_OUT names"]
fn the_published_history_takes_an_index_forward() {
    let var = |name: &str| {
        let value = std::env::var(name).unwrap_or_else(|_| panic!("{name} names a folder"));
        Utf8PathBuf::from(value)
    };
    let (index, out) = (var("REFORGE_HISTORY_INDEX"), var("REFORGE_HISTORY_OUT"));
    let stamp = Stamp::read(&index).unwrap();
    let (_dir, root) = scratch();
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let last = Arc::new(Mutex::new(Received::default()));
    let heard = Arc::clone(&last);
    let mut fetcher = Fetcher::new(root.join("clone"), PUBLISHED, &cancel)
        .on_progress(move |received| *heard.lock().unwrap() = received);
    let tip = fetcher.tip().unwrap();
    let repo = fetcher.fetch(&stamp.head, stamp.families).unwrap();
    let received = *last.lock().unwrap();
    eprintln!(
        "from {} to {tip}: {} objects asked for, {} bytes, in {:.1?}",
        stamp.head,
        received.objects,
        received.bytes,
        started.elapsed()
    );
    let appended = Indexer::new(&repo, PUBLISHED)
        .append_to(&index, &out)
        .unwrap();
    eprintln!("appended in {:.1?}", started.elapsed());
    assert_eq!(appended.head, tip);
    assert_eq!(appended.builds, repo.commits().unwrap().len());
}
