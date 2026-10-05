//! The app's clone of the census history bootstrapped from a release's archive: the archive
//! downloaded, resumed when it stopped, checked against its digest, and read as a stream, of
//! which only the pack's files and the branch's commit are taken. The pack is made by
//! `git pack-objects`, and an update fetches with `git upload-pack`, so the tests need git on the
//! path.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{Families, Stamp};
use reforge_app::archive::{ArchiveNote, ArchiveProgress, Bootstrap, CloneView, Clones};
use reforge_app::census::Updater;
use reforge_app::error::AppError;
use reforge_app::index::{ARCHIVE_FILE, Asset, IndexRelease, InstallStep, Ranged, Source};
use reforge_census::{GitRepo, Indexer};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const WAD: &str = "files/data/x.wad.client";
const TOP: &str = "ltk-census-history.git";

/// A texture's entry file with these bytes, at path 1 or 2 of the WAD.
fn texture(path: u64, bytes: char) -> (String, String) {
    let digest: String = std::iter::repeat_n(bytes, 64).collect();
    let text = format!(
        "sha256: \"{digest}\"\nkind: \"tex\"\ntexture:\n format: \"bc3\"\n width: 64\n \
         height: 64\n mips: 7\n top: \"00000000000000aa\"\n"
    );
    (format!("{WAD}/00/{path:016x}.yaml"), text)
}

/// A bare repository of builds on the census branch, served as the published history is.
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

    /// Commits a build of these files on top of the branch.
    fn push(&mut self, version: &str, files: &[(String, String)]) {
        let empty = gix::ObjectId::empty_tree(gix::hash::Kind::Sha1);
        let mut tree = self.repo.edit_tree(empty).unwrap();
        let build = format!("version: \"{version}\"\nmanifest: \"0\"\ndate: \"2026-10-05\"\n");
        let census = [("census.yaml".to_string(), "format: 2\n".to_string())];
        let fixed = [("build.yaml".to_string(), build)];
        for (path, text) in census.iter().chain(&fixed).chain(files) {
            let blob = self.repo.write_blob(text).unwrap().detach();
            tree.upsert(path.as_str(), gix::object::tree::EntryKind::Blob, blob)
                .unwrap();
        }
        let tree = tree.write().unwrap().detach();
        let signature = gix::actor::SignatureRef {
            name: "census".into(),
            email: "census@localhost".into(),
            time: "1700000000 +0000",
        };
        let parents = self
            .commits
            .last()
            .map(|c| gix::ObjectId::from_hex(c.as_bytes()).unwrap());
        let commit = self
            .repo
            .commit_as(
                signature,
                signature,
                "refs/heads/history-v2",
                version,
                tree,
                parents,
            )
            .unwrap();
        self.commits.push(commit.to_string());
    }

    /// The index of the builds up to the one at `at`, in `out`.
    fn index(&self, at: usize, out: &Utf8Path) {
        let mut repo = GitRepo::open(&self.dir).unwrap();
        repo.stop_at(&self.commits[at]).unwrap();
        Indexer::new(&repo, "server")
            .build(out, Families::ALL)
            .unwrap();
    }

    /// The pack of every object the build at `at` reaches, written by git into `out`: its file
    /// names and their bytes.
    fn pack(&self, at: usize, out: &Utf8Path) -> Vec<(String, Vec<u8>)> {
        fs::create_dir_all(out).unwrap();
        let mut git = Command::new("git")
            .args(["-C", self.dir.as_str(), "pack-objects", "--revs", "-q"])
            .arg(out.join("pack").as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = git.stdin.as_mut().unwrap();
        std::io::Write::write_all(stdin, format!("{}\n", self.commits[at]).as_bytes()).unwrap();
        assert!(git.wait().unwrap().success(), "git pack-objects failed");
        let mut files: Vec<(String, Vec<u8>)> = fs::read_dir(out)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let name = entry.file_name().to_string_lossy().into_owned();
                (name, fs::read(entry.path()).unwrap())
            })
            .collect();
        files.sort();
        files
    }
}

