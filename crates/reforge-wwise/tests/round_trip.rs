//! A bank and a package assembled byte by byte, without the writer. The layout of a well-formed
//! file is stated here, and the reader and the writer are both held to it.

use reforge_wwise::{Bank, BankKind, Object, Wem};

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

#[test]
fn a_bank_assembled_by_hand_writes_back_byte_for_byte() {
    // Version 145, the bank id, the language, an alignment of sixteen, the project id.
    let mut header = u32s(&[145, 0x0bad_cafe, 0x1770_5d3e]);
    header.extend(16u16.to_le_bytes());
    header.extend(0u16.to_le_bytes());
    header.extend(u32s(&[250]));
    assert_eq!(header.len(), 20);

    // `BKHD` takes 28 bytes and `DIDX` 32, so the body of `DATA` starts at byte 68. The first
    // clip sits at byte 80 (12 into `DATA`), the second at byte 96 (28 into `DATA`).
    let index = u32s(&[1001, 12, 13, 1002, 28, 7]);
    let mut data = vec![0; 12];
    data.extend([0xa1; 13]);
    data.extend([0; 3]);
    data.extend([0xa2; 7]);

    // Two objects: a type byte, the size of the id and the body, the id, the body.
    let mut hirc = u32s(&[2]);
    hirc.push(2);
    hirc.extend(u32s(&[4 + 6, 77]));
    hirc.extend([1, 2, 3, 4, 5, 6]);
    hirc.push(0x63);
    hirc.extend(u32s(&[4, 78]));

    let mut bytes = section(b"BKHD", &header);
    bytes.extend(section(b"DIDX", &index));
    bytes.extend(section(b"DATA", &data));
    bytes.extend(section(b"HIRC", &hirc));
    bytes.extend(section(b"STID", &[7, 0, 0, 0, 1, 2, 3]));
    bytes.extend(section(b"PLAT", &[]));
    assert_eq!(&bytes[80..93], &[0xa1; 13]);
    assert_eq!(&bytes[96..103], &[0xa2; 7]);

    let bank = Bank::read(&bytes).unwrap();
    assert_eq!(bank.kind, BankKind::Bnk);
    assert_eq!(bank.version, 145);
    assert_eq!(bank.id(), 0x0bad_cafe);
    assert_eq!(bank.header, header);
    assert_eq!(bank.alignment, 16);
    let clips = vec![
        Wem {
            id: 1001,
            bytes: vec![0xa1; 13],
        },
        Wem {
            id: 1002,
            bytes: vec![0xa2; 7],
        },
    ];
    assert_eq!(bank.media, clips);
    let objects = vec![
        Object {
            kind: 2,
            id: 77,
            body: vec![1, 2, 3, 4, 5, 6],
        },
        Object {
            kind: 0x63,
            id: 78,
            body: Vec::new(),
        },
    ];
    assert_eq!(bank.objects, objects);
    assert!(bank.has_hirc());
    assert_eq!(bank.write(), bytes);
}

#[test]
fn a_package_assembled_by_hand_writes_back_byte_for_byte() {
    // The magic, version 1, two entries, their offsets: 20 bytes, padded to 24.
    let mut bytes = b"r3d2".to_vec();
    bytes.extend(u32s(&[1, 2, 24, 64]));
    bytes.extend([0; 4]);
    // The entry at 24: the clip's offset, its size, the name's length, the name. Padded to 64.
    bytes.extend(u32s(&[88, 21, 11]));
    bytes.extend(utf16("2931053.wem"));
    bytes.extend([0; 6]);
    assert_eq!(bytes.len(), 64);
    bytes.extend(u32s(&[112, 8, 6]));
    bytes.extend(utf16("17.wem"));
    // The clips, each at a multiple of eight, the last one unpadded.
    assert_eq!(bytes.len(), 88);
    bytes.extend([5; 21]);
    bytes.extend([0; 3]);
    assert_eq!(bytes.len(), 112);
    bytes.extend([6; 8]);

    let package = Bank::read(&bytes).unwrap();
    assert_eq!(package.kind, BankKind::Wpk);
    assert_eq!(package.version, 1);
    assert_eq!(package.id(), 0);
    assert!(package.objects.is_empty() && !package.has_hirc());
    let clips = vec![
        Wem {
            id: 2_931_053,
            bytes: vec![5; 21],
        },
        Wem {
            id: 17,
            bytes: vec![6; 8],
        },
    ];
    assert_eq!(package.media, clips);
    assert_eq!(package.write(), bytes);
}
