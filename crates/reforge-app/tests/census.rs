//! The history index taken forward from the census history: the builds after its head fetched
//! and appended to a copy in a folder of its own, the index read by the runs left as it is. The
//! census history here is a bare repository on disk served by `git upload-pack`, so the tests
//! need git on the path.

use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{Families, Stamp};
use reforge_app::census::Updater;
use reforge_app::error::AppError;
use reforge_census::{GitRepo, Indexer};
use tempfile::TempDir;

const WAD: &str = "files/data/x.wad.client";

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
}

/// Two builds: path 1 changes its bytes, and path 2 appears.
fn two_builds(dir: Utf8PathBuf) -> Server {
    let mut server = Server::new(dir);
    server.push("1.1.1", &[texture(1, 'a')]);
    server.push("1.2.1", &[texture(1, 'b'), texture(2, 'a')]);
    server
}

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
    (dir, root)
}

#[test]
fn an_index_behind_the_history_is_taken_forward_into_a_folder_of_its_own() {
    let (_dir, root) = scratch();
    let server = two_builds(root.join("server"));
    let (indexes, census) = (root.join("indexes"), root.join("census"));
    let first = indexes.join("first");
    server.index(0, &first);

    let cancel = AtomicBool::new(false);
    let updater = Updater::new(server.dir.as_str(), &census, &indexes, &cancel);
    assert!(updater.behind(&first).unwrap());
    let updated = updater.update(&first).unwrap().expect("a new index");

    let head = &server.commits[1];
    assert_eq!(updated, indexes.join(format!("head-{}", &head[..12])));
    let stamp = Stamp::read(&updated).unwrap();
    assert_eq!((&stamp.head, stamp.builds), (head, 2));
    assert_eq!(
        Stamp::read(&first).unwrap().head,
        server.commits[0],
        "the index read is left as it is"
    );
    assert!(!updater.behind(&updated).unwrap());
    assert_eq!(updater.update(&updated).unwrap(), None);
}

#[test]
fn an_index_of_another_history_is_refused() {
    let (_dir, root) = scratch();
    let server = two_builds(root.join("server"));
    let mut other = Server::new(root.join("other"));
    other.push("9.1.1", &[texture(1, 'c')]);
    let index = root.join("indexes").join("other");
    other.index(0, &index);

    let cancel = AtomicBool::new(false);
    let (census, indexes) = (root.join("census"), root.join("indexes"));
    let updater = Updater::new(server.dir.as_str(), &census, &indexes, &cancel);
    let error = updater.update(&index).unwrap_err();
    assert_eq!(
        error,
        AppError::IndexForeign {
            head: other.commits[0].clone()
        }
    );
}

#[test]
fn a_history_that_does_not_answer_is_a_fetch_error() {
    let (_dir, root) = scratch();
    let server = two_builds(root.join("server"));
    let index = root.join("indexes").join("first");
    server.index(0, &index);

    let cancel = AtomicBool::new(false);
    let (census, indexes, absent) = (
        root.join("census"),
        root.join("indexes"),
        root.join("absent"),
    );
    let updater = Updater::new(absent.as_str(), &census, &indexes, &cancel);
    let error = updater.update(&index).unwrap_err();
    assert!(matches!(error, AppError::CensusFetch { .. }), "{error}");
}

#[test]
fn a_folder_with_no_index_does_not_update() {
    let (_dir, root) = scratch();
    let cancel = AtomicBool::new(false);
    let census = root.join("census");
    let updater = Updater::new("unused", &census, &root, &cancel);
    let error = updater.update(&root.join("absent")).unwrap_err();
    assert!(matches!(error, AppError::IndexUnreadable { .. }), "{error}");
}
