//! A repair's package: the files of the mod the client reaches, at their own paths, a bin
//! written again where a fixup changed an object of it, every other file with the bytes it
//! has. The game is `reforge_testkit::skins`.

use ltk_meta::property::values;
use ltk_meta::{PropertyKind, PropertyValueEnum};
use reforge::diag::{Cancel, Cancelled, Diagnostics, Stage};
use reforge::env::Env;
use reforge::fixups::{FixInputs, FixupSet, ObjectSet, Rule};
use reforge::game::{NameBook, Schema};
use reforge::load::{Dating, Fault, LoadOptions};
use reforge::model::entity::ModMeta;
use reforge::model::{ContentId, Dump, DumpCx, Mod, NoNames, Spelled, bin, name_hash, path_hash};
use reforge::package::dds_to_tex;
use reforge::package::{
    BinWrite, BuildError, CheckSelection, CheckSet, Empty, FieldUndeclared, InPlace,
    InPlaceBuilder, Layer, LeftBehind, Made, Origin, ProjectConfig, RoundTripFailed, TexRefusal,
    TextureContained, TypeMismatch,
};
use reforge_testkit::bins::{
    Skin, bin as bin_of, blob, entry, float, linked_bin, object, particle, text,
};
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::schema::{field, kind};
use reforge_testkit::skins::{
    self, BASE_PARTICLE_TEX, BASE_TEX, S3_PARTICLE_TEX, S3_TEX, WAD, base_skin, skin_bin,
};
use reforge_testkit::{BrokenHistory, FixRun, MemoryHistory, ModArchive, PackRun, SchemaRows};

const ENTRY: &str = "Characters/Test/Skins/Skin0";
const MESH: &str = "assets/mod/a/mesh.skn";
const SHARED: &str = "data/mod/a/shared.bin";
const KEPT_TEX: &str = "assets/mod/a/kept.tex";
/// A texture only a value the fixup removes names.
const OLD_TEX: &str = "assets/mod/a/old.tex";
/// A texture nothing names.
const UNUSED_TEX: &str = "assets/mod/a/unused.tex";
/// A texture an object names by its hash.
const LINKED_TEX: &str = "assets/mod/a/linked.tex";

/// A target whose `Material` holds a count, a texture and a texture by its hash, and nothing
/// called `junk` or `oldTexture`.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            "Material",
            &[
                field("count", kind("U8")),
                field("textureName", kind("String")),
                field("textureLink", kind("File")),
            ],
        )
        .build()
}

fn env() -> Env {
    Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .build()
        .expect("an environment")
}

/// One fixup: the two fields the target's `Material` does not declare, removed.
fn set() -> FixupSet {
    FixupSet::empty().with(Rule::remove_fields("Material", ["junk", "oldTexture"]).id("no-junk"))
}

fn own_skin() -> Skin {
    Skin {
        mesh: MESH.into(),
        ..base_skin()
    }
}

fn material() -> ltk_meta::BinObject {
    object(
        "M",
        "Material",
        vec![
            ("junk", float(1.0)),
            ("oldTexture", text(OLD_TEX)),
            ("textureName", text(KEPT_TEX)),
            (
                "textureLink",
                values::WadChunkLink::new(path_hash(LINKED_TEX)).into(),
            ),
            ("count", values::U8::new(2).into()),
        ],
    )
}

fn skin_bytes() -> Vec<u8> {
    let objects = vec![
        entry(ENTRY, &own_skin()),
        material(),
        particle("P", "assets/mod/a/particle.tex"),
    ];
    linked_bin(objects, &[SHARED])
}

fn shared_bytes() -> Vec<u8> {
    linked_bin(vec![particle("Q", "assets/mod/a/shared.tex")], &[])
}

/// A skin bin with a material the fixup changes and a particle it does not, a bin it links
/// that no fixup changes, and the files they name, one of them only through a value the
/// fixup removes, and a file nothing names.
fn the_mod() -> ModArchive {
    ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_bytes())
        .file(WAD, SHARED, shared_bytes())
        .file(WAD, MESH, blob(MESH))
        .file(WAD, KEPT_TEX, blob(KEPT_TEX))
        .file(WAD, OLD_TEX, blob(OLD_TEX))
        .file(WAD, LINKED_TEX, blob(LINKED_TEX))
        .file(WAD, UNUSED_TEX, blob(UNUSED_TEX))
}

