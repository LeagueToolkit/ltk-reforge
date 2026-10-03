//! The reader against bytes a mod could carry: clips larger than the data they are read from,
//! files cut short, fields overwritten. Each is read or refused with its error, and nothing
//! panics.

use reforge_wwise::{ACTION_PLAY, Bank, BankError, BankKind, Object, Wem, kind};

fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn section(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend(u32s(&[body.len() as u32]));
    out.extend(body);
    out
}

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn pad_to_eight(out: &mut Vec<u8>) {
    out.resize(out.len().next_multiple_of(8), 0);
}

/// A `BKHD` body of version 145: 20 bytes, so its section is 28.
fn header() -> Vec<u8> {
    let mut h = u32s(&[145, 0x0bad_cafe, 0x1770_5d3e]);
    h.extend(16u16.to_le_bytes());
    h.extend(0u16.to_le_bytes());
    h.extend(u32s(&[250]));
    h
}

fn bkhd() -> Vec<u8> {
    section(b"BKHD", &header())
}

/// A bank of a header and the given sections.
fn bank_of(sections: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = bkhd();
    for s in sections {
        bytes.extend(s);
    }
    bytes
}

/// A package of version 1: the count it states, the offsets, then `rest`, which starts at byte
/// 12 plus four per offset.
fn package_of(count: u32, offsets: &[u32], rest: &[u8]) -> Vec<u8> {
    let mut bytes = b"r3d2".to_vec();
    bytes.extend(u32s(&[1, count]));
    bytes.extend(u32s(offsets));
    bytes.extend(rest);
    bytes
}

/// A bank whose `DATA` is 1024 bytes and whose index names all of it `entries` times.
fn bank_naming_one_range(entries: u32) -> Vec<u8> {
    let index: Vec<u8> = (0..entries).flat_map(|i| u32s(&[i, 0, 1024])).collect();
    bank_of(&[section(b"DIDX", &index), section(b"DATA", &[0x5a; 1024])])
}

#[test]
fn a_bank_whose_clips_are_larger_than_its_data_is_refused() {
    let once = Bank::read(&bank_naming_one_range(1)).unwrap();
    assert_eq!(once.media.len(), 1);
    assert_eq!(once.media[0].bytes, vec![0x5a; 1024]);

    // 13 KB of file would read into 1 MB of clips.
    let bytes = bank_naming_one_range(1000);
    assert!(bytes.len() < 14_000);
    assert_eq!(
        Bank::read(&bytes),
        Err(BankError::ClipsLargerThanData {
            kind: BankKind::Bnk,
            clips: 1_024_000,
            data: 1024
        })
    );
    assert_eq!(
        Bank::read(&bank_naming_one_range(2)),
        Err(BankError::ClipsLargerThanData {
            kind: BankKind::Bnk,
            clips: 2048,
            data: 1024
        })
    );

    // Two clips that share ten bytes of a 64-byte `DATA`.
    let overlapping = bank_of(&[
        section(b"DIDX", &u32s(&[1, 0, 40, 2, 30, 34])),
        section(b"DATA", &[0; 64]),
    ]);
    assert_eq!(
        Bank::read(&overlapping),
        Err(BankError::ClipsLargerThanData {
            kind: BankKind::Bnk,
            clips: 74,
            data: 64
        })
    );
}

#[test]
fn clips_no_larger_than_their_data_are_read() {
    // Two clips that fill `DATA`.
    let filling = bank_of(&[
        section(b"DIDX", &u32s(&[1, 0, 32, 2, 32, 32])),
        section(b"DATA", &[7; 64]),
    ]);
    let bank = Bank::read(&filling).unwrap();
    assert_eq!(bank.media.len(), 2);
    assert!(bank.media.iter().all(|w| w.bytes == [7; 32]));

    // The rule is on the sizes: two entries naming the same four bytes of sixteen fit.
    let shared = bank_of(&[
        section(b"DIDX", &u32s(&[1, 4, 4, 2, 4, 4])),
        section(b"DATA", &[7; 16]),
    ]);
    let bank = Bank::read(&shared).unwrap();
    assert_eq!(bank.media[0].bytes, bank.media[1].bytes);
}

