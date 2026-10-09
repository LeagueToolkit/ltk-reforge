//! The in-game tests of a material asking a game shader for a variant the shader cache lacks
//! (#GAME-BEHAVIOR-0029). For one champion's base skin it writes one mod per case: the game's
//! `skin0.bin` with a material on the whole model, as a `.fantome`.
//!
//! The material is Vex's skin 33 shadow material on `Diffuse_Alpha_mult`, the control of
//! `shader_tests`, which draws. The cases differ in the defines the material asks for:
//!
//! | case | the material's defines |
//! |---|---|
//! | `control` | as the game holds them, `NUM_BLEND_WEIGHTS=4` among its macros |
//! | `no-blend-weights` | `NUM_BLEND_WEIGHTS` left out of its macros |
//! | `switch-<name>` | a static switch flipped to a value no program of the shader was compiled with |
//!
//! The vertex programs of 223 game shaders are compiled only with `NUM_BLEND_WEIGHTS=4`. The
//! second case asks whether the client adds that define itself: if the game draws it, it does;
//! if it logs `Unable to find correct hash` for the `.vs-dx11` and crashes at the draw, it does
//! not, and a material without the macro is a variant the cache lacks. The third case is the
//! positive control: the crash of the record on a second shader.
//!
//! Each case's key is checked against the cache before its mod is written, and the example
//! prints what the cache says of it.
//!
//! `cargo run -p reforge --example shader_variant_tests -- <game> <champion> <out>` writes
//! `<out>/shader-variant-test-<champion>-<case>.fantome`.

use std::collections::BTreeMap;

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::game::{Game, Install, ShaderStage, ShaderVariants, shader_file};
use reforge::model::value::{as_str, field_of, get, items};
use reforge::model::walk::structs_mut;
use reforge::model::{BinHash, name_hash};
use reforge_testkit::bins::embedded;
use reforge_testkit::ingame::{self, GameBin};

const VEX_SKIN: &str = "data/characters/vex/skins/skin33.bin";
const VEX_MATERIAL: &str = "Characters/Vex/Skins/Skin33/Materials/Vex_Skin33_Shadow_Mat";
const SHADER_BIN: &str = "data/shaders/shaders.bin";
const SHADER: &str = "Shaders/SkinnedMesh/Diffuse_Alpha_mult";
const BLEND_WEIGHTS: &str = "NUM_BLEND_WEIGHTS";

fn link(hash: BinHash) -> V {
    V::ObjectLink(values::ObjectLink::new(hash))
}

fn truth(value: &V) -> bool {
    match value {
        V::Bool(flag) => flag.value,
        V::BitBool(flag) => flag.value,
        _ => false,
    }
}

/// The pairs of a map of strings to strings.
fn string_pairs(value: Option<&V>) -> Vec<(String, String)> {
    let Some(V::Map(map)) = value else {
        return Vec::new();
    };
    let entries = map.entries().iter();
    entries
        .filter_map(|(name, value)| Some((as_str(name)?.to_owned(), as_str(value)?.to_owned())))
        .collect()
}

/// The defines of the material's pass on the shader, laid over each other as the record has it:
/// feature defines, static switches as the material sets them or by default, the material's
/// macros, the pass's macros. The material's one child technique adds `TRANSITION=1`; the client
/// asks for that variant too, and the example judges the technique's own alone, which is enough
/// to tell the cases apart.
fn defines(shader: &BinObject, material: &BinObject) -> BTreeMap<String, String> {
    let mut defines = BTreeMap::new();
    defines.extend(string_pairs(get(shader, &["featureDefines"])));
    let set: BTreeMap<&str, bool> = get(material, &["switches"])
        .map_or(&[][..], items)
        .iter()
        .filter_map(|switch| {
            let name = field_of(switch, "name").and_then(as_str)?;
            Some((name, field_of(switch, "on").is_none_or(truth)))
        })
        .collect();
    for switch in get(shader, &["staticSwitches"]).map_or(&[][..], items) {
        let Some(name) = field_of(switch, "name").and_then(as_str) else {
            continue;
        };
        let by_default = field_of(switch, "onByDefault").is_some_and(truth);
        let on = set.get(name).copied().unwrap_or(by_default);
        defines.insert(name.to_owned(), if on { "1" } else { "0" }.to_owned());
    }
    defines.extend(string_pairs(get(material, &["shaderMacros"])));
    for technique in get(material, &["techniques"]).map_or(&[][..], items) {
        for pass in field_of(technique, "passes").map_or(&[][..], items) {
            defines.extend(string_pairs(field_of(pass, "shaderMacros")));
        }
    }
    defines
}

