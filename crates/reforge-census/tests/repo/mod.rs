//! A census history written to a git repository, for the tests that read one through gix.

use std::collections::BTreeMap;

use reforge_census::format::BuildFile;

/// A build's `build.yaml`, as the census writes it.
fn build_yaml(build: &BuildFile) -> String {
    let d = &build.details;
    let mut yaml = format!(
        "version: \"{}\"\npatch: \"{}\"\nmanifest: \"{}\"\nsource: \"{}\"\n",
        build.build, d.patch, build.manifest, d.source
    );
    for (key, value) in [
        ("solution", &d.solution),
        ("release", &d.release),
        ("exe", &d.exe),
    ] {
        if !value.is_empty() {
            yaml.push_str(&format!("{key}: \"{value}\"\n"));
        }
    }
    yaml.push_str(&format!(
        "date: \"{}\"\nlegacyBins: {}\nrealms:\n",
        build.date, d.legacy_bins
    ));
    for realm in &d.realms {
        yaml.push_str(&format!(" - \"{realm}\"\n"));
    }
    yaml
}

/// Commits a build on top of `parent` on the census branch of a repository, and answers the
/// commit.
pub fn commit(
    repo: &gix::Repository,
    parent: Option<&str>,
    build: &BuildFile,
    files: &BTreeMap<String, String>,
) -> String {
    let empty = gix::ObjectId::empty_tree(gix::hash::Kind::Sha1);
    let mut tree = repo.edit_tree(empty).unwrap();
    let mut put = |path: &str, text: &str| {
        let blob = repo.write_blob(text).unwrap().detach();
        tree.upsert(path, gix::object::tree::EntryKind::Blob, blob)
            .unwrap();
    };
    put("census.yaml", "format: 2\n");
    put("build.yaml", &build_yaml(build));
    for (path, text) in files {
        put(path, text);
    }
    let tree = tree.write().unwrap().detach();
    let census = gix::actor::SignatureRef {
        name: "census".into(),
        email: "census@localhost".into(),
        time: "1700000000 +0000",
    };
    let parents = parent.map(|c| gix::ObjectId::from_hex(c.as_bytes()).unwrap());
    let branch = "refs/heads/history-v2";
    let made = repo.commit_as(
        census,
        census,
        branch,
        build.build.to_string(),
        tree,
        parents,
    );
    made.unwrap().to_string()
}