#[test]
fn a_repair_ships_the_files_the_client_uses_at_their_own_paths() {
    let env = env();
    let run = PackRun::of(&env, &set(), &the_mod());
    let package = run.package();
    let mut paths: Vec<String> = package
        .chunks()
        .map(|(layer, key, _)| {
            assert_eq!((layer, key.wad.as_str()), (&Layer::Base, WAD));
            key.path.to_string()
        })
        .collect();
    paths.sort();
    let mut expected = [
        skin_bin("test", 0).as_str(),
        SHARED,
        MESH,
        KEPT_TEX,
        LINKED_TEX,
    ]
    .map(str::to_string);
    expected.sort();
    assert_eq!(paths, expected);
    // The file only the removed value named stays behind, and so does the one nothing names.
    assert!(!run.ships(OLD_TEX) && !run.ships(UNUSED_TEX));
    assert_eq!(
        run.only::<InPlace>(),
        &InPlace {
            kept: 5,
            rewritten: 1,
            whole: 0,
            made: 0,
            left: LeftBehind {
                unused: 1,
                orphaned: 1,
                ..LeftBehind::default()
            },
            unnamed: 0,
        }
    );
    assert_eq!(package.config.name, "Synthetic-reforgetests");
    assert_eq!(package.config.version, "1.0.0");
    assert_eq!(package.config.champions, ["test"]);
    assert!(
        run.messages().contains(
            &"package.in-place 5 files kept at their paths, 1 bin written again; 2 files left \
              out: 1 that nothing loads or names, 1 that only removed values named"
                .to_string()
        ),
        "{:#?}",
        run.messages()
    );
}

#[test]
fn a_bin_with_a_changed_object_is_written_again_and_its_other_objects_keep_their_bytes() {
    let env = env();
    let run = PackRun::of(&env, &set(), &the_mod());
    let chunk = run.chunk(&skin_bin("test", 0));
    let Origin::Rewritten { written, .. } = &chunk.origin else {
        panic!("the skin bin is written again: {:?}", chunk.origin);
    };
    assert_eq!(written.how, BinWrite::Delta);
    assert_eq!(written.entries.len(), 3);
    assert_eq!(written.edited.len(), 1);
    assert_ne!(&*chunk.bytes, skin_bytes().as_slice());

    let before = bin::parse(&skin_bytes()).expect("the archive's bin reads");
    let after = bin::parse(&chunk.bytes).expect("the bin written again reads");
    assert_eq!(after.dependencies, [SHARED]);
    let raw = |parsed: &bin::ParsedBin, name: &str| {
        let mut objects = parsed.objects.iter();
        let found = objects.find(|object| object.value.path_hash == name_hash(name));
        found.expect("the bin holds the object").raw
    };
    assert_eq!(raw(&after, ENTRY), raw(&before, ENTRY));
    assert_eq!(raw(&after, "P"), raw(&before, "P"));
    assert_ne!(raw(&after, "M"), raw(&before, "M"));
    let material = &after.objects[1].value;
    assert_eq!(material, run.fix.object("M"));
    assert!(!material.properties.contains_key(&name_hash("junk")));
    assert!(material.properties.contains_key(&name_hash("count")));
}

#[test]
fn a_bin_none_of_whose_objects_changed_keeps_its_bytes() {
    let env = env();
    let run = PackRun::of(&env, &set(), &the_mod());
    let chunk = run.chunk(SHARED);
    assert!(
        matches!(chunk.origin, Origin::Kept(_)),
        "{:?}",
        chunk.origin
    );
    assert_eq!(&*chunk.bytes, shared_bytes().as_slice());
    // Every other file too.
    assert_eq!(&*run.chunk(MESH).bytes, blob(MESH).as_slice());
}

#[test]
fn a_repaired_mod_repaired_again_changes_nothing() {
    let env = env();
    let first = PackRun::of(&env, &set(), &the_mod());
    let second = PackRun::of(&env, &set(), &first.archive());
    assert!(second.fix.changes.is_empty(), "{:?}", second.fix.changes);
    let chunks = |run: &PackRun<'_>| -> Vec<(String, ContentId)> {
        let chunks = run.package().chunks();
        chunks
            .map(|(_, key, chunk)| (key.path.to_string(), ContentId::of(&chunk.bytes)))
            .collect()
    };
    assert_eq!(chunks(&second), chunks(&first));
    let origins = second.package().chunks().map(|(_, _, chunk)| &chunk.origin);
    assert!(
        origins
            .into_iter()
            .all(|origin| matches!(origin, Origin::Kept(_)))
    );
    let found = second.only::<InPlace>();
    assert_eq!((found.kept, found.rewritten, found.left.total()), (5, 0, 0));
}

