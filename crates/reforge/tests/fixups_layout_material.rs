//! The fixups of a material: a sampler's slot and texture under the names the build reads
//! them by, and the default technique 13.5 retired.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind as Kind, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::{DefaultTechnique, SamplerFields};
use reforge::fixups::{After, Changed, FixupSet, Lost, Outcome, Removed};
use reforge::game::Schema;
use reforge::model::value::{as_str, field_of, items};
use reforge::model::{FieldPath, name_hash};
use reforge_testkit::bins::{embedded, float, object, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const SAMPLER_ID: &str = "material-sampler-fields";
const TECHNIQUE_ID: &str = "material-default-technique";
const MATERIAL: &str = "StaticMaterialDef";
const SAMPLER: &str = "StaticMaterialShaderSamplerDef";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// The sampler's three layouts: the slot in `samplerName` up to build 200, the texture in
/// `texturePath` from 150, a file from 260. A material's `defaultTechnique` went at 121.
fn schema() -> Schema {
    let string = || kind("String");
    SchemaRows::new(300)
        .class(
            SAMPLER,
            &[
                field("TextureName", string()),
                field("samplerName", string()).until(200),
                field("texturePath", string())
                    .from(150)
                    .then(260, kind("File")),
            ],
        )
        .class(
            MATERIAL,
            &[
                field("name", string()),
                field("defaultTechnique", string()).until(120),
            ],
        )
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// The target at build 300. With `dated`, a history in which the mod's dated file first
/// shipped at build 250, after both changes.
fn env(dated: bool) -> Env {
    let mut env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema());
    if dated {
        let history = MemoryHistory::new(&["1.0.50", "1.0.250", "16.19.300"]);
        env = env.history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)));
    }
    env.build().expect("an environment")
}

/// A target at an older build of the same schema.
fn env_at(build: &str) -> Env {
    Env::builder()
        .game(skins::game_at(build))
        .schema(schema())
        .build()
        .expect("an environment")
}

fn archive(objects: Vec<BinObject>) -> ModArchive {
    skin_archive(objects).file(skins::WAD, DATED, dated_bytes())
}

fn set() -> FixupSet {
    FixupSet::empty().with(SamplerFields).with(DefaultTechnique)
}

fn sampler(fields: &[(&str, &str)]) -> V {
    let fields = fields.iter().map(|(name, value)| (*name, text(value)));
    embedded(SAMPLER, fields.collect())
}

/// A material whose samplers are these.
fn material(entry: &str, samplers: Vec<V>) -> BinObject {
    let list = values::Container::new(Kind::Embedded, samplers).expect("a list of embeds");
    let list = V::UnorderedContainer(values::UnorderedContainer(list));
    object(entry, MATERIAL, vec![("samplerValues", list)])
}

/// A field of the sampler at an index of a material, as text.
fn sampler_field<'a>(
    run: &'a FixRun<'_>,
    entry: &str,
    index: usize,
    name: &str,
) -> Option<&'a str> {
    let samplers = items(run.field(entry, "samplerValues")?);
    field_of(samplers.get(index)?, name).and_then(as_str)
}

fn sampler_names(run: &FixRun<'_>, entry: &str, index: usize) -> Vec<reforge::model::BinHash> {
    let samplers = items(run.field(entry, "samplerValues").expect("the samplers"));
    match &samplers[index] {
        V::Embedded(values::Embedded(sampler)) => sampler.properties.keys().copied().collect(),
        other => panic!("no embed: {other:?}"),
    }
}

/// The path of a field of the sampler at an index.
fn in_sampler(index: usize, name: &str) -> FieldPath {
    let samplers = FieldPath::root().field(name_hash(MATERIAL), name_hash("samplerValues"));
    samplers
        .item(index)
        .field(name_hash(SAMPLER), name_hash(name))
}

/// The old layout, and the one of 15.3 with the texture in `texturePath` already.
fn old_material() -> BinObject {
    let old = sampler(&[
        ("samplerName", "Diffuse_Texture"),
        ("TextureName", "assets/mod/a/body.tex"),
    ]);
    let mid = sampler(&[
        ("samplerName", "Mask_Texture"),
        ("texturePath", "assets/mod/a/own.tex"),
    ]);
    material("Materials/Body", vec![old, mid])
}

