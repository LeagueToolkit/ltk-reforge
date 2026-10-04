//! Every name and path a port makes (#ADR-0027): the namespace, a unit's directory, the place
//! of a file by its first user, the name of a carried object, the paths of the port's bins and
//! patches. No other code spells one.

use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

use ltk_meta::BinObject;
use serde::Serialize;

use crate::model::entity::{FileKind, FileNode, Mod, ModMeta};
use crate::model::{
    BinHash, ChunkPath, Dump, DumpCx, Layer, Slot, Spell, Spelled, name_hash, value,
};

/// The names a port makes for the hashes it writes: what its hash tables list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MadeNames {
    /// Chunk paths.
    pub paths: BTreeSet<String>,
    /// Entry names.
    pub entries: BTreeSet<String>,
}

/// The owner of a port's names and paths. It keeps which are taken, so no two files and no
/// two objects get one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Layout {
    /// `<name>.<author>` from the archive's metadata, the mod id for a part that is missing.
    pub namespace: String,
    /// The names made.
    pub made: MadeNames,
    /// The characters of the mod, by their directory names: a file's stem is tidied of them.
    characters: Vec<String>,
    #[serde(skip)]
    taken_names: HashSet<String>,
    #[serde(skip)]
    taken_paths: HashSet<String>,
}

/// Lowercase letters, digits, `_` and `-` of a text.
fn slug(text: &str) -> String {
    let kept = |c: char| {
        let c = c.to_ascii_lowercase();
        (c.is_ascii_alphanumeric() || c == '_' || c == '-').then_some(c)
    };
    text.chars().filter_map(kept).collect()
}