/// #MOD-ARTIFACT-0007: a chunk named by its path hash, with no path any table spells.
#[test]
fn a_file_no_table_names_goes_under_its_hash() {
    let env = env();
    // A chunk named by its hash, at a path the game ships and no bin of the mod spells.
    let hash = path_hash(S3_TEX);
    let hashed = format!("{:016x}.tex", hash.0);
    let archive = the_mod().file(WAD, &hashed, blob("a texture under its hash"));
    let run = PackRun::of(&env, &set(), &archive);
    let mut chunks = run.package().chunks();
    let (_, key, chunk) = chunks
        .find(|(_, key, _)| key.hash() == hash)
        .expect("the chunk is shipped");
    // The extension is the kind the bytes are of, and these are of none.
    assert_eq!(key.path.as_str(), format!("{:016x}.dat", hash.0));
    assert_ne!(key.path.hash(), hash);
    assert_eq!(&*chunk.bytes, blob("a texture under its hash").as_slice());
    assert_eq!(run.only::<InPlace>().unnamed, 1);
}

#[test]
fn the_package_names_the_files_its_bins_name_by_hash() {
    let env = env();
    let run = PackRun::of(&env, &set(), &the_mod());
    let paths: Vec<&str> = run.package().tables.paths().collect();
    assert_eq!(paths, [LINKED_TEX]);
    assert_eq!(run.package().tables.entries().count(), 0);
}

/// #GAME-BEHAVIOR-0003: a DDS that TEX's container does not hold stays a DDS at its `.tex`
/// path.
#[test]
fn a_dds_at_a_tex_path_goes_into_texs_container_with_its_blocks() {
    let env = env();
    let whole = dds(DdsPixels::Dxt1, (8, 8), 4);
    let refused = dds(DdsPixels::Dxt3, (8, 8), 1);
    let at_dds = dds(DdsPixels::Dxt1, (8, 8), 1);
    // Paths the game ships: each file is an override.
    let archive = the_mod()
        .file(WAD, BASE_TEX, whole.clone())
        .file(WAD, S3_TEX, refused.clone())
        .file(WAD, S3_PARTICLE_TEX, at_dds.clone());
    let run = PackRun::of(&env, &set(), &archive);

    let contained = run.chunk(BASE_TEX);
    assert!(matches!(contained.origin, Origin::Contained(_)));
    assert_eq!(&contained.bytes[..4], b"TEX\0");
    assert_eq!(contained.bytes.len(), 12 + (whole.len() - 128));
    // A format TEX does not hold stays the DDS it is, and so does a DDS at a `.dds` path.
    assert_eq!(&*run.chunk(S3_TEX).bytes, refused.as_slice());
    assert!(matches!(run.chunk(S3_TEX).origin, Origin::Kept(_)));
    assert_eq!(&*run.chunk(S3_PARTICLE_TEX).bytes, at_dds.as_slice());
    let found = run.only::<TextureContained>();
    assert_eq!(found.contained, 1);
    assert_eq!(
        found.kept.iter().collect::<Vec<_>>(),
        [(&TexRefusal::Format, &1)]
    );
    assert!(
        run.messages().contains(
            &"package.texture-contained 1 DDS texture at .tex paths put in TEX's container, \
              their blocks as they are; kept as DDS, which the client reads too: 1 a format \
              TEX does not hold as it is"
                .to_string()
        ),
        "{:#?}",
        run.messages()
    );
}

#[test]
fn a_package_that_would_hold_nothing_ends_the_run() {
    let env = env();
    let meta = ModMeta {
        file_name: Spelled::new("empty.fantome"),
        sha256: ContentId([0; 32]),
        mod_id: "000000000000".to_string(),
        name: None,
        author: None,
        version: None,
        description: None,
    };
    let mut log = Diagnostics::new();
    let mut names = NameBook::new();
    let empty = Mod::new(meta);
    let dating = Dating::find(&empty, env.history()).expect("no history answers");
    let mut inputs = FixInputs {
        env: &env,
        dating: &dating,
        names: &mut names,
    };
    let (fixed, changes) = FixupSet::empty()
        .apply(
            empty,
            &ObjectSet::every(),
            &mut inputs,
            &mut log.at(Stage::Fix),
        )
        .expect("the fixups apply");
    let built = InPlaceBuilder::new(&fixed, &changes).build(&mut log.at(Stage::Build));
    let Err(BuildError::Halted(halted)) = built else {
        panic!("nothing to build");
    };
    assert_eq!(log.halted(), Some(halted));
    assert_eq!(log.of::<Empty>().next(), Some(&Empty { files: 0 }));
}