/// A package of one 256-byte clip and one entry, which `offsets` offsets all name.
fn package_naming_one_entry(offsets: u32) -> Vec<u8> {
    let mut bytes = b"r3d2".to_vec();
    bytes.extend(u32s(&[1, offsets]));
    let entry_at = (12 + 4 * offsets).next_multiple_of(8);
    bytes.extend((0..offsets).flat_map(|_| entry_at.to_le_bytes()));
    pad_to_eight(&mut bytes);
    assert_eq!(bytes.len(), entry_at as usize);
    let name = utf16("7.wem");
    let data_at = (entry_at + 12 + name.len() as u32).next_multiple_of(8);
    bytes.extend(u32s(&[data_at, 256, 5]));
    bytes.extend(name);
    pad_to_eight(&mut bytes);
    bytes.extend([0x5a; 256]);
    bytes
}

#[test]
fn a_package_whose_clips_are_larger_than_the_file_is_refused() {
    let once = Bank::read(&package_naming_one_entry(1)).unwrap();
    assert_eq!(
        once.media,
        vec![Wem {
            id: 7,
            bytes: vec![0x5a; 256]
        }]
    );

    for offsets in [2, 64, 4096] {
        let bytes = package_naming_one_entry(offsets);
        assert_eq!(
            Bank::read(&bytes),
            Err(BankError::ClipsLargerThanData {
                kind: BankKind::Wpk,
                clips: u64::from(offsets) * 256,
                data: bytes.len() as u64
            }),
            "{offsets} offsets"
        );
    }
}

/// A node's base parameters up to its parent, as version 145 writes them.
fn node_head(parent: u32) -> Vec<u8> {
    let mut b = vec![0, 0, 0, 0, 0];
    b.extend(u32s(&[0, parent]));
    b
}

fn sound(id: u32, clip: u32, parent: u32) -> Object {
    let mut body = u32s(&[0x0004_0001]);
    body.push(0);
    body.extend(u32s(&[clip, 1234]));
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
    body.extend(u32s(&[target]));
    body.extend([0, 1, 0x0e]);
    body.extend(0.5f32.to_le_bytes());
    body.extend([0, 4]);
    body.extend(u32s(&[bank]));
    Object {
        kind: kind::ACTION,
        id,
        body,
    }
}

fn objects() -> Vec<Object> {
    vec![
        Object {
            kind: kind::RANDOM,
            id: 50,
            body: node_head(0),
        },
        Object {
            kind: kind::MUSIC_SEGMENT,
            id: 51,
            body: [vec![0], node_head(50)].concat(),
        },
        sound(10, 1000, 50),
        sound(11, 1001, 50),
        play(20, 50, 0x0bad_cafe),
        Object::event(900, &[20], 145),
    ]
}

/// A well-formed bank with clips, objects and a section the reader keeps as bytes.
fn valid_bank() -> Vec<u8> {
    let mut bank = Bank::new_bnk(header(), true);
    bank.media = vec![
        Wem {
            id: 1000,
            bytes: vec![1; 13],
        },
        Wem {
            id: 1001,
            bytes: vec![2; 7],
        },
        Wem {
            id: 1002,
            bytes: vec![3; 32],
        },
    ];
    bank.objects = objects();
    let mut bytes = bank.write();
    bytes.extend(section(b"STID", &[1, 2, 3, 4, 5]));
    bytes
}

fn valid_package() -> Vec<u8> {
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
    Bank::new_wpk(media).write()
}

const VERSIONS: [u32; 8] = [0, 89, 90, 122, 123, 145, 146, u32::MAX];

/// Every reader of an object, on a body of any bytes.
fn read_object(object: &Object) {
    for version in VERSIONS {
        object.parent(version);
        object.event_actions(version);
    }
    object.media();
    object.action();
    object.play_bank();
    object.clone().set_play_bank(1);
}

/// Everything a caller does with a bank that read.
fn use_bank(bank: &Bank) {
    bank.objects.iter().for_each(read_object);
    bank.id();
    bank.cut_to(&[1000].into_iter().collect());
    bank.write();
}

#[test]
fn every_prefix_of_a_bank_and_of_a_package_reads_or_is_refused() {
    for bytes in [valid_bank(), valid_package()] {
        let whole = Bank::read(&bytes).unwrap();
        assert_eq!(whole.write(), bytes);
        let mut read = 0;
        for end in 0..bytes.len() {
            if let Ok(bank) = Bank::read(&bytes[..end]) {
                use_bank(&bank);
                read += 1;
            }
        }
        // A bank cut between two sections is a shorter bank; cut anywhere else it is refused.
        assert!(read < bytes.len() / 8, "{read} prefixes read");
    }
}

#[test]
fn no_overwritten_field_makes_the_reader_panic() {
    for bytes in [valid_bank(), valid_package()] {
        for at in 0..bytes.len() {
            for value in [0u32, 1, 0x7fff_ffff, 0xffff_fff0, u32::MAX] {
                let mut changed = bytes.clone();
                let end = (at + 4).min(bytes.len());
                changed[at..end].copy_from_slice(&value.to_le_bytes()[..end - at]);
                if let Ok(bank) = Bank::read(&changed) {
                    use_bank(&bank);
                }
            }
        }
    }
}

