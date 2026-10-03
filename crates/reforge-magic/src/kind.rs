//! The kinds of file [`identify`](crate::identify) tells apart, and what each one is.

use serde::Serialize;

use crate::Categories;
use crate::table;

/// A kind of file: a League format, or one of the other formats mods carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    // League
    /// A property bin, `PROP`.
    PropertyBin,
    /// A property bin patch, `PTCH`.
    PropertyBinOverride,
    /// An animation, `.anm`, compressed or not.
    Animation,
    /// A skeleton, `.skl`, with its magic at the start or at byte 4.
    Skeleton,
    /// A skinned mesh, `.skn`.
    SimpleSkin,
    /// The skin weights of a legacy mesh, `.wgt`.
    SkinWeights,
    /// A static mesh as binary, `.scb`.
    StaticMeshBinary,
    /// A static mesh as text, `.sco`.
    StaticMeshAscii,
    /// A texture in Riot's container, `.tex`.
    Texture,
    /// A texture as DDS.
    TextureDds,
    /// A Wwise sound bank, `.bnk`.
    WwiseBank,
    /// A Wwise package of audio files, `.wpk`.
    WwisePackage,
    /// A map's geometry, `.mapgeo`.
    MapGeometry,
    /// A legacy map's geometry, `.wgeo`.
    WorldGeometry,
    /// A preload list, `.preload`.
    Preload,
    /// A table of localised strings, `RST`.
    StringTable,
    /// Compiled Lua, `.luaobj`.
    LuaObj,
    /// A map's light grid. It has no magic.
    LightGrid,
    /// The legacy key-value binary of `.troybin` and `.inibin`.
    Inibin,
    /// A legacy particle system as INI text.
    Troy,
    /// A WAD archive.
    Wad,
    /// A WAD's table of subchunks, `<wad>.subchunktoc`: known by its name, it has no magic.
    SubchunkToc,
    /// A champion's item recommendations, `.rec`: known by its name.
    ItemRecommendations,
    /// `r3d2` followed by nothing this crate knows.
    R3d2,
    // Bin sources
    /// A bin as ritobin's text.
    RitobinText,
    /// A bin as ritobin's JSON.
    RitobinJson,
    // Images
    /// A PNG image.
    Png,
    /// A JPEG image.
    Jpeg,
    /// A GIF image.
    Gif,
    /// A Windows bitmap.
    Bmp,
    /// A Targa image. It has no magic.
    Tga,
    /// A TIFF image, of either byte order.
    Tiff,
    /// A Windows icon.
    Ico,
    /// A WebP image.
    WebP,
    /// An SVG image.
    Svg,
    /// A Photoshop document.
    Psd,
    /// An OpenEXR image.
    Exr,
    /// A GIMP image.
    Xcf,
    /// A paint.net image.
    Pdn,
    /// Maya's IFF image, as its swatch thumbnails are.
    MayaImage,
    /// Maya's swatch cache, `MayaIcons`.
    MayaSwatches,
    // Authoring scenes and models
    /// A Maya scene as text, `.ma`.
    MayaAscii,
    /// A Maya scene as binary, `.mb`.
    MayaBinary,
    /// An FBX scene as binary.
    FbxBinary,
    /// An FBX scene as text.
    FbxAscii,
    /// A Blender scene.
    Blend,
    /// A glTF scene as one binary file, `.glb`.
    GltfBinary,
    /// A glTF scene as JSON.
    Gltf,
    /// A Wavefront OBJ mesh.
    Obj,
    /// A 3ds Max scene: an OLE file, known by its name.
    Max,
    /// A ZBrush tool or project: known by its name.
    ZBrush,
    /// A Substance project, graph or archive: known by its name.
    Substance,
    // Audio and video
    /// A RIFF wave file. A Wwise `.wem` that is one is this kind.
    Wav,
    /// A Wwise audio file that is not a RIFF wave: known by its name.
    Wem,
    /// An Ogg stream.
    Ogg,
    /// An MP3 file that starts with an ID3 tag.
    Mp3,
    /// A FLAC file.
    Flac,
    /// An MP4 file, or another container of its family.
    Mp4,
    /// A WebM or Matroska file.
    Webm,
    /// An AVI file.
    Avi,
    // Fonts
    /// A TrueType font.
    Ttf,
    /// An OpenType font with CFF outlines.
    Otf,
    /// A TrueType collection.
    Ttc,
    /// A WOFF font.
    Woff,
    /// A WOFF2 font.
    Woff2,
    // Text and documents
    /// JSON that is not a bin's.
    Json,
    /// XML that is not an image.
    Xml,
    /// An HTML page.
    Html,
    /// INI text that is not a particle system.
    Ini,
    /// Markdown: text named `.md`.
    Markdown,
    /// Text that nothing else names.
    Text,
    /// A PDF document.
    Pdf,
    // Executables
    /// A Windows executable or library.
    Pe,
    /// An ELF executable or library.
    Elf,
    /// A Mach-O executable or library.
    MachO,
    /// Text that runs: it starts with `#!`, or has a script's extension.
    Script,
    // Archives
    /// A ZIP archive, or a format built on one.
    Zip,
    /// A RAR archive.
    Rar,
    /// A 7z archive.
    SevenZip,
    /// A gzip stream.
    Gzip,
    /// A Zstandard stream.
    Zstd,
    /// An xz stream.
    Xz,
    /// A bzip2 stream.
    Bzip2,
    /// A Microsoft cabinet.
    Cab,
    // OS leftovers
    /// The macOS Finder's record of a folder, `.DS_Store`.
    DsStore,
    /// The macOS sidecar of a file, `._<name>`, or anything under `__MACOSX/`.
    AppleDouble,
    /// The Windows thumbnail cache of a folder, `Thumbs.db`.
    ThumbsDb,
    /// The Windows settings of a folder, `desktop.ini`.
    DesktopIni,
    /// The Windows icon cache, `IconCache*.db`.
    IconCache,
    /// A Windows shortcut, `.lnk`.
    Shortcut,
    /// An OLE compound file nothing else names.
    Ole,
    /// A file of no bytes.
    Empty,
    /// A file nothing here names.
    Unknown,
}

/// What a kind is: its MIME type, its usual extension and its categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// The kind's MIME type.
    pub mime: &'static str,
    /// The extension a file of this kind usually has, without its dot.
    pub extension: Option<&'static str>,
    /// The categories the kind is in.
    pub categories: Categories,
}

impl Kind {
    /// The number of kinds. `Unknown` is the last one.
    pub(crate) const COUNT: usize = Kind::Unknown as usize + 1;

    /// The kind's MIME type, usual extension and categories.
    pub fn info(self) -> Info {
        let row = table::row_of(self);
        Info {
            mime: row.mime,
            extension: row.extension,
            categories: row.categories,
        }
    }
}