/// Two builds: path 1 changes its bytes, and path 2 appears.
fn two_builds(dir: Utf8PathBuf) -> Server {
    let mut server = Server::new(dir);
    server.push("1.1.1", &[texture(1, 'a')]);
    server.push("1.2.1", &[texture(1, 'b'), texture(2, 'a')]);
    server
}

/// Bytes zstd does not shrink, so an archive holding them spans many reads.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

/// A repository archive as a release holds it: the pack, `packed-refs` naming `branch` at
/// `head`, and members the app takes nothing of.
fn archive(pack: &[(String, Vec<u8>)], branch: &str, head: &str, padding: usize) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut add = |path: &str, bytes: &[u8]| {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, format!("{TOP}/{path}"), bytes)
            .unwrap();
    };
    add("HEAD", b"ref: refs/heads/main\n");
    add("config", b"[core]\n\tbare = true\n");
    add("description", &noise(padding));
    add("hooks/pre-push", b"#!/bin/sh\necho hooked\n");
    add("objects/pack/notes.txt", b"not a pack\n");
    add(
        "packed-refs",
        format!("# pack-refs with: peeled fully-peeled sorted \n{head} {branch}\n").as_bytes(),
    );
    for (name, bytes) in pack {
        add(&format!("objects/pack/{name}"), bytes);
    }
    let keep = pack
        .iter()
        .find_map(|(name, _)| name.strip_suffix(".pack"))
        .unwrap();
    add(&format!("objects/pack/{keep}.keep"), b"");
    zstd::encode_all(builder.into_inner().unwrap().as_slice(), 3).unwrap()
}

/// A release's files in a folder, served by their names, from any place, under one entity tag.
struct Folder {
    dir: TempDir,
    etag: Mutex<String>,
    asked: Mutex<Vec<u64>>,
}

impl Folder {
    fn new(archive: &[u8]) -> Self {
        let folder = Self {
            dir: tempfile::tempdir().unwrap(),
            etag: Mutex::new("\"one\"".to_string()),
            asked: Mutex::new(Vec::new()),
        };
        fs::write(folder.path(ARCHIVE_FILE), archive).unwrap();
        folder
    }

    fn path(&self, name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(self.dir.path().join(name)).unwrap()
    }

    fn asset(&self) -> Asset {
        let bytes = fs::read(self.path(ARCHIVE_FILE)).unwrap();
        Asset {
            name: ARCHIVE_FILE.to_string(),
            size: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            url: ARCHIVE_FILE.to_string(),
        }
    }

    /// A release holding the archive. Its index's files are not read by a bootstrap.
    fn release(&self) -> IndexRelease {
        let none = Asset {
            name: String::new(),
            size: 0,
            sha256: String::new(),
            url: String::new(),
        };
        IndexRelease {
            tag: "history-v2-snapshot-0".to_string(),
            stamp: none.clone(),
            builds: none.clone(),
            db: none,
            archive: Some(self.asset()),
        }
    }
}

impl Source for Folder {
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError> {
        Ok(Box::new(fs::File::open(self.path(url)).unwrap()))
    }

    fn open_from(&self, url: &str, from: u64) -> Result<Ranged, AppError> {
        self.asked.lock().unwrap().push(from);
        let mut file = fs::File::open(self.path(url)).unwrap();
        file.seek(SeekFrom::Start(from)).unwrap();
        Ok(Ranged {
            reader: Box::new(file),
            start: from,
            etag: Some(self.etag.lock().unwrap().clone()),
        })
    }
}

struct Machine {
    _dir: TempDir,
    root: Utf8PathBuf,
}

impl Machine {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
        Self { _dir: dir, root }
    }

    fn census(&self) -> Utf8PathBuf {
        self.root.join("census-history")
    }

    fn clones(&self) -> Clones {
        Clones {
            whole: self.census(),
            partial: self.root.join("census-partial"),
        }
    }

    fn downloads(&self) -> Utf8PathBuf {
        self.root.join("downloads")
    }

    /// Bootstraps the clone; `stop_after` sets the cancel once the download holds that many
    /// bytes.
    fn bootstrap(
        &self,
        folder: &Folder,
        release: &IndexRelease,
        stop_after: Option<u64>,
    ) -> Result<ArchiveNote, AppError> {
        let cancel = AtomicBool::new(false);
        let mut progress = |seen: ArchiveProgress| {
            if seen.step == InstallStep::Download && stop_after.is_some_and(|at| seen.done >= at) {
                cancel.store(true, Ordering::Relaxed);
            }
        };
        Bootstrap::new(folder, &cancel, &mut progress).run(
            release,
            &self.clones(),
            &self.downloads(),
        )
    }
}