/// Both files of the shader's programs in the cache.
fn cache(install: &Install, object_path: &str) -> Vec<(ShaderStage, ShaderVariants)> {
    ShaderStage::ALL
        .into_iter()
        .map(|stage| {
            let path = shader_file(object_path, stage);
            let bytes = install
                .read(path)
                .unwrap_or_else(|error| panic!("{object_path} {stage:?} does not read: {error}"))
                .unwrap_or_else(|| panic!("the cache holds no {object_path} {stage:?}"));
            let held = ShaderVariants::read(&bytes)
                .unwrap_or_else(|| panic!("{object_path} {stage:?} is no TOC3.0 file"));
            (stage, held)
        })
        .collect()
}

/// What the cache says of the defines: each stage, its filtered string and whether the key is
/// held.
fn judge(
    files: &[(ShaderStage, ShaderVariants)],
    defines: &BTreeMap<String, String>,
) -> Vec<String> {
    files
        .iter()
        .map(|(stage, held)| {
            let filtered = held.filtered(defines.iter().map(|(n, v)| (n.as_str(), v.as_str())));
            let verdict = if held.holds(&filtered) {
                "held"
            } else {
                "MISSING"
            };
            format!("  {stage:?}: {verdict}  {filtered}")
        })
        .collect()
}

fn all_held(files: &[(ShaderStage, ShaderVariants)], defines: &BTreeMap<String, String>) -> bool {
    files.iter().all(|(_, held)| {
        let filtered = held.filtered(defines.iter().map(|(n, v)| (n.as_str(), v.as_str())));
        held.holds(&filtered)
    })
}

/// The material without `NUM_BLEND_WEIGHTS` in its macros.
fn without_blend_weights(material: &BinObject) -> BinObject {
    let mut material = material.clone();
    let macros = name_hash("shaderMacros");
    let Some(V::Map(map)) = material.properties.get_mut(&macros) else {
        panic!("the material holds no shaderMacros map");
    };
    let (key_kind, value_kind) = (map.key_kind(), map.value_kind());
    let kept: Vec<(V, V)> = map
        .entries()
        .iter()
        .filter(|(name, _)| as_str(name) != Some(BLEND_WEIGHTS))
        .cloned()
        .collect();
    *map = values::Map::new(key_kind, value_kind, kept)
        .unwrap_or_else(|error| panic!("the macros kept make no map: {error}"));
    material
}

/// The material with one static switch set, replacing any switch of that name.
fn with_switch(material: &BinObject, name: &str, on: bool) -> BinObject {
    let mut material = material.clone();
    let switches = name_hash("switches");
    // `StaticMaterialDef.switches` is a `List2<Embed<StaticMaterialSwitchDef>>` (rito-meta,
    // 16.19): a list of another kind, or of pointers, the client skips whole (#GAME-BEHAVIOR-0002).
    let switch = embedded(
        "StaticMaterialSwitchDef",
        vec![
            ("name", V::String(values::String::from(name))),
            ("on", V::Bool(values::Bool::new(on))),
        ],
    );
    let existing: Vec<V> = match material.properties.get(&switches) {
        Some(V::UnorderedContainer(list)) => list
            .0
            .items()
            .iter()
            .filter(|s| field_of(s, "name").and_then(as_str) != Some(name))
            .cloned()
            .collect(),
        Some(other) => panic!("the material's switches are no List2: {other:?}"),
        None => Vec::new(),
    };
    let mut kept = existing;
    kept.push(switch);
    let list = values::Container::new(ltk_meta::PropertyKind::Embedded, kept)
        .unwrap_or_else(|error| panic!("the switches make no list: {error}"));
    material.properties.insert(
        switches,
        V::UnorderedContainer(values::UnorderedContainer(list)),
    );
    material
}

