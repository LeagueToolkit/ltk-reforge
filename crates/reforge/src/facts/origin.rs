//! Which game skin a unit's content derives from, its origin (#ADR-0013). The evidence is
//! gathered per slot, a chroma counted as its parent, and the verdict is read from it:
//!
//! - strong, by name: the resolver, the graph, the skin name, the mesh, skeleton or texture,
//!   or an audio tag names another skin. A name of the unit's own slot says nothing;
//! - strong, by closeness: the graph or the resolver has strictly fewer edits against one
//!   skin's than against any other's;
//! - weak: the unit's game objects, or the game files its WAD ships, mostly come from one
//!   skin.
//!
//! Each side wins only where it leads strictly. Borrowed material is no evidence
//! (#MOD-ARTIFACT-0003): another champion's folder never counts, and effects files do not
//! count among the files.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ltk_meta::{BinObject, PropertyValueEnum};
use serde::{Deserialize, Serialize};

use super::copies::Copies;
use super::findings::{OriginAmbiguous, OriginConflicting, OriginNone};
use super::objects::ObjectProvenance;
use super::{Catalogs, load_pool, pooled};
use crate::diag::Sink;
use crate::fixups::FixedMod;
use crate::game::{CatalogSkin, SkinCatalog};
use crate::load::FileProvenance;
use crate::model::entity::{Mod, SkinUnit, UnitId};
use crate::model::hash::skin_paths;
use crate::model::known::field;
use crate::model::{
    BinHash, SideTable, SkinEntry, Slot, Spell, Spelled, WadHash, name_hash, value,
};

/// How much a piece of evidence weighs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weight {
    /// It decides only where no strong evidence does.
    Weak,
    /// A name or a closeness that points at one skin.
    Strong,
}

impl Weight {
    /// The weight's name in a dump.
    pub fn as_str(self) -> &'static str {
        match self {
            Weight::Weak => "weak",
            Weight::Strong => "strong",
        }
    }
}

/// What a piece of evidence is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Aspect {
    /// The resolver the entry names.
    Resolver,
    /// The graph the entry names.
    Graph,
    /// The entry's skin name.
    SkinName,
    /// The mesh file.
    Mesh,
    /// The skeleton file.
    Skeleton,
    /// The mesh's texture file.
    Texture,
    /// A tag of the audio block.
    AudioTag,
    /// The graph's clips against each skin's.
    GraphCloseness,
    /// The resolver's keys against each skin's.
    ResolverCloseness,
    /// The game objects the unit loads.
    Objects,
    /// The game files the WAD ships.
    Files,
}

impl Aspect {
    /// The aspect's name in a dump and a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Aspect::Resolver => "resolver",
            Aspect::Graph => "graph",
            Aspect::SkinName => "skin-name",
            Aspect::Mesh => "mesh",
            Aspect::Skeleton => "skeleton",
            Aspect::Texture => "texture",
            Aspect::AudioTag => "audio-tag",
            Aspect::GraphCloseness => "graph-closeness",
            Aspect::ResolverCloseness => "resolver-closeness",
            Aspect::Objects => "objects",
            Aspect::Files => "files",
        }
    }
}

/// What was tallied for weak evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tallied {
    /// Game objects.
    Objects,
    /// Game files.
    Files,
}

impl Tallied {
    fn as_str(self) -> &'static str {
        match self {
            Tallied::Objects => "game objects",
            Tallied::Files => "game files",
        }
    }
}

/// What a piece of evidence rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Basis {
    /// A link that is a game skin's own, or names its slot.
    Link {
        /// The object named.
        #[serde(
            serialize_with = "super::entry_hex",
            deserialize_with = "super::hex_entry"
        )]
        entry: BinHash,
    },
    /// A skin name or an audio tag, as the entry spells it.
    Name {
        /// The text.
        text: Spelled,
    },
    /// The base skin's file.
    BaseFile,
    /// A premium skin's file.
    SkinFile,
    /// A file at a path under a skin's folder.
    SkinFolder {
        /// The slot the folder names: the skin itself, or a chroma of it.
        folder: Slot,
    },
    /// The fewest edits against one skin's map.
    Closer {
        /// The edits against that skin's.
        edits: u32,
        /// The edits against the base's, when the base has the map.
        base: Option<u32>,
    },
    /// Most of the unit's game material is one premium skin's.
    Mostly {
        /// What was counted.
        what: Tallied,
        /// How many are that skin's.
        count: u32,
        /// How many are the base's.
        base: u32,
    },
    /// The unit's game material is the base's.
    BaseOnly {
        /// What was counted.
        what: Tallied,
        /// How many are the base's.
        count: u32,
    },
}

