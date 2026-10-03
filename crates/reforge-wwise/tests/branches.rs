//! The branches the version-145 fixtures of `bank.rs` do not take: other bank versions, a header
//! that states no alignment, a package with empty slots.
//!
//! No bank of another version is at hand. Each fixture lays the fields out as the reader expects
//! them at that version, from a layout written out here per version. The tests hold the version
//! thresholds in place; they do not show that the layouts are Wwise's.

use reforge_wwise::{
    ACTION_PLAY, Bank, BankKind, CLIENT_VERSION, Object, Wem, id_of, is_bnk, is_wpk, kind,
};

fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

const PARENT: u32 = 0x00c0_ffee;

/// How a node's base parameters are laid out up to its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layout {
    /// The bytes of one initial effect.
    effect: usize,
    /// Whether the metadata effects are there.
    metadata: bool,
    /// Whether the attachment byte is there.
    attachment: bool,
}

const TO_89: Layout = Layout {
    effect: 7,
    metadata: false,
    attachment: false,
};
const TO_136: Layout = Layout {
    effect: 7,
    metadata: false,
    attachment: true,
};
const TO_145: Layout = Layout {
    effect: 7,
    metadata: true,
    attachment: true,
};
const FROM_146: Layout = Layout {
    effect: 6,
    metadata: true,
    attachment: false,
};

/// The versions on each side of every threshold, with the layout the reader expects there.
const VERSIONS: [(u32, Layout); 8] = [
    (72, TO_89),
    (89, TO_89),
    (90, TO_136),
    (136, TO_136),
    (137, TO_145),
    (145, TO_145),
    (146, FROM_146),
    (150, FROM_146),
];

/// A node's base parameters up to its parent, then some more bytes.
fn node_head(layout: Layout, effects: u8, parent: u32) -> Vec<u8> {
    let mut b = vec![0, effects];
    if effects > 0 {
        b.push(0); // bypass bits
        b.extend(vec![0xee; usize::from(effects) * layout.effect]);
    }
    if layout.metadata {
        b.extend([0, 1]); // one metadata effect
        b.extend([0xdd; 6]);
    }
    if layout.attachment {
        b.push(0);
    }
    b.extend(u32s(&[0x0b0b_0b0b, parent])); // override bus
    b.extend([9, 9, 9]);
    b
}

fn node(kind: u8, body: Vec<u8>) -> Object {
    Object { kind, id: 1, body }
}

/// A sound's fields before its node parameters: a codec, or a source plugin and its parameters.
fn sound_head(clip: u32, source_plugin: bool) -> Vec<u8> {
    let plugin = if source_plugin {
        0x0065_0002
    } else {
        0x0004_0001
    };
    let mut b = u32s(&[plugin]);
    b.push(0); // stream type
    b.extend(u32s(&[clip, 1234]));
    b.push(0);
    if source_plugin {
        b.extend(u32s(&[5]));
        b.extend([0xcc; 5]);
    }
    b
}

fn play(id: u32, target: u32, bank: u32) -> Object {
    let mut body = (u16::from(ACTION_PLAY) << 8 | 3).to_le_bytes().to_vec();
    body.extend(u32s(&[target]));
    body.extend([0, 0, 0, 4]); // not a bus, no properties, no ranged properties, fade curve
    body.extend(u32s(&[bank]));
    Object {
        kind: kind::ACTION,
        id,
        body,
    }
}

#[test]
fn a_nodes_parent_reads_at_every_version() {
    for (version, layout) in VERSIONS {
        for effects in [0, 2] {
            let head = node_head(layout, effects, PARENT);
            for kind in [kind::RANDOM, kind::SWITCH, kind::ACTOR_MIXER, kind::LAYER] {
                let parent = node(kind, head.clone()).parent(version);
                assert_eq!(parent, Some(PARENT), "kind {kind}, version {version}");
            }
            let orphan = node(kind::RANDOM, node_head(layout, effects, 0));
            assert_eq!(orphan.parent(version), None, "version {version}");
        }
    }
}

#[test]
fn a_nodes_parent_does_not_read_across_a_version_threshold() {
    let thresholds = [
        (89, TO_89, 90, TO_136),
        (136, TO_136, 137, TO_145),
        (145, TO_145, 146, FROM_146),
    ];
    for (below, layout_below, above, layout_above) in thresholds {
        for effects in [0, 2] {
            let low = node(kind::RANDOM, node_head(layout_below, effects, PARENT));
            let high = node(kind::RANDOM, node_head(layout_above, effects, PARENT));
            assert_eq!(low.parent(below), Some(PARENT));
            assert_eq!(high.parent(above), Some(PARENT));
            assert_ne!(low.parent(above), Some(PARENT), "{below} read as {above}");
            assert_ne!(high.parent(below), Some(PARENT), "{above} read as {below}");
        }
    }
}