#[test]
fn a_bin_of_the_old_numbering_is_written_whole() {
    let env = env();
    // A material the fixup changes, with a null pointer under the kind byte the old
    // numbering has for a pointer: the kinds after `Hash` sat one lower before
    // `WadChunkLink` took a number among them.
    let null = PropertyValueEnum::Struct(values::Struct::default());
    let material = object("M", "Material", vec![("junk", float(1.0)), ("child", null)]);
    let mut old = bin_of(vec![material]);
    let pointer: u8 = PropertyKind::Struct.into();
    let mut field = name_hash("child").0.to_le_bytes().to_vec();
    field.push(pointer);
    let at = old.windows(field.len()).position(|bytes| bytes == field);
    old[at.expect("the field and its kind") + 4] = 19;
    let skin = linked_bin(vec![entry(ENTRY, &own_skin())], &[SHARED]);
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin)
        .file(WAD, SHARED, old.clone())
        .file(WAD, MESH, blob(MESH));
    let mut run = PackRun::of(&env, &set(), &archive);

    let chunk = run.chunk(SHARED);
    let Origin::Rewritten { written, .. } = &chunk.origin else {
        panic!("the bin is written again: {:?}", chunk.origin);
    };
    assert_eq!(written.how, BinWrite::Whole);
    let found = run.only::<InPlace>();
    assert_eq!((found.rewritten, found.whole), (1, 1));
    // The bin written reads in the numbering of the target build, as the fixup left it.
    let mut windows = chunk.bytes.windows(field.len());
    assert!(windows.any(|bytes| bytes == field), "the pointer's kind");
    let after = bin::parse(&chunk.bytes).expect("the bin written again reads");
    assert_eq!(&after.objects[0].value, run.fix.object("M"));
    assert!(
        !after.objects[0]
            .value
            .properties
            .contains_key(&name_hash("junk"))
    );
    let round_trip = CheckSet::from(&CheckSelection::standard().without("types"));
    run.check(&round_trip);
    assert_eq!(run.fix.log.of::<RoundTripFailed>().count(), 0);
}

#[test]
fn a_cancel_ends_the_build_before_its_first_file() {
    // #ADR-0097: a step of a repair's build is one file of the mod.
    let env = env();
    let mut fix = FixRun::of(&env, &set(), &the_mod());
    let raised = fix.log.len();
    let cancel = Cancel::new();
    cancel.cancel();
    fix.log.cancel_on(cancel);
    let builder = InPlaceBuilder::new(&fix.fixed, &fix.changes);
    let built = builder.build(&mut fix.log.at(Stage::Build));
    assert!(
        matches!(built, Err(BuildError::Cancelled(Cancelled(Stage::Build)))),
        "{built:?}"
    );
    assert_eq!((fix.log.len(), fix.log.halted()), (raised, None));
    assert_eq!(fix.log.cancelled(), Some(Cancelled(Stage::Build)));
}

#[test]
fn a_history_that_does_not_answer_while_a_texture_is_judged_is_a_fault() {
    let env = env();
    // A DDS at the base texture's `.tex` path: in TEX's container it is judged again.
    let archive = the_mod().file(WAD, BASE_TEX, dds(DdsPixels::Dxt1, (8, 8), 4));
    let mut fix = FixRun::of(&env, &set(), &archive);
    let raised = fix.log.len();
    let history = BrokenHistory(MemoryHistory::new(&["16.19.300"]));
    let broken = Env::builder()
        .game(skins::game_at("16.19.300"))
        .history(Box::new(history))
        .build()
        .expect("an environment");
    let builder = InPlaceBuilder::new(&fix.fixed, &fix.changes);
    let builder = builder.judged(&broken, &LoadOptions::default());
    let built = builder.build(&mut fix.log.at(Stage::Build));
    assert!(
        matches!(built, Err(BuildError::Fault(Fault::History(_)))),
        "{built:?}"
    );
    // A fault is no diagnostic, and ends no run.
    assert_eq!((fix.log.len(), fix.log.halted()), (raised, None));
}