fn names(dir: &Utf8Path) -> Vec<String> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn server_and_archive(root: &Utf8Path, padding: usize) -> (Server, Folder) {
    let server = two_builds(root.join("server"));
    let pack = server.pack(0, &root.join("pack"));
    let head = &server.commits[0];
    let folder = Folder::new(&archive(&pack, "refs/heads/history-v2", head, padding));
    (server, folder)
}

#[test]
fn an_archive_makes_a_whole_clone_at_its_branch_of_its_pack_alone() {
    let machine = Machine::new();
    let (server, folder) = server_and_archive(&machine.root, 0);

    let note = machine.bootstrap(&folder, &folder.release(), None).unwrap();

    let head = &server.commits[0];
    assert_eq!(&note.head, head);
    assert_eq!(
        machine.clones().view(),
        CloneView::Whole {
            tag: "history-v2-snapshot-0".to_string()
        }
    );
    assert_eq!(&GitRepo::open(&machine.census()).unwrap().head(), head);
    let packs = names(&machine.census().join("objects").join("pack"));
    assert!(
        packs.iter().all(|name| name.starts_with("pack-")),
        "{packs:?}"
    );
    assert!(
        packs.iter().any(|name| name.ends_with(".keep")),
        "{packs:?}"
    );
    assert!(!machine.census().join("hooks").join("pre-push").exists());
    assert!(
        names(&machine.downloads()).is_empty(),
        "the download is removed"
    );
}

#[test]
fn an_update_after_a_bootstrap_fetches_on_top_of_it() {
    let machine = Machine::new();
    let (server, folder) = server_and_archive(&machine.root, 0);
    machine.bootstrap(&folder, &folder.release(), None).unwrap();
    let indexes = machine.root.join("indexes");
    let first = indexes.join("first");
    server.index(0, &first);

    let (cancel, census) = (AtomicBool::new(false), machine.census());
    let updater = Updater::new(server.dir.as_str(), &census, &indexes, &cancel);
    let updated = updater.update(&first).unwrap().expect("a new index");

    let stamp = Stamp::read(&updated).unwrap();
    assert_eq!((&stamp.head, stamp.builds), (&server.commits[1], 2));
    assert_eq!(
        machine.clones().view(),
        CloneView::Whole {
            tag: "history-v2-snapshot-0".to_string()
        }
    );
}

#[test]
fn updates_fetch_into_the_whole_clone_once_a_bootstrap_made_one() {
    let machine = Machine::new();
    let (_server, folder) = server_and_archive(&machine.root, 0);
    let clones = machine.clones();
    gix::init_bare(clones.partial.as_std_path()).unwrap();
    assert_eq!(clones.view(), CloneView::Partial);
    assert_eq!(clones.for_updates(), clones.partial);

    machine.bootstrap(&folder, &folder.release(), None).unwrap();

    assert!(matches!(clones.view(), CloneView::Whole { .. }));
    assert_eq!(clones.for_updates(), clones.whole);
    assert!(!clones.partial.exists(), "the fetched clone is removed");
}

#[test]
fn a_second_bootstrap_takes_the_place_of_the_whole_clone() {
    let machine = Machine::new();
    let (_server, folder) = server_and_archive(&machine.root, 0);
    machine.bootstrap(&folder, &folder.release(), None).unwrap();
    fs::write(machine.census().join("held"), "").unwrap();

    machine.bootstrap(&folder, &folder.release(), None).unwrap();

    assert!(!machine.census().join("held").exists());
    assert!(matches!(machine.clones().view(), CloneView::Whole { .. }));
    assert_eq!(
        names(&machine.root),
        ["census-history", "downloads", "pack", "server"]
    );
}

