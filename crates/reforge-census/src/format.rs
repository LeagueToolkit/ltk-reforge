//! Format 2 of the census history, as the index reads it: `build.yaml`, a WAD entry's
//! `xx/<path hash>.yaml`, and a bin entry's `.rito` text. Each reader takes a file's text and
//! keeps what the index asks of it. A field or a section it does not know is skipped: the
//! format adds optional fields without a new number.
//!
//! The format is closed (#ADR-0035): what the history does not carry is read off an entry's
//! text here, or is not known.

use std::borrow::Cow;
use std::sync::LazyLock;

use indexmap::IndexMap;
use ltk_meta::PropertyValueEnum as P;
use ltk_meta::property::values::Struct;
use reforge::game::Build;
use reforge::history::{Joint, Mesh, Skeleton, Submesh, Texture};
use reforge::model::value::{items, looks_like_path, walk};
use reforge::model::{
    BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, name_hash, path_hash,
};

/// The format this crate reads, as `census.yaml` states it.
pub const FORMAT: u32 = 2;

/// The branch that holds the builds. `main` holds only a README and a license.
pub const BRANCH: &str = "history";

/// One build, from `build.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFile {
    /// The build.
    pub build: Build,
    /// The id of its release manifest.
    pub manifest: String,
    /// The day it went live.
    pub date: String,
}

/// A commit's `build.yaml`. None when it states no version that is a build.
pub fn parse_build(text: &str) -> Option<BuildFile> {
    let (mut version, mut manifest, mut date) = (None, String::new(), String::new());
    for line in text.lines().filter_map(Line::of).filter(|l| l.indent == 0) {
        let value = || unquote(line.value).map(Cow::into_owned).unwrap_or_default();
        match line.key {
            "version" => version = Build::parse(&value()),
            "manifest" => manifest = value(),
            "date" => date = value(),
            _ => {}
        }
    }
    Some(BuildFile {
        build: version?,
        manifest,
        date,
    })
}

/// The format number `census.yaml` states.
pub fn census_format(text: &str) -> Option<u32> {
    text.lines()
        .filter_map(Line::of)
        .find(|l| l.indent == 0 && l.key == "format")
        .and_then(|l| l.value.parse().ok())
}

/// What a WAD entry's file says that the index keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// The key of the entry's bytes.
    pub content: FileKey,
    /// What the bytes are by their magic (`bin`, `tex`, ...), empty when unknown.
    pub kind: String,
    /// A bin's objects.
    pub objects: Vec<ObjectRecord>,
    /// A bank's wems.
    pub clips: Vec<ClipRecord>,
    /// An events bank's events.
    pub events: Vec<EventRecord>,
    /// A skeleton's joints.
    pub skeleton: Option<Skeleton>,
    /// A mesh's layout and submeshes.
    pub mesh: Option<Mesh>,
    /// A texture's header and the hash of its top mip.
    pub texture: Option<Texture>,
}

/// One object of a bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectRecord {
    /// The object's entry hash.
    pub entry: BinHash,
    /// The object's class.
    pub class: BinHash,
    /// The object's key.
    pub object: ObjectKey,
    /// The object's norm key.
    pub norm: NormKey,
}

/// One wem of a bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipRecord {
    /// The wem's id.
    pub id: u32,
    /// Its size in bytes.
    pub size: u64,
    /// The clip's key.
    pub clip: ClipKey,
}

/// One event of an events bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    /// The event's Wwise id.
    pub id: u32,
    /// The ids of the wems its play actions reach.
    pub wems: Vec<u32>,
}

/// One line of the format's YAML subset: its indent (a sequence item's at its dash), whether it
/// starts a sequence item, its key unquoted and its value as written.
struct Line<'a> {
    indent: usize,
    item: bool,
    key: &'a str,
    value: &'a str,
}

impl<'a> Line<'a> {
    fn of(raw: &'a str) -> Option<Self> {
        let body = raw.trim_start_matches(' ');
        if body.is_empty() {
            return None;
        }
        let indent = raw.len() - body.len();
        let (item, body) = match body.strip_prefix("- ") {
            Some(rest) => (true, rest),
            None => (false, body),
        };
        // A quoted key is a hex id and holds no escape. A quoted scalar with no key follows a
        // dash.
        let (key, value) = match body.strip_prefix('"') {
            Some(rest) => match rest.split_once('"') {
                Some((key, after)) if after.starts_with(':') => (key, after[1..].trim_start()),
                _ => ("", body),
            },
            None => match body.split_once(':') {
                Some((key, value)) => (key, value.trim_start()),
                None => ("", body),
            },
        };
        Some(Self {
            indent,
            item,
            key,
            value,
        })
    }