/// One piece of evidence for a skin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// What it is read from.
    pub aspect: Aspect,
    /// The skin it points at, a chroma counted as its parent.
    pub slot: Slot,
    /// How much it weighs.
    pub weight: Weight,
    /// What it rests on.
    pub basis: Basis,
}

impl Evidence {
    /// What the evidence rests on, as one phrase. A name a mod spells is in it as written: a
    /// caller cleans the phrase before it prints it.
    pub fn detail(&self, names: &dyn Spell) -> String {
        let slot = self.slot;
        match &self.basis {
            Basis::Link { entry } => names.entry_or_hex(*entry),
            Basis::Name { text } => text.raw().to_string(),
            Basis::BaseFile => "the base skin's".to_string(),
            Basis::SkinFile => format!("skin {slot}'s"),
            Basis::SkinFolder { folder } => format!("a path under skin{folder}'s folder"),
            Basis::Closer { edits, base } => {
                let base = base.map_or("-".to_string(), |base| base.to_string());
                format!("{edits} edits against skin {slot}, {base} against the base")
            }
            Basis::Mostly { what, count, base } => {
                format!(
                    "{count} {} of skin {slot}, {base} of the base",
                    what.as_str()
                )
            }
            Basis::BaseOnly { what, count } => format!("{count} {} of the base", what.as_str()),
        }
    }
}

/// What was gathered of one unit's origin.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginEvidence {
    /// The evidence, one piece per aspect and skin, in the order found.
    pub items: Vec<Evidence>,
    /// Whether the mesh the entry names is a file of the mod's own bytes.
    pub own_mesh: bool,
}

/// The skin a unit's content derives from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", content = "slot", rename_all = "lowercase")]
pub enum Verdict {
    /// The base skin.
    Base,
    /// A premium skin.
    Premium(Slot),
    /// Nothing of the game's rig or identity: the author's own.
    Custom,
    /// No evidence decides.
    Unknown,
}

impl Verdict {
    /// The verdict as a dump writes it: `premium(3)`.
    pub fn text(self) -> String {
        match self {
            Verdict::Base => "base".to_string(),
            Verdict::Premium(slot) => format!("premium({slot})"),
            Verdict::Custom => "custom".to_string(),
            Verdict::Unknown => "unknown".to_string(),
        }
    }
}

/// A unit's origin: the verdict, and the evidence it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// The skin.
    #[serde(flatten)]
    pub verdict: Verdict,
    /// The weight of the evidence that decided. None when none did.
    pub strength: Option<Weight>,
    /// The premium skins that weigh the same as the best, the best first. Empty when one leads.
    pub ties: Vec<Slot>,
    /// The premium skin with strong evidence beside strong evidence for the base.
    pub conflicting: Option<Slot>,
    /// The evidence.
    pub evidence: OriginEvidence,
}

