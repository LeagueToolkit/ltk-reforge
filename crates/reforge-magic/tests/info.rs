//! What a kind is: its MIME type, usual extension and categories, for every kind, and how kinds,
//! categories and identities serialise.

use reforge_magic::{Categories, Confidence, HEAD_LEN, Identity, Kind, identify};

/// Every kind in the order of `Kind`, with its MIME type, usual extension and category names.
const KINDS: &[(Kind, &str, Option<&str>, &[&str])] = &[
    (
        Kind::PropertyBin,
        "application/x-league-bin",
        Some("bin"),
        &["game"],
    ),
    (
        Kind::PropertyBinOverride,
        "application/x-league-bin-override",
        Some("bin"),
        &["game"],
    ),
    (
        Kind::Animation,
        "application/x-league-anm",
        Some("anm"),
        &["game", "model"],
    ),
    (
        Kind::Skeleton,
        "application/x-league-skl",
        Some("skl"),
        &["game", "model"],
    ),
    (
        Kind::SimpleSkin,
        "application/x-league-skn",
        Some("skn"),
        &["game", "model"],
    ),
    (
        Kind::SkinWeights,
        "application/x-league-wgt",
        Some("wgt"),
        &["game", "model"],
    ),
    (
        Kind::StaticMeshBinary,
        "application/x-league-scb",
        Some("scb"),
        &["game", "model"],
    ),
    (
        Kind::StaticMeshAscii,
        "text/x-league-sco",
        Some("sco"),
        &["game", "text", "model"],
    ),
    (
        Kind::Texture,
        "image/x-league-tex",
        Some("tex"),
        &["game", "image"],
    ),
    (
        Kind::TextureDds,
        "image/vnd-ms.dds",
        Some("dds"),
        &["game", "image"],
    ),
    (
        Kind::WwiseBank,
        "audio/x-wwise-bnk",
        Some("bnk"),
        &["game", "audio"],
    ),
    (
        Kind::WwisePackage,
        "audio/x-wwise-wpk",
        Some("wpk"),
        &["game", "audio"],
    ),
    (
        Kind::MapGeometry,
        "application/x-league-mapgeo",
        Some("mapgeo"),
        &["game", "model"],
    ),
    (
        Kind::WorldGeometry,
        "application/x-league-wgeo",
        Some("wgeo"),
        &["game", "model"],
    ),
    (
        Kind::Preload,
        "text/x-league-preload",
        Some("preload"),
        &["game", "text"],
    ),
    (
        Kind::StringTable,
        "application/x-league-stringtable",
        Some("stringtable"),
        &["game"],
    ),
    (
        Kind::LuaObj,
        "application/x-lua-bytecode",
        Some("luaobj"),
        &["game"],
    ),
    (
        Kind::LightGrid,
        "application/x-league-lightgrid",
        Some("lightgrid"),
        &["game"],
    ),
    (
        Kind::Inibin,
        "application/x-league-inibin",
        Some("troybin"),
        &["game"],
    ),
    (
        Kind::Troy,
        "text/x-league-troy",
        Some("troy"),
        &["game", "text"],
    ),
    (
        Kind::Wad,
        "application/x-league-wad",
        Some("wad.client"),
        &["game", "archive"],
    ),
    (
        Kind::SubchunkToc,
        "application/x-league-subchunktoc",
        Some("subchunktoc"),
        &["game"],
    ),
    (
        Kind::ItemRecommendations,
        "application/x-league-rec",
        Some("rec"),
        &["game"],
    ),
    (Kind::R3d2, "application/x-league-r3d2", None, &["game"]),
    (
        Kind::RitobinText,
        "text/x-ritobin",
        Some("py"),
        &["text", "source"],
    ),
    (
        Kind::RitobinJson,
        "application/x-ritobin+json",
        Some("json"),
        &["text", "source"],
    ),
    (Kind::Png, "image/png", Some("png"), &["image"]),
    (Kind::Jpeg, "image/jpeg", Some("jpg"), &["image"]),
    (Kind::Gif, "image/gif", Some("gif"), &["image"]),
    (Kind::Bmp, "image/bmp", Some("bmp"), &["image"]),
    (Kind::Tga, "image/x-tga", Some("tga"), &["image"]),
    (Kind::Tiff, "image/tiff", Some("tif"), &["image"]),
    (Kind::Ico, "image/x-icon", Some("ico"), &["image"]),
    (Kind::WebP, "image/webp", Some("webp"), &["image"]),
    (Kind::Svg, "image/svg+xml", Some("svg"), &["text", "image"]),
    (
        Kind::Psd,
        "image/vnd.adobe.photoshop",
        Some("psd"),
        &["image", "source"],
    ),
    (Kind::Exr, "image/x-exr", Some("exr"), &["image", "source"]),
    (Kind::Xcf, "image/x-xcf", Some("xcf"), &["image", "source"]),
    (
        Kind::Pdn,
        "image/x-paint.net",
        Some("pdn"),
        &["image", "source"],
    ),
    (
        Kind::MayaImage,
        "image/x-maya-iff",
        Some("iff"),
        &["image", "source"],
    ),
    (
        Kind::MayaSwatches,
        "application/x-maya-swatches",
        Some("mayaSwatches"),
        &["image", "source"],
    ),
    (
        Kind::MayaAscii,
        "application/x-maya-ascii",
        Some("ma"),
        &["text", "model", "source"],
    ),
    (
        Kind::MayaBinary,
        "application/x-maya-binary",
        Some("mb"),
        &["model", "source"],
    ),
    (
        Kind::FbxBinary,
        "application/vnd.autodesk.fbx",
        Some("fbx"),
        &["model", "source"],
    ),
    (
        Kind::FbxAscii,
        "application/vnd.autodesk.fbx",
        Some("fbx"),
        &["text", "model", "source"],
    ),
    (
        Kind::Blend,
        "application/x-blender",
        Some("blend"),
        &["model", "source"],
    ),
    (
        Kind::GltfBinary,
        "model/gltf-binary",
        Some("glb"),
        &["model", "source"],
    ),
    (
        Kind::Gltf,
        "model/gltf+json",
        Some("gltf"),
        &["text", "model", "source"],
    ),
    (
        Kind::Obj,
        "model/obj",
        Some("obj"),
        &["text", "model", "source"],
    ),
    (
        Kind::Max,
        "application/x-3ds-max",
        Some("max"),
        &["model", "source"],
    ),
    (
        Kind::ZBrush,
        "application/x-zbrush",
        Some("ztl"),
        &["model", "source"],
    ),
    (
        Kind::Substance,
        "application/x-substance",
        Some("spp"),
        &["image", "source"],
    ),
    (Kind::Wav, "audio/wav", Some("wav"), &["audio"]),
    (Kind::Wem, "audio/x-wwise-wem", Some("wem"), &["audio"]),
    (Kind::Ogg, "audio/ogg", Some("ogg"), &["audio"]),
    (Kind::Mp3, "audio/mpeg", Some("mp3"), &["audio"]),
    (Kind::Flac, "audio/flac", Some("flac"), &["audio"]),
    (Kind::Mp4, "video/mp4", Some("mp4"), &["video"]),
    (Kind::Webm, "video/webm", Some("webm"), &["video"]),
    (Kind::Avi, "video/x-msvideo", Some("avi"), &["video"]),
    (Kind::Ttf, "font/ttf", Some("ttf"), &["font"]),
    (Kind::Otf, "font/otf", Some("otf"), &["font"]),
    (Kind::Ttc, "font/collection", Some("ttc"), &["font"]),
    (Kind::Woff, "font/woff", Some("woff"), &["font"]),
    (Kind::Woff2, "font/woff2", Some("woff2"), &["font"]),
    (Kind::Json, "application/json", Some("json"), &["text"]),
    (Kind::Xml, "application/xml", Some("xml"), &["text"]),
    (Kind::Html, "text/html", Some("html"), &["text", "document"]),
    (Kind::Ini, "text/x-ini", Some("ini"), &["text"]),
    (
        Kind::Markdown,
        "text/markdown",
        Some("md"),
        &["text", "document"],
    ),
    (Kind::Text, "text/plain", Some("txt"), &["text"]),
    (Kind::Pdf, "application/pdf", Some("pdf"), &["document"]),
    (
        Kind::Pe,
        "application/vnd.microsoft.portable-executable",
        Some("exe"),
        &["executable"],
    ),
    (Kind::Elf, "application/x-executable", None, &["executable"]),
    (
        Kind::MachO,
        "application/x-mach-binary",
        None,
        &["executable"],
    ),
    (Kind::Script, "text/x-script", None, &["text", "executable"]),
    (Kind::Zip, "application/zip", Some("zip"), &["archive"]),
    (Kind::Rar, "application/vnd.rar", Some("rar"), &["archive"]),
    (
        Kind::SevenZip,
        "application/x-7z-compressed",
        Some("7z"),
        &["archive"],
    ),
    (Kind::Gzip, "application/gzip", Some("gz"), &["archive"]),
    (Kind::Zstd, "application/zstd", Some("zst"), &["archive"]),
    (Kind::Xz, "application/x-xz", Some("xz"), &["archive"]),
    (
        Kind::Bzip2,
        "application/x-bzip2",
        Some("bz2"),
        &["archive"],
    ),
    (
        Kind::Cab,
        "application/vnd.ms-cab-compressed",
        Some("cab"),
        &["archive"],
    ),
    (
        Kind::DsStore,
        "application/x-apple-ds-store",
        Some("DS_Store"),
        &["os-artifact"],
    ),
    (
        Kind::AppleDouble,
        "multipart/appledouble",
        None,
        &["os-artifact"],
    ),
    (
        Kind::ThumbsDb,
        "application/x-ms-thumbs-db",
        Some("db"),
        &["os-artifact"],
    ),
    (
        Kind::DesktopIni,
        "text/x-ini",
        Some("ini"),
        &["text", "os-artifact"],
    ),
    (
        Kind::IconCache,
        "application/x-ms-iconcache",
        Some("db"),
        &["os-artifact"],
    ),
    (
        Kind::Shortcut,
        "application/x-ms-shortcut",
        Some("lnk"),
        &["os-artifact"],
    ),
    (Kind::Ole, "application/x-ole-storage", None, &[]),
    (Kind::Empty, "application/x-empty", None, &[]),
    (Kind::Unknown, "application/octet-stream", None, &[]),
];