    fn text(&self) -> String {
        unquote(self.value).unwrap_or_default().into_owned()
    }

    fn number(&self) -> u32 {
        self.value.parse().unwrap_or_default()
    }

    fn hash(&self) -> u64 {
        hex64(self.value).unwrap_or_default()
    }
}

/// A double-quoted scalar's text: `\"` and `\\` escaped, a control character as `\xHH`.
fn unquote(value: &str) -> Option<Cow<'_, str>> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    if !inner.contains('\\') {
        return Some(Cow::Borrowed(inner));
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'x' => {
                let hex: String = chars.by_ref().take(2).collect();
                out.push(char::from(u8::from_str_radix(&hex, 16).ok()?));
            }
            escaped => out.push(escaped),
        }
    }
    Some(Cow::Owned(out))
}

fn unquoted(value: &str) -> Option<&str> {
    value.strip_prefix('"')?.strip_suffix('"')
}

fn hex64(value: &str) -> Option<u64> {
    u64::from_str_radix(unquoted(value)?, 16).ok()
}

fn hex32(value: &str) -> Option<u32> {
    u32::from_str_radix(unquoted(value)?, 16).ok()
}

/// An object whose three keys have not all been read yet.
struct PartialObject {
    entry: u32,
    class: Option<u32>,
    content: Option<ObjectKey>,
    norm: Option<u64>,
}

/// A wem whose size and hash have not both been read yet.
struct PartialClip {
    id: u32,
    size: Option<u64>,
    hash: Option<u64>,
}

