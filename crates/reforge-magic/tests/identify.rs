//! What `identify` makes of first bytes and names, the pitfalls first: the `r3d2` family, magics
//! at an offset, short magics that text can start with, and the formats with no magic at all.

use reforge_magic::{Confidence, HEAD_LEN, HeadKind, Kind, identify};

fn kind(head: &[u8]) -> Kind {
    identify(head, None).kind
}

fn named(head: &[u8], name: &str) -> Kind {
    identify(head, Some(name)).kind
}

fn how(head: &[u8], name: Option<&str>) -> (Kind, Confidence) {
    let identity = identify(head, name);
    (identity.kind, identity.confidence)
}

/// Bytes no magic, text check or heuristic accepts.
const OPAQUE: &[u8] = b"\x99\x98\x97\x96\x95";

const OLE: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1";

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

#[test]
fn the_r3d2_family_matches_whole_before_any_looser_r3d2() {
    assert_eq!(kind(b"r3d2anmd\x05\0\0\0"), Kind::Animation);
    assert_eq!(kind(b"r3d2canm\x01\0\0\0"), Kind::Animation);
    assert_eq!(kind(b"r3d2sklt\x02\0\0\0"), Kind::Skeleton);
    assert_eq!(kind(b"r3d2Mesh\x02\0\0\0"), Kind::StaticMeshBinary);
    assert_eq!(kind(b"r3d2wght"), Kind::SkinWeights);
    assert_eq!(kind(b"r3d2\x01\0\0\0\x10\0\0\0"), Kind::WwisePackage);
    assert_eq!(kind(b"r3d2\x07\0\0\0"), Kind::R3d2);
}

#[test]
fn magics_at_an_offset() {
    // The newer skeleton: its magic at byte 4.
    assert_eq!(kind(b"\x10\0\0\0\xc3\x4f\xfd\x22\0\0"), Kind::Skeleton);
    assert_eq!(kind(b"\x1bLuaQ\0"), Kind::LuaObj);
    assert_eq!(kind(b"RIFF\0\0\0\0WAVEfmt "), Kind::Wav);
    assert_eq!(
        identify(b"RIFF\0\0\0\0WAVEfmt ", Some("x/516735348.wem")).kind,
        Kind::Wav,
        "the bytes win over the name"
    );
    assert_eq!(kind(b"\0\0\0\x20ftypisom"), Kind::Mp4);
    assert_eq!(kind(b"FOR4\0\0\0\x20Maya"), Kind::MayaBinary);
    assert_eq!(
        kind(b"FOR4\0\x000\xf4CIMGFVER\0\0\0\x04"),
        Kind::MayaImage,
        "a swatch thumbnail"
    );
}

