//! The in-game tests of how the client draws a material's shader (#GAME-BEHAVIOR-0022,
//! #GAME-CHANGE-0024). For one champion's base skin it writes one mod per case: the game's
//! `skin0.bin` with a material on the whole model, as a `.fantome`.
//!
//! The material is Vex's skin 33 shadow material as the game holds it since 16.10, when Riot
//! relinked it from `Cloth_Base` to `Diffuse_Alpha_mult`, drawing the champion's own texture. The
//! cases differ in the shader the material's pass links:
//!
//! | case | the pass's shader |
//! |---|---|
//! | `control` | `Diffuse_Alpha_mult`, a shader of the game's shader bin |
//! | `removed` | `Cloth_Base`, which left the shader bin at 16.10 |
//! | `never-existed` | a name no shader had |
//! | `own-copy` | `Diffuse_Alpha_mult`, its definition copied into the skin bin |
//! | `own-new-name` | that definition copied into the skin bin under a name of its own |
//!
//! `cargo run -p reforge --example shader_tests -- <game> <champion> <out>` writes
//! `<out>/shader-test-<champion>-<case>.fantome`.

use camino::{Utf8Path, Utf8PathBuf};
use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject, PropertyValueEnum as V};
use reforge::game::{Game, Install};
use reforge::model::value::{as_str, get};
use reforge::model::walk::structs_mut;
use reforge::model::{BinHash, WadName, bin, name_hash, path_hash};
use reforge_testkit::archive::{DEFLATED, zip};

const VEX_SKIN: &str = "data/characters/vex/skins/skin33.bin";
const VEX_MATERIAL: &str = "Characters/Vex/Skins/Skin33/Materials/Vex_Skin33_Shadow_Mat";
const SHADER_BIN: &str = "data/shaders/shaders.bin";
const SHADER: &str = "Shaders/SkinnedMesh/Diffuse_Alpha_mult";
const REMOVED: &str = "Shaders/SkinnedMesh/Cloth_Base";
const NEVER: &str = "Shaders/SkinnedMesh/Reforge_Test_No_Such_Shader";
const OWN: &str = "Shaders/SkinnedMesh/Reforge_Test_Own_Shader";

/// One mod: the shader its material links, and the definition its skin bin adds, if any.
struct Case {
    name: &'static str,
    shader: &'static str,
    /// The entry name the game's `Diffuse_Alpha_mult` definition is copied under.
    defines: Option<&'static str>,
    /// What the case asks, for the mod's description.
    asks: &'static str,
}

const CASES: [Case; 5] = [
    Case {
        name: "control",
        shader: SHADER,
        defines: None,
        asks: "The control: the material links a shader of the game's shader bin.",
    },
    Case {
        name: "removed",
        shader: REMOVED,
        defines: None,
        asks: "The material links Cloth_Base, a shader Riot removed at 16.10.",
    },
    Case {
        name: "never-existed",
        shader: NEVER,
        defines: None,
        asks: "The material links a shader name no build held.",
    },
    Case {
        name: "own-copy",
        shader: SHADER,
        defines: Some(SHADER),
        asks: "The skin bin defines Diffuse_Alpha_mult, a copy of the game's definition.",
    },
    Case {
        name: "own-new-name",
        shader: OWN,
        defines: Some(OWN),
        asks: "The skin bin defines a shader of its own name, a copy of Diffuse_Alpha_mult.",
    },
];

/// A game bin read whole: its objects and its links.
struct GameBin {
    objects: Vec<BinObject>,
    links: Vec<String>,
}

impl GameBin {
    fn read(game: &Install, path: &str) -> Self {
        let bytes = game
            .read(path_hash(path))
            .unwrap_or_else(|error| panic!("{path} does not read from the install: {error}"))
            .unwrap_or_else(|| panic!("the install holds no {path}"));
        let parsed =
            bin::parse(&bytes).unwrap_or_else(|error| panic!("{path} does not parse: {error}"));
        Self {
            objects: parsed.objects.into_iter().map(|o| o.value).collect(),
            links: parsed.dependencies,
        }
    }

    fn object(&self, entry_name: &str) -> &BinObject {
        let hash = name_hash(entry_name);
        self.objects
            .iter()
            .find(|object| object.path_hash == hash)
            .unwrap_or_else(|| panic!("no object {entry_name} in the bin"))
    }
}

fn link(hash: BinHash) -> V {
    V::ObjectLink(values::ObjectLink::new(hash))
}

/// Vex's material under `entry_name`, its pass linking `shader` and its diffuse sampler drawing
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

/// The fantome of one case: the skin bin at the champion's WAD, and its `info.json`.
fn fantome(wad: &WadName, skin_path: &str, skin: &[u8], champion: &str, case: &Case) -> Vec<u8> {
    let info = serde_json::json!({
        "Name": format!("Shader test: {} ({champion})", case.name),
        "Author": "reforge",
        "Version": "1.0.0",
        "Description": case.asks,
    });
    let info = serde_json::to_vec_pretty(&info)
        .unwrap_or_else(|error| panic!("the info of {} does not write: {error}", case.name));
    let entry = format!("WAD/{wad}/{skin_path}");
    zip(&[
        ("META/info.json", info.as_slice(), DEFLATED),
        (entry.as_str(), skin, DEFLATED),
    ])
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [game, champion, out] = args.as_slice() else {
        panic!("usage: shader_tests <game> <champion> <out>; got {args:?}");
    };
    let champion = champion.to_lowercase();
    let install = Install::open(Utf8Path::new(game))
        .unwrap_or_else(|error| panic!("the install at {game} does not open: {error}"));

    let skin_path = format!("data/characters/{champion}/skins/skin0.bin");
    let skin = GameBin::read(&install, &skin_path);
    let wad = install
        .wads(path_hash(&skin_path))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no WAD of the install holds {skin_path}"));
    let entry_name = format!("Characters/{champion}/Skins/Skin0");
    let texture = get(skin.object(&entry_name), &["skinMeshProperties", "texture"])
        .unwrap_or_else(|| panic!("{entry_name} names no texture of its mesh"))
        .clone();
    let vex = GameBin::read(&install, VEX_SKIN);
    let shaders = GameBin::read(&install, SHADER_BIN);
    let definition = shaders.object(SHADER);

    let out = Utf8PathBuf::from(out);
    fs_err::create_dir_all(&out).unwrap_or_else(|error| panic!("{out} is not made: {error}"));
    let material_name = format!("Characters/{champion}/Skins/Skin0/Materials/Reforge_Shader_Test");
    let mesh = name_hash("SkinMeshDataProperties");
    for case in &CASES {
        let mut objects = skin.objects.clone();
        for object in &mut objects {
            if object.path_hash == name_hash(&entry_name) {
                structs_mut(object, |mut s| {
                    if s.class() == mesh {
                        s.fields_mut()
                            .insert(name_hash("Material"), link(name_hash(&material_name)));
                    }
                });
            }
        }
        let vex_material = vex.object(VEX_MATERIAL);
        objects.push(material(
            vex_material,
            &material_name,
            name_hash(case.shader),
            &texture,
        ));
        if let Some(name) = case.defines {
            let mut copy = definition.clone();
            copy.path_hash = name_hash(name);
            objects.push(copy);
        }
        let bytes = bin::write(&Bin::new(objects, skin.links.clone()));
        let written = fantome(&wad, &skin_path, &bytes, &champion, case);
        let file = out.join(format!("shader-test-{champion}-{}.fantome", case.name));
        fs_err::write(&file, written)
            .unwrap_or_else(|error| panic!("{file} is not written: {error}"));
        println!("{file}");
    }
}