/// A name part: lowercase letters, digits, `_`, `-` and `.`, any other character as `_`.
fn plain(text: &str) -> String {
    let kept = |c: char| match c.to_ascii_lowercase() {
        c if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') => c,
        _ => '_',
    };
    text.chars().map(kept).collect()
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// Whether a path's file stem is sixteen hex digits, which a manager reads as the chunk's hash
/// wherever the file sits (#MOD-ARTIFACT-0007).
fn reads_as_hash(path: &ChunkPath) -> bool {
    let name = path.parts().last().unwrap_or_default();
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether a part between a stem and its extension is a tag a repath tool left
/// (#MOD-ARTIFACT-0026): `skins_katarina_skin59`, `pie_c_12_19`.
fn is_repath_tag(part: &str) -> bool {
    let skin = |token: &str| token.strip_prefix("skin").is_some_and(digits);
    let tokens: Vec<&str> = part.split('_').collect();
    match tokens.as_slice() {
        ["skins", _, last] => skin(last) || *last == "base",
        [_, last] => skin(last),
        ["pie", "c", a, b] => digits(a) && digits(b),
        // Takes `tftset7` and leaves the `tft_set7` that mods carry (#MOD-ARTIFACT-0026): the
        // rule is ltk-forge's, with no mod behind it (`docs/design/ROADMAP.md`, "Open").
        [set] => set
            .strip_prefix("tft")
            .and_then(|rest| rest.strip_prefix("set"))
            .is_some_and(digits),
        _ => false,
    }
}

/// A file's stem as a port names it: the author's own, with a repath tool's tags dropped
/// (#MOD-ARTIFACT-0026), then a leading `<character>_skin<N>_` or `<character>_base_`, which
/// the unit's directory says. Each step is kept only when something is left.
pub fn tidy_stem(stem: &str, characters: &[String]) -> String {
    let mut parts = stem.split('.');
    let mut out = parts.next().unwrap_or(stem).to_string();
    for part in parts.filter(|part| !is_repath_tag(part)) {
        out.push('.');
        out.push_str(part);
    }
    for character in characters {
        let Some(rest) = out.strip_prefix(&format!("{character}_")) else {
            continue;
        };
        let rest = match rest.split_once('_') {
            Some((skin, tail))
                if skin == "base" || skin.strip_prefix("skin").is_some_and(digits) =>
            {
                tail
            }
            _ => continue,
        };
        if !rest.is_empty() {
            out = rest.to_string();
        }
        break;
    }
    out
}

/// A folder's name from an effect's or a material's name: tidied as a file's stem is, and
/// without a leading `<character>_`.
pub fn folder_name(name: &str, characters: &[String]) -> String {
    let tidy = tidy_stem(&plain(name), characters);
    let stripped = characters.iter().find_map(|character| {
        let rest = tidy.strip_prefix(&format!("{character}_"))?;
        (!rest.is_empty()).then(|| rest.to_string())
    });
    stripped.unwrap_or(tidy)
}

/// The folder a file of a kind goes in, under its unit's directory.
fn kind_folder(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Skn | FileKind::Skl => "",
        FileKind::Anm => "animations",
        FileKind::Tex | FileKind::Dds => "textures",
        FileKind::Scb | FileKind::Sco => "meshes",
        FileKind::Bnk | FileKind::Wpk => "audio",
        FileKind::Bin | FileKind::Ptch => "bins",
        FileKind::Other => "files",
    }
}

/// The name an object declares for itself: a particle system's `particleName`, a material's
/// `name`. With `checked`, only one that hashes to the object's entry.
fn self_name(object: &BinObject, checked: bool) -> Option<String> {
    ["particleName", "name", "mName"].iter().find_map(|field| {
        let name = value::get(object, &[field]).and_then(value::as_str)?;
        let wanted = !name.is_empty() && (!checked || name_hash(name) == object.path_hash);
        wanted.then(|| name.to_string())
    })
}

impl Layout {
    /// The layout of a mod's port.
    pub fn of(m: &Mod) -> Self {
        Self {
            namespace: Self::namespace_of(&m.meta),
            characters: m.characters.values().map(|c| c.name.clone()).collect(),
            ..Self::default()
        }
    }

    /// `<name>.<author>`, each part reduced to lowercase letters, digits, `_` and `-`. The
    /// mod id stands in for a part that is missing or reduces to nothing.
    pub fn namespace_of(meta: &ModMeta) -> String {
        let part = |text: Option<&Spelled>| {
            let slug = text
                .map(|text| slug(text.raw()))
                .filter(|slug| !slug.is_empty());
            slug.unwrap_or_else(|| meta.mod_id.clone())
        };
        format!(
            "{}.{}",
            part(meta.name.as_ref()),
            part(meta.author.as_ref())
        )
    }

    /// The characters of the mod, by their directory names.
    pub fn characters(&self) -> &[String] {
        &self.characters
    }

    /// A unit's directory: `mods/<ns>/characters/<character>/skin<T>`.
    pub fn unit_dir(&self, character: &str, target: Slot) -> String {
        format!(
            "mods/{}/characters/{character}/skin{target}",
            self.namespace
        )
    }

    /// The name of a unit's graph: `Mods/<ns>/Characters/<C>/Animations/Skin<T>`.
    pub fn graph_name(&self, character: &str, target: Slot) -> String {
        format!(
            "Mods/{}/Characters/{character}/Animations/Skin{target}",
            self.namespace
        )
    }

    /// The folder, under a skin's names, of an object of a class: particles and materials as
    /// the game files them, the rest directly.
    fn object_folder(class: BinHash) -> &'static str {
        if class == name_hash("VfxSystemDefinitionData") {
            "Particles/"
        } else if class == name_hash("StaticMaterialDef") {
            "Materials/"
        } else {
            ""
        }
    }

    /// The last part of the name a carried object is known by: the name a table spells for
    /// its entry, else the name it declares, else its entry hash.
    pub fn leaf(names: &dyn Spell, object: &BinObject) -> String {
        let named = names.entry(object.path_hash);
        let named = named.or_else(|| self_name(object, true));
        // An author's typo does not hash to the entry, and still reads better than a hash.
        let named = named.or_else(|| self_name(object, false));
        let leaf = named.and_then(|name| name.rsplit('/').next().map(str::to_string));
        let leaf = leaf.filter(|leaf| !leaf.is_empty());
        leaf.unwrap_or_else(|| format!("{:08x}", object.path_hash.0))
    }

    /// A name not taken yet: `wanted`, else `wanted-2`, `wanted-3` and so on.
    fn unique(&mut self, wanted: &str) -> String {
        if self.taken_names.insert(wanted.to_ascii_lowercase()) {
            return wanted.to_string();
        }
        let mut n = 2;
        loop {
            let name = format!("{wanted}-{n}");
            if self.taken_names.insert(name.to_ascii_lowercase()) {
                return name;
            }
            n += 1;
        }
    }

    /// The name a unit's graph takes, kept as a name made.
    pub fn name_graph(&mut self, character: &str, target: Slot) -> BinHash {
        let name = self.unique(&self.graph_name(character, target));
        self.made_entry(name)
    }

    /// The name a carried object takes under its unit's names:
    /// `Mods/<ns>/Characters/<C>/Skins/Skin<T>/<folder><leaf>`.
    pub fn name_object(
        &mut self,
        character: &str,
        target: Slot,
        class: BinHash,
        leaf: &str,
    ) -> BinHash {
        let folder = Self::object_folder(class);
        let wanted = format!(
            "Mods/{}/Characters/{character}/Skins/Skin{target}/{folder}{leaf}",
            self.namespace
        );
        let name = self.unique(&wanted);
        self.made_entry(name)
    }

    /// Keeps an entry name the port made, and gives its hash.
    pub fn made_entry(&mut self, name: String) -> BinHash {
        let hash = name_hash(&name);
        self.made.entries.insert(name);
        hash
    }

    /// Where the files an object names go, by what the object is: an effect's under
    /// `vfx/<effect>`, a material's under `materials/<material>`. None for any other object,
    /// whose files go by their kind.
    pub fn owner_folder(&self, object: &BinObject, name: &str) -> Option<String> {
        let name = folder_name(name, &self.characters);
        if object.class_hash == name_hash("VfxSystemDefinitionData") {
            Some(format!("vfx/{name}"))
        } else if object.class_hash == name_hash("StaticMaterialDef") {
            Some(format!("materials/{name}"))
        } else {
            None
        }
    }

    /// The first path of `candidates` that is a path a writer may use, that a manager reads as
    /// a path and that is not taken.
    fn take(&mut self, mut candidates: impl Iterator<Item = String>) -> Option<ChunkPath> {
        let path = candidates.find_map(|candidate| {
            let path = ChunkPath::parse(&candidate).ok();
            let path = path.filter(|path| !reads_as_hash(path))?;
            self.taken_paths
                .insert(path.as_str().to_string())
                .then_some(path)
        })?;
        self.made.paths.insert(path.as_str().to_string());
        Some(path)
    }

    /// Where an own file ships: under its user's folder when it has one, else by its kind,
    /// with its name tidied and its extension from its bytes. A file no table names is named
    /// `unnamed-<hash>` (#ADR-0054). A texture ships as `.tex`, as the game's do
    /// (#GAME-CHANGE-0007).
    ///
    /// # Panics
    ///
    /// When `unnamed-<hash>` makes no path under the unit's directory, which it does.
    pub fn place_file(
        &mut self,
        file: &FileNode,
        names: &dyn Spell,
        dir: &str,
        owner: Option<&str>,
    ) -> ChunkPath {
        let spelled = file.path.as_ref().map(|path| path.as_str().to_string());
        let spelled = spelled.or_else(|| names.path(file.hash));
        let leaf = spelled.as_deref().and_then(|path| path.rsplit('/').next());
        let leaf = leaf.map(str::to_ascii_lowercase);
        let hashed = format!("unnamed-{:016x}", file.hash.0);
        let (stem, spelled_ext) = match leaf.as_deref().and_then(|leaf| leaf.rsplit_once('.')) {
            Some((stem, ext)) => (tidy_stem(&plain(stem), &self.characters), Some(plain(ext))),
            None => (hashed.clone(), None),
        };
        let extension = match file.kind {
            FileKind::Other => spelled_ext.unwrap_or_else(|| "dat".to_string()),
            FileKind::Dds => FileKind::Tex.as_str().to_string(),
            kind => kind.as_str().to_string(),
        };
        let folder = owner.unwrap_or_else(|| kind_folder(file.kind));
        let base = |stem: &str| match folder {
            "" => format!("{dir}/{stem}"),
            folder => format!("{dir}/{folder}/{stem}"),
        };
        // A name is taken with its extension: a mesh and a skeleton may share a stem.
        let numbered = |base: String, extension: String| {
            (1usize..).map(move |n| match n {
                1 => format!("{base}.{extension}"),
                n => format!("{base}-{n}.{extension}"),
            })
        };
        // A stem that is no file name on some system gives way to the hash.
        let by_name = numbered(base(&stem), extension.clone()).take(4096);
        let by_hash = numbered(base(&hashed), "dat".to_string());
        let place = self.take(by_name.chain(by_hash));
        place.unwrap_or_else(|| panic!("no path under {dir} for the file {:016x}", file.hash.0))
    }

    /// The path of the bin of a unit's carried objects, in the unit's directory.
    pub fn objects_bin(dir: &str) -> Option<ChunkPath> {
        ChunkPath::parse(&format!("{dir}/vfx.bin")).ok()
    }

    /// The path of the bin of a unit's graph, in the unit's directory.
    pub fn graph_bin(dir: &str) -> Option<ChunkPath> {
        ChunkPath::parse(&format!("{dir}/animations.bin")).ok()
    }

    /// The path, in its layer, of the patch of a unit's skin bin.
    pub fn skin_patch(character: &str, target: Slot) -> Option<ChunkPath> {
        ChunkPath::parse(&format!("overrides/{character}-skin{target}.ptch")).ok()
    }

    /// The path, in a voice layer, of the layer's patch of a unit's skin bin (#ADR-0028).
    pub fn layer_patch(character: &str, target: Slot, layer: &Layer) -> Option<ChunkPath> {
        let name = layer.name();
        ChunkPath::parse(&format!("overrides/{character}-skin{target}-{name}.ptch")).ok()
    }

    /// A part of a bank unit's name: letters, digits and `_`, any other character as `_`.
    pub fn unit_part(text: &str) -> String {
        let kept = |c: char| match c {
            c if c.is_ascii_alphanumeric() || c == '_' => c,
            _ => '_',
        };
        text.chars().map(kept).collect()
    }

    /// The eight characters of the mod's id that keep a tag and a unit's name apart from
    /// every other mod's.
    pub fn tag_id(meta: &ModMeta) -> String {
        meta.mod_id.chars().take(8).collect()
    }

    /// The directory of a unit's banks: `<unit>/audio`, and `<unit>/audio/<xx_xx>` for the
    /// banks of a locale.
    pub fn audio_dir(dir: &str, locale: Option<&str>) -> String {
        match locale {
            Some(locale) => format!("{dir}/audio/{}", locale.to_ascii_lowercase()),
            None => format!("{dir}/audio"),
        }
    }

    /// Where a bank the port makes ships: `<audio dir>/<stem>_<part>`, kept as a name made.
    /// An events bank's name holds `_events` (#GAME-BEHAVIOR-0013). None for a name that is
    /// no path.
    pub fn place_bank(&mut self, audio_dir: &str, stem: &str, part: &str) -> Option<ChunkPath> {
        let path = ChunkPath::parse(&format!("{audio_dir}/{stem}_{part}")).ok()?;
        self.taken_paths.insert(path.as_str().to_string());
        self.made_path(&path);
        Some(path)
    }

    /// The path, in the HUD layer, of the patch of a champion's root bin.
    pub fn hud_patch(character: &str) -> Option<ChunkPath> {
        ChunkPath::parse(&format!("overrides/{character}-hud.ptch")).ok()
    }

    /// Where a spell icon or a HUD texture ships: `mods/<ns>/characters/<character>/hud/`,
    /// under its own file name (#ADR-0029). None for a name that is no path.
    pub fn place_hud(&self, character: &str, leaf: &str) -> Option<ChunkPath> {
        let dir = format!("mods/{}/characters/{character}/hud", self.namespace);
        ChunkPath::parse(&format!("{dir}/{leaf}")).ok()
    }

    /// Keeps a path the port made as a name its hash tables list.
    pub fn made_path(&mut self, path: &ChunkPath) {
        self.made.paths.insert(path.as_str().to_string());
    }
}