impl Origin {
    /// The verdict on gathered evidence. The premium skin with the most strong evidence wins
    /// where it leads the base strictly, else the base where it leads strictly; a tie on
    /// strong evidence is decided by the weak the same way. With no evidence at all and a mesh
    /// of the mod's own, the work is the author's.
    pub fn judge(evidence: OriginEvidence) -> Self {
        let mut strong: BTreeMap<Slot, u32> = BTreeMap::new();
        let mut weak: BTreeMap<Slot, u32> = BTreeMap::new();
        for item in &evidence.items {
            let tally = match item.weight {
                Weight::Strong => &mut strong,
                Weight::Weak => &mut weak,
            };
            *tally.entry(item.slot).or_default() += 1;
        }
        let of = |tally: &BTreeMap<Slot, u32>, slot: Slot| tally.get(&slot).copied().unwrap_or(0);
        let score = |slot: Slot| (of(&strong, slot), of(&weak, slot));
        let premium: BTreeSet<Slot> = strong
            .keys()
            .chain(weak.keys())
            .copied()
            .filter(|slot| !slot.is_base())
            .collect();
        // Of equal scores the lowest slot.
        let best = premium
            .iter()
            .copied()
            .max_by_key(|&slot| (score(slot), std::cmp::Reverse(slot)));
        let mut ties = Vec::new();
        let mut conflicting = None;
        if let Some(best) = best {
            let same = premium
                .iter()
                .copied()
                .filter(|&slot| slot != best && score(slot) == score(best));
            let same: Vec<Slot> = same.collect();
            if !same.is_empty() {
                ties.push(best);
                ties.extend(same);
            }
            if of(&strong, best) > 0 && of(&strong, Slot::BASE) > 0 {
                conflicting = Some(best);
            }
        }
        let base = score(Slot::BASE);
        let (verdict, strength) = match best.map(|best| (best, score(best))) {
            Some((best, (s, _))) if s > base.0 => (Verdict::Premium(best), Some(Weight::Strong)),
            Some((_, (s, _))) if s < base.0 => (Verdict::Base, Some(Weight::Strong)),
            None if base.0 > 0 => (Verdict::Base, Some(Weight::Strong)),
            Some((best, (_, w))) if w > base.1 => (Verdict::Premium(best), Some(Weight::Weak)),
            Some((_, (_, w))) if w < base.1 => (Verdict::Base, Some(Weight::Weak)),
            None if base.1 > 0 => (Verdict::Base, Some(Weight::Weak)),
            _ if evidence.own_mesh && evidence.items.is_empty() => (Verdict::Custom, None),
            _ => (Verdict::Unknown, None),
        };
        Self {
            verdict,
            strength,
            ties,
            conflicting,
            evidence,
        }
    }

    /// The premium skin, when the origin is one.
    pub fn premium(&self) -> Option<Slot> {
        match self.verdict {
            Verdict::Premium(slot) => Some(slot),
            _ => None,
        }
    }
}

/// A map compared key by key against another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Closeness {
    /// Keys with equal values.
    pub same: u32,
    /// Keys whose values are equal but for the files they name.
    pub repathed: u32,
    /// Keys with other values.
    pub changed: u32,
    /// Keys only ours has.
    pub added: u32,
    /// Keys only theirs has.
    pub removed: u32,
}

impl Closeness {
    /// The map at a field of two objects, compared. None when either holds no map there.
    pub fn of(ours: &BinObject, theirs: &BinObject, field: &str) -> Option<Self> {
        fn map<'a>(
            object: &'a BinObject,
            field: &str,
        ) -> Option<&'a [(PropertyValueEnum, PropertyValueEnum)]> {
            match value::get(object, &[field]) {
                Some(PropertyValueEnum::Map(map)) => Some(map.entries()),
                _ => None,
            }
        }
        let map = |object| map(object, field);
        let (ours, theirs) = (map(ours)?, map(theirs)?);
        let mut out = Self::default();
        for (key, value) in ours {
            match theirs.iter().find(|(other, _)| other == key) {
                None => out.added += 1,
                Some((_, other)) if other == value => out.same += 1,
                Some((_, other)) if value::without_paths(other) == value::without_paths(value) => {
                    out.repathed += 1
                }
                Some(_) => out.changed += 1,
            }
        }
        let kept = |key: &PropertyValueEnum| ours.iter().any(|(other, _)| other == key);
        out.removed = theirs.iter().filter(|(key, _)| !kept(key)).count() as u32;
        Some(out)
    }

    /// What differs beyond moved files.
    pub fn edits(&self) -> u32 {
        self.changed + self.added + self.removed
    }
}

/// Reads the object a game skin names: its resolver, its graph.
type LinkOf = fn(&CatalogSkin) -> Option<BinHash>;
/// Reads a file a game skin names: its mesh, its skeleton, its texture.
type FileOf = fn(&CatalogSkin) -> Option<WadHash>;

/// The evidence of one unit, as it is gathered.
struct Collector<'a> {
    catalog: &'a SkinCatalog,
    /// The slots whose names are the unit's own: its slot, the slot its entry's name spells,
    /// and for a spread the base, since a bin written over every slot is named from skin0.
    own: Vec<Slot>,
    /// The unit's character, and the characters of its group. Only their skin folders are
    /// evidence.
    character: &'a str,
    group: Vec<&'a str>,
    items: Vec<Evidence>,
}

