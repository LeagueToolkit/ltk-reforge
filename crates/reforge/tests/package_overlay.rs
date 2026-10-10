//! `Overlay`: a package's patches applied to the game's bins as LTK Manager's overlay applies
//! them, over packages built by hand, and a PTCH's records as `PtchBuilder` writes them. The
//! game is `reforge_testkit::skins`, its base skin bin linking a bin of its own and its
//! skin 7 bin of a version the overlay does not take.

use ltk_meta::Bin;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::entity::{FileId, UnitId};
use reforge::model::known::field;
use reforge::model::{
    BinHash, ByteView, ChunkPath, NoNames, SkinEntry, Spelled, WadName, name_hash, path_hash,
};
use reforge::package::{
    Chunk, ChunkKey, Held, Layer, Origin, Outcome, Overlay, Package, Patch, PatchOf, ProjectConfig,
    PtchBuilder, Unnamed,
};
use reforge_testkit::MemoryGame;
use reforge_testkit::bins::{bin, entry, linked_bin, particle, resolver, text};
use reforge_testkit::facts::env;
use reforge_testkit::skins::{self, WAD, base_skin, skin_bin, skin7};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const SKIN7: &str = "Characters/Test/Skins/Skin7";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
/// A bin of the game the game's base skin bin links.
const SHARED_BIN: &str = "data/characters/test/shared.bin";
/// A bin the package ships.
const OWN_BIN: &str = "mods/synthetic/characters/test/skin0/vfx.bin";
const OWN_TEX: &str = "mods/synthetic/characters/test/skin0/files/body.tex";

/// A bin of these objects as `PROP` version 1 writes it: the header holds no list of links.
fn version_one(objects: Vec<ltk_meta::BinObject>) -> Vec<u8> {
    let whole = bin(objects);
    assert_eq!(whole[4..12], [3, 0, 0, 0, 0, 0, 0, 0], "version 3, no link");
    let mut bytes = b"PROP".to_vec();
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(&whole[12..]);
    bytes
}

fn game() -> MemoryGame {
    let mut game = skins::game();
    let base = vec![entry(SKIN0, &base_skin()), resolver(RESOURCES0)];
    game.insert(&skin_bin("test", 0), linked_bin(base, &[SHARED_BIN]));
    game.insert(SHARED_BIN, bin(vec![particle("Shared", skins::BASE_TEX)]));
    game.insert(
        &skin_bin("test", 7),
        version_one(vec![entry(SKIN7, &skin7())]),
    );
    game
}

fn game_env() -> Env {
    env(game(), NoHistory)
}

fn path(path: &str) -> ChunkPath {
    ChunkPath::parse(path).unwrap_or_else(|error| panic!("{path} is no path: {error}"))
}

fn package() -> Package {
    Package::new(ProjectConfig {
        name: "Synthetic".into(),
        display_name: Spelled::new("Synthetic"),
        version: "1.0.0".into(),
        description: Spelled::new("A package built by hand."),
        authors: Vec::new(),
        champions: vec!["Test".into()],
    })
}

/// A PTCH that sets the texture of a skin entry's mesh block.
fn retexture(entry_name: &str) -> PtchBuilder<'static> {
    let mut ptch = PtchBuilder::new(&NoNames);
    let fields = [field::SKIN_MESH_PROPERTIES, field::TEXTURE].map(name_hash);
    ptch.set(name_hash(entry_name), &fields, text(OWN_TEX))
        .expect("the port's own code names the fields");
    ptch
}

/// A patch of a unit's skin bin, at `at` in its layer.
fn patch(target: &str, at: &str, ptch: PtchBuilder<'_>, links: &[&str]) -> Patch {
    Patch {
        target: path(target),
        path: path(at),
        records: ptch.len(),
        bytes: ptch.bytes().into(),
        links: links.iter().map(|link| path(link)).collect(),
        of: PatchOf::Skin {
            unit: UnitId::new(0),
            entry: name_hash(SKIN0),
        },
    }
}

fn skin0_patch(ptch: PtchBuilder<'_>, links: &[&str]) -> Patch {
    patch(
        &skin_bin("test", 0),
        "overrides/test-skin0.ptch",
        ptch,
        links,
    )
}