#[test]
fn a_sounds_parent_follows_its_source() {
    for (version, layout) in VERSIONS {
        for source_plugin in [false, true] {
            let mut body = sound_head(1000, source_plugin);
            body.extend(node_head(layout, 1, PARENT));
            let sound = node(kind::SOUND, body);
            assert_eq!(sound.media(), Some(1000));
            assert_eq!(
                sound.parent(version),
                Some(PARENT),
                "version {version}, source plugin {source_plugin}"
            );
        }
    }
}

#[test]
fn a_music_nodes_parent_follows_a_flags_byte_from_version_90() {
    for (version, layout) in VERSIONS {
        let head = node_head(layout, 1, PARENT);
        let with_flags = [vec![0], head.clone()].concat();
        let body = if version > 89 { with_flags } else { head };
        for kind in [
            kind::MUSIC_SEGMENT,
            kind::MUSIC_SWITCH,
            kind::MUSIC_PLAYLIST,
        ] {
            let parent = node(kind, body.clone()).parent(version);
            assert_eq!(parent, Some(PARENT), "kind {kind}, version {version}");
        }
    }
    // The flags byte alone separates 89 from 90 for a music node.
    let without = node(kind::MUSIC_SEGMENT, node_head(TO_89, 1, PARENT));
    assert_eq!(without.parent(89), Some(PARENT));
    let with = node(
        kind::MUSIC_SEGMENT,
        [vec![0], node_head(TO_136, 1, PARENT)].concat(),
    );
    assert_eq!(with.parent(90), Some(PARENT));
    assert_ne!(with.parent(89), Some(PARENT));
}

#[test]
fn an_object_that_is_no_node_has_no_parent() {
    let head = node_head(TO_145, 0, PARENT);
    for kind in [kind::ACTION, kind::EVENT, 1, 8, 11, 99] {
        assert_eq!(node(kind, head.clone()).parent(145), None, "kind {kind}");
    }
    let random = node(kind::RANDOM, head);
    assert_eq!(random.media(), None);
    assert_eq!(random.action(), None);
    assert_eq!(random.play_bank(), None);
    assert_eq!(random.event_actions(145), None);
}

#[test]
fn an_action_that_does_not_play_names_no_bank() {
    // A stop action: type 1 in the high byte.
    let mut stop = play(20, 50, 7);
    stop.body[1] = 1;
    let action = stop.action().unwrap();
    assert_eq!((action.action_type, action.target), (1, 50));
    assert_eq!(action.bank_at, None);
    assert_eq!(stop.play_bank(), None);
    let before = stop.clone();
    stop.set_play_bank(9);
    assert_eq!(stop, before);
}

#[test]
fn an_event_counts_its_actions_in_four_bytes_up_to_version_122() {
    let actions = [20, 21];
    for version in [72, 122] {
        let event = Object::event(5, &actions, version);
        assert_eq!(event.body, u32s(&[2, 20, 21]), "version {version}");
        assert_eq!(event.event_actions(version), Some(actions.to_vec()));
    }
    for version in [123, 145, 150] {
        let event = Object::event(5, &actions, version);
        assert_eq!(event.body, [vec![2], u32s(&[20, 21])].concat());
        assert_eq!(event.event_actions(version), Some(actions.to_vec()));
    }
    let fixed = Object::event(5, &actions, 122);
    assert_ne!(fixed.event_actions(123), Some(actions.to_vec()));
    let var = Object::event(5, &actions, 123);
    assert_ne!(var.event_actions(122), Some(actions.to_vec()));
}

#[test]
fn a_count_over_127_takes_two_bytes_from_version_123() {
    let actions: Vec<u32> = (1000..1200).collect();
    let event = Object::event(5, &actions, CLIENT_VERSION);
    // 200 is one group of 128 and 72: the high seven bits first, with the top bit set.
    assert_eq!(event.body[..2], [0x81, 0x48]);
    assert_eq!(event.body.len(), 2 + 4 * 200);
    assert_eq!(event.event_actions(CLIENT_VERSION), Some(actions));
}

/// A `BKHD` body of the given version and alignment.
fn header(version: u32, id: u32, alignment: u16) -> Vec<u8> {
    let mut h = u32s(&[version, id, 0x1770_5d3e]);
    h.extend(alignment.to_le_bytes());
    h.extend(0u16.to_le_bytes());
    h.extend(u32s(&[250]));
    h
}