impl Collector<'_> {
    fn push(&mut self, aspect: Aspect, slot: Slot, weight: Weight, basis: Basis) {
        let slot = self.catalog.canon(slot);
        if !self
            .items
            .iter()
            .any(|item| item.aspect == aspect && item.slot == slot)
        {
            self.items.push(Evidence {
                aspect,
                slot,
                weight,
                basis,
            });
        }
    }

    /// A link equal to game skins' own links, else naming a slot: evidence for that skin,
    /// unless the name is of one of the unit's own slots. Game skins link across slots, so
    /// the catalog is asked before the name. A link several skins share says nothing.
    fn link(
        &mut self,
        aspect: Aspect,
        link: BinHash,
        name_of: fn(&str, Slot) -> String,
        of_skin: LinkOf,
    ) {
        let named = |slot: &Slot| name_hash(&name_of(self.character, *slot)) == link;
        if self.own.iter().any(named) {
            return;
        }
        let skins = self.catalog.skins.values();
        let mut hits: Vec<Slot> = skins
            .filter(|skin| of_skin(skin) == Some(link))
            .map(|skin| skin.slot)
            .collect();
        if hits.is_empty() {
            hits = self.catalog.skins.keys().copied().filter(named).collect();
        }
        let canon: BTreeSet<Slot> = hits.iter().map(|slot| self.catalog.canon(*slot)).collect();
        if let (Some(&slot), 1) = (canon.first(), canon.len()) {
            self.push(aspect, slot, Weight::Strong, Basis::Link { entry: link });
        }
    }

    /// A rig file. `own` says the mod ships its own bytes at the path: at the base's path that
    /// is where any skin0 mod puts its file, and says nothing; at a premium skin's path the
    /// paste kept the path.
    fn file(
        &mut self,
        aspect: Aspect,
        file: WadHash,
        own: bool,
        spelled: Option<&str>,
        of_skin: FileOf,
    ) {
        let base = self.catalog.base().and_then(of_skin);
        let skins = || {
            self.catalog
                .skins
                .values()
                .filter(|skin| of_skin(skin) == Some(file))
        };
        if base == Some(file) {
            // The base's file counts for the base only where no premium skin uses it too.
            if !own && !skins().any(|skin| skin.canon != Slot::BASE) {
                self.push(aspect, Slot::BASE, Weight::Strong, Basis::BaseFile);
            }
            return;
        }
        let hits: Vec<Slot> = skins().map(|skin| skin.canon).collect();
        if let Some(&slot) = hits.first()
            && hits.iter().all(|hit| *hit == slot)
        {
            self.push(aspect, slot, Weight::Strong, Basis::SkinFile);
            return;
        }
        if let Some((character, slot)) = spelled.and_then(skin_paths::skin_folder)
            && self.group.contains(&character.as_str())
            && !self.own.contains(&slot)
        {
            self.push(
                aspect,
                slot,
                Weight::Strong,
                Basis::SkinFolder { folder: slot },
            );
        }
    }

    /// The slot with the most, where it leads the base strictly, is weak evidence. Of equal
    /// counts the lowest slot.
    fn tally(&mut self, aspect: Aspect, what: Tallied, counts: &BTreeMap<Slot, u32>) {
        let base = counts.get(&Slot::BASE).copied().unwrap_or(0);
        let premium = counts.iter().filter(|(slot, _)| !slot.is_base());
        let best = premium.max_by_key(|(slot, count)| (**count, std::cmp::Reverse(**slot)));
        match best {
            Some((&slot, &count)) if count > base => {
                self.push(
                    aspect,
                    slot,
                    Weight::Weak,
                    Basis::Mostly { what, count, base },
                );
            }
            _ if base > 0 => {
                self.push(
                    aspect,
                    Slot::BASE,
                    Weight::Weak,
                    Basis::BaseOnly { what, count: base },
                );
            }
            _ => {}
        }
    }
}

/// What the origins are found from.
pub struct OriginInputs<'a> {
    /// The game's skins of each character.
    pub catalogs: &'a Catalogs,
    /// The roles of the units.
    pub copies: &'a Copies,
    /// Whose bytes the files are.
    pub files: &'a FileProvenance,
    /// Whose the objects are.
    pub objects: &'a ObjectProvenance,
    /// Spells a file a bin names by its hash.
    pub names: &'a dyn Spell,
}