#[test]
fn a_repairs_config_says_the_mod_was_repaired() {
    let env = env();
    let run = PackRun::of(&env, &set(), &the_mod());
    let config = &run.package().config;
    assert_eq!(config, &ProjectConfig::of(&run.fix.fixed, Made::Repaired));
    let said = config.description.raw();
    assert!(
        said.starts_with("Repaired by reforge from synthetic.fantome ("),
        "{said}"
    );
}

#[test]
fn a_package_prints() {
    let env = env();
    let run = PackRun::of(&env, &set(), &the_mod());
    let dump = run.package().dump(&DumpCx::new(&NoNames));
    let lines: Vec<&str> = dump.lines().collect();
    assert_eq!(lines[0], "package Synthetic-reforgetests 1.0.0 chunks=5");
    assert!(
        lines.iter().any(|line| line
            .starts_with(&format!("     base {WAD}/{} ", skin_bin("test", 0)))
            && line.contains(" delta %f")),
        "{dump}"
    );
    assert_eq!(
        lines.last(),
        Some(&format!("     names path {LINKED_TEX}").as_str())
    );
}

#[test]
fn a_mod_in_a_packed_wad_repaired_again_changes_nothing() {
    let env = env();
    let set = FixupSet::standard();
    // The base particle's texture at the `.dds` path the game dropped for the `.tex`, which
    // the mod's own bin names.
    let old = BASE_PARTICLE_TEX.replace(".tex", ".dds");
    let skin = bin_of(vec![entry(ENTRY, &base_skin()), particle("P", &old)]);
    let texture = dds(DdsPixels::Dxt3, (8, 8), 1);
    // A WAD packed whole: no chunk of it comes with its path, and no table of this run
    // spells one. Only the bin spells the texture's.
    let chunks: [(&str, &[u8]); 2] = [(&skin_bin("test", 0), &skin), (&old, &texture)];
    let archive = ModArchive::new().packed(WAD, &chunks);
    let first = PackRun::of(&env, &set, &archive);
    // The file the game reads is made on the first repair.
    assert!(first.ships(&old) && first.ships(BASE_PARTICLE_TEX));
    assert_eq!(&*first.chunk(BASE_PARTICLE_TEX).bytes, texture.as_slice());

    let second = PackRun::of(&env, &set, &first.archive());
    assert_eq!(second.fix.changes.counts().edits(), 0);
    let chunks = |run: &PackRun<'_>| -> Vec<(u64, String)> {
        let chunks = run.package().chunks();
        let mut chunks: Vec<(u64, String)> = chunks
            .map(|(_, key, chunk)| (key.hash().0, ContentId::of(&chunk.bytes).hex()))
            .collect();
        chunks.sort();
        chunks
    };
    assert_eq!(chunks(&second), chunks(&first));
}

/// A texture at a path the game shipped once, which nothing names.
const FORMER_TEX: &str = "assets/characters/test/skins/base/test_tx_cm.dds";
/// A texture at a path the game shipped once, which the mod's skin bin names.
const FORMER_NAMED: &str = "assets/characters/test/skins/base/test_glow.dds";
/// A bin at a path the game shipped once, which nothing loads.
const FORMER_BIN: &str = "data/characters/test/skins/test_shared.bin";
/// The base texture as the game shipped it at the first build of the history.
const STALE_BYTES: &[u8] = b"the base texture at 16.18";

/// The same target with a past: at the build before it the game shipped three paths it
/// does not ship, and other bytes at the base texture's path.
fn env_with_a_past() -> Env {
    let history = MemoryHistory::new(&["16.18.200", "16.19.300"])
        .with_path(FORMER_TEX, 0..=0)
        .with_path(FORMER_NAMED, 0..=0)
        .with_path(FORMER_BIN, 0..=0)
        .with_content(BASE_TEX, STALE_BYTES, 0..=0);
    Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .history(Box::new(history))
        .build()
        .expect("an environment")
}

/// The mod with a bin and a texture at former paths that nothing loads or names, a texture
/// at a former path its skin bin names, and a stale copy of the base texture.
fn the_mod_with_leftovers() -> ModArchive {
    let objects = vec![
        entry(ENTRY, &own_skin()),
        material(),
        particle("P", FORMER_NAMED),
    ];
    // A bin no fixup reads: its material holds what the target does not declare.
    let unloaded = bin_of(vec![object("Old", "Material", vec![("junk", float(1.0))])]);
    the_mod()
        .file(WAD, &skin_bin("test", 1), linked_bin(objects, &[SHARED]))
        .file(WAD, FORMER_NAMED, blob(FORMER_NAMED))
        .file(WAD, FORMER_TEX, blob(FORMER_TEX))
        .file(WAD, FORMER_BIN, unloaded)
        .file(WAD, BASE_TEX, STALE_BYTES)
}

