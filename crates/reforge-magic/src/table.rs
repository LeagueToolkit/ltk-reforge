//! One row per kind: what [`Kind::info`] reports, and what names the kind without a check of its
//! own, its fixed magics and its extensions. The checks that are code are in `sniff`.

use crate::Categories as C;
use crate::Kind;

/// What one kind is, and the fixed magics and extensions that name it.
#[derive(Clone, Copy)]
pub(crate) struct Row {
    kind: Kind,
    pub(crate) mime: &'static str,
    /// The extension a file of this kind usually has. It is reported, never matched: a ritobin
    /// text is usually `.py`, and text named `.py` is a script.
    pub(crate) extension: Option<&'static str>,
    pub(crate) categories: C,
    /// The byte strings a file of this kind starts with.
    magics: &'static [&'static [u8]],
    /// The extensions that name this kind where the bytes name nothing: the format has no magic,
    /// or its magic is a container's.
    named: &'static [&'static str],
    /// The extensions that name this kind where the file is text whose opening names nothing.
    text_named: &'static [&'static str],
}

const fn row(kind: Kind, mime: &'static str) -> Row {
    Row {
        kind,
        mime,
        extension: None,
        categories: C::NONE,
        magics: &[],
        named: &[],
        text_named: &[],
    }
}

impl Row {
    const fn extension(self, extension: &'static str) -> Row {
        Row {
            extension: Some(extension),
            ..self
        }
    }

    const fn categories(self, categories: &[C]) -> Row {
        let mut all = C::NONE;
        let mut at = 0;
        while at < categories.len() {
            all = all.union(categories[at]);
            at += 1;
        }
        Row {
            categories: all,
            ..self
        }
    }

    const fn magics(self, magics: &'static [&'static [u8]]) -> Row {
        Row { magics, ..self }
    }

    const fn named(self, named: &'static [&'static str]) -> Row {
        Row { named, ..self }
    }

    const fn text_named(self, text_named: &'static [&'static str]) -> Row {
        Row { text_named, ..self }
    }
}