#[test]
fn a_patch_applies_to_the_games_bin_which_loads_the_packages_bins_and_the_games() {
    let env = game_env();
    let mut package = package();
    let wad = WadName::parse(WAD).expect("a WAD name");
    let own = Chunk::new(
        ByteView::from(bin(vec![particle("Own", OWN_TEX)])),
        Origin::Kept(FileId::new(0)),
    );
    package.insert(Layer::Base, ChunkKey::new(wad, path(OWN_BIN)), own);
    package.patch(Layer::Base, skin0_patch(retexture(SKIN0), &[OWN_BIN]));

    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    assert!(overlay.manifests.is_empty(), "{:?}", overlay.manifests);
    let applied = overlay.of(&Layer::Base, 0).expect("the patch's outcome");
    assert_eq!(applied.wad.as_ref().map(WadName::as_str), Some(WAD));
    assert!(applied.game.is_some());
    let Outcome::Applied(after) = &applied.outcome else {
        panic!("the patch applies: {:?}", applied.outcome);
    };
    assert!(after.skipped.is_empty(), "{:?}", after.skipped);
    // The entry is the first object of the game's bin, and its mesh block is what changed.
    assert_eq!(
        after.changed,
        [(0, vec![name_hash(field::SKIN_MESH_PROPERTIES)])]
    );
    let texture = SkinEntry::read(&after.objects[0]).texture();
    assert_eq!(texture.map(|file| file.hash()), Some(path_hash(OWN_TEX)));
    assert_eq!(after.dependencies, [SHARED_BIN, OWN_BIN]);

    let loaded = after.loaded.as_ref().expect("what a base patch loads");
    let held = |name: &str| loaded.objects.get(&name_hash(name));
    assert!(
        matches!(held(SKIN0), Some(Held::Package(_))),
        "the patched bin"
    );
    assert!(
        matches!(held("Own"), Some(Held::Package(_))),
        "the package's bin"
    );
    assert!(
        matches!(held("Shared"), Some(Held::Game(_, 0))),
        "the game's bin"
    );
    let shared = held("Shared").map(Held::object);
    assert_eq!(shared, Some(&particle("Shared", skins::BASE_TEX)));
    assert_eq!(
        loaded.shipped.iter().collect::<Vec<_>>(),
        [&name_hash("Own")]
    );
    assert!(loaded.missing_links.is_empty());
}

#[test]
fn a_patch_of_a_bin_the_game_lacks_has_no_game_bin() {
    let env = game_env();
    let mut package = package();
    let target = skin_bin("test", 5);
    let lacking = patch(&target, "overrides/test-skin5.ptch", retexture(SKIN0), &[]);
    package.patch(Layer::Base, lacking);
    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    let applied = overlay.of(&Layer::Base, 0).expect("the patch's outcome");
    assert!(
        matches!(applied.outcome, Outcome::NoGameBin),
        "{:?}",
        applied.outcome
    );
    assert!(applied.game.is_none() && applied.wad.is_none());
}

#[test]
fn a_bin_the_overlay_does_not_take_fails_with_the_overlays_reason() {
    let env = game_env();
    let mut package = package();
    let target = skin_bin("test", 7);
    let refused = patch(&target, "overrides/test-skin7.ptch", retexture(SKIN7), &[]);
    package.patch(Layer::Base, refused);
    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    let applied = overlay.of(&Layer::Base, 0).expect("the patch's outcome");
    // The game's bin reads, and the overlay takes a `PROP` of version 2 or 3 only.
    assert!(applied.game.is_some());
    let Outcome::Failed { reason } = &applied.outcome else {
        panic!("the overlay refuses the bin: {:?}", applied.outcome);
    };
    assert!(!reason.is_empty());
}

#[test]
fn a_record_on_an_entry_the_bin_lacks_is_skipped_and_the_others_apply() {
    let env = game_env();
    let mut package = package();
    let mut ptch = retexture("Characters/Test/Skins/Skin99");
    let fields = [name_hash(field::CHAMPION_SKIN_NAME)];
    ptch.set(name_hash(SKIN0), &fields, text("Renamed"))
        .expect("the field has a name");
    package.patch(Layer::Base, skin0_patch(ptch, &[]));
    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    let Some(Outcome::Applied(after)) = overlay.of(&Layer::Base, 0).map(|p| &p.outcome) else {
        panic!("the patch applies: {overlay:?}");
    };
    assert_eq!(after.skipped.len(), 1, "{:?}", after.skipped);
    assert!(
        after.skipped[0].starts_with("OverrideRecordSkipped"),
        "{:?}",
        after.skipped
    );
    assert_eq!(after.changed, [(0, fields.to_vec())]);
}