impl OriginInputs<'_> {
    /// The evidence of one unit's origin.
    fn gather(
        &self,
        m: &Mod,
        id: UnitId,
        unit: &SkinUnit,
        catalog: &SkinCatalog,
    ) -> OriginEvidence {
        let character = &m.characters[unit.character];
        let mut own = vec![unit.source_slot];
        own.extend(unit.entry_name_slot);
        if self.copies.is_spread(id) {
            own.push(Slot::BASE);
        }
        let group = m
            .groups
            .iter()
            .find(|group| group.characters.contains(&unit.character));
        let names = |g: &crate::model::entity::Group| {
            g.characters
                .iter()
                .map(|c| m.characters[*c].name.as_str())
                .collect()
        };
        let mut c = Collector {
            catalog,
            own,
            character: &character.name,
            group: group.map_or_else(|| vec![character.name.as_str()], names),
            items: Vec::new(),
        };
        let pool = load_pool(m, unit);
        let wad = m.files[unit.file].wad;
        let mut own_mesh = false;

        if let Some(object) = unit.entry.map(|entry| &m.objects[entry].value) {
            let entry = SkinEntry::read(object);
            if let Some(link) = entry.resolver() {
                c.link(Aspect::Resolver, link, skin_paths::resolver_entry, |skin| {
                    skin.resolver
                });
            }
            if let Some(link) = entry.graph() {
                c.link(Aspect::Graph, link, skin_paths::graph_entry, |skin| {
                    skin.graph
                });
            }
            if let Some(name) = entry.skin_name() {
                let same = |skin: &&CatalogSkin| {
                    skin.name
                        .as_deref()
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                };
                let first = catalog.skins.values().find(same).map(|skin| skin.canon);
                if let Some(slot) = first.filter(|slot| !c.own.contains(slot)) {
                    c.push(
                        Aspect::SkinName,
                        slot,
                        Weight::Strong,
                        Basis::Name {
                            text: Spelled::new(name),
                        },
                    );
                }
            }
            let files: [(Aspect, Option<value::FileRef>, FileOf); 3] = [
                (Aspect::Mesh, entry.mesh(), |skin| skin.mesh),
                (Aspect::Skeleton, entry.skeleton(), |skin| skin.skeleton),
                (Aspect::Texture, entry.texture(), |skin| skin.texture),
            ];
            for (aspect, file, of_skin) in files {
                let Some(file) = file else {
                    continue;
                };
                // A shipped copy of a game file counts for the path its bytes sit at.
                let shipped = m.file_at(wad, file.hash());
                let at = shipped.and_then(|f| self.files.game_paths(f).first().copied());
                let own = shipped.is_some_and(|f| !self.files.is_game(f));
                own_mesh |= own && aspect == Aspect::Mesh;
                let spelled = file
                    .spelled()
                    .map(str::to_string)
                    .or_else(|| self.names.path(file.hash()));
                c.file(
                    aspect,
                    at.unwrap_or(file.hash()),
                    own,
                    spelled.as_deref(),
                    of_skin,
                );
            }
            for tag in entry.audio_tags() {
                let lower = tag.to_ascii_lowercase();
                let number = lower
                    .strip_prefix(character.name.as_str())
                    .and_then(|rest| rest.strip_prefix("skin"));
                if let Some(slot) = number.and_then(|n| n.parse().ok()).map(Slot)
                    && !c.own.contains(&slot)
                    && catalog.skins.contains_key(&slot)
                {
                    c.push(
                        Aspect::AudioTag,
                        slot,
                        Weight::Strong,
                        Basis::Name {
                            text: Spelled::new(tag),
                        },
                    );
                }
            }

            // The unit's own graph and resolver against each game skin's.
            let maps: [(Aspect, Option<BinHash>, &str, LinkOf); 2] = [
                (
                    Aspect::GraphCloseness,
                    entry.graph(),
                    field::CLIP_DATA_MAP,
                    |skin| skin.graph,
                ),
                (
                    Aspect::ResolverCloseness,
                    entry.resolver(),
                    field::RESOURCE_MAP,
                    |skin| skin.resolver,
                ),
            ];
            for (aspect, link, field, of_skin) in maps {
                // A bin the unit's bins do not link is not loaded: its objects are no evidence.
                let Some(ours) = link.and_then(|link| pooled(m, &pool, link)) else {
                    continue;
                };
                let ours = &m.objects[ours].value;
                let mut fewest: BTreeMap<Slot, u32> = BTreeMap::new();
                for skin in catalog.skins.values() {
                    let theirs = of_skin(skin).and_then(|entry| catalog.object(entry));
                    if let Some(closeness) =
                        theirs.and_then(|theirs| Closeness::of(ours, theirs, field))
                    {
                        let edits = fewest.entry(skin.canon).or_insert(u32::MAX);
                        *edits = (*edits).min(closeness.edits());
                    }
                }
                let base = fewest.get(&Slot::BASE).copied();
                let Some(min) = fewest.values().copied().min() else {
                    continue;
                };
                let mut closest = fewest
                    .iter()
                    .filter(|(_, edits)| **edits == min)
                    .map(|(slot, _)| *slot);
                if let (Some(slot), None) = (closest.next(), closest.next())
                    && (base.is_none() || slot.is_base() || base > Some(min))
                {
                    c.push(
                        aspect,
                        slot,
                        Weight::Strong,
                        Basis::Closer { edits: min, base },
                    );
                }
            }
        }

        let mut by_slot: BTreeMap<Slot, u32> = BTreeMap::new();
        for &object in &pool {
            if let Some(slot) = self.objects.origin(object).and_then(|origin| origin.slot()) {
                *by_slot.entry(catalog.canon(slot)).or_default() += 1;
            }
        }
        c.tally(Aspect::Objects, Tallied::Objects, &by_slot);
        let mut by_folder: BTreeMap<Slot, u32> = BTreeMap::new();
        for &file in group.map_or(&[][..], |group| &m.wads[group.wad].files) {
            if let Some(fact) = self.files.fact(file)
                && fact.is_game()
                && !fact.vfx
                && let Some(folder) = &fact.skin_folder
                && c.group.contains(&folder.character.as_str())
                && catalog.skins.contains_key(&folder.slot)
            {
                *by_folder.entry(catalog.canon(folder.slot)).or_default() += 1;
            }
        }
        c.tally(Aspect::Files, Tallied::Files, &by_folder);
        OriginEvidence {
            items: c.items,
            own_mesh,
        }
    }
}

