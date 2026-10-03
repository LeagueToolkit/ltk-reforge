//! The readers of census format 2: `build.yaml`, `census.yaml`, an entry's file and an entry's
//! text, each on the text the census writes.

#[expect(
    dead_code,
    reason = "this test reads the fixture's texts and not its history"
)]
mod common;

use reforge::history::{Joint, Skeleton, Submesh, Texture};
use reforge::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, path_hash};
use reforge_census::format::{
    ClipRecord, EventRecord, ObjectRecord, census_format, parse_build, parse_entry,
    parse_object_text,
};

use self::common::{A, B, BANK, NAMED, object_text};

#[test]
fn a_build_file_gives_its_version_manifest_and_date() {
    let text = "version: \"16.19.8207193\"\npatch: \"16.19\"\nmanifest: \"ad6896ce5cf475a1\"\n\
                source: \"rman\"\ndate: \"2026-09-21\"\nlegacyBins: false\nrealms:\n - \"BR1\"\n \
                - \"EUW1\"\n";
    let build = parse_build(text).unwrap();
    assert_eq!(build.build.as_str(), "16.19.8207193");
    assert_eq!(build.manifest, "ad6896ce5cf475a1");
    assert_eq!(build.date, "2026-09-21");
    assert_eq!(parse_build("patch: \"16.19\"\n"), None);
    assert_eq!(parse_build("version: \"soon\"\n"), None);
    // A second manifest of one version: two builds of the history are written so.
    let second = parse_build("version: \"14.8.5756478__2\"\n").unwrap();
    assert_eq!(second.build.as_str(), "14.8.5756478__2");
}

#[test]
fn the_census_file_gives_its_format() {
    let text = "format: 2\ndescription: \"League of Legends builds\"\nfiles:\n  build.yaml: \
                \"schema/build.schema.json\"\n";
    assert_eq!(census_format(text), Some(2));
    assert_eq!(census_format("description: \"format: 3\"\n"), None);
}

#[test]
fn an_entry_without_a_digest_is_no_entry() {
    assert_eq!(parse_entry("kind: \"tex\"\n"), None);
    assert_eq!(parse_entry("sha256: \"abc\"\nkind: \"tex\"\n"), None);
}

#[test]
fn a_bins_entry_gives_its_objects() {
    let text = format!(
        "sha256: \"{A}\"\nchecksum: \"0000000000000001\"\nkind: \"bin\"\nlinks:\n - \
         \"data/x.bin\"\nobjects:\n \"00000001\":\n  class: \"00000002\"\n  content: \"{B}\"\n  \
         norm: \"f000000000000007\"\n \"0000000a\":\n  class: \"00000003\"\n  content: \"{A}\"\n  \
         norm: \"0000000000000008\"\n \"0000000b\":\n  class: \"00000003\"\n"
    );
    let entry = parse_entry(&text).unwrap();
    assert_eq!(entry.content, FileKey::parse(A).unwrap());
    assert_eq!(entry.kind, "bin");
    let first = ObjectRecord {
        entry: BinHash(1),
        class: BinHash(2),
        object: ObjectKey::parse(B).unwrap(),
        norm: NormKey(0xf000_0000_0000_0007),
    };
    let second = ObjectRecord {
        entry: BinHash(0xa),
        class: BinHash(3),
        object: ObjectKey::parse(A).unwrap(),
        norm: NormKey(8),
    };
    // The third object lacks its keys and is left out.
    assert_eq!(entry.objects, [first, second]);
    assert!(entry.clips.is_empty() && entry.skeleton.is_none() && entry.texture.is_none());
}

#[test]
fn a_banks_entry_gives_its_wems_and_events() {
    let text = format!(
        "sha256: \"{A}\"\nkind: \"bnk\"\nbank:\n version: 145\n bankId: \"00000005\"\n media:\n  \
         \"00000001\":\n   size: 10\n   hash: \"00000000000000c1\"\n  \"00000002\":\n   size: \
         20\n   hash: \"00000000000000c2\"\n\n events:\n  \"000000e1\":\n   - \"00000001\"\n   - \
         \"00000002\"\n  \"000000e2\": []\n\n"
    );
    let entry = parse_entry(&text).unwrap();
    let clip = |id, size, key| ClipRecord {
        id,
        size,
        clip: ClipKey(key),
    };
    assert_eq!(entry.clips, [clip(1, 10, 0xc1), clip(2, 20, 0xc2)]);
    let event = |id, wems: &[u32]| EventRecord {
        id,
        wems: wems.to_vec(),
    };
    assert_eq!(entry.events, [event(0xe1, &[1, 2]), event(0xe2, &[])]);
}