/// The material under `entry_name`, its pass linking the shader and its diffuse sampler drawing
/// `texture`.
fn material(vex: &BinObject, entry_name: &str, shader: BinHash, texture: &V) -> BinObject {
    let mut material = vex.clone();
    material.path_hash = name_hash(entry_name);
    material.properties.insert(
        name_hash("name"),
        V::String(values::String::from(entry_name)),
    );
    let (pass, sampler) = (
        name_hash("StaticMaterialPassDef"),
        name_hash("StaticMaterialShaderSamplerDef"),
    );
    structs_mut(&mut material, |mut s| {
        if s.class() == pass {
            s.fields_mut().insert(name_hash("shader"), link(shader));
        }
        let diffuse = s.fields().get(&name_hash("TextureName")).and_then(as_str);
        if s.class() == sampler && diffuse == Some("Diffuse_Texture") {
            s.fields_mut()
                .insert(name_hash("texturePath"), texture.clone());
        }
    });
    material
}

struct Case {
    name: String,
    material: BinObject,
    asks: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (install, skin, out) = ingame::open("shader_variant_tests", &args);
    let champion = &skin.champion;
    let entry_name = skin.entry_name();
    let texture = get(
        skin.bin.object(&entry_name),
        &["skinMeshProperties", "texture"],
    )
    .unwrap_or_else(|| panic!("{entry_name} names no texture of its mesh"))
    .clone();
    let vex = GameBin::read(&install, VEX_SKIN);
    let shaders = GameBin::read(&install, SHADER_BIN);
    let definition = shaders.object(SHADER);
    let object_path = get(definition, &["objectPath"])
        .and_then(as_str)
        .unwrap_or_else(|| panic!("{SHADER} has no objectPath"));
    let files = cache(&install, object_path);

    let material_name = format!("Characters/{champion}/Skins/Skin0/Materials/Reforge_Variant_Test");
    let control = material(
        vex.object(VEX_MATERIAL),
        &material_name,
        name_hash(SHADER),
        &texture,
    );
    let control_defines = defines(definition, &control);
    assert_eq!(
        control_defines.get(BLEND_WEIGHTS).map(String::as_str),
        Some("4"),
        "the control's material does not set {BLEND_WEIGHTS}=4; pick another material"
    );
    assert!(
        all_held(&files, &control_defines),
        "the control's variant is not in the cache; the test says nothing then"
    );

    let mut cases = vec![
        Case {
            name: "control".into(),
            material: control.clone(),
            asks: "The control: Vex's shadow material on Diffuse_Alpha_mult, NUM_BLEND_WEIGHTS=4 among its macros.".into(),
        },
        Case {
            name: "no-blend-weights".into(),
            material: without_blend_weights(&control),
            asks: "The same material with NUM_BLEND_WEIGHTS left out of its macros. The shader's vertex programs are compiled only with it.".into(),
        },
    ];
    let no_blend = defines(definition, &cases[1].material);
    assert!(
        !all_held(&files, &no_blend),
        "the cache holds the variant without {BLEND_WEIGHTS}; this shader cannot ask the question"
    );

    // The positive control: the first static switch whose flip asks for a variant the cache lacks.
    let flipped = get(definition, &["staticSwitches"])
        .map_or(&[][..], items)
        .iter()
        .filter_map(|switch| field_of(switch, "name").and_then(as_str))
        .find_map(|name| {
            let on = control_defines.get(name).map(String::as_str) == Some("1");
            let candidate = with_switch(&control, name, !on);
            let lacked = !all_held(&files, &defines(definition, &candidate));
            lacked.then_some((name, !on, candidate))
        });
    match flipped {
        Some((name, on, candidate)) => cases.push(Case {
            name: format!("switch-{}", name.to_ascii_lowercase()),
            material: candidate,
            asks: format!(
                "The same material with the static switch {name} {}: no program of the shader was compiled with it.",
                if on { "on" } else { "off" }
            ),
        }),
        None => println!("no static switch of {SHADER} flips to a variant the cache lacks; no positive control"),
    }

    let mesh = name_hash("SkinMeshDataProperties");
    for case in &cases {
        println!("{}:", case.name);
        for line in judge(&files, &defines(definition, &case.material)) {
            println!("{line}");
        }
        let mut bin = skin.bin.clone();
        structs_mut(bin.object_mut(&entry_name), |mut s| {
            if s.class() == mesh {
                s.fields_mut()
                    .insert(name_hash("Material"), link(name_hash(&material_name)));
            }
        });
        bin.objects.push(case.material.clone());
        let name = format!("Shader variant test: {} ({champion})", case.name);
        let stem = format!("shader-variant-test-{champion}-{}", case.name);
        skin.write_mod(&out, &stem, &name, &case.asks, &bin.to_bytes());
    }
}