/// The origin of each representative unit.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Origins {
    units: SideTable<SkinUnit, Origin>,
}

impl Origins {
    /// The origins of a mod's representative units: the evidence gathered, then judged.
    pub fn find(fixed: &FixedMod, inputs: &OriginInputs<'_>) -> Self {
        let m: &Mod = fixed;
        let mut out = Self::default();
        for id in inputs.copies.representatives() {
            let unit = &m.units[id];
            let Some(catalog) = inputs.catalogs.of(unit.character) else {
                continue;
            };
            let evidence = inputs.gather(m, id, unit, catalog);
            out.units.insert(id, Origin::judge(evidence));
        }
        out
    }

    /// A unit's origin. None for a copy.
    pub fn of(&self, unit: UnitId) -> Option<&Origin> {
        self.units.get(unit)
    }

    /// Every representative with its origin, in unit order.
    pub fn iter(&self) -> impl Iterator<Item = (UnitId, &Origin)> {
        self.units.iter()
    }

    /// The premium skin a unit derives from, when its origin is one.
    pub fn premium(&self, unit: UnitId) -> Option<Slot> {
        self.of(unit).and_then(Origin::premium)
    }

    /// Raises what the verdicts leave open: a tie between premium skins, strong evidence on
    /// both sides, no evidence that decides.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for (unit, origin) in self.iter() {
            if !origin.ties.is_empty() {
                sink.raise(OriginAmbiguous {
                    unit,
                    slots: origin.ties.clone(),
                });
            }
            if let Some(slot) = origin.conflicting {
                sink.raise(OriginConflicting { unit, slot });
            }
            if origin.verdict == Verdict::Unknown {
                sink.raise(OriginNone { unit });
            }
        }
    }

    /// The origins as text, each with its evidence.
    pub fn dump_text(&self, m: &Mod, names: &dyn Spell) -> String {
        let mut out = String::new();
        for (unit, origin) in self.iter() {
            let strength = origin
                .strength
                .map_or(String::new(), |weight| format!(" {}", weight.as_str()));
            let _ = writeln!(
                out,
                "origin {unit} {} = {}{strength}",
                m.unit_name(unit),
                origin.verdict.text()
            );
            for item in &origin.evidence.items {
                let detail = item.detail(names);
                let _ = writeln!(
                    out,
                    "   {:6} {:18} -> {}  {}",
                    item.weight.as_str(),
                    item.aspect.as_str(),
                    item.slot,
                    crate::model::clean(&detail)
                );
            }
        }
        out
    }
}