#[test]
fn a_skeletons_entry_gives_its_joints_with_their_parents() {
    let text = format!(
        "sha256: \"{A}\"\nkind: \"skl\"\nskeleton:\n influences: 2\n name: \"Rig\"\n joints:\n  \
         - name: \"Root\"\n  - name: \"Hip \\\"L\\\"\"\n    parent: \"Root\"\n  - name: \"Hip \
         \\\"L\\\"\"\n    parent: \"Root\"\n  - name: \"Toe\"\n    parent: 2\n"
    );
    let entry = parse_entry(&text).unwrap();
    let joint = |name: &str, parent: Option<&str>| Joint {
        name: name.into(),
        parent: parent.map(Into::into),
    };
    let expected = Skeleton {
        influences: 2,
        joints: vec![
            joint("Root", None),
            joint("Hip \"L\"", Some("Root")),
            joint("Hip \"L\"", Some("Root")),
            // An ordinal where two joints have the parent's name.
            joint("Toe", Some("#2")),
        ],
    };
    assert_eq!(entry.skeleton, Some(expected));
    assert_eq!(entry.mesh, None);
}

#[test]
fn a_meshs_entry_gives_its_submeshes() {
    let text = format!(
        "sha256: \"{A}\"\nkind: \"skn\"\nmesh:\n vertexType: \"basic\"\n vertices: 30\n indices: \
         60\n submeshes:\n  - name: \"Body\"\n    vertices: 20\n    indices: 42\n    positions: \
         \"00000000000000b1\"\n    rounded: \"00000000000000b2\"\n  - name: \"Cape\"\n    \
         vertices: 10\n    indices: 18\n    positions: \"00000000000000c1\"\n    rounded: \
         \"00000000000000c2\"\n"
    );
    let mesh = parse_entry(&text).unwrap().mesh.unwrap();
    assert_eq!(mesh.vertex_type, "basic");
    assert_eq!((mesh.vertices, mesh.indices), (30, 60));
    let submesh = |material: &str, vertices, indices, positions, rounded| Submesh {
        vertices,
        indices,
        positions,
        rounded,
        material: material.into(),
    };
    let expected = [
        submesh("Body", 20, 42, 0xb1, 0xb2),
        submesh("Cape", 10, 18, 0xc1, 0xc2),
    ];
    assert_eq!(mesh.submeshes, expected);
}

#[test]
fn a_textures_entry_gives_its_header() {
    let entry = parse_entry(&common::texture(A)).unwrap();
    let expected = Texture {
        format: "bc3".into(),
        width: 64,
        height: 64,
        mips: 7,
        top: 0xaa,
    };
    assert_eq!(entry.texture, Some(expected));
    assert_eq!(entry.kind, "tex");
}

#[test]
fn a_section_the_reader_does_not_know_is_skipped() {
    let text = format!(
        "sha256: \"{A}\"\nkind: \"tex\"\nlater:\n width: 1\n list:\n  - name: \"x\"\ntexture:\n \
         width: 8\n unheard: 3\n"
    );
    let entry = parse_entry(&text).unwrap();
    assert_eq!(entry.texture.unwrap().width, 8);
}

#[test]
fn an_entrys_text_gives_what_the_object_names() {
    let text = parse_object_text(&object_text()).unwrap();
    let mut paths = vec![WadHash(1), path_hash(NAMED), path_hash(BANK)];
    paths.sort_unstable();
    assert_eq!(text.paths, paths);
    assert_eq!(text.tags, ["Test"]);
    assert_eq!(text.units.len(), 1);
    let unit = &text.units[0];
    assert_eq!(unit.name, "Test_Base_VO");
    assert!(unit.voice);
    assert_eq!(unit.banks, [path_hash(BANK)]);
    assert_eq!(unit.events, [reforge_wwise::id_of("Play_vo_Test")]);
}

#[test]
fn a_text_cut_short_does_not_parse() {
    let text = object_text();
    assert_eq!(parse_object_text(&text[..text.len() / 2]), None);
}
