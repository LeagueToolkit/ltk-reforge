//! The readers of census format 2: `build.yaml`, `census.yaml`, an entry's file and an entry's
//! text, each on the text the census writes.

#[expect(
    dead_code,
    reason = "this test reads the fixture's texts and not its history"
)]
mod common;
mod link;

use ltk_meta::PropertyKind as Kind;
use reforge::game::FieldType;
use reforge::history::tables::{ClassField, JointList};
use reforge::history::{Joint, Skeleton, Submesh, Texture};
use reforge::model::{
    BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, name_hash, path_hash,
};
use reforge_census::format::{
    ClipRecord, EventRecord, ObjectRecord, census_format, parse_build, parse_entry,
    parse_object_text, parse_wad,
};

use self::common::{A, B, BANK, NAMED, object_text};
use self::link::linking;

#[test]
fn a_build_file_gives_its_version_manifest_and_date() {
    let text = "version: \"16.19.8207193\"\npatch: \"16.19\"\nmanifest: \"ad6896ce5cf475a1\"\n\
                source: \"rman\"\ndate: \"2026-09-21\"\nlegacyBins: false\nrealms:\n - \"BR1\"\n \
                - \"EUW1\"\n";
    let build = parse_build(text).unwrap();
    assert_eq!(build.build.as_str(), "16.19.8207193");
    assert_eq!(build.manifest, "ad6896ce5cf475a1");
    assert_eq!(build.date, "2026-09-21");
    let details = &build.details;
    assert_eq!(
        (details.patch.as_str(), details.source.as_str()),
        ("16.19", "rman")
    );
    assert_eq!(details.realms, ["BR1", "EUW1"]);
    assert!(!details.legacy_bins && details.solution.is_empty() && details.exe.is_empty());
    assert_eq!(parse_build("patch: \"16.19\"\n"), None);
    assert_eq!(parse_build("version: \"soon\"\n"), None);
    // A second manifest of one version: two builds of the history are written so.
    let second = parse_build("version: \"14.8.5756478__2\"\n").unwrap();
    assert_eq!(second.build.as_str(), "14.8.5756478__2");
}

#[test]
fn a_build_read_from_rads_gives_its_solution_release_and_executable() {
    let text = "version: \"8.20.2483196\"\npatch: \"8.20\"\nmanifest: \"e39c38647162cf28\"\n\
                source: \"rads\"\nsolution: \"lol_game_client_sln\"\nrelease: \"0.0.1.250\"\n\
                exe: \"8.20.249.4223\"\ndate: \"2018-10-03\"\nlegacyBins: true\nrealms:\n - \
                \"rads:live\"\n";
    let details = parse_build(text).unwrap().details;
    assert_eq!(details.source, "rads");
    assert_eq!(details.solution, "lol_game_client_sln");
    assert_eq!(details.release, "0.0.1.250");
    assert_eq!(details.exe, "8.20.249.4223");
    assert!(details.legacy_bins);
    assert_eq!(details.realms, ["rads:live"]);
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
    assert_eq!(entry.checksum, 1);
    assert_eq!(entry.links, Some(vec![path_hash("data/x.bin")]));
}

#[test]
fn a_bin_that_links_nothing_has_links_and_an_entry_that_is_no_bin_has_none() {
    let bin = format!("sha256: \"{A}\"\nkind: \"bin\"\nlinks: []\n");
    let bin = parse_entry(&bin).unwrap();
    assert_eq!((bin.links, bin.checksum), (Some(Vec::new()), 0));
    let patch = parse_entry(&format!("sha256: \"{A}\"\nkind: \"bin\"\n")).unwrap();
    assert_eq!(patch.links, None);
}

