//! A census history in memory, shared by the integration tests.

use std::collections::BTreeMap;

use reforge::game::Build;
use reforge::model::{WadHash, name_hash, path_hash};
use reforge_census::format::BuildFile;
use reforge_census::{BlobId, CensusError, Change, ChangeSource};
use sha2::{Digest, Sha256};

/// A history in memory: per commit, the build and the whole tree under `files/` as path to
/// text. The commit of build `n` is `c<n>`.
#[derive(Clone)]
pub struct Memory {
    /// The commits, oldest first.
    pub commits: Vec<(BuildFile, BTreeMap<String, String>)>,
}

/// A text's blob id in memory: the leading twenty bytes of its SHA-256.
fn blob_id(text: &str) -> BlobId {
    let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
    let mut id = [0; 20];
    id.copy_from_slice(&digest[..20]);
    id
}

impl Memory {
    fn at(&self, commit: &str) -> &(BuildFile, BTreeMap<String, String>) {
        let found = commit[1..].parse::<usize>().ok();
        found
            .and_then(|i| self.commits.get(i))
            .unwrap_or_else(|| panic!("{commit:?} is no commit of the history in memory"))
    }
}

impl ChangeSource for Memory {
    fn commits(&self) -> Result<Vec<String>, CensusError> {
        Ok((0..self.commits.len()).map(|i| format!("c{i}")).collect())
    }

    fn build(&self, commit: &str) -> Result<BuildFile, CensusError> {
        Ok(self.at(commit).0.clone())
    }

    fn changes(
        &self,
        commit: &str,
        parent: Option<&str>,
        texts: bool,
    ) -> Result<Vec<Change>, CensusError> {
        let empty = BTreeMap::new();
        let new = &self.at(commit).1;
        let old = parent.map_or(&empty, |parent| &self.at(parent).1);
        let mut paths: Vec<&String> = old.keys().chain(new.keys()).collect();
        paths.sort();
        paths.dedup();
        let mut out = Vec::new();
        for path in paths {
            if !texts && path.ends_with(".rito") {
                continue;
            }
            let (o, n) = (old.get(path), new.get(path));
            if o != n {
                out.push(Change {
                    path: path.clone(),
                    old: o.map(|text| blob_id(text)),
                    new: n.map(|text| blob_id(text)),
                });
            }
        }
        Ok(out)
    }

    fn read(&self, id: &BlobId) -> Result<Vec<u8>, CensusError> {
        let mut texts = self.commits.iter().flat_map(|(_, files)| files.values());
        let text = texts.find(|text| blob_id(text) == *id);
        let text = text.unwrap_or_else(|| panic!("no blob {id:02x?} in the history in memory"));
        Ok(text.clone().into_bytes())
    }
}

/// The `build.yaml` of a build at this version.
pub fn build(version: &str) -> BuildFile {
    BuildFile {
        build: Build::parse(version).unwrap(),
        manifest: "0000000000000000".into(),
        date: "2026-01-01".into(),
    }
}

/// The digest of bytes A, as the census writes it.
pub const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// The digest of bytes B.
pub const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

const WAD: &str = "files/data/x.wad.client";

/// The bank the object's unit loads, and the entry it is in the WAD.
pub const BANK: &str = "ASSETS/Sounds/Test_VO_events.bnk";
/// A path the object names by its spelling.
pub const NAMED: &str = "ASSETS/Characters/Test/Test.dds";

/// The path hash of `BANK`.
pub fn bank() -> WadHash {
    path_hash(BANK)
}

/// The file of the WAD entry at a path hash, and its text.
pub fn entry(path: u64, text: String) -> (String, String) {
    (format!("{WAD}/{:02x}/{path:016x}.yaml", path >> 56), text)
}

fn digest(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}

/// A texture's entry file with these bytes.
pub fn texture(bytes: &str) -> String {
    format!(
        "sha256: \"{bytes}\"\nchecksum: \"0000000000000001\"\nkind: \"tex\"\ntexture:\n format: \
         \"bc3\"\n width: 64\n height: 64\n mips: 7\n top: \"00000000000000aa\"\n"
    )
}

/// A skeleton's entry file: a root, and a hip under it when `hip`.
pub fn skeleton(hip: bool) -> String {
    format!(
        "sha256: \"{}\"\nkind: \"skl\"\nskeleton:\n influences: 4\n joints:\n  - name: \
         \"Root\"\n{}",
        digest(if hip { 'd' } else { 'c' }),
        if hip {
            "  - name: \"Hip\"\n    parent: \"Root\"\n"
        } else {
            ""
        }
    )
}

/// The header of the events bank: version 145, bank id 5.
pub const HEADER: [u8; 8] = [0x91, 0, 0, 0, 5, 0, 0, 0];