#[test]
fn a_link_a_patch_adds_that_leads_to_no_bin_is_a_missing_link() {
    let env = game_env();
    let mut package = package();
    package.patch(Layer::Base, skin0_patch(retexture(SKIN0), &[OWN_BIN]));
    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    let Some(Outcome::Applied(after)) = overlay.of(&Layer::Base, 0).map(|p| &p.outcome) else {
        panic!("the patch applies: {overlay:?}");
    };
    let loaded = after.loaded.as_ref().expect("what a base patch loads");
    assert_eq!(loaded.missing_links, [OWN_BIN]);
}

#[test]
fn a_manifest_the_overlay_does_not_load_is_held_with_the_reason_and_no_patch_applies() {
    let env = game_env();
    let mut package = package();
    // The overlay takes an override file by its `.ptch` extension.
    let misnamed = patch(
        &skin_bin("test", 0),
        "overrides/test-skin0.bin",
        retexture(SKIN0),
        &[],
    );
    package.patch(Layer::Base, misnamed);
    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    let [(layer, reason)] = &overlay.manifests[..] else {
        panic!("one manifest does not load: {:?}", overlay.manifests);
    };
    assert_eq!(layer, &Layer::Base);
    assert!(!reason.is_empty());
    assert!(overlay.patches.is_empty());
}

#[test]
fn another_layers_patch_applies_over_the_base_layers_result() {
    let env = game_env();
    let mut package = package();
    package.patch(Layer::Base, skin0_patch(retexture(SKIN0), &[]));
    let mut renamed = PtchBuilder::new(&NoNames);
    let name = [name_hash(field::CHAMPION_SKIN_NAME)];
    renamed
        .set(name_hash(SKIN0), &name, text("Voiced"))
        .expect("the field has a name");
    let voice = Layer::Voice(None);
    let over = patch(
        &skin_bin("test", 0),
        "overrides/test-skin0-voice.ptch",
        renamed,
        &[],
    );
    package.patch(voice.clone(), over);

    let overlay = Overlay::apply(&package, env.game()).expect("the game reads");
    let layers: Vec<&Layer> = overlay.patches.iter().map(|patch| &patch.layer).collect();
    assert_eq!(layers, [&Layer::Base, &voice]);
    let Some(Outcome::Applied(after)) = overlay.of(&voice, 0).map(|p| &p.outcome) else {
        panic!("the layer's patch applies: {overlay:?}");
    };
    // The base layer's texture stands under the layer's name, and only the name changed.
    let patched = SkinEntry::read(&after.objects[0]);
    let texture = patched.texture();
    assert_eq!(texture.map(|file| file.hash()), Some(path_hash(OWN_TEX)));
    assert_eq!(after.changed, [(0, name.to_vec())]);
    assert!(after.loaded.is_none(), "only a base patch is loaded");
}

/// #GAME-BEHAVIOR-0010: a record's path is plain names, each hashed lowercased, so a field
/// no table names is not written.
#[test]
fn a_records_path_is_spelled_by_names_and_a_field_no_table_names_is_refused() {
    let mut ptch = PtchBuilder::new(&NoNames);
    let unnamed = BinHash(0x1234_5678);
    let fields = [name_hash(field::SKIN_MESH_PROPERTIES), unnamed];
    assert_eq!(
        ptch.set(name_hash(SKIN0), &fields, text(OWN_TEX)),
        Err(Unnamed(unnamed))
    );
    assert!(ptch.is_empty());

    // The names as the port's code spells them, in their own case, find the field.
    let named = [field::SKIN_MESH_PROPERTIES, field::TEXTURE].map(name_hash);
    let spelled = ptch.path(&named).expect("both fields have names");
    assert_eq!(spelled.to_string(), "skinMeshProperties.texture");
    ptch.set(name_hash(SKIN0), &named, text(OWN_TEX))
        .expect("both fields have names");
    let mut patched = Bin::new([entry(SKIN0, &base_skin())], Vec::<String>::new());
    let report = ptch.build().apply(&mut patched);
    assert_eq!((report.applied, report.skipped.len()), (1, 0));
    let texture = SkinEntry::read(&patched.objects[&name_hash(SKIN0)]).texture();
    assert_eq!(texture.map(|file| file.hash()), Some(path_hash(OWN_TEX)));
}