#[test]
fn short_magics_are_held_to_what_follows_them() {
    assert_eq!(kind(b"PROP\x03\0\0\0\x01\0\0\0"), Kind::PropertyBin);
    assert_eq!(kind(b"PROPERTY of the author\n"), Kind::Text);
    assert_eq!(kind(b"MZ is not a header, it is a sentence.\n"), Kind::Text);
    let mut pe = vec![0u8; 64];
    pe[..2].copy_from_slice(b"MZ");
    pe[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    assert_eq!(kind(&pe), Kind::Pe);
    assert_eq!(kind(b"RST\x05\0\0\0"), Kind::StringTable);
    assert_eq!(kind(b"RSTUV words\n"), Kind::Text);
}

#[test]
fn text_is_read_for_what_it_says() {
    assert_eq!(
        kind(b"#PROP_text\ntype: string = \"PROP\""),
        Kind::RitobinText
    );
    assert_eq!(
        kind(b"\xef\xbb\xbf#PROP_text\n"),
        Kind::RitobinText,
        "after a byte-order mark"
    );
    assert_eq!(
        named(b"#PROP_text\n", "skin0.py"),
        Kind::RitobinText,
        "a .py is often a bin as text"
    );
    assert_eq!(kind(b"{\n  \"entries\": {\n"), Kind::RitobinJson);
    assert_eq!(
        kind(b"{ \"type\": \"PROP\", \"version\": 3"),
        Kind::RitobinJson
    );
    assert_eq!(kind(b"{\n  \"name\": 1 }"), Kind::Json);
    assert_eq!(kind(b"[\n  \"assets/a.tex\""), Kind::Json);
    assert_eq!(kind(b"[Localized]\nkey=1"), Kind::Ini);
    assert_eq!(named(b"[System]\nGroupPart1=x", "a.troy"), Kind::Troy);
    assert_eq!(kind(b"[.ShellClassInfo]\nIconResource=x"), Kind::DesktopIni);
    assert_eq!(named(b"import os\n", "tool.py"), Kind::Script);
    assert_eq!(kind(b"<?xml version=\"1.0\"?><svg"), Kind::Svg);
    assert_eq!(kind(b"; FBX 7.4.0 project file"), Kind::FbxAscii);
    assert_eq!(kind(b"//Maya ASCII 2020 scene"), Kind::MayaAscii);
}

#[test]
fn a_name_wins_where_it_says_what_the_file_is() {
    assert_eq!(named(b"\0\0\0\x01Bud1", "folder/.DS_Store"), Kind::DsStore);
    assert_eq!(
        named(b"\x00\x05\x16\x07", "__MACOSX/a/._skin.dds"),
        Kind::AppleDouble
    );
    assert_eq!(named(OLE, "folder/Thumbs.db"), Kind::ThumbsDb);
    assert_eq!(
        named(OLE, "scene.max"),
        Kind::Max,
        "an OLE file named a Max scene"
    );
    assert_eq!(kind(OLE), Kind::Ole);
    assert_eq!(
        identify(b"\x07\x07\x07\x07", Some("a.spp")).confidence,
        Confidence::Name
    );
    assert_eq!(
        named(
            b"\x8a\x01\x00\x00\x10",
            "data/final/champions/lux.wad.subchunktoc"
        ),
        Kind::SubchunkToc
    );
    assert_eq!(
        named(
            b"\x01\x02\x00\x00\x01\x00\x01\x00\x18\x00\x0b\x00",
            "assets/recommendations/items/champion_98_items.rec"
        ),
        Kind::ItemRecommendations
    );
    assert_eq!(kind(b"MayaIcons\0\0\0"), Kind::MayaSwatches);
}

#[test]
fn formats_with_no_magic_come_last() {
    // A TGA's header: no colour map, true colour.
    let tga = identify(b"\0\0\x02\0\0\0\0\0\0\0\0\0\x00\x01", None);
    assert_eq!(
        (tga.kind, tga.confidence),
        (Kind::Tga, Confidence::Heuristic)
    );
    // A `.troybin` as the datasets carry them: version 2, string table 0x69, flags 0x1bb0.
    assert_eq!(kind(b"\x02i\x00\xb0\x1b\x03\x00vU\xe8"), Kind::Inibin);
    assert_eq!(
        kind(b"\x02\xff\xff\xff\xff\x00\x00"),
        Kind::Unknown,
        "flags 14 and 15 are never set"
    );
    assert_eq!(kind(b""), Kind::Empty);
    assert_eq!(kind(b"\x99\x98\x97\x96\x95"), Kind::Unknown);
}

#[test]
fn every_fixed_magic_names_its_kind() {
    let magics: [(&[u8], Kind); 50] = [
        (b"r3d2anmd", Kind::Animation),
        (b"r3d2canm", Kind::Animation),
        (b"r3d2sklt", Kind::Skeleton),
        (b"r3d2Mesh", Kind::StaticMeshBinary),
        (b"r3d2wght", Kind::SkinWeights),
        (b"Kaydara FBX Binary", Kind::FbxBinary),
        (b"//Maya ASCII", Kind::MayaAscii),
        (b"MayaIcons", Kind::MayaSwatches),
        (b"gimp xcf", Kind::Xcf),
        (b"\x00\x00\x00\x01Bud1", Kind::DsStore),
        (b"\x00\x05\x16\x07", Kind::AppleDouble),
        (b"L\x00\x00\x00\x01\x14\x02\x00", Kind::Shortcut),
        (b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1", Kind::Ole),
        (b"\x89PNG\r\n\x1a\n", Kind::Png),
        (b"Rar!\x1a\x07", Kind::Rar),
        (b"7z\xbc\xaf\x27\x1c", Kind::SevenZip),
        (b"\xfd7zXZ\x00", Kind::Xz),
        (b"\x1bLuaQ", Kind::LuaObj),
        (b"PreLoad", Kind::Preload),
        (b"BLENDER", Kind::Blend),
        (b"GIF87a", Kind::Gif),
        (b"GIF89a", Kind::Gif),
        (b"%PDF-", Kind::Pdf),
        (b"v/1\x01", Kind::Exr),
        (b"PK\x03\x04", Kind::Zip),
        (b"PK\x05\x06", Kind::Zip),
        (b"PTCH", Kind::PropertyBinOverride),
        (b"TEX\x00", Kind::Texture),
        (b"DDS ", Kind::TextureDds),
        (b"BKHD", Kind::WwiseBank),
        (b"OEGM", Kind::MapGeometry),
        (b"WGEO", Kind::WorldGeometry),
        (b"[Obj", Kind::StaticMeshAscii),
        (b"\x33\x22\x11\x00", Kind::SimpleSkin),
        (b"8BPS", Kind::Psd),
        (b"PDN3", Kind::Pdn),
        (b"glTF", Kind::GltfBinary),
        (b"OggS", Kind::Ogg),
        (b"fLaC", Kind::Flac),
        (b"OTTO", Kind::Otf),
        (b"ttcf", Kind::Ttc),
        (b"wOFF", Kind::Woff),
        (b"wOF2", Kind::Woff2),
        (b"\x00\x01\x00\x00", Kind::Ttf),
        (b"MSCF", Kind::Cab),
        (b"\x28\xb5\x2f\xfd", Kind::Zstd),
        (b"\x1a\x45\xdf\xa3", Kind::Webm),
        (b"\x7fELF", Kind::Elf),
        (b"II*\x00", Kind::Tiff),
        (b"MM\x00*", Kind::Tiff),
    ];
    for (magic, expected) in magics {
        assert_eq!(
            how(magic, None),
            (expected, Confidence::Magic),
            "{magic:?} alone"
        );
        let mut file = magic.to_vec();
        file.extend_from_slice(b"\x07\x00\xff rest");
        assert_eq!(
            how(&file, None),
            (expected, Confidence::Magic),
            "{magic:?} and more"
        );
        // A container's magic is the one an extension refines.
        if expected != Kind::Ole {
            assert_eq!(
                how(&file, Some("a.tga")),
                (expected, Confidence::Magic),
                "{magic:?} named a TGA"
            );
        }
    }
}

#[test]
fn a_fixed_magic_cut_short_is_not_its_kind() {
    assert_eq!(kind(b"r3d2anm"), Kind::R3d2);
    assert_eq!(kind(b"\x89PNG\r\n\x1a"), Kind::Unknown);
    assert_eq!(kind(b"GIF89"), Kind::Text);
    assert_eq!(kind(b"Kaydara FBX"), Kind::Text);
}

#[test]
fn a_short_magic_is_its_kind_at_the_versions_the_format_has() {
    for version in 1..=3u8 {
        assert_eq!(
            kind(&[b'P', b'R', b'O', b'P', version, 0, 0, 0]),
            Kind::PropertyBin
        );
    }
    assert_eq!(kind(b"PROP\x00\0\0\0"), Kind::Unknown);
    assert_eq!(kind(b"PROP\x04\0\0\0"), Kind::Unknown);
    assert_eq!(
        kind(b"PROP\x03\0"),
        Kind::Unknown,
        "the version is four bytes"
    );

    for version in 2..=5u8 {
        assert_eq!(kind(&[b'R', b'S', b'T', version, 0, 0]), Kind::StringTable);
    }
    assert_eq!(kind(b"RST\x01\0\0"), Kind::Unknown);
    assert_eq!(kind(b"RST\x06\0\0"), Kind::Unknown);

    assert_eq!(kind(b"BZh1\x31\x41\x59\x26"), Kind::Bzip2);
    assert_eq!(kind(b"BZh9\x31\x41\x59\x26"), Kind::Bzip2);
    assert_eq!(kind(b"BZh0 is not a block size\n"), Kind::Text);

    assert_eq!(kind(b"\x1f\x8b\x08\0\0\0\0\0"), Kind::Gzip);
    assert_eq!(
        kind(b"\x1f\x8b\x07\0\0\0\0\0"),
        Kind::Unknown,
        "deflate is method 8"
    );

    for version in 2..=4u8 {
        assert_eq!(kind(&[b'I', b'D', b'3', version, 0, 0]), Kind::Mp3);
    }
    assert_eq!(kind(b"ID3\x05\0\0"), Kind::Unknown);
    assert_eq!(kind(b"ID3 tags are described here\n"), Kind::Text);
}

#[test]
fn an_executable_header_points_at_its_pe_header() {
    let header = |e_lfanew: u32| {
        let mut head = vec![0u8; HEAD_LEN];
        head[..2].copy_from_slice(b"MZ");
        head[0x3C..0x40].copy_from_slice(&e_lfanew.to_le_bytes());
        head
    };
    assert_eq!(how(&header(0x40), None), (Kind::Pe, Confidence::Magic));
    assert_eq!(kind(&header(0xFFF)), Kind::Pe);
    assert_eq!(kind(&header(0x3F)), Kind::Unknown, "inside the DOS header");
    assert_eq!(kind(&header(0x1000)), Kind::Unknown);
    assert_eq!(
        kind(&header(0x80)[..0x3F]),
        Kind::Unknown,
        "cut before the pointer ends"
    );
}

#[test]
fn a_container_is_named_by_the_form_it_holds() {
    assert_eq!(kind(b"\xff\xd8\xff\xe0\x00\x10JFIF"), Kind::Jpeg);
    assert_eq!(kind(b"RIFF\x24\0\0\0WEBPVP8 "), Kind::WebP);
    assert_eq!(kind(b"RIFF\x24\0\0\0AVI LIST"), Kind::Avi);
    assert_eq!(kind(b"RIFF\x24\0\0\0XWMAfmt "), Kind::Unknown);
    assert_eq!(
        how(b"RIFF\x24\0\0\0XWMAfmt ", Some("x/516735348.wem")),
        (Kind::Wem, Confidence::Name),
        "a RIFF file that is no wave is what its name says"
    );
    assert_eq!(
        kind(b"RIFF\x24\0\0\0WAV"),
        Kind::Unknown,
        "cut inside the form"
    );
    assert_eq!(
        kind(b"FOR8\0\0\0\0\0\0\0\x20CIMG"),
        Kind::Unknown,
        "the form is at byte 8"
    );
    assert_eq!(kind(b"FOR8\0\0\0\x20CIMG"), Kind::MayaImage);
    assert_eq!(kind(b"FOR8\0\0\0\0\0\0\0\x20Maya"), Kind::MayaBinary);
    assert_eq!(kind(b"FOR4\0\0\0\x20XXXXHEAD"), Kind::Unknown);
}

#[test]
fn a_wad_is_rw_and_a_version_that_exists() {
    assert_eq!(how(b"RW\x03\x04", None), (Kind::Wad, Confidence::Magic));
    assert_eq!(kind(b"RW\x01\x00\0\0\0\0"), Kind::Wad);
    assert_eq!(kind(b"RW\x02\x00\0\0\0\0"), Kind::Wad);
    assert_eq!(kind(b"RW\x03\x08\0\0\0\0"), Kind::Wad);
    assert_eq!(kind(b"RW\x00\x00\0\0\0\0"), Kind::Unknown);
    assert_eq!(kind(b"RW\x04\x00\0\0\0\0"), Kind::Unknown);
    assert_eq!(kind(b"RW\x03\x09\0\0\0\0"), Kind::Unknown);
    assert_eq!(
        kind(b"RW\x03"),
        Kind::Unknown,
        "the minor version is missing"
    );
    assert_eq!(kind(b"RW is not a header\n"), Kind::Text);
}

#[test]
fn text_is_printable_utf8() {
    assert_eq!(how(b"plain words\n", None), (Kind::Text, Confidence::Magic));
    assert_eq!(kind(b"tabs\tand\r\nline ends"), Kind::Text);
    assert_eq!(kind("plain caf\u{e9}".as_bytes()), Kind::Text);
    assert_eq!(
        kind(b"words \xe4\xbd"),
        Kind::Text,
        "a head cut inside a character"
    );
    assert_eq!(kind(b"words \xff more"), Kind::Unknown, "not UTF-8");
    assert_eq!(
        kind(b"words \x01 more"),
        Kind::Unknown,
        "a control character"
    );
    assert_eq!(kind(b" \n\t\r\n"), Kind::Unknown, "whitespace alone");
    assert_eq!(
        kind(b"\xef\xbb\xbf"),
        Kind::Unknown,
        "a byte-order mark alone"
    );
    assert_eq!(
        kind(b"\n\n  #PROP_text\n"),
        Kind::RitobinText,
        "after leading whitespace"
    );
}

#[test]
fn text_that_opens_with_a_brace_or_a_bracket() {
    let scene = b"{\"asset\": {\"version\": \"2.0\"}}";
    assert_eq!(named(scene, "scene.gltf"), Kind::Gltf);
    assert_eq!(kind(scene), Kind::Json);
    assert_eq!(
        kind(b"{ \"type\": \"mesh\" }"),
        Kind::Json,
        "a type that is not a bin's"
    );
    assert_eq!(
        named(b"{\n  \"entries\": {", "scene.gltf"),
        Kind::RitobinJson,
        "what the text says wins over the name"
    );
    assert_eq!(kind(b"[1, 2, 3]"), Kind::Json);
    assert_eq!(
        kind(b"[Section\nkey=1"),
        Kind::Json,
        "no closing bracket on the line"
    );
    assert_eq!(kind(b"[_private]\nkey=1"), Kind::Ini);
    assert_eq!(kind(b"[.hidden]\nkey=1"), Kind::Ini);
    assert_eq!(named(b"[Localized]\nkey=1", "a.ini"), Kind::Ini);
    assert_eq!(
        named(b"[.ShellClassInfo]\nIconResource=x", "a.troy"),
        Kind::DesktopIni,
        "the section wins over the name"
    );
}

#[test]
fn markup_and_scripts_are_known_by_their_opening() {
    assert_eq!(
        kind(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"),
        Kind::Svg
    );
    assert_eq!(kind(b"<?xml version=\"1.0\"?>\n<root/>"), Kind::Xml);
    assert_eq!(kind(b"<!DOCTYPE html>\n<html>"), Kind::Html);
    assert_eq!(kind(b"<!doctype html>"), Kind::Html);
    assert_eq!(kind(b"<HTML><body>"), Kind::Html);
    assert_eq!(kind(b"<html lang=\"en\">"), Kind::Html);
    assert_eq!(kind(b"<p>words</p>"), Kind::Text);
    assert_eq!(kind(b"#!/bin/sh\necho words\n"), Kind::Script);
    assert_eq!(named(b"#!/usr/bin/env python\n", "notes.txt"), Kind::Script);
}

#[test]
fn text_with_nothing_in_its_opening_is_what_its_extension_says() {
    let cases = [
        ("particles/a.troy", Kind::Troy),
        ("readme.md", Kind::Markdown),
        ("README.MD", Kind::Markdown),
        ("scene.ma", Kind::MayaAscii),
        ("mesh.obj", Kind::Obj),
        ("scene.gltf", Kind::Gltf),
        ("config.ini", Kind::Ini),
        ("data.json", Kind::Json),
        ("data.xml", Kind::Xml),
        ("tool.py", Kind::Script),
        ("install.bat", Kind::Script),
        ("install.cmd", Kind::Script),
        ("install.ps1", Kind::Script),
        ("install.vbs", Kind::Script),
        ("install.sh", Kind::Script),
        ("init.lua", Kind::Script),
        ("notes.txt", Kind::Text),
        ("notes", Kind::Text),
        ("skin0.bin", Kind::Text),
        ("a.tga", Kind::Text),
    ];
    for (name, expected) in cases {
        assert_eq!(
            how(b"some words\n", Some(name)),
            (expected, Confidence::Magic),
            "{name}"
        );
    }
}

#[test]
fn an_extension_names_a_format_with_no_magic() {
    let cases = [
        ("a.tga", Kind::Tga),
        ("A.TGA", Kind::Tga),
        ("a.troybin", Kind::Inibin),
        ("a.inibin", Kind::Inibin),
        ("lux.wad.subchunktoc", Kind::SubchunkToc),
        ("a.rec", Kind::ItemRecommendations),
        ("516735348.wem", Kind::Wem),
        ("a.max", Kind::Max),
        ("a.ztl", Kind::ZBrush),
        ("a.zpr", Kind::ZBrush),
        ("a.spp", Kind::Substance),
        ("a.sbs", Kind::Substance),
        ("a.sbsar", Kind::Substance),
        ("a.mb", Kind::MayaBinary),
        ("a.fbx", Kind::FbxBinary),
        ("a.blend", Kind::Blend),
        ("a.obj", Kind::Obj),
        ("a.lightgrid", Kind::LightGrid),
        ("a.ico", Kind::Ico),
    ];
    for (name, expected) in cases {
        assert_eq!(
            how(OPAQUE, Some(name)),
            (expected, Confidence::Name),
            "{name}"
        );
    }
    for name in ["a.dds", "a.png", "a.md", "a.py", "a", "a.", "a.tga/b"] {
        assert_eq!(
            how(OPAQUE, Some(name)),
            (Kind::Unknown, Confidence::Magic),
            "{name}"
        );
    }
}

#[test]
fn an_ole_file_is_what_its_extension_says_or_an_ole_file() {
    assert_eq!(how(OLE, Some("scene.max")), (Kind::Max, Confidence::Name));
    assert_eq!(how(OLE, Some("letter.doc")), (Kind::Ole, Confidence::Magic));
    assert_eq!(how(OLE, Some("readme.md")), (Kind::Ole, Confidence::Magic));
    assert_eq!(how(OLE, None), (Kind::Ole, Confidence::Magic));
    assert_eq!(
        how(PNG, Some("scene.max")),
        (Kind::Png, Confidence::Magic),
        "no other magic is"
    );
}

#[test]
fn a_file_browsers_leftover_is_known_by_its_name_whatever_it_holds() {
    let leftover = |head: &[u8], name: &str, expected: Kind| {
        assert_eq!(
            how(head, Some(name)),
            (expected, Confidence::Name),
            "{name}"
        );
    };
    leftover(PNG, "__MACOSX/skin/a.png", Kind::AppleDouble);
    leftover(PNG, "mod\\__macosx\\a.png", Kind::AppleDouble);
    leftover(b"some words\n", "skin/._a.dds", Kind::AppleDouble);
    leftover(OPAQUE, "._a", Kind::AppleDouble);
    leftover(b"", ".DS_Store", Kind::DsStore);
    leftover(b"", "skin\\Thumbs.db", Kind::ThumbsDb);
    leftover(OLE, "ehthumbs.db", Kind::ThumbsDb);
    leftover(b"some words\n", "skin/Desktop.ini", Kind::DesktopIni);
    leftover(PNG, "desktop.ini", Kind::DesktopIni);
    leftover(OPAQUE, "IconCache.db", Kind::IconCache);
    leftover(OPAQUE, "iconcache_48.db", Kind::IconCache);

    assert_eq!(named(b"some words\n", "iconcache.txt"), Kind::Text);
    assert_eq!(named(OPAQUE, "my_iconcache.db"), Kind::Unknown);
    assert_eq!(named(PNG, "thumbs.db.png"), Kind::Png);
    assert_eq!(named(PNG, "a._b"), Kind::Png);
}

#[test]
fn a_file_of_no_bytes_is_empty_whatever_its_extension() {
    assert_eq!(how(b"", None), (Kind::Empty, Confidence::Magic));
    assert_eq!(how(b"", Some("a.tga")), (Kind::Empty, Confidence::Magic));
    assert_eq!(
        how(b"", Some("readme.md")),
        (Kind::Empty, Confidence::Magic)
    );
}

#[test]
fn a_bitmap_has_no_reserved_bytes_set() {
    let bmp = b"BM\x46\0\0\0\0\0\0\0\x36\0\0\0";
    assert_eq!(how(bmp, None), (Kind::Bmp, Confidence::Heuristic));
    assert_eq!(
        kind(&bmp[..13]),
        Kind::Unknown,
        "cut inside the file header"
    );
    assert_eq!(kind(b"BM\x46\0\0\0\x01\0\0\0\x36\0\0\0"), Kind::Unknown);
    assert_eq!(kind(b"BM is not a header\n"), Kind::Text);
}

#[test]
fn an_icon_has_type_one_and_at_least_one_image() {
    assert_eq!(
        how(b"\0\0\x01\0\x01\0\x10\x10", None),
        (Kind::Ico, Confidence::Heuristic)
    );
    assert_eq!(kind(b"\0\0\x01\0\x03\0"), Kind::Ico);
    assert_eq!(kind(b"\0\0\x04\0\x01\0\x10\x10"), Kind::Unknown, "type 4");
}

#[test]
fn a_tga_has_a_colour_map_type_and_an_image_type_that_exist() {
    for image_type in [1, 2, 3, 9, 10, 11] {
        for colour_map in [0, 1] {
            let head = [0, colour_map, image_type];
            assert_eq!(
                how(&head, None),
                (Kind::Tga, Confidence::Heuristic),
                "{head:?}"
            );
        }
    }
    assert_eq!(kind(&[0, 2, 2]), Kind::Unknown, "colour map type 2");
    for image_type in [0, 4, 8, 12] {
        assert_eq!(
            kind(&[0, 0, image_type, 0x7f]),
            Kind::Unknown,
            "image type {image_type}"
        );
    }
    assert_eq!(kind(&[0, 0]), Kind::Unknown, "cut before the image type");
}

#[test]
fn an_inibin_has_flags_and_a_first_section_that_is_not_empty() {
    assert_eq!(
        how(b"\x02i\x00\xb0\x1b\x03\x00", None),
        (Kind::Inibin, Confidence::Heuristic)
    );
    assert_eq!(
        kind(b"\x02i\x00\xb0\x1b\x00\x00"),
        Kind::Unknown,
        "no value in the section"
    );
    assert_eq!(
        kind(b"\x02i\x00\x00\x00\x03\x00"),
        Kind::Unknown,
        "no flags"
    );
    assert_eq!(
        kind(b"\x02i\x00\xb0\x1b\x03"),
        Kind::Unknown,
        "cut inside the count"
    );
    assert_eq!(
        kind(b"\x01i\x00\xb0\x1b\x03\x00"),
        Kind::Unknown,
        "version 1"
    );
}

#[test]
fn a_light_grid_starts_with_version_3() {
    assert_eq!(
        how(b"\x03\0\0\0\x10\0\0\0", None),
        (Kind::LightGrid, Confidence::Heuristic)
    );
    assert_eq!(kind(b"\x03\0\0"), Kind::Unknown, "cut inside the version");
    assert_eq!(kind(b"\x04\0\0\0\x10\0\0\0"), Kind::Unknown);
}

#[test]
fn a_head_two_heuristics_accept_is_the_first_ones() {
    // Version 2 with a string table of 0x0200 bytes is an inibin's opening and a TGA's.
    assert_eq!(kind(b"\x02\x00\x02\x01\x00\x01\x00"), Kind::Tga);
}

#[test]
fn nothing_known_is_unknown() {
    assert_eq!(how(OPAQUE, None), (Kind::Unknown, Confidence::Magic));
    assert_eq!(
        how(OPAQUE, Some("a.xyz")),
        (Kind::Unknown, Confidence::Magic)
    );
}

#[test]
fn what_the_head_says_keeps_what_the_name_decides_for_later() {
    let tga = b"\0\0\x02\0\0\0\0\0\0\0\0\0\x00\x01";
    let cases: [(&[u8], HeadKind); 7] = [
        (b"", HeadKind::Empty),
        (b"PROP\x03\0\0\0", HeadKind::Magic(Kind::PropertyBin)),
        (b"{\"asset\": 1}", HeadKind::Object),
        (b"[System]\nx=1\n", HeadKind::Section),
        (b"some words", HeadKind::Text),
        (tga, HeadKind::Heuristic(Kind::Tga)),
        (OPAQUE, HeadKind::Unknown),
    ];
    let names = [
        None,
        Some("a.gltf"),
        Some("a.troy"),
        Some("a.md"),
        Some("a.wem"),
        Some("a.max"),
        Some("x/Thumbs.db"),
    ];
    for (head, says) in cases {
        assert_eq!(HeadKind::of(head), says, "{head:?}");
        for name in names {
            assert_eq!(
                says.identify(name),
                identify(head, name),
                "{head:?} {name:?}"
            );
        }
        let kept = serde_json::to_string(&says).expect("it writes");
        assert_eq!(serde_json::from_str::<HeadKind>(&kept).ok(), Some(says));
    }
    assert_eq!(HeadKind::Object.identify(Some("a.gltf")).kind, Kind::Gltf);
    assert_eq!(HeadKind::Section.identify(Some("a.troy")).kind, Kind::Troy);
    assert_eq!(HeadKind::Text.identify(Some("a.md")).kind, Kind::Markdown);
    assert_eq!(
        how(tga, Some("a.wem")),
        (Kind::Wem, Confidence::Name),
        "a name before a heuristic"
    );
    assert_eq!(
        HeadKind::Magic(Kind::Ole).identify(Some("a.max")).kind,
        Kind::Max
    );
}

#[test]
fn the_head_is_read_to_its_length_and_no_further() {
    let mut text = vec![b'a'; HEAD_LEN];
    text.extend_from_slice(b"\0\x01\x02 binary after the head");
    assert_eq!(HeadKind::of(&text), HeadKind::Text);
    assert_eq!(kind(&text), Kind::Text);
}

#[test]
fn text_whose_fifth_byte_is_inside_a_character_is_identified_and_does_not_panic() {
    // Two three-byte characters: byte 5 is the last byte of the second one.
    let text = "读我 readme".as_bytes();
    assert_eq!(identify(text, Some("readme.txt")).kind, Kind::Text);
    assert_eq!(identify("한국어 설명".as_bytes(), None).kind, Kind::Text);
}