#[test]
fn a_kind_has_a_mime_and_categories() {
    let exe = Kind::Pe.info();
    assert!(
        exe.categories.contains(Categories::EXECUTABLE)
            && !exe.categories.contains(Categories::GAME)
    );
    assert_eq!(Kind::Psd.info().mime, "image/vnd.adobe.photoshop");
    assert_eq!(
        Kind::RitobinText.info().categories.names(),
        ["text", "source"]
    );
    assert_eq!(Kind::Wad.info().categories.names(), ["game", "archive"]);
    assert_eq!(
        serde_json::to_string(&Kind::DsStore.info().categories).unwrap(),
        r#"["os-artifact"]"#
    );
    assert_eq!(Kind::Exr.info().extension, Some("exr"));
    assert_eq!(Kind::Unknown.info().extension, None);
}

#[test]
fn every_kind_has_the_mime_extension_and_categories_listed() {
    // `Unknown` is the last variant, so a list in the order of `Kind` that ends on it is whole.
    assert_eq!(KINDS.len(), Kind::Unknown as usize + 1);
    for (at, &(kind, mime, extension, categories)) in KINDS.iter().enumerate() {
        assert_eq!(kind as usize, at, "{kind:?} is out of the order of Kind");
        let info = kind.info();
        assert_eq!(
            (info.mime, info.extension, info.categories.names()),
            (mime, extension, categories.to_vec()),
            "{kind:?}"
        );
    }
}