#[test]
fn a_samplers_moved_fields_follow_riots_rename() {
    let env = env(false);
    let run = FixRun::of(&env, &set(), &archive(vec![old_material()]));
    let get = |index: usize, name: &str| sampler_field(&run, "Materials/Body", index, name);
    assert_eq!(get(0, "TextureName"), Some("Diffuse_Texture"));
    assert_eq!(get(0, "texturePath"), Some("assets/mod/a/body.tex"));
    assert_eq!(get(0, "samplerName"), None);
    assert_eq!(get(1, "TextureName"), Some("Mask_Texture"));
    assert_eq!(get(1, "texturePath"), Some("assets/mod/a/own.tex"));
    assert_eq!(get(1, "samplerName"), None);
    // A field moved takes the last place.
    let names = sampler_names(&run, "Materials/Body", 0);
    assert_eq!(names, ["texturePath", "TextureName"].map(name_hash));

    let rows: Vec<(&FieldPath, &After)> = run
        .changes
        .of(SAMPLER_ID)
        .iter()
        .map(|change| (&change.path, &change.after))
        .collect();
    let moved = |index: usize, to: &str| After::Moved {
        to: in_sampler(index, to),
    };
    assert_eq!(
        rows,
        [
            (&in_sampler(0, "TextureName"), &moved(0, "texturePath")),
            (&in_sampler(0, "samplerName"), &moved(0, "TextureName")),
            (&in_sampler(1, "samplerName"), &moved(1, "TextureName")),
        ]
    );
    let [changed] = run.raised::<Changed>(SAMPLER_ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (3, 1));
    let spelled = run.changes.of(SAMPLER_ID)[0].path.spelled(&run.speller());
    assert!(spelled.ends_with("[0].TextureName"), "{spelled}");
}

#[test]
fn the_texture_path_a_sampler_holds_stays() {
    let env = env(false);
    let all = sampler(&[
        ("samplerName", "Diffuse_Texture"),
        ("TextureName", "assets/mod/a/old.tex"),
        ("texturePath", "assets/mod/a/own.tex"),
    ]);
    let objects = vec![material("M", vec![all])];
    let run = FixRun::of(&env, &set(), &archive(objects));
    let get = |name: &str| sampler_field(&run, "M", 0, name);
    assert_eq!(get("texturePath"), Some("assets/mod/a/own.tex"));
    assert_eq!(get("TextureName"), Some("Diffuse_Texture"));
    let outcomes: Vec<Outcome> = run.changes.iter().map(|c| c.outcome()).collect();
    assert_eq!(outcomes, [Outcome::Lost, Outcome::Changed]);
    assert_eq!(
        run.changes.of(SAMPLER_ID)[0].path,
        in_sampler(0, "TextureName")
    );
    assert_eq!(run.raised::<Lost>(SAMPLER_ID).len(), 1);
}

#[test]
fn a_sampler_with_no_sampler_name_is_left() {
    let env = env(false);
    let new = sampler(&[
        ("TextureName", "Diffuse_Texture"),
        ("texturePath", "assets/mod/a/body.tex"),
    ]);
    let objects = vec![material("M", vec![new])];
    let run = FixRun::of(&env, &set(), &archive(objects.clone()));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("M"), &objects[0]);
}

#[test]
fn a_samplers_fields_move_whatever_the_mods_date() {
    // As ltk-forge does (#ADR-0004): a mod made at 250, when no build since read
    // `samplerName`, is laid out all the same.
    let env = env(true);
    let run = FixRun::of(&env, &set(), &archive(vec![old_material()]));
    assert_eq!(run.dating.build(), Some(250));
    let get = |index: usize, name: &str| sampler_field(&run, "Materials/Body", index, name);
    assert_eq!(get(0, "TextureName"), Some("Diffuse_Texture"));
    assert_eq!(get(0, "texturePath"), Some("assets/mod/a/body.tex"));
    assert_eq!(run.changes.of(SAMPLER_ID).len(), 3);
}

