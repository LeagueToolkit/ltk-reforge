//! A repair's package: the mod's files at their own paths, a bin written again where a fixup
//! changed an object of it, every other file with the bytes it has. The game is
//! `reforge_testkit::skins`.

use ltk_meta::property::values;
use reforge::diag::{Diagnostics, Stage};
use reforge::env::Env;
use reforge::fixups::{FixInputs, FixupSet, ObjectSet, Rule};
use reforge::game::{NameBook, Schema};
use reforge::load::Dating;
use reforge::model::entity::ModMeta;
use reforge::model::{ContentId, Dump, DumpCx, Mod, NoNames, Spelled, bin, name_hash, path_hash};
use reforge::package::{
    BinWrite, Empty, InPlace, InPlaceBuilder, Layer, Origin, TexRefusal, TextureContained,
};
use reforge_testkit::bins::{
    Skin, bin as bin_of, blob, entry, float, linked_bin, object, particle, text,
};
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::schema::{field, kind};
use reforge_testkit::skins::{
    self, BASE_PARTICLE_TEX, BASE_TEX, S3_PARTICLE_TEX, S3_TEX, WAD, base_skin, skin_bin,
};
use reforge_testkit::{ModArchive, PackRun, SchemaRows};

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
            unused: 1,
            orphaned: 1,
            unnamed: 0,
        }
    );
    assert_eq!(package.config.name, "Synthetic-reforgetests");
    assert_eq!(package.config.version, "1.0.0");
    assert_eq!(package.config.champions, ["test"]);
    assert!(
        run.messages().contains(
            &"package.in-place 5 files kept at their paths, 1 bin written again; 1 file left \
              behind that nothing loads; 1 file left behind that only removed values named"
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
    assert_eq!(
        (found.kept, found.rewritten, found.unused, found.orphaned),
        (5, 0, 0, 0)
    );
}

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
    let halted = built.expect_err("nothing to build");
    assert_eq!(log.halted(), Some(halted));
    assert_eq!(log.of::<Empty>().next(), Some(&Empty { files: 0 }));
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