#[test]
fn the_object_readers_take_any_prefix_of_a_body() {
    for object in objects() {
        for end in 0..=object.body.len() {
            read_object(&Object {
                body: object.body[..end].to_vec(),
                ..object.clone()
            });
        }
    }
}

#[test]
fn an_event_that_states_more_actions_than_it_holds_reads_as_none() {
    let fixed = Object {
        kind: kind::EVENT,
        id: 1,
        body: u32s(&[u32::MAX, 20]),
    };
    assert_eq!(fixed.event_actions(122), None);
    // The variable-size count of 0x0fff_ffff, then one action.
    let var = Object {
        kind: kind::EVENT,
        id: 1,
        body: [vec![0xff, 0xff, 0xff, 0x7f], u32s(&[20])].concat(),
    };
    assert_eq!(var.event_actions(145), None);
    // A count that never ends.
    let endless = Object {
        kind: kind::EVENT,
        id: 1,
        body: vec![0xff; 16],
    };
    assert_eq!(endless.event_actions(145), None);
}

#[test]
fn each_way_a_bank_fails_to_read_has_its_error() {
    let hirc = |body: &[u8]| bank_of(&[section(b"HIRC", body)]);
    let sound_object = [vec![2], u32s(&[4, 77])].concat();
    let cases: Vec<(Vec<u8>, BankError)> = vec![
        (
            b"RIFF....WAVE".to_vec(),
            BankError::NotABank {
                len: 12,
                head: b"RIFF....".to_vec(),
            },
        ),
        (
            b"BKH".to_vec(),
            BankError::NotABank {
                len: 3,
                head: b"BKH".to_vec(),
            },
        ),
        (
            [bkhd(), b"DIDX\0".to_vec()].concat(),
            BankError::SectionHeaderCutShort { at: 28, len: 33 },
        ),
        (
            [bkhd(), b"DATA".to_vec(), u32s(&[100]), vec![0; 10]].concat(),
            BankError::SectionPastEnd {
                tag: *b"DATA",
                at: 28,
                size: 100,
                len: 46,
            },
        ),
        (
            [bkhd(), b"DATA".to_vec(), u32s(&[u32::MAX])].concat(),
            BankError::SectionPastEnd {
                tag: *b"DATA",
                at: 28,
                size: u32::MAX,
                len: 36,
            },
        ),
        (
            bank_of(&[bkhd()]),
            BankError::DuplicateSection {
                tag: *b"BKHD",
                at: 28,
            },
        ),
        (
            bank_of(&[
                section(b"DATA", &[]),
                section(b"STID", &[1]),
                section(b"DATA", &[]),
            ]),
            BankError::DuplicateSection {
                tag: *b"DATA",
                at: 45,
            },
        ),
        (
            section(b"BKHD", &[0; 7]),
            BankError::HeaderTooShort { size: 7 },
        ),
        (section(b"BKHD", &[]), BankError::HeaderTooShort { size: 0 }),
        (hirc(&[0; 3]), BankError::HircWithoutCount { size: 3 }),
        (
            hirc(&[u32s(&[2]), sound_object.clone(), vec![3, 1]].concat()),
            BankError::ObjectCutShort {
                index: 1,
                count: 2,
                at: 13,
                len: 15,
            },
        ),
        (
            hirc(&u32s(&[u32::MAX])),
            BankError::ObjectCutShort {
                index: 0,
                count: u32::MAX,
                at: 4,
                len: 4,
            },
        ),
        (
            hirc(&[u32s(&[1]), vec![2], u32s(&[50]), vec![0; 6]].concat()),
            BankError::ObjectPastSection {
                index: 0,
                kind: 2,
                size: 50,
                left: 6,
            },
        ),
        (
            hirc(&[u32s(&[1]), vec![2], u32s(&[3]), vec![0; 3]].concat()),
            BankError::ObjectWithoutId {
                index: 0,
                kind: 2,
                size: 3,
            },
        ),
        (
            bank_of(&[section(b"DIDX", &u32s(&[1, 0, 4, 2, 4, 4]))]),
            BankError::IndexWithoutData { clips: 2 },
        ),
        (
            bank_of(&[
                section(b"DIDX", &u32s(&[7, 10, 8])),
                section(b"DATA", &[0; 16]),
            ]),
            BankError::ClipPastData {
                id: 7,
                offset: 10,
                size: 8,
                data: 16,
            },
        ),
        (
            bank_of(&[
                section(b"DIDX", &u32s(&[7, u32::MAX, u32::MAX])),
                section(b"DATA", &[0; 16]),
            ]),
            BankError::ClipPastData {
                id: 7,
                offset: u32::MAX,
                size: u32::MAX,
                data: 16,
            },
        ),
        (
            b"r3d2\x01\0\0\0\x01\0\0".to_vec(),
            BankError::PackageHeaderCutShort { len: 11 },
        ),
        (
            package_of(3, &[16, 16], &[]),
            BankError::PackageOffsetsCutShort { count: 3, len: 20 },
        ),
        (
            package_of(u32::MAX, &[], &[]),
            BankError::PackageOffsetsCutShort {
                count: u32::MAX,
                len: 12,
            },
        ),
        (
            package_of(1, &[100], &[]),
            BankError::EntryCutShort {
                entry: 0,
                offset: 100,
                len: 16,
            },
        ),
        (
            package_of(2, &[0, 20], &u32s(&[0, 0])),
            BankError::EntryCutShort {
                entry: 1,
                offset: 20,
                len: 28,
            },
        ),
        (
            package_of(1, &[16], &[u32s(&[0, 0, 5]), vec![0; 4]].concat()),
            BankError::EntryNameCutShort {
                entry: 0,
                name_len: 5,
                left: 4,
            },
        ),
        (
            package_of(1, &[16], &u32s(&[0, 0, u32::MAX])),
            BankError::EntryNameCutShort {
                entry: 0,
                name_len: u32::MAX,
                left: 0,
            },
        ),
        (
            package_of(1, &[16], &[u32s(&[0, 0, 5]), utf16("x.wem")].concat()),
            BankError::EntryName {
                entry: 0,
                name: "x.wem".to_owned(),
            },
        ),
        (
            // One more than 32 bits hold.
            package_of(
                1,
                &[16],
                &[u32s(&[0, 0, 14]), utf16("4294967296.wem")].concat(),
            ),
            BankError::EntryName {
                entry: 0,
                name: "4294967296.wem".to_owned(),
            },
        ),
        (
            // Eleven digits: more than an id of 32 bits has.
            package_of(
                1,
                &[16],
                &[u32s(&[0, 0, 15]), utf16("00000000017.wem")].concat(),
            ),
            BankError::EntryName {
                entry: 0,
                name: "00000000017.wem".to_owned(),
            },
        ),
        (
            package_of(1, &[16], &[u32s(&[1000, 4, 5]), utf16("5.wem")].concat()),
            BankError::ClipPastData {
                id: 5,
                offset: 1000,
                size: 4,
                data: 38,
            },
        ),
        (
            package_of(
                1,
                &[16],
                &[u32s(&[u32::MAX, u32::MAX, 5]), utf16("5.wem")].concat(),
            ),
            BankError::ClipPastData {
                id: 5,
                offset: u32::MAX,
                size: u32::MAX,
                data: 38,
            },
        ),
    ];
    for (bytes, error) in cases {
        assert_eq!(Bank::read(&bytes), Err(error.clone()), "{error}");
    }
}