#[test]
fn an_identity_has_its_kinds_info() {
    let identity = identify(b"RIFF\0\0\0\0WAVEfmt ", None);
    assert_eq!(identity.info(), Kind::Wav.info());
    let built = Identity {
        kind: Kind::Wad,
        confidence: Confidence::Magic,
    };
    assert_eq!(built.info(), Kind::Wad.info());
}

#[test]
fn categories_are_a_set() {
    let both = Categories::GAME | Categories::MODEL;
    assert_eq!(both, Categories::GAME.union(Categories::MODEL));
    assert_eq!(both | Categories::GAME, both);
    assert!(both.contains(Categories::GAME) && both.contains(Categories::MODEL));
    assert!(both.contains(both));
    assert!(!both.contains(Categories::TEXT));
    assert!(
        !Categories::GAME.contains(both),
        "every category of the other, not any"
    );
    assert_ne!(both, Categories::GAME);
    assert_eq!(Categories::default(), Categories::NONE);
    assert_eq!(Categories::NONE | Categories::NONE, Categories::NONE);
}

#[test]
fn the_empty_set_is_contained_in_no_set() {
    assert!(!Categories::GAME.contains(Categories::NONE));
    assert!(!Categories::NONE.contains(Categories::NONE));
    assert!(!Categories::NONE.contains(Categories::GAME));
}

