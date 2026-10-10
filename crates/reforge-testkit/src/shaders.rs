//! Shaders in memory, for tests: a `TOC3.0` table of one file of a shader's programs, written
//! as `ltk_shader` reads one, a game shader's definition, and a material on one.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind as Kind, PropertyValueEnum as V};
use reforge::game::variant_key;

use crate::bins::{embedded, flag, link, object, text};

/// A map of strings to strings.
pub fn strings(pairs: &[(&str, &str)]) -> V {
    let entries = pairs.iter().map(|(k, v)| (text(k), text(v))).collect();
    V::Map(values::Map::new(Kind::String, Kind::String, entries).expect("a map of strings"))
}

/// A list of embeds.
pub fn embeds(items: Vec<V>) -> V {
    V::Container(values::Container::new(Kind::Embedded, items).expect("a list of embeds"))
}

/// A `CustomShaderDef` whose programs are at its entry name, with these feature defines and
/// static switches, each with whether it is on by default.
pub fn shader(entry: &str, features: &[(&str, &str)], switches: &[(&str, bool)]) -> BinObject {
    let switches = switches.iter().map(|&(name, on)| {
        embedded(
            "ShaderStaticSwitch",
            vec![("name", text(name)), ("onByDefault", flag(on))],
        )
    });
    object(
        entry,
        "CustomShaderDef",
        vec![
            ("objectPath", text(entry)),
            ("featureDefines", strings(features)),
            ("staticSwitches", embeds(switches.collect())),
        ],
    )
}

/// What a material asks of its shader's variants.
#[derive(Default)]
pub struct Asks<'a> {
    /// Its switches, each with whether it is on.
    pub switches: &'a [(&'a str, bool)],
    /// Its macros.
    pub macros: &'a [(&'a str, &'a str)],
    /// The pass's macros.
    pub pass_macros: &'a [(&'a str, &'a str)],
    /// The child techniques of its technique, each by its name and its macros.
    pub children: &'a [(&'a str, &'a [(&'a str, &'a str)])],
}

/// A `StaticMaterialDef` of one technique of one pass on a shader, with what it asks.
pub fn material(entry: &str, shader: &str, technique: &str, asks: &Asks<'_>) -> BinObject {
    let switches = asks.switches.iter().map(|&(name, on)| {
        embedded(
            "StaticMaterialSwitchDef",
            vec![("name", text(name)), ("on", flag(on))],
        )
    });
    let pass = embedded(
        "StaticMaterialPassDef",
        vec![
            ("shader", link(shader)),
            ("shaderMacros", strings(asks.pass_macros)),
        ],
    );
    let of_technique = embedded(
        "StaticMaterialTechniqueDef",
        vec![("name", text(technique)), ("passes", embeds(vec![pass]))],
    );
    let children = asks.children.iter().map(|&(name, macros)| {
        embedded(
            "StaticMaterialChildTechniqueDef",
            vec![
                ("name", text(name)),
                ("parentName", text(technique)),
                ("shaderMacros", strings(macros)),
            ],
        )
    });
    object(
        entry,
        "StaticMaterialDef",
        vec![
            ("switches", embeds(switches.collect())),
            ("shaderMacros", strings(asks.macros)),
            ("techniques", embeds(vec![of_technique])),
            ("childTechniques", embeds(children.collect())),
        ],
    )
}

/// A `TOC3.0` file whose variants are made of these base defines and are these filtered defines,
/// each written as the client's log writes it.
pub fn toc(base: &[(&str, &str)], variants: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    bytes(&mut out, "TOC3.0");
    // The count of variants, of base defines, of bundles, and the stage.
    for n in [variants.len(), base.len(), variants.len(), 0] {
        out.extend(count(n));
    }
    bytes(&mut out, "baseDefines");
    for (name, value) in base {
        bytes(&mut out, name);
        bytes(&mut out, value);
    }
    bytes(&mut out, "shaders");
    for filtered in variants {
        out.extend(variant_key(filtered).to_le_bytes());
    }
    for id in 0..variants.len() {
        out.extend(count(id));
    }
    out
}

fn bytes(out: &mut Vec<u8>, text: &str) {
    out.extend(count(text.len()));
    out.extend(text.as_bytes());
}

fn count(n: usize) -> [u8; 4] {
    let n = u32::try_from(n).unwrap_or_else(|_| panic!("{n} does not fit a TOC3.0 count"));
    n.to_le_bytes()
}
