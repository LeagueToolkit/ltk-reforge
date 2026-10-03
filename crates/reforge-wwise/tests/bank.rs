//! Banks and packages built by hand at version 145: what the reader takes off each object, the
//! clips' alignment from the file's start, and that everything writes back as it was read.
//! ltk-forge held the same reader and writer to the real banks of the in-game samples and of
//! 16.19 with `forge bank`: each wrote back byte for byte (2026-09-29).

use reforge_wwise::{ACTION_PLAY, Bank, BankKind, CLIENT_VERSION, Object, Wem, id_of, kind};

fn header(id: u32, alignment: u16) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend(CLIENT_VERSION.to_le_bytes());
    h.extend(id.to_le_bytes());
    h.extend(0x1770_5d3eu32.to_le_bytes()); // language
    h.extend(alignment.to_le_bytes());
    h.extend(0u16.to_le_bytes());
    h.extend(250u32.to_le_bytes());
    h
}

/// A node's base parameters up to its parent, as version 145 writes them, then some more bytes.
fn node_head(parent: u32) -> Vec<u8> {
    let mut b = vec![0, 0]; // no effects
    b.extend([0, 0]); // no metadata effects
    b.push(0); // attachment parameters
    b.extend(0u32.to_le_bytes()); // override bus
    b.extend(parent.to_le_bytes());
    b.extend([9, 9, 9]);
    b
}

fn sound(id: u32, clip: u32, parent: u32) -> Object {
    let mut body = 0x0004_0001u32.to_le_bytes().to_vec(); // a codec, not a source plugin
    body.push(0); // stream type
    body.extend(clip.to_le_bytes());
    body.extend(1234u32.to_le_bytes());
    body.push(0);
    body.extend(node_head(parent));
    Object {
        kind: kind::SOUND,
        id,
        body,
    }
}

fn play(id: u32, target: u32, bank: u32) -> Object {
    let mut body = (u16::from(ACTION_PLAY) << 8 | 3).to_le_bytes().to_vec();
    body.extend(target.to_le_bytes());
    body.push(0); // not a bus
    body.push(1); // one property: its id, then its value
    body.push(0x0e);
    body.extend(0.5f32.to_le_bytes());
    body.push(0); // no ranged properties
    body.push(4); // fade curve
    body.extend(bank.to_le_bytes());
    Object {
        kind: kind::ACTION,
        id,
        body,
    }
}

fn events_bank() -> Bank {
    let id = id_of("test_skin03_sfx_events");
    let mut b = Bank::new_bnk(header(id, 16), true);
    b.objects = vec![
        Object {
            kind: kind::RANDOM,
            id: 50,
            body: node_head(0),
        },
        sound(10, 1000, 50),
        sound(11, 1001, 50),
        play(20, 50, id),
        Object::event(id_of("Play_sfx_TestSkin03_Dance"), &[20], CLIENT_VERSION),
    ];
    b
}

#[test]
fn the_reader_takes_what_a_cut_needs() {
    let bytes = events_bank().write();
    let b = Bank::read(&bytes).unwrap();
    assert_eq!(b.kind, BankKind::Bnk);
    assert_eq!(b.version, CLIENT_VERSION);
    assert_eq!(b.id(), id_of("test_skin03_sfx_events"));
    assert!(b.has_hirc() && b.media.is_empty());
    assert_eq!(b.object(10).unwrap().media(), Some(1000));
    assert_eq!(b.object(10).unwrap().parent(b.version), Some(50));
    assert_eq!(b.object(50).unwrap().parent(b.version), None);
    let action = b.object(20).unwrap().action().unwrap();
    assert_eq!((action.action_type, action.target), (ACTION_PLAY, 50));
    assert_eq!(b.object(20).unwrap().play_bank(), Some(b.id()));
    let event = b.objects.iter().find(|o| o.kind == kind::EVENT).unwrap();
    assert_eq!(event.event_actions(b.version), Some(vec![20]));
    assert_eq!(b.write(), bytes, "writes back byte for byte");
}

#[test]
fn a_play_action_is_pointed_at_another_bank() {
    let mut b = events_bank();
    let before = b.object(20).unwrap().body.clone();
    b.objects[3].set_play_bank(7);
    assert_eq!(b.object(20).unwrap().play_bank(), Some(7));
    let after = &b.object(20).unwrap().body;
    assert_eq!(before.len(), after.len());
    let at = b.object(20).unwrap().action().unwrap().bank_at.unwrap();
    let changed: Vec<usize> = before
        .iter()
        .zip(after)
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .collect();
    assert!(
        changed.iter().all(|i| (at..at + 4).contains(i)),
        "only the bank id's bytes change: {changed:?}"
    );
}

#[test]
fn clips_align_from_the_start_of_the_file() {
    let mut b = Bank::new_bnk(header(id_of("test_base_sfx_audio"), 16), false);
    b.media = vec![
        Wem {
            id: 1,
            bytes: vec![1; 13],
        },
        Wem {
            id: 2,
            bytes: vec![2; 7],
        },
        Wem {
            id: 3,
            bytes: vec![3; 32],
        },
    ];
    let bytes = b.write();
    let back = Bank::read(&bytes).unwrap();
    assert_eq!(back.media, b.media);
    assert!(!back.has_hirc());
    // Each clip's bytes sit at a multiple of sixteen in the file.
    for w in &back.media {
        let at = bytes
            .windows(w.bytes.len())
            .position(|x| x == w.bytes.as_slice())
            .unwrap();
        assert_eq!(at % 16, 0, "clip {} at {at}", w.id);
    }
    assert_eq!(back.write(), bytes);
}

#[test]
fn a_package_round_trips() {
    let media = vec![
        Wem {
            id: 2_931_053,
            bytes: vec![5; 21],
        },
        Wem {
            id: 17,
            bytes: vec![6; 8],
        },
    ];
    let bytes = Bank::new_wpk(media.clone()).write();
    let back = Bank::read(&bytes).unwrap();
    assert_eq!(back.kind, BankKind::Wpk);
    assert_eq!(back.media, media);
    assert_eq!(back.write(), bytes);
}

#[test]
fn ids_are_fnv1_of_the_lowercase_name() {
    // As the game's own bank and event names give them (16.19).
    assert_eq!(id_of("braum_base_sfx_audio"), 3_580_963_463);
    assert_eq!(id_of("Braum_Base_SFX_Audio"), 3_580_963_463);
}

#[test]
fn not_a_bank_is_refused() {
    assert!(Bank::read(b"RIFF....").is_err());
    assert!(Bank::read(b"r3d2Mesh").is_err(), "a mesh spells r3d2 on");
}

#[test]
fn a_cut_keeps_every_node_and_the_events_that_reach_the_clips() {
    let b = events_bank();
    // Clip 1000 is under the random container the event's action plays; 1001 beside it is not.
    let cut = b.cut_to(&[1000].into_iter().collect());
    assert_eq!(cut.events, vec![id_of("Play_sfx_TestSkin03_Dance")]);
    assert_eq!(
        cut.objects.len(),
        b.objects.len(),
        "nothing dropped: the event reaches the clip"
    );
    assert_eq!(cut.silent, 1, "sound 11 plays a clip the cut does not hold");

    // A clip nothing plays: the event and its action go, every node stays.
    let cut = b.cut_to(&[77].into_iter().collect());
    assert!(cut.events.is_empty());
    assert!(
        cut.objects
            .iter()
            .all(|o| o.kind != kind::EVENT && o.kind != kind::ACTION)
    );
    assert_eq!(cut.objects.len(), 3);
}