#[test]
fn categories_are_named_in_the_order_of_their_flags() {
    let every = [
        (Categories::GAME, "game"),
        (Categories::TEXT, "text"),
        (Categories::IMAGE, "image"),
        (Categories::AUDIO, "audio"),
        (Categories::VIDEO, "video"),
        (Categories::FONT, "font"),
        (Categories::MODEL, "model"),
        (Categories::SOURCE, "source"),
        (Categories::EXECUTABLE, "executable"),
        (Categories::ARCHIVE, "archive"),
        (Categories::OS_ARTIFACT, "os-artifact"),
        (Categories::DOCUMENT, "document"),
    ];
    let mut all = Categories::NONE;
    for (category, name) in every {
        assert_eq!(category.names(), [name]);
        // Joined last first: the names come out in flag order, not in the order of the unions.
        all = category | all;
    }
    assert_eq!(all.names(), every.map(|(_, name)| name));
    assert_eq!(Categories::NONE.names(), [""; 0]);
    assert_eq!(
        (Categories::DOCUMENT | Categories::TEXT).names(),
        ["text", "document"]
    );
}

#[test]
fn categories_serialise_as_the_list_of_their_names() {
    let json = |categories: Categories| serde_json::to_string(&categories).unwrap();
    assert_eq!(json(Categories::NONE), "[]");
    assert_eq!(
        json(Categories::SOURCE | Categories::MODEL),
        r#"["model","source"]"#
    );
    assert_eq!(
        json(Kind::StaticMeshAscii.info().categories),
        r#"["game","text","model"]"#
    );
}

#[test]
fn a_kind_serialises_in_kebab_case() {
    let json = |kind: Kind| serde_json::to_string(&kind).unwrap();
    assert_eq!(json(Kind::PropertyBin), r#""property-bin""#);
    assert_eq!(
        json(Kind::PropertyBinOverride),
        r#""property-bin-override""#
    );
    assert_eq!(json(Kind::R3d2), r#""r3d2""#);
    assert_eq!(json(Kind::WebP), r#""web-p""#);
    assert_eq!(json(Kind::MachO), r#""mach-o""#);
    assert_eq!(json(Kind::SevenZip), r#""seven-zip""#);
    assert_eq!(json(Kind::Bzip2), r#""bzip2""#);
    assert_eq!(json(Kind::DsStore), r#""ds-store""#);
    assert_eq!(json(Kind::ZBrush), r#""z-brush""#);
}

#[test]
fn an_identity_serialises_as_its_kind_and_confidence() {
    let json =
        |head: &[u8], name: Option<&str>| serde_json::to_string(&identify(head, name)).unwrap();
    assert_eq!(
        json(b"RIFF\0\0\0\0WAVEfmt ", None),
        r#"{"kind":"wav","confidence":"magic"}"#
    );
    assert_eq!(
        json(b"\0\0\x02\0\0\0\0\0", None),
        r#"{"kind":"tga","confidence":"heuristic"}"#
    );
    assert_eq!(
        json(b"\x99\x98\x97\x96\x95", Some("a.troybin")),
        r#"{"kind":"inibin","confidence":"name"}"#
    );
}

#[test]
fn kinds_sort_in_the_order_they_are_declared() {
    assert!(Kind::PropertyBin < Kind::PropertyBinOverride);
    assert!(Kind::Empty < Kind::Unknown);
    assert!(Confidence::Magic < Confidence::Heuristic && Confidence::Heuristic < Confidence::Name);
}

#[test]
fn the_head_reaches_the_farthest_byte_a_check_reads() {
    // A DOS header's pointer to its PE header ends at byte 0x40.
    assert_eq!(HEAD_LEN, 0x40);
}