#[test]
fn a_wads_file_gives_its_version_file_id_and_tags() {
    let text =
        "version: \"3.4\"\nfileId: \"27f7f49ff6d41bc4\"\ntags:\n - \"en_AU\"\n - \"en_GB\"\n";
    let wad = parse_wad(text);
    assert_eq!(wad.version, "3.4");
    assert_eq!(wad.file_id, 0x27f7_f49f_f6d4_1bc4);
    assert_eq!(wad.tags, ["en_AU", "en_GB"]);
    let none = parse_wad("version: \"3.1\"\nfileId: \"0000000000000001\"\ntags: []\n");
    assert!(none.tags.is_empty());
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

/// The events bank of two sounds' worth of a champion's bank, cut to an action and its event,
/// as the census writes it at 16.19.8207193.
fn events_bank_text() -> String {
    format!(
        "sha256: \"{A}\"\nchecksum: \"f78c45a5f3b0fe7c\"\nkind: \"bnk\"\nbank:\n version: 145\n \
         bankId: \"76c106e7\"\n header: \
         \"91000000e706c1763e5d701710000000fa000000000000005ff66d161cc3390eb13e2d936fd4b034\"\n \
         events:\n  \"4cdaedcd\":\n   - \"1e421809\"\n\n  \"720db6a3\": []\n\n objects:\n  - id: \
         \"1970540b\"\n    type: 3\n    body: \"03043a2a262700000004e706c17600000000\"\n  - id: \
         \"4cdaedcd\"\n    type: 4\n    body: \"010b547019\"\n"
    )
}

#[test]
fn an_events_banks_entry_gives_its_header_and_objects_as_stored() {
    let entry = parse_entry(&events_bank_text()).unwrap();
    assert_eq!(entry.events.len(), 2);
    let bank = entry.bank.unwrap();
    assert_eq!((bank.version(), bank.bank_id()), (145, 0x76c1_06e7));
    assert_eq!(bank.header.len(), 40);
    assert_eq!(bank.header[36..], [0x6f, 0xd4, 0xb0, 0x34]);
    let action = &bank.objects[0];
    assert_eq!((action.kind, action.id), (3, 0x1970_540b));
    assert_eq!(action.body.len(), 18);
    assert_eq!(action.action().unwrap().target, 0x2726_2a3a);
    let event = &bank.objects[1];
    assert_eq!((event.kind, event.id), (4, 0x4cda_edcd));
    assert_eq!(event.event_actions(145), Some(vec![0x1970_540b]));

    // The header and the objects are what a bank is written from, and they read back.
    let mut written = reforge_wwise::Bank::new_bnk(bank.header.clone(), true);
    written.objects = bank.objects.clone();
    let read = reforge_wwise::Bank::read(&written.write()).unwrap();
    assert_eq!((read.header, read.objects), (bank.header, bank.objects));
}

#[test]
fn a_bank_with_no_hierarchy_gives_no_events_bank() {
    let media = format!(
        "sha256: \"{A}\"\nkind: \"bnk\"\nbank:\n version: 145\n bankId: \"00000005\"\n media:\n  \
         \"00000001\":\n   size: 10\n   hash: \"00000000000000c1\"\n\n"
    );
    let entry = parse_entry(&media).unwrap();
    assert_eq!(entry.bank, None);
    assert_eq!(entry.clips.len(), 1);
}

#[test]
fn a_hierarchy_with_no_objects_is_an_events_bank() {
    for objects in ["", " objects: []\n"] {
        let text = format!(
            "sha256: \"{A}\"\nkind: \"bnk\"\nbank:\n version: 145\n bankId: \"00000005\"\n \
             header: \"9100000005000000\"\n{objects}"
        );
        let bank = parse_entry(&text).unwrap().bank.unwrap();
        assert_eq!((bank.version(), bank.bank_id()), (145, 5));
        assert!(bank.objects.is_empty());
    }
}

#[test]
fn a_bank_the_bank_reader_refuses_gives_no_events_bank() {
    let bank = |header: &str, object: &str| {
        let text = format!(
            "sha256: \"{A}\"\nkind: \"bnk\"\nbank:\n version: 145\n bankId: \"00000005\"\n \
             header: \"{header}\"\n objects:\n{object}"
        );
        parse_entry(&text).unwrap().bank
    };
    let whole = "  - id: \"000000e1\"\n    type: 4\n    body: \"00\"\n";
    assert!(bank("9100000005000000", whole).is_some());
    // A header too short for a version and a bank id.
    assert_eq!(bank("91000000", whole), None);
    // An object too short for an id.
    assert_eq!(
        bank("9100000005000000", "  - type: 4\n    body: \"00\"\n"),
        None
    );
    // Bytes that are not whole.
    let odd = "  - id: \"000000e1\"\n    type: 4\n    body: \"000\"\n";
    assert_eq!(bank("9100000005000000", odd), None);
}

#[test]
fn an_animation_clips_entry_gives_the_joints_it_animates() {
    let entry = parse_entry(&common::animation('5')).unwrap();
    assert_eq!(entry.kind, "anm");
    assert_eq!(entry.joints, Some(JointList(common::JOINTS.into())));
    assert_eq!(entry.skeleton, None);

    let none = format!("sha256: \"{A}\"\nkind: \"anm\"\nclip:\n joints: []\n");
    assert_eq!(
        parse_entry(&none).unwrap().joints,
        Some(JointList::default())
    );
    // An entry of a history without the key.
    let before = format!("sha256: \"{A}\"\nkind: \"anm\"\n");
    assert_eq!(parse_entry(&before).unwrap().joints, None);
    // A list short of a joint is not the clip's.
    let cut =
        format!("sha256: \"{A}\"\nkind: \"anm\"\nclip:\n joints:\n  - \"0102a113\"\n  - \"06\n");
    assert_eq!(parse_entry(&cut).unwrap().joints, None);
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
fn a_texts_schema_is_the_type_of_every_field_of_every_struct_in_it() {
    let text = parse_object_text(&object_text()).unwrap();
    let ty = |kind, item, class: Option<&str>| FieldType {
        kind,
        key: None,
        item,
        class: class.map(name_hash),
    };
    let field = |class: BinHash, field: &str, ty| ClassField {
        class,
        field: name_hash(field),
        ty,
    };
    let (object, unit) = (BinHash(2), name_hash("BankUnit"));
    let list = |item| ty(Kind::Container, Some(item), None);
    let mut expected = vec![
        field(object, "tagEventList", list(Kind::String)),
        field(object, "texture", ty(Kind::WadChunkLink, None, None)),
        field(object, "iconSquare", ty(Kind::String, None, None)),
        field(
            object,
            "bankUnits",
            ty(
                Kind::UnorderedContainer,
                Some(Kind::Embedded),
                Some("BankUnit"),
            ),
        ),
        field(unit, "name", ty(Kind::String, None, None)),
        field(unit, "bankPath", list(Kind::String)),
        field(unit, "events", list(Kind::String)),
        field(unit, "voiceOver", ty(Kind::Bool, None, None)),
    ];
    expected.sort_unstable();
    assert_eq!(text.schema, expected);
}

#[test]
fn a_texts_links_are_every_link_it_holds_once() {
    let text = parse_object_text(&linking(0x11, &[0x13, 0x12, 0])).unwrap();
    assert_eq!(text.links, [BinHash(0x12), BinHash(0x13)]);
    assert!(text.paths.is_empty());
}

#[test]
fn a_text_cut_short_does_not_parse() {
    let text = object_text();
    assert_eq!(parse_object_text(&text[..text.len() / 2]), None);
}