#[test]
fn an_archive_without_the_branch_leaves_the_clones_held() {
    let machine = Machine::new();
    let server = two_builds(machine.root.join("server"));
    let pack = server.pack(0, &machine.root.join("pack"));
    let folder = Folder::new(&archive(&pack, "refs/heads/main", &server.commits[0], 0));
    let clones = machine.clones();
    gix::init_bare(clones.partial.as_std_path()).unwrap();

    let error = machine
        .bootstrap(&folder, &folder.release(), None)
        .unwrap_err();

    assert!(
        matches!(error, AppError::ArchiveUnreadable { .. }),
        "{error}"
    );
    assert_eq!(clones.view(), CloneView::Partial);
    assert!(!machine.census().exists());
    assert!(!machine.root.join("census-history.new").exists());
}

#[test]
fn a_download_that_stopped_resumes_from_where_it_stopped() {
    let machine = Machine::new();
    let (_server, folder) = server_and_archive(&machine.root, 400_000);
    let release = folder.release();

    let stopped = machine.bootstrap(&folder, &release, Some(100_000));
    assert!(matches!(stopped, Err(AppError::Cancelled)), "{stopped:?}");
    assert_eq!(machine.clones().view(), CloneView::None);

    machine.bootstrap(&folder, &release, None).unwrap();

    let asked = folder.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(
        asked[1] >= 100_000,
        "the second download resumed: {asked:?}"
    );
    assert!(matches!(machine.clones().view(), CloneView::Whole { .. }));
}

#[test]
fn a_download_of_another_version_starts_again() {
    let machine = Machine::new();
    let (_server, folder) = server_and_archive(&machine.root, 400_000);
    let release = folder.release();
    machine
        .bootstrap(&folder, &release, Some(100_000))
        .unwrap_err();
    *folder.etag.lock().unwrap() = "\"two\"".to_string();

    machine.bootstrap(&folder, &release, None).unwrap();

    let asked = folder.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked[2], 0, "the download started again: {asked:?}");
}

#[test]
fn an_archive_of_other_bytes_than_the_release_lists_is_not_unpacked() {
    let machine = Machine::new();
    let (_server, folder) = server_and_archive(&machine.root, 0);
    let mut release = folder.release();
    if let Some(archive) = release.archive.as_mut() {
        archive.sha256 = "0".repeat(64);
    }

    let error = machine.bootstrap(&folder, &release, None).unwrap_err();

    assert!(matches!(error, AppError::IndexDigest { .. }), "{error}");
    assert_eq!(machine.clones().view(), CloneView::None);
    assert!(
        names(&machine.downloads()).is_empty(),
        "{:?}",
        names(&machine.downloads())
    );
}

/// The newest published archive bootstrapped into a scratch folder, its clone opened at the
/// branch the release's index was built to.
#[test]
#[ignore = "downloads the published census history, some 1.8 GB, and unpacks it, 5.4 GB"]
fn the_published_archive_bootstraps_a_clone() {
    let machine = Machine::new();
    let web = reforge_app::index::Web::new().unwrap();
    let release = web.latest().unwrap().expect("a release holds an index");
    let view = release.view(&web).unwrap();
    let cancel = AtomicBool::new(false);
    let mut progress = |_| {};
    let started = std::time::Instant::now();
    let note = Bootstrap::new(&web, &cancel, &mut progress)
        .run(&release, &machine.clones(), &machine.downloads())
        .unwrap();
    eprintln!("{} bootstrapped in {:.0?}", note.tag, started.elapsed());
    assert_eq!(note.head, view.head, "the archive is at the index's head");
    assert_eq!(GitRepo::open(&machine.census()).unwrap().head(), view.head);
}

#[test]
fn a_release_without_an_archive_bootstraps_nothing() {
    let machine = Machine::new();
    let (_server, folder) = server_and_archive(&machine.root, 0);
    let mut release = folder.release();
    release.archive = None;

    let error = machine.bootstrap(&folder, &release, None).unwrap_err();

    assert_eq!(error, AppError::NoArchive);
}
