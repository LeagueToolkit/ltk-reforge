//! Format 2 of the census history, as the index reads it: `build.yaml`, a WAD entry's
//! `xx/<path hash>.yaml`, and a bin entry's `.rito` text. Each reader takes a file's text and
//! keeps what the index asks of it. A field or a section it does not know is skipped: the
//! format adds optional fields without a new number.
//!
//! The format is closed (#ADR-0035): what the history does not carry is read off an entry's
//! text here, or is not known.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::io::Cursor;
use std::sync::LazyLock;

use indexmap::IndexMap;
use ltk_meta::PropertyValueEnum as P;
use ltk_meta::property::values::Struct;
use ltk_meta::{Bin, BinObject};
use reforge::game::{Build, FieldType};
use reforge::history::tables::{ClassField, JointList};
use reforge::history::{BuildDetails, EventsBank, Joint, Mesh, Skeleton, Submesh, Texture};
use reforge::model::value::{items, looks_like_path, walk};
use reforge::model::{
    BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, name_hash, path_hash,
};
use reforge_wwise::Object;

/// The format this crate reads, as `census.yaml` states it.
pub const FORMAT: u32 = 2;

/// The branch that holds the builds: format 2 with a clip's joints and an events bank's header
/// and objects. `main` holds only a README and a license.
pub const BRANCH: &str = "history-v2";

/// One build, from `build.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFile {
    /// The build.
    pub build: Build,
    /// The id of its release manifest.
    pub manifest: String,
    /// The day it went live.
    pub date: String,
    /// The rest of the file.
    pub details: BuildDetails,
}

/// A commit's `build.yaml`. None when it states no version that is a build.
pub fn parse_build(text: &str) -> Option<BuildFile> {
    let (mut version, mut manifest, mut date) = (None, String::new(), String::new());
    let mut details = BuildDetails::default();
    let mut section = "";
    for line in text.lines().filter_map(Line::of) {
        if line.indent > 0 {
            if section == "realms" && line.item {
                details.realms.push(line.text());
            }
            continue;
        }
        section = line.key;
        let value = || line.text();
        match line.key {
            "version" => version = Build::parse(&value()),
            "patch" => details.patch = value(),
            "manifest" => manifest = value(),
            "source" => details.source = value(),
            "solution" => details.solution = value(),
            "release" => details.release = value(),
            "exe" => details.exe = value(),
            "date" => date = value(),
            "legacyBins" => details.legacy_bins = line.value == "true",
            _ => {}
        }
    }
    Some(BuildFile {
        build: version?,
        manifest,
        date,
        details,
    })
}

/// The format number `census.yaml` states.
pub fn census_format(text: &str) -> Option<u32> {
    text.lines()
        .filter_map(Line::of)
        .find(|l| l.indent == 0 && l.key == "format")
        .and_then(|l| l.value.parse().ok())
}

/// A WAD, from `_wad.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WadFile {
    /// The WAD format's `major.minor`.
    pub version: String,
    /// The manifest's id for this version of the file.
    pub file_id: u64,
    /// The manifest's tags for the file, sorted.
    pub tags: Vec<String>,
}

/// A WAD's `_wad.yaml`.
pub fn parse_wad(text: &str) -> WadFile {
    let mut wad = WadFile::default();
    let mut section = "";
    for line in text.lines().filter_map(Line::of) {
        if line.indent == 0 {
            section = line.key;
            match line.key {
                "version" => wad.version = line.text(),
                "fileId" => wad.file_id = line.hash(),
                _ => {}
            }
        } else if section == "tags" && line.item {
            wad.tags.push(line.text());
        }
    }
    wad
}

/// What a WAD entry's file says that the index keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// The key of the entry's bytes.
    pub content: FileKey,
    /// The checksum the WAD's table holds for the entry, 0 in a WAD without them.
    pub checksum: u64,
    /// What the bytes are by their magic (`bin`, `tex`, ...), empty when unknown.
    pub kind: String,
    /// The path hash of each bin a bin links. None for an entry that is no bin, and for a bin
    /// the census could not read (a `PTCH`), which has no `links` key.
    pub links: Option<Vec<WadHash>>,
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
    /// The joints an animation clip animates (#ADR-0042).
    pub joints: Option<JointList>,
    /// An events bank's header and objects (#ADR-0042). None for a bank with no `HIRC`
    /// section, and for one `reforge_wwise::Bank::read` refuses: a header too short for a
    /// version and a bank id, an object too short for an id.
    pub bank: Option<EventsBank>,
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