#[test]
fn the_sampler_fixup_runs_where_the_target_reads_the_new_names() {
    // At 180 the target reads `samplerName`, and at 120 no `texturePath`.
    for build in ["1.0.180", "1.0.120"] {
        let env = env_at(build);
        let run = FixRun::of(&env, &set(), &archive(vec![old_material()]));
        assert!(run.changes.of(SAMPLER_ID).is_empty(), "{build}");
        assert_eq!(run.object("Materials/Body"), &old_material());
    }
}

fn technique(entry: &str, value: V) -> BinObject {
    let fields = vec![("name", text(entry)), ("defaultTechnique", value)];
    object(entry, MATERIAL, fields)
}

#[test]
fn a_retired_default_technique_is_removed_and_a_choice_is_lost() {
    let env = env(false);
    let objects = vec![
        technique("Materials/Body", text("normal")),
        technique("Materials/Cape", text("Normal")),
        technique("Materials/Glow", text("glowing")),
        technique("Materials/Odd", float(1.0)),
    ];
    let run = FixRun::of(&env, &set(), &archive(objects));
    for entry in [
        "Materials/Body",
        "Materials/Cape",
        "Materials/Glow",
        "Materials/Odd",
    ] {
        assert!(run.field(entry, "defaultTechnique").is_none(), "{entry}");
        assert_eq!(run.field(entry, "name"), Some(&text(entry)));
    }

    // `normal`, in any case, names the technique the client uses anyway.
    let outcomes: Vec<Outcome> = run.changes.iter().map(|c| c.outcome()).collect();
    let (removed, lost) = (Outcome::Removed, Outcome::Lost);
    assert_eq!(outcomes, [removed, removed, lost, lost]);
    let [removed] = run.raised::<Removed>(TECHNIQUE_ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((removed.0.values, removed.0.objects), (2, 2));
    let [lost] = run.raised::<Lost>(TECHNIQUE_ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((lost.0.values, lost.0.objects), (2, 2));

    // The change holds the material and the technique it chose.
    let glow = &run.changes.of(TECHNIQUE_ID)[2];
    assert_eq!(glow.object, run.id("Materials/Glow"));
    let path = FieldPath::root().field(name_hash(MATERIAL), name_hash("defaultTechnique"));
    assert_eq!(glow.path, path);
    let before = glow.before.as_ref().expect("a value before");
    assert_eq!(
        (before.kind, before.text.as_str()),
        ("String", "\"glowing\"")
    );
    assert_eq!(glow.path.spelled(&run.speller()), "defaultTechnique");
}

#[test]
fn a_default_technique_goes_whatever_the_mods_date() {
    // As ltk-forge does (#ADR-0004): a choice is counted lost in a mod made at 250 too, when
    // no build since read the field.
    let env = env(true);
    let objects = vec![technique("Materials/Glow", text("glowing"))];
    let run = FixRun::of(&env, &set(), &archive(objects));
    assert_eq!(run.dating.build(), Some(250));
    let [change] = run.changes.of(TECHNIQUE_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.after, After::Lost);
}

#[test]
fn the_technique_fixup_runs_where_the_target_does_not_read_the_field() {
    let env = env_at("1.0.110");
    let objects = vec![technique("Materials/Glow", text("glowing"))];
    let run = FixRun::of(&env, &set(), &archive(objects.clone()));
    assert!(run.changes.is_empty());
    assert_eq!(run.object("Materials/Glow"), &objects[0]);
}

#[test]
fn the_fixups_applied_twice_change_nothing_the_second_time() {
    let all = sampler(&[
        ("samplerName", "Diffuse_Texture"),
        ("TextureName", "assets/mod/a/old.tex"),
        ("texturePath", "assets/mod/a/own.tex"),
    ]);
    let objects = vec![
        old_material(),
        material("All", vec![all]),
        technique("Materials/Glow", text("glowing")),
        technique("Materials/Cape", text("normal")),
    ];
    let entries = ["Materials/Body", "All", "Materials/Glow", "Materials/Cape"];
    for dated in [false, true] {
        let env = env(dated);
        let once = FixRun::of(&env, &set(), &archive(objects.clone()));
        assert!(!once.changes.is_empty());
        let after = entries.map(|entry| once.object(entry).clone());
        let twice = once.again(&set());
        assert!(twice.changes.is_empty(), "{:?}", twice.changes);
        assert_eq!(after, entries.map(|entry| twice.object(entry).clone()));
    }
}