impl Dump for Layout {
    fn dump(&self, _: &DumpCx<'_>) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "layout {} paths={} entries={}",
            self.namespace,
            self.made.paths.len(),
            self.made.entries.len()
        );
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_stem_is_tidied_in_plain_steps() {
        let characters = vec!["katarina".to_string()];
        let tidy = |stem: &str| tidy_stem(stem, &characters);
        assert_eq!(tidy("katarina_skin59_blade.skins_katarina_skin59"), "blade");
        assert_eq!(tidy("glow.pie_c_12_19"), "glow");
        assert_eq!(tidy("glow.tftset7"), "glow");
        assert_eq!(tidy("glow.tft_set7"), "glow.tft_set7");
        assert_eq!(tidy("katarina_base_tx_cm"), "tx_cm");
        assert_eq!(tidy("garen_base_tx_cm"), "garen_base_tx_cm");
        assert_eq!(tidy("katarina_skin59_"), "katarina_skin59_");
        assert_eq!(tidy("body.v2"), "body.v2");
    }

    #[test]
    fn an_effects_folder_drops_the_characters_name() {
        let characters = vec!["katarina".to_string()];
        assert_eq!(folder_name("Katarina_BA_Tar", &characters), "ba_tar");
        assert_eq!(folder_name("Garen Q/Trail", &characters), "garen_q_trail");
    }
}