/// An events bank's entry file: wem 1 with this hash, and event 0xe1, which plays it. Its
/// hierarchy is sound 1 (type 2), whose body is the clip's low byte, and the event (type 4).
pub fn events_bank(bytes: char, clip: u64) -> String {
    let header: String = HEADER.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "sha256: \"{}\"\nkind: \"bnk\"\nbank:\n version: 145\n bankId: \"00000005\"\n header: \
         \"{header}\"\n media:\n  \"00000001\":\n   size: 10\n   hash: \"{clip:016x}\"\n\n \
         events:\n  \"000000e1\":\n   - \"00000001\"\n\n objects:\n  - id: \"00000001\"\n    \
         type: 2\n    body: \"{clip:02x}\"\n  - id: \"000000e1\"\n    type: 4\n    body: \
         \"0101000000\"\n",
        digest(bytes)
    )
}

/// The joints the fixture's animation clip animates.
pub const JOINTS: [u32; 2] = [0x0102_a113, 0xf604_ab91];

/// An animation clip's entry file with these bytes, which animates `JOINTS`.
pub fn animation(bytes: char) -> String {
    let joints: String = JOINTS
        .iter()
        .map(|j| format!("  - \"{j:08x}\"\n"))
        .collect();
    format!(
        "sha256: \"{}\"\nkind: \"anm\"\nclip:\n joints:\n{joints}",
        digest(bytes)
    )
}

/// The `.rito` text of object 1 (class 2) of bin 2: an audio tag, a file by its chunk hash (path
/// 1), and a voice unit loading `BANK` and listing one event.
pub fn object_text() -> String {
    let h = |name: &str| name_hash(name).0;
    format!(
        r#"#PROP_text
type: string = "PROP"
version: u32 = 3
linked: list[string] = {{}}
entries: map[hash,embed] = {{
    0x00000001 = 0x00000002 {{
        {tags:#010x}: list[string] = {{
            "Test"
        }}
        {texture:#010x}: file = 0x0000000000000001
        {icon:#010x}: string = "{NAMED}"
        {units:#010x}: list2[embed] = {{
            {unit:#010x} {{
                {name:#010x}: string = "Test_Base_VO"
                {paths:#010x}: list[string] = {{
                    "{BANK}"
                }}
                {events:#010x}: list[string] = {{
                    "Play_vo_Test"
                }}
                {voice:#010x}: bool = true
            }}
        }}
    }}
}}
"#,
        tags = h("tagEventList"),
        texture = h("texture"),
        icon = h("iconSquare"),
        units = h("bankUnits"),
        unit = h("BankUnit"),
        name = h("name"),
        paths = h("bankPath"),
        events = h("events"),
        voice = h("voiceOver"),
    )
}

/// Three builds:
///
/// - path 1: a texture, bytes A, then B, then A; path 3: bytes A in the last build only.
/// - path 2: a bin of one object throughout (key A, entry 1, class 2, norm 7), whose text names
///   path 1 and `NAMED` and has a voice unit loading `BANK`.
/// - path 4: a skeleton that gains a joint in the last build.
/// - `BANK`: wem 1 is clip 1 in the first build and clip 2 after; event 0xe1 plays it. The
///   bank's sound object changes with the clip, and its event object stays.
/// - path 7: an animation clip whose bytes change in the last build and whose joints stay.
pub fn memory() -> Memory {
    let tree = |i: usize| {
        let bin = format!(
            "sha256: \"{}\"\nkind: \"bin\"\nlinks: []\nobjects:\n \"00000001\":\n  class: \
             \"00000002\"\n  content: \"{A}\"\n  norm: \"0000000000000007\"\n",
            digest('9')
        );
        let text = format!("{WAD}/00/0000000000000002.bin/00/00000001.rito");
        let wad = "version: \"3.4\"\nfileId: \"0000000000000001\"\ntags: []\n";
        let events = if i == 0 {
            events_bank('e', 1)
        } else {
            events_bank('f', 2)
        };
        let mut files = BTreeMap::from([
            entry(1, texture(if i == 1 { B } else { A })),
            entry(2, bin),
            (text, object_text()),
            entry(4, skeleton(i == 2)),
            entry(7, animation(if i == 2 { '6' } else { '5' })),
            entry(bank().0, events),
            (format!("{WAD}/_wad.yaml"), wad.to_string()),
        ]);
        if i == 2 {
            files.extend([entry(3, texture(A))]);
        }
        files
    };
    Memory {
        commits: vec![
            (build("1.1.1"), tree(0)),
            (build("1.2.1"), tree(1)),
            (build("1.3.1"), tree(2)),
        ],
    }
}