/// A WAD entry's file, `xx/<path hash>.yaml`. None when it has no `sha256`.
pub fn parse_entry(text: &str) -> Option<Entry> {
    let mut entry = Entry::default();
    let mut content = None;
    let mut objects: Vec<PartialObject> = Vec::new();
    let mut media: Vec<PartialClip> = Vec::new();
    let (mut section, mut list) = ("", "");
    for line in text.lines().filter_map(Line::of) {
        if line.indent == 0 {
            (section, list) = (line.key, "");
            match line.key {
                "sha256" => content = unquoted(line.value).and_then(FileKey::parse),
                "kind" => entry.kind = line.text(),
                // An entry is one file, so a later section of a rig replaces an earlier one.
                "skeleton" => {
                    (entry.skeleton, entry.mesh) = (Some(Skeleton::default()), None);
                }
                "mesh" => (entry.skeleton, entry.mesh) = (None, Some(Mesh::default())),
                "texture" => entry.texture = Some(Texture::default()),
                _ => {}
            }
            continue;
        }
        let id = || u32::from_str_radix(line.key, 16).ok();
        match (section, line.indent) {
            ("objects", 1) => objects.extend(id().map(|entry| PartialObject {
                entry,
                class: None,
                content: None,
                norm: None,
            })),
            ("objects", 2) => {
                let Some(o) = objects.last_mut() else {
                    continue;
                };
                match line.key {
                    "class" => o.class = hex32(line.value),
                    "content" => o.content = unquoted(line.value).and_then(ObjectKey::parse),
                    "norm" => o.norm = hex64(line.value),
                    _ => {}
                }
            }
            ("bank" | "skeleton" | "mesh", 1) if line.value.is_empty() || line.value == "[]" => {
                list = line.key;
            }
            ("bank", 2) => match list {
                "media" => media.extend(id().map(|id| PartialClip {
                    id,
                    size: None,
                    hash: None,
                })),
                "events" => entry.events.extend(id().map(|id| EventRecord {
                    id,
                    wems: Vec::new(),
                })),
                _ => {}
            },
            ("bank", 3) => match list {
                "media" => {
                    let Some(m) = media.last_mut() else { continue };
                    match line.key {
                        "size" => m.size = line.value.parse().ok(),
                        "hash" => m.hash = hex64(line.value),
                        _ => {}
                    }
                }
                "events" if line.item => {
                    if let (Some(event), Some(wem)) = (entry.events.last_mut(), hex32(line.value)) {
                        event.wems.push(wem);
                    }
                }
                _ => {}
            },
            ("skeleton", indent) => {
                let Some(skeleton) = &mut entry.skeleton else {
                    continue;
                };
                match (indent, list, line.key) {
                    (1, _, "influences") => skeleton.influences = line.number(),
                    (2, "joints", "name") if line.item => skeleton.joints.push(Joint {
                        name: line.text(),
                        parent: None,
                    }),
                    (4, "joints", "parent") => {
                        if let Some(joint) = skeleton.joints.last_mut() {
                            // An ordinal, unquoted, where another joint has the parent's name.
                            let ordinal = || format!("#{}", line.value);
                            let name = unquote(line.value).map(Cow::into_owned);
                            joint.parent = Some(name.unwrap_or_else(ordinal));
                        }
                    }
                    _ => {}
                }
            }
            ("mesh", indent) => {
                let Some(mesh) = &mut entry.mesh else {
                    continue;
                };
                match (indent, list, line.key) {
                    (1, _, "vertexType") => mesh.vertex_type = line.text(),
                    (1, _, "vertices") => mesh.vertices = line.number(),
                    (1, _, "indices") => mesh.indices = line.number(),
                    (2, "submeshes", "name") if line.item => mesh.submeshes.push(Submesh {
                        material: line.text(),
                        ..Submesh::default()
                    }),
                    (4, "submeshes", key) => {
                        let Some(submesh) = mesh.submeshes.last_mut() else {
                            continue;
                        };
                        match key {
                            "vertices" => submesh.vertices = line.number(),
                            "indices" => submesh.indices = line.number(),
                            "positions" => submesh.positions = line.hash(),
                            "rounded" => submesh.rounded = line.hash(),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            ("texture", 1) => {
                let Some(texture) = &mut entry.texture else {
                    continue;
                };
                match line.key {
                    "format" => texture.format = line.text(),
                    "width" => texture.width = line.number(),
                    "height" => texture.height = line.number(),
                    "mips" => texture.mips = line.number(),
                    "top" => texture.top = line.hash(),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    entry.content = content?;
    let whole_objects = objects.into_iter().filter_map(|o| {
        Some(ObjectRecord {
            entry: BinHash(o.entry),
            class: BinHash(o.class?),
            object: o.content?,
            norm: NormKey(o.norm?),
        })
    });
    entry.objects = whole_objects.collect();
    let whole_clips = media.into_iter().filter_map(|m| {
        Some(ClipRecord {
            id: m.id,
            size: m.size?,
            clip: ClipKey(m.hash?),
        })
    });
    entry.clips = whole_clips.collect();
    Some(entry)
}

/// What a bin entry's text names: files, audio tags, bank units.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectText {
    /// The files it names, sorted: every chunk link, and every string shaped like a path
    /// (`reforge::model::value::looks_like_path`).
    pub paths: Vec<WadHash>,
    /// The audio tags in a `tagEventList`, sorted.
    pub tags: Vec<String>,
    /// Its bank units, in order.
    pub units: Vec<UnitRecord>,
}

/// One bank unit of an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitRecord {
    /// The unit's name.
    pub name: String,
    /// Whether the unit is voice.
    pub voice: bool,
    /// The paths of the banks it loads.
    pub banks: Vec<WadHash>,
    /// The Wwise ids of the events it lists.
    pub events: Vec<u32>,
}

struct AudioNames {
    bank_unit: BinHash,
    name: BinHash,
    bank_path: BinHash,
    events: BinHash,
    voice_over: BinHash,
    tag_event_list: BinHash,
}

static AUDIO: LazyLock<AudioNames> = LazyLock::new(|| AudioNames {
    bank_unit: name_hash("BankUnit"),
    name: name_hash("name"),
    bank_path: name_hash("bankPath"),
    events: name_hash("events"),
    voice_over: name_hash("voiceOver"),
    tag_event_list: name_hash("tagEventList"),
});

/// A bin entry's `.rito` text. None when it does not parse.
pub fn parse_object_text(text: &str) -> Option<ObjectText> {
    // Syntax errors stay in the tree. Only type errors reach the built bin's diagnostics.
    let cst = ltk_ritobin::Cst::parse(text);
    if !cst.errors.is_empty() {
        return None;
    }
    let bin = cst.build_bin(text).into_result().ok()?;
    let mut out = ObjectText::default();
    for object in bin.objects.values() {
        tags_of(&object.properties, &mut out.tags);
        for value in object.properties.values() {
            walk(value, &mut |v| match v {
                P::String(s) if looks_like_path(&s.value) => out.paths.push(path_hash(&s.value)),
                P::WadChunkLink(l) if l.value.0 != 0 => out.paths.push(l.value),
                P::Struct(s) => structure(s, &mut out),
                P::Embedded(e) => structure(&e.0, &mut out),
                _ => {}
            });
        }
    }
    out.paths.sort_unstable();
    out.paths.dedup();
    out.tags.sort_unstable();
    out.tags.dedup();
    Some(out)
}

fn structure(s: &Struct, out: &mut ObjectText) {
    tags_of(&s.properties, &mut out.tags);
    if s.class_hash != AUDIO.bank_unit {
        return;
    }
    let field = |name: BinHash| s.properties.get(&name);
    let strings = |name: BinHash| {
        let values = field(name).map(items).unwrap_or_default().iter();
        values.filter_map(|v| match v {
            P::String(s) => Some(s.value.as_str()),
            _ => None,
        })
    };
    let voice = match field(AUDIO.voice_over) {
        Some(P::Bool(b)) => b.value,
        Some(P::BitBool(b)) => b.value,
        _ => false,
    };
    out.units.push(UnitRecord {
        name: match field(AUDIO.name) {
            Some(P::String(s)) => s.value.clone(),
            _ => String::new(),
        },
        voice,
        banks: strings(AUDIO.bank_path).map(path_hash).collect(),
        events: strings(AUDIO.events).map(reforge_wwise::id_of).collect(),
    });
}

fn tags_of(properties: &IndexMap<BinHash, P>, tags: &mut Vec<String>) {
    let Some(value) = properties.get(&AUDIO.tag_event_list) else {
        return;
    };
    tags.extend(items(value).iter().filter_map(|v| match v {
        P::String(s) => Some(s.value.clone()),
        _ => None,
    }));
}

/// The path a WAD entry's file, `.../xx/<path hash>.yaml`, is named for.
pub fn entry_file(path: &str) -> Option<WadHash> {
    let stem = path.rsplit('/').next()?.strip_suffix(".yaml")?;
    hex_name(stem, 16).and_then(|s| u64::from_str_radix(s, 16).ok().map(WadHash))
}

/// The bin's path and the entry hash a bin entry's text,
/// `.../<path hash>.bin/yy/<entry>.rito`, is named for.
pub fn object_file(path: &str) -> Option<(WadHash, BinHash)> {
    let mut parts = path.rsplit('/');
    let name = parts.next()?.strip_suffix(".rito")?;
    let entry = u32::from_str_radix(hex_name(name, 8)?, 16).ok()?;
    parts.next()?;
    Some((bin_dir(parts.next()?)?, BinHash(entry)))
}

/// The bin path a bin's directory of entry texts, `<path hash>.bin`, is named for.
pub fn bin_dir(name: &str) -> Option<WadHash> {
    let stem = hex_name(name.strip_suffix(".bin")?, 16)?;
    u64::from_str_radix(stem, 16).ok().map(WadHash)
}

fn hex_name(name: &str, len: usize) -> Option<&str> {
    let hex = name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    (name.len() == len && hex).then_some(name)
}

/// A bin entry's text in its WAD's directory: `xx/<bin>.bin/yy/<entry>.rito`.
pub fn object_path(bin: WadHash, entry: BinHash) -> String {
    format!(
        "{:02x}/{:016x}.bin/{:02x}/{:08x}.rito",
        bin.0 >> 56,
        bin.0,
        entry.0 >> 24,
        entry.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_unquote() {
        assert_eq!(
            unquote("\"a\\\"b\\\\c\\x09\"").as_deref(),
            Some("a\"b\\c\t")
        );
        assert_eq!(unquote("12"), None);
    }

    #[test]
    fn file_names() {
        assert_eq!(
            entry_file("files/data/x.wad.client/0a/0a1b2c3d4e5f6071.yaml"),
            Some(WadHash(0x0a1b_2c3d_4e5f_6071))
        );
        assert_eq!(entry_file("files/data/x.wad.client/_wad.yaml"), None);
        assert_eq!(
            object_file("files/x.wad.client/01/014ca4df836b3e73.bin/35/3537586f.rito"),
            Some((WadHash(0x014c_a4df_836b_3e73), BinHash(0x3537_586f)))
        );
        assert_eq!(
            object_path(WadHash(0x014c_a4df_836b_3e73), BinHash(0x3537_586f)),
            "01/014ca4df836b3e73.bin/35/3537586f.rito"
        );
    }
}