#[test]
fn a_bank_of_version_122_is_cut_and_written_at_its_version() {
    let id = id_of("old_skin01_sfx_events");
    let sound = |object: u32, clip: u32, parent: u32| Object {
        kind: kind::SOUND,
        id: object,
        body: [sound_head(clip, false), node_head(TO_136, 0, parent)].concat(),
    };
    let container = |object: u32| Object {
        kind: kind::RANDOM,
        id: object,
        body: node_head(TO_136, 0, 0),
    };
    let mut bank = Bank::new_bnk(header(122, id, 16), true);
    assert_eq!(bank.version, 122);
    bank.objects = vec![
        container(50),
        sound(10, 1000, 50),
        sound(11, 1001, 50),
        container(60),
        sound(12, 1002, 60),
        play(20, 50, id),
        play(21, 60, id),
        Object::event(900, &[20, 21], 122),
        Object::event(901, &[21], 122),
    ];

    let bytes = bank.write();
    let back = Bank::read(&bytes).unwrap();
    assert_eq!(back.version, 122);
    assert_eq!(back.objects, bank.objects);
    assert_eq!(back.object(10).unwrap().parent(back.version), Some(50));
    assert_eq!(back.write(), bytes);

    // Clip 1000 is under container 50: event 900 keeps the action that plays it and loses the
    // other, and event 901 goes.
    let cut = back.cut_to(&[1000].into_iter().collect());
    assert_eq!(cut.events, vec![900]);
    let kept = cut.objects.iter().find(|o| o.kind == kind::EVENT).unwrap();
    assert_eq!(kept.body, u32s(&[1, 20]), "the count stays four bytes");
    let actions: Vec<u32> = cut
        .objects
        .iter()
        .filter(|o| o.kind == kind::ACTION)
        .map(|o| o.id)
        .collect();
    assert_eq!(actions, vec![20]);
    assert_eq!(cut.objects.len(), 7, "every node, one action, one event");
    assert_eq!(cut.silent, 1, "sound 11 is under the kept action");
}

#[test]
fn a_header_that_states_no_alignment_gives_sixteen() {
    let short = Bank::new_bnk(u32s(&[CLIENT_VERSION, 7]), false);
    assert_eq!(short.alignment, 16);
    let zero = Bank::new_bnk(header(CLIENT_VERSION, 7, 0), false);
    assert_eq!(zero.alignment, 16);

    let mut bank = short;
    bank.media = vec![
        Wem {
            id: 1,
            bytes: vec![1; 5],
        },
        Wem {
            id: 2,
            bytes: vec![2; 5],
        },
    ];
    let bytes = bank.write();
    let back = Bank::read(&bytes).unwrap();
    assert_eq!((back.alignment, back.id()), (16, 7));
    assert_eq!(back.media, bank.media);
    assert_eq!(back.write(), bytes);
}

#[test]
fn clips_align_to_the_alignment_the_header_states() {
    let mut bank = Bank::new_bnk(header(CLIENT_VERSION, 7, 32), false);
    assert_eq!(bank.alignment, 32);
    bank.media = vec![
        Wem {
            id: 1,
            bytes: vec![0xa1; 5],
        },
        Wem {
            id: 2,
            bytes: vec![0xa2; 5],
        },
    ];
    let bytes = bank.write();
    // `BKHD` takes 28 bytes and `DIDX` 32, so `DATA` starts at byte 68: the clips sit at 96
    // and 128.
    assert_eq!(bytes[96..101], [0xa1; 5]);
    assert_eq!(bytes[128..133], [0xa2; 5]);
    let back = Bank::read(&bytes).unwrap();
    assert_eq!(back.alignment, 32);
    assert_eq!(back.write(), bytes);
}

#[test]
fn a_header_too_short_for_a_version_or_an_id_gives_the_clients_version_and_no_id() {
    let mut bank = Bank::new_bnk(vec![1, 2, 3], false);
    assert_eq!(bank.version, CLIENT_VERSION);
    assert_eq!(bank.id(), 0);
    bank.set_id(9);
    assert_eq!(bank.header, vec![1, 2, 3]);

    let mut bank = Bank::new_bnk(header(CLIENT_VERSION, 7, 16), true);
    bank.set_id(9);
    assert_eq!(bank.id(), 9);
    assert_eq!(bank.header, header(CLIENT_VERSION, 9, 16));
}

#[test]
fn a_package_keeps_its_version_and_skips_its_zero_offsets() {
    // Version 2, three offsets of which the second names an entry, at byte 24.
    let mut bytes = b"r3d2".to_vec();
    bytes.extend(u32s(&[2, 3, 0, 24, 0]));
    bytes.extend(u32s(&[48, 4, 6]));
    bytes.extend(utf16("17.wem"));
    bytes.extend([6; 4]);
    assert_eq!(bytes.len(), 52);

    let package = Bank::read(&bytes).unwrap();
    assert_eq!((package.kind, package.version), (BankKind::Wpk, 2));
    assert_eq!(
        package.media,
        vec![Wem {
            id: 17,
            bytes: vec![6; 4]
        }]
    );
    // The writer writes an entry per clip: the empty slots are not written back.
    let written = Bank::read(&package.write()).unwrap();
    assert_eq!(written, package);
}

#[test]
fn a_package_is_told_by_its_magic_and_a_small_version() {
    assert!(is_wpk(b"r3d2\x0f\0\0\0"));
    assert!(!is_wpk(b"r3d2\x10\0\0\0"));
    assert!(!is_wpk(b"r3d2\x01\0\0"), "seven bytes");
    assert!(is_bnk(b"BKHD\0\0\0\0"));
    assert!(!is_bnk(b"BKHD\0\0\0"), "seven bytes");
    assert!(!is_bnk(b"r3d2\x01\0\0\0"));
}