#[test]
fn a_package_entrys_id_is_the_digits_its_name_starts_with() {
    for (name, id) in [
        ("4294967295.wem", u32::MAX),
        ("0000000017.wem", 17),
        ("17", 17),
        ("17_old.wem", 17),
    ] {
        let name_len = name.encode_utf16().count() as u32;
        let entry = [u32s(&[0, 0, name_len]), utf16(name)].concat();
        let package = Bank::read(&package_of(1, &[16], &entry)).unwrap();
        assert_eq!(package.media, vec![Wem { id, bytes: vec![] }], "{name}");
    }
}

#[test]
fn an_error_shows_the_files_bytes_escaped_and_a_names_start() {
    let tag = [bkhd(), b"\x1b[2J".to_vec(), u32s(&[100])].concat();
    let name = "\u{1b}]0;\u{202e}\u{7}".repeat(2000);
    let entry = [
        u32s(&[0, 0, name.encode_utf16().count() as u32]),
        utf16(&name),
    ]
    .concat();
    let named = package_of(1, &[16], &entry);
    for bytes in [b"\x1b[2J\x07\xff".to_vec(), tag, named] {
        let message = Bank::read(&bytes).unwrap_err().to_string();
        assert!(
            message.chars().all(|c| c.is_ascii() && !c.is_control()),
            "{message:?}"
        );
        assert!(message.len() < 1000, "{} bytes", message.len());
    }
}