static KINDS: [Row; Kind::COUNT] = [
    // League
    row(Kind::PropertyBin, "application/x-league-bin")
        .extension("bin")
        .categories(&[C::GAME]),
    row(
        Kind::PropertyBinOverride,
        "application/x-league-bin-override",
    )
    .extension("bin")
    .categories(&[C::GAME])
    .magics(&[b"PTCH"]),
    row(Kind::Animation, "application/x-league-anm")
        .extension("anm")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"r3d2anmd", b"r3d2canm"]),
    row(Kind::Skeleton, "application/x-league-skl")
        .extension("skl")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"r3d2sklt"]),
    row(Kind::SimpleSkin, "application/x-league-skn")
        .extension("skn")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"\x33\x22\x11\x00"]),
    row(Kind::SkinWeights, "application/x-league-wgt")
        .extension("wgt")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"r3d2wght"]),
    row(Kind::StaticMeshBinary, "application/x-league-scb")
        .extension("scb")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"r3d2Mesh"]),
    row(Kind::StaticMeshAscii, "text/x-league-sco")
        .extension("sco")
        .categories(&[C::GAME, C::MODEL, C::TEXT])
        .magics(&[b"[Obj"]),
    row(Kind::Texture, "image/x-league-tex")
        .extension("tex")
        .categories(&[C::GAME, C::IMAGE])
        .magics(&[b"TEX\x00"]),
    row(Kind::TextureDds, "image/vnd-ms.dds")
        .extension("dds")
        .categories(&[C::GAME, C::IMAGE])
        .magics(&[b"DDS "]),
    row(Kind::WwiseBank, "audio/x-wwise-bnk")
        .extension("bnk")
        .categories(&[C::GAME, C::AUDIO])
        .magics(&[b"BKHD"]),
    row(Kind::WwisePackage, "audio/x-wwise-wpk")
        .extension("wpk")
        .categories(&[C::GAME, C::AUDIO]),
    row(Kind::MapGeometry, "application/x-league-mapgeo")
        .extension("mapgeo")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"OEGM"]),
    row(Kind::WorldGeometry, "application/x-league-wgeo")
        .extension("wgeo")
        .categories(&[C::GAME, C::MODEL])
        .magics(&[b"WGEO"]),
    row(Kind::Preload, "text/x-league-preload")
        .extension("preload")
        .categories(&[C::GAME, C::TEXT])
        .magics(&[b"PreLoad"]),
    row(Kind::StringTable, "application/x-league-stringtable")
        .extension("stringtable")
        .categories(&[C::GAME]),
    row(Kind::LuaObj, "application/x-lua-bytecode")
        .extension("luaobj")
        .categories(&[C::GAME])
        .magics(&[b"\x1bLuaQ"]),
    row(Kind::LightGrid, "application/x-league-lightgrid")
        .extension("lightgrid")
        .categories(&[C::GAME])
        .named(&["lightgrid"]),
    row(Kind::Inibin, "application/x-league-inibin")
        .extension("troybin")
        .categories(&[C::GAME])
        .named(&["troybin", "inibin"]),
    row(Kind::Troy, "text/x-league-troy")
        .extension("troy")
        .categories(&[C::GAME, C::TEXT])
        .text_named(&["troy"]),
    row(Kind::Wad, "application/x-league-wad")
        .extension("wad.client")
        .categories(&[C::GAME, C::ARCHIVE]),
    row(Kind::SubchunkToc, "application/x-league-subchunktoc")
        .extension("subchunktoc")
        .categories(&[C::GAME])
        .named(&["subchunktoc"]),
    row(Kind::ItemRecommendations, "application/x-league-rec")
        .extension("rec")
        .categories(&[C::GAME])
        .named(&["rec"]),
    row(Kind::R3d2, "application/x-league-r3d2").categories(&[C::GAME]),
    // Bin sources
    row(Kind::RitobinText, "text/x-ritobin")
        .extension("py")
        .categories(&[C::TEXT, C::SOURCE]),
    row(Kind::RitobinJson, "application/x-ritobin+json")
        .extension("json")
        .categories(&[C::TEXT, C::SOURCE]),
    // Images
    row(Kind::Png, "image/png")
        .extension("png")
        .categories(&[C::IMAGE])
        .magics(&[b"\x89PNG\r\n\x1a\n"]),
    row(Kind::Jpeg, "image/jpeg")
        .extension("jpg")
        .categories(&[C::IMAGE]),
    row(Kind::Gif, "image/gif")
        .extension("gif")
        .categories(&[C::IMAGE])
        .magics(&[b"GIF87a", b"GIF89a"]),
    row(Kind::Bmp, "image/bmp")
        .extension("bmp")
        .categories(&[C::IMAGE]),
    row(Kind::Tga, "image/x-tga")
        .extension("tga")
        .categories(&[C::IMAGE])
        .named(&["tga"]),
    row(Kind::Tiff, "image/tiff")
        .extension("tif")
        .categories(&[C::IMAGE])
        .magics(&[b"II*\x00", b"MM\x00*"]),
    row(Kind::Ico, "image/x-icon")
        .extension("ico")
        .categories(&[C::IMAGE])
        .named(&["ico"]),
    row(Kind::WebP, "image/webp")
        .extension("webp")
        .categories(&[C::IMAGE]),
    row(Kind::Svg, "image/svg+xml")
        .extension("svg")
        .categories(&[C::IMAGE, C::TEXT]),
    row(Kind::Psd, "image/vnd.adobe.photoshop")
        .extension("psd")
        .categories(&[C::IMAGE, C::SOURCE])
        .magics(&[b"8BPS"]),
    row(Kind::Exr, "image/x-exr")
        .extension("exr")
        .categories(&[C::IMAGE, C::SOURCE])
        .magics(&[b"v/1\x01"]),
    row(Kind::Xcf, "image/x-xcf")
        .extension("xcf")
        .categories(&[C::IMAGE, C::SOURCE])
        .magics(&[b"gimp xcf"]),
    row(Kind::Pdn, "image/x-paint.net")
        .extension("pdn")
        .categories(&[C::IMAGE, C::SOURCE])
        .magics(&[b"PDN3"]),
    row(Kind::MayaImage, "image/x-maya-iff")
        .extension("iff")
        .categories(&[C::IMAGE, C::SOURCE]),
    row(Kind::MayaSwatches, "application/x-maya-swatches")
        .extension("mayaSwatches")
        .categories(&[C::IMAGE, C::SOURCE])
        .magics(&[b"MayaIcons"]),
    // Authoring scenes and models
    row(Kind::MayaAscii, "application/x-maya-ascii")
        .extension("ma")
        .categories(&[C::MODEL, C::SOURCE, C::TEXT])
        .magics(&[b"//Maya ASCII"])
        .text_named(&["ma"]),
    row(Kind::MayaBinary, "application/x-maya-binary")
        .extension("mb")
        .categories(&[C::MODEL, C::SOURCE])
        .named(&["mb"]),
    row(Kind::FbxBinary, "application/vnd.autodesk.fbx")
        .extension("fbx")
        .categories(&[C::MODEL, C::SOURCE])
        .magics(&[b"Kaydara FBX Binary"])
        .named(&["fbx"]),
    row(Kind::FbxAscii, "application/vnd.autodesk.fbx")
        .extension("fbx")
        .categories(&[C::MODEL, C::SOURCE, C::TEXT]),
    row(Kind::Blend, "application/x-blender")
        .extension("blend")
        .categories(&[C::MODEL, C::SOURCE])
        .magics(&[b"BLENDER"])
        .named(&["blend"]),
    row(Kind::GltfBinary, "model/gltf-binary")
        .extension("glb")
        .categories(&[C::MODEL, C::SOURCE])
        .magics(&[b"glTF"]),
    row(Kind::Gltf, "model/gltf+json")
        .extension("gltf")
        .categories(&[C::MODEL, C::SOURCE, C::TEXT])
        .text_named(&["gltf"]),
    row(Kind::Obj, "model/obj")
        .extension("obj")
        .categories(&[C::MODEL, C::SOURCE, C::TEXT])
        .named(&["obj"])
        .text_named(&["obj"]),
    row(Kind::Max, "application/x-3ds-max")
        .extension("max")
        .categories(&[C::MODEL, C::SOURCE])
        .named(&["max"]),
    row(Kind::ZBrush, "application/x-zbrush")
        .extension("ztl")
        .categories(&[C::MODEL, C::SOURCE])
        .named(&["ztl", "zpr"]),
    row(Kind::Substance, "application/x-substance")
        .extension("spp")
        .categories(&[C::IMAGE, C::SOURCE])
        .named(&["spp", "sbs", "sbsar"]),
    // Audio and video
    row(Kind::Wav, "audio/wav")
        .extension("wav")
        .categories(&[C::AUDIO]),
    row(Kind::Wem, "audio/x-wwise-wem")
        .extension("wem")
        .categories(&[C::AUDIO])
        .named(&["wem"]),
    row(Kind::Ogg, "audio/ogg")
        .extension("ogg")
        .categories(&[C::AUDIO])
        .magics(&[b"OggS"]),
    row(Kind::Mp3, "audio/mpeg")
        .extension("mp3")
        .categories(&[C::AUDIO]),
    row(Kind::Flac, "audio/flac")
        .extension("flac")
        .categories(&[C::AUDIO])
        .magics(&[b"fLaC"]),
    row(Kind::Mp4, "video/mp4")
        .extension("mp4")
        .categories(&[C::VIDEO]),
    row(Kind::Webm, "video/webm")
        .extension("webm")
        .categories(&[C::VIDEO])
        .magics(&[b"\x1a\x45\xdf\xa3"]),
    row(Kind::Avi, "video/x-msvideo")
        .extension("avi")
        .categories(&[C::VIDEO]),
    // Fonts
    row(Kind::Ttf, "font/ttf")
        .extension("ttf")
        .categories(&[C::FONT])
        .magics(&[b"\x00\x01\x00\x00"]),
    row(Kind::Otf, "font/otf")
        .extension("otf")
        .categories(&[C::FONT])
        .magics(&[b"OTTO"]),
    row(Kind::Ttc, "font/collection")
        .extension("ttc")
        .categories(&[C::FONT])
        .magics(&[b"ttcf"]),
    row(Kind::Woff, "font/woff")
        .extension("woff")
        .categories(&[C::FONT])
        .magics(&[b"wOFF"]),
    row(Kind::Woff2, "font/woff2")
        .extension("woff2")
        .categories(&[C::FONT])
        .magics(&[b"wOF2"]),
    // Text and documents
    row(Kind::Json, "application/json")
        .extension("json")
        .categories(&[C::TEXT])
        .text_named(&["json"]),
    row(Kind::Xml, "application/xml")
        .extension("xml")
        .categories(&[C::TEXT])
        .text_named(&["xml"]),
    row(Kind::Html, "text/html")
        .extension("html")
        .categories(&[C::TEXT, C::DOCUMENT]),
    row(Kind::Ini, "text/x-ini")
        .extension("ini")
        .categories(&[C::TEXT])
        .text_named(&["ini"]),
    row(Kind::Markdown, "text/markdown")
        .extension("md")
        .categories(&[C::TEXT, C::DOCUMENT])
        .text_named(&["md"]),
    row(Kind::Text, "text/plain")
        .extension("txt")
        .categories(&[C::TEXT]),
    row(Kind::Pdf, "application/pdf")
        .extension("pdf")
        .categories(&[C::DOCUMENT])
        .magics(&[b"%PDF-"]),
    // Executables
    row(Kind::Pe, "application/vnd.microsoft.portable-executable")
        .extension("exe")
        .categories(&[C::EXECUTABLE]),
    row(Kind::Elf, "application/x-executable")
        .categories(&[C::EXECUTABLE])
        .magics(&[b"\x7fELF"]),
    row(Kind::MachO, "application/x-mach-binary").categories(&[C::EXECUTABLE]),
    row(Kind::Script, "text/x-script")
        .categories(&[C::EXECUTABLE, C::TEXT])
        .text_named(&["py", "bat", "cmd", "ps1", "vbs", "sh", "lua"]),
    // Archives
    row(Kind::Zip, "application/zip")
        .extension("zip")
        .categories(&[C::ARCHIVE])
        .magics(&[b"PK\x03\x04", b"PK\x05\x06"]),
    row(Kind::Rar, "application/vnd.rar")
        .extension("rar")
        .categories(&[C::ARCHIVE])
        .magics(&[b"Rar!\x1a\x07"]),
    row(Kind::SevenZip, "application/x-7z-compressed")
        .extension("7z")
        .categories(&[C::ARCHIVE])
        .magics(&[b"7z\xbc\xaf\x27\x1c"]),
    row(Kind::Gzip, "application/gzip")
        .extension("gz")
        .categories(&[C::ARCHIVE]),
    row(Kind::Zstd, "application/zstd")
        .extension("zst")
        .categories(&[C::ARCHIVE])
        .magics(&[b"\x28\xb5\x2f\xfd"]),
    row(Kind::Xz, "application/x-xz")
        .extension("xz")
        .categories(&[C::ARCHIVE])
        .magics(&[b"\xfd7zXZ\x00"]),
    row(Kind::Bzip2, "application/x-bzip2")
        .extension("bz2")
        .categories(&[C::ARCHIVE]),
    row(Kind::Cab, "application/vnd.ms-cab-compressed")
        .extension("cab")
        .categories(&[C::ARCHIVE])
        .magics(&[b"MSCF"]),
    // OS leftovers
    row(Kind::DsStore, "application/x-apple-ds-store")
        .extension("DS_Store")
        .categories(&[C::OS_ARTIFACT])
        .magics(&[b"\x00\x00\x00\x01Bud1"]),
    row(Kind::AppleDouble, "multipart/appledouble")
        .categories(&[C::OS_ARTIFACT])
        .magics(&[b"\x00\x05\x16\x07"]),
    row(Kind::ThumbsDb, "application/x-ms-thumbs-db")
        .extension("db")
        .categories(&[C::OS_ARTIFACT]),
    row(Kind::DesktopIni, "text/x-ini")
        .extension("ini")
        .categories(&[C::OS_ARTIFACT, C::TEXT]),
    row(Kind::IconCache, "application/x-ms-iconcache")
        .extension("db")
        .categories(&[C::OS_ARTIFACT]),
    row(Kind::Shortcut, "application/x-ms-shortcut")
        .extension("lnk")
        .categories(&[C::OS_ARTIFACT])
        .magics(&[b"L\x00\x00\x00\x01\x14\x02\x00"]),
    row(Kind::Ole, "application/x-ole-storage").magics(&[b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"]),
    row(Kind::Empty, "application/x-empty"),
    row(Kind::Unknown, "application/octet-stream"),
];

// `row_of` indexes the table by the variant.
const _: () = {
    let mut at = 0;
    while at < KINDS.len() {
        assert!(
            KINDS[at].kind as usize == at,
            "a row of KINDS is not at its kind's place in Kind"
        );
        at += 1;
    }
};

/// The row of a kind.
pub(crate) fn row_of(kind: Kind) -> &'static Row {
    &KINDS[kind as usize]
}