/// The bytes a quoted run of hex digits spells, two digits a byte.
fn hex_bytes(value: &str) -> Option<Vec<u8>> {
    let (pairs, rest) = unquoted(value)?.as_bytes().as_chunks::<2>();
    let digit = |d: u8| {
        char::from(d)
            .to_digit(16)
            .and_then(|d| u8::try_from(d).ok())
    };
    let byte = |[high, low]: &[u8; 2]| Some(digit(*high)? << 4 | digit(*low)?);
    let bytes: Option<Vec<u8>> = pairs.iter().map(byte).collect();
    bytes.filter(|_| rest.is_empty())
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

/// An object of a bank's hierarchy whose three keys have not all been read yet.
#[derive(Default)]
struct PartialBankObject {
    id: Option<u32>,
    kind: Option<u8>,
    body: Option<Vec<u8>>,
}

impl PartialBankObject {
    fn read(&mut self, line: &Line<'_>) {
        match line.key {
            "id" => self.id = hex32(line.value),
            "type" => self.kind = line.value.parse().ok(),
            "body" => self.body = hex_bytes(line.value),
            _ => {}
        }
    }

    fn whole(self) -> Option<Object> {
        Some(Object {
            kind: self.kind?,
            id: self.id?,
            body: self.body?,
        })
    }
}

/// A WAD entry's file, `xx/<path hash>.yaml`. None when it has no `sha256`.
pub fn parse_entry(text: &str) -> Option<Entry> {
    let mut entry = Entry::default();
    let mut content = None;
    let mut objects: Vec<PartialObject> = Vec::new();
    let mut media: Vec<PartialClip> = Vec::new();
    let mut header: Option<Vec<u8>> = None;
    let mut hierarchy: Vec<PartialBankObject> = Vec::new();
    // None once a joint does not read: a list short of a joint is not the clip's.
    let mut joints: Option<Vec<u32>> = None;
    let (mut section, mut list) = ("", "");
    for line in text.lines().filter_map(Line::of) {
        if line.indent == 0 {
            (section, list) = (line.key, "");
            match line.key {
                "sha256" => content = unquoted(line.value).and_then(FileKey::parse),
                "checksum" => entry.checksum = line.hash(),
                "kind" => entry.kind = line.text(),
                "links" => entry.links = Some(Vec::new()),
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
            ("links", 1) if line.item => {
                let links = entry.links.get_or_insert_default();
                links.push(path_hash(&line.text()));
            }
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
            ("bank", 1) if line.key == "header" => header = hex_bytes(line.value),
            ("bank" | "skeleton" | "mesh", 1) if line.value.is_empty() || line.value == "[]" => {
                list = line.key;
            }
            ("clip", 1) if line.key == "joints" => (list, joints) = (line.key, Some(Vec::new())),
            ("clip", 2) if list == "joints" && line.item => {
                joints = joints.zip(hex32(line.value)).map(|(mut joints, joint)| {
                    joints.push(joint);
                    joints
                });
            }
            ("bank", 2 | 4) if list == "objects" => {
                if line.item {
                    hierarchy.push(PartialBankObject::default());
                }
                if let Some(object) = hierarchy.last_mut() {
                    object.read(&line);
                }
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
    entry.joints = joints.map(|joints| JointList(joints.into()));
    let hierarchy: Option<Vec<Object>> = hierarchy.into_iter().map(|o| o.whole()).collect();
    entry.bank = header
        .zip(hierarchy)
        .and_then(|(header, objects)| EventsBank::new(header, objects));
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
    /// The type each field of its object and of every struct in it had, sorted, each once.
    pub schema: Vec<ClassField>,
    /// The entry hash of each object it links, sorted, each once.
    pub links: Vec<BinHash>,
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

/// One object of a bin entry's text, as `ltk_meta` writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextObject {
    /// The object's class.
    pub class: BinHash,
    /// The object from its entry hash on, with no size before it: the bytes its object key
    /// digests.
    pub bytes: Vec<u8>,
}

/// A bin entry's `.rito` text as a bin. None when it does not parse.
fn text_bin(text: &str) -> Option<Bin> {
    // Syntax errors stay in the tree. Only type errors reach the built bin's diagnostics.
    let cst = ltk_ritobin::Cst::parse(text);
    if !cst.errors.is_empty() {
        return None;
    }
    cst.build_bin(text).into_result().ok()
}

/// An object as `ltk_meta` writes it, from its entry hash on. None when it does not write.
fn object_bytes(object: &BinObject) -> Option<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    object.to_writer(&mut out).ok()?;
    let mut bytes = out.into_inner();
    // The size before the object: a bin's table of contents holds it, and the key leaves it out.
    bytes.drain(..4.min(bytes.len()));
    Some(bytes)
}

/// A bin entry's `.rito` text: what it names, and its objects as `ltk_meta` writes them. None
/// when it does not parse. An object that does not write is left out.
pub fn read_object_text(text: &str) -> Option<(ObjectText, Vec<TextObject>)> {
    let bin = text_bin(text)?;
    let objects = bin.objects.values().filter_map(|object| {
        let bytes = object_bytes(object)?;
        let class = object.class_hash;
        Some(TextObject { class, bytes })
    });
    let objects = objects.collect();
    Some((names(&bin), objects))
}

/// A bin entry's `.rito` text. None when it does not parse.
pub fn parse_object_text(text: &str) -> Option<ObjectText> {
    text_bin(text).map(|bin| names(&bin))
}

/// What a bin's objects name.
fn names(bin: &Bin) -> ObjectText {
    let mut out = ObjectText::default();
    for object in bin.objects.values() {
        fields(object.class_hash, &object.properties, &mut out.schema);
        tags_of(&object.properties, &mut out.tags);
        for value in object.properties.values() {
            walk(value, &mut |v| match v {
                P::String(s) if looks_like_path(&s.value) => out.paths.push(path_hash(&s.value)),
                P::WadChunkLink(l) if l.value.0 != 0 => out.paths.push(l.value),
                P::ObjectLink(l) if l.value.0 != 0 => out.links.push(l.value),
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
    out.schema.sort_unstable();
    out.schema.dedup();
    out.links.sort_unstable();
    out.links.dedup();
    out
}

/// The type of each field of a struct of a class, and of the fields of every struct its values
/// hold.
fn fields(class: BinHash, properties: &IndexMap<BinHash, P>, out: &mut Vec<ClassField>) {
    for (field, value) in properties {
        let field = *field;
        out.extend(types(value).map(|ty| ClassField { class, field, ty }));
        nested(value, out);
    }
}

/// The fields of the structs a value holds.
fn nested(value: &P, out: &mut Vec<ClassField>) {
    match value {
        P::Struct(s) => fields(s.class_hash, &s.properties, out),
        P::Embedded(e) => fields(e.0.class_hash, &e.0.properties, out),
        P::Container(c) => c.items().iter().for_each(|item| nested(item, out)),
        P::UnorderedContainer(c) => c.0.items().iter().for_each(|item| nested(item, out)),
        P::Optional(o) => o.value().into_iter().for_each(|item| nested(item, out)),
        P::Map(m) => m.entries().iter().for_each(|(_, item)| nested(item, out)),
        _ => {}
    }
}

/// The types a value has: one, or of a list, an option or a map of structs one per class its
/// items have.
fn types(value: &P) -> impl Iterator<Item = FieldType> {
    let ty = FieldType::of(value);
    let items: Vec<&P> = match value {
        P::Container(c) => c.items().iter().collect(),
        P::UnorderedContainer(c) => c.0.items().iter().collect(),
        P::Optional(o) => o.value().into_iter().collect(),
        P::Map(m) => m.entries().iter().map(|(_, item)| item).collect(),
        _ => return vec![ty].into_iter(),
    };
    let class_of = |item: &&P| match item {
        P::Embedded(e) => Some(e.0.class_hash),
        P::Struct(s) => Some(s.class_hash),
        _ => None,
    };
    let classes: BTreeSet<BinHash> = items.iter().filter_map(class_of).collect();
    let classes = classes.into_iter().filter(|class| class.0 != 0);
    let mut typed: Vec<FieldType> = classes
        .map(|class| FieldType {
            class: Some(class),
            ..ty
        })
        .collect();
    if typed.is_empty() {
        typed.push(FieldType { class: None, ..ty });
    }
    typed.into_iter()
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

/// The WAD's file, `files/<manifest path>/_wad.yaml`.
const WAD_FILE: &str = "_wad.yaml";

/// The WAD a file of the history is in, as its path in the manifest: the directory under `files/`
/// that holds a WAD's `_wad.yaml`, its entries' files and its bins' entry texts.
pub fn wad_of(path: &str) -> Option<&str> {
    let depth = if is_wad_file(path) {
        1
    } else if object_file(path).is_some() {
        4
    } else if entry_file(path).is_some() {
        2
    } else {
        return None;
    };
    let mut dir = path;
    for _ in 0..depth {
        dir = dir.rsplit_once('/')?.0;
    }
    dir.strip_prefix("files/")
}

/// Whether a file of the history is a WAD's `_wad.yaml`.
pub fn is_wad_file(path: &str) -> bool {
    path.rsplit('/').next() == Some(WAD_FILE)
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

    #[test]
    fn a_file_is_in_the_wad_whose_directory_holds_it() {
        assert_eq!(
            wad_of("files/data/x.wad.client/0a/0a1b2c3d4e5f6071.yaml"),
            Some("data/x.wad.client")
        );
        assert_eq!(
            wad_of("files/data/x.wad.client/01/014ca4df836b3e73.bin/35/3537586f.rito"),
            Some("data/x.wad.client")
        );
        assert_eq!(
            wad_of("files/data/x.wad.client/_wad.yaml"),
            Some("data/x.wad.client")
        );
        assert_eq!(wad_of("build.yaml"), None);
        assert!(is_wad_file("files/data/x.wad.client/_wad.yaml"));
    }
}