#[test]
fn a_file_the_client_does_not_reach_is_left_out_and_counted_by_its_reason() {
    let env = env_with_a_past();
    let run = PackRun::of(&env, &set(), &the_mod_with_leftovers());
    for left in [FORMER_TEX, FORMER_BIN, BASE_TEX, OLD_TEX, UNUSED_TEX] {
        assert!(!run.ships(left), "{left} is shipped");
    }
    let found = run.only::<InPlace>();
    assert_eq!(
        found.left,
        LeftBehind {
            unused: 1,
            former_path: 2,
            stale: 1,
            orphaned: 1,
            ..LeftBehind::default()
        }
    );
    assert_eq!(found.kept, run.package().len());
    assert_eq!(
        serde_json::to_value(found).expect("a finding is data"),
        serde_json::json!({
            "kept": 7, "rewritten": 2, "whole": 0, "made": 0,
            "unused": 1, "former_path": 2, "stale": 1, "orphaned": 1,
            "unnamed": 0,
        })
    );
    assert!(
        run.messages().contains(
            &"package.in-place 7 files kept at their paths, 2 bins written again; 5 files left \
              out: 1 that nothing loads or names, 2 at a former path that nothing names, 1 stale \
              copies of the game's, 1 that only removed values named"
                .to_string()
        ),
        "{:#?}",
        run.messages()
    );
}

#[test]
fn a_file_a_loaded_bin_names_at_a_former_path_is_kept() {
    let env = env_with_a_past();
    let run = PackRun::of(&env, &set(), &the_mod_with_leftovers());
    assert_eq!(
        &*run.chunk(FORMER_NAMED).bytes,
        blob(FORMER_NAMED).as_slice()
    );
}

#[test]
fn a_stale_copy_is_kept_where_the_caller_keeps_stale_copies() {
    let env = env_with_a_past();
    let options = LoadOptions::new().keep_stale(true);
    let run = PackRun::under(&env, &set(), &the_mod_with_leftovers(), options);
    assert_eq!(&*run.chunk(BASE_TEX).bytes, STALE_BYTES);
    assert_eq!(run.only::<InPlace>().left.stale, 0);
}

#[test]
fn a_bin_nothing_loads_is_not_in_the_package_for_the_type_check_to_read() {
    let env = env_with_a_past();
    let mut run = PackRun::of(&env, &set(), &the_mod_with_leftovers());
    run.check(&CheckSet::standard());
    // The one field the target does not declare is in the bin left out.
    assert_eq!(run.fix.log.of::<FieldUndeclared>().count(), 0);
    assert_eq!(run.fix.log.of::<TypeMismatch>().count(), 0);
}

#[test]
fn a_texture_that_is_a_stale_copy_once_in_texs_container_is_left_out() {
    // The base texture as a DDS, made of the TEX the game shipped at an earlier build: as a
    // DDS it is no bytes the game held, and in TEX's container it is the game's older file.
    let texture = dds(DdsPixels::Dxt1, (8, 8), 4);
    let as_tex = dds_to_tex(&texture).expect("TEX holds it");
    let history = MemoryHistory::new(&["16.18.200", "16.19.300"]);
    let history = history.with_content(BASE_TEX, &as_tex, 0..=0);
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .history(Box::new(history))
        .build()
        .expect("an environment");
    let archive = the_mod().file(WAD, BASE_TEX, texture);
    let run = PackRun::of(&env, &set(), &archive);
    assert!(!run.ships(BASE_TEX));
    assert_eq!(run.only::<InPlace>().left.stale, 1);
    assert_eq!(run.fix.log.of::<TextureContained>().count(), 0);
    // Repaired again, the package is the same: the second run finds no file to judge stale.
    let second = PackRun::of(&env, &set(), &run.archive());
    assert_eq!(second.package().len(), run.package().len());

    // A caller that keeps stale copies gets it, in TEX's container.
    let options = LoadOptions::new().keep_stale(true);
    let kept = PackRun::under(&env, &set(), &archive, options);
    assert_eq!(&*kept.chunk(BASE_TEX).bytes, as_tex.as_slice());
    assert_eq!(kept.only::<TextureContained>().contained, 1);
}