/// The kind whose fixed magic the head starts with.
pub(crate) fn by_magic(head: &[u8]) -> Option<Kind> {
    let starts = |row: &&Row| row.magics.iter().any(|magic| head.starts_with(magic));
    KINDS.iter().find(starts).map(|row| row.kind)
}

/// The kind an extension names where the bytes name nothing.
pub(crate) fn by_extension(ext: &str) -> Option<Kind> {
    let named = |row: &&Row| row.named.contains(&ext);
    KINDS.iter().find(named).map(|row| row.kind)
}

/// The kind an extension names where the file is text whose opening names nothing.
pub(crate) fn by_text_extension(ext: &str) -> Option<Kind> {
    let named = |row: &&Row| row.text_named.contains(&ext);
    KINDS.iter().find(named).map(|row| row.kind)
}

#[cfg(test)]
mod tests {
    use super::KINDS;

    // The lookups take the first row that matches, and the rows are in `Kind`'s order: a head
    // or an extension two rows could claim would make that order decide.

    #[test]
    fn no_magic_starts_with_another() {
        let magics: Vec<&[u8]> = KINDS.iter().flat_map(|row| row.magics).copied().collect();
        for (at, magic) in magics.iter().enumerate() {
            for (other_at, other) in magics.iter().enumerate() {
                assert!(
                    at == other_at || !magic.starts_with(other),
                    "{magic:?} starts with {other:?}"
                );
            }
        }
    }

    #[test]
    fn an_extension_names_one_kind() {
        let named: Vec<&str> = KINDS.iter().flat_map(|row| row.named).copied().collect();
        let text_named: Vec<&str> = KINDS
            .iter()
            .flat_map(|row| row.text_named)
            .copied()
            .collect();
        for extensions in [named, text_named] {
            let mut distinct = extensions.clone();
            distinct.sort_unstable();
            distinct.dedup();
            assert_eq!(
                distinct.len(),
                extensions.len(),
                "an extension is in two rows"
            );
        }
    }
}
