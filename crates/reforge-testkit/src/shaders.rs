//! Files of the shader cache in memory, for tests: a `TOC3.0` table of one file of a shader's
//! programs, written as `ltk_shader` reads one.

use reforge::game::variant_key;

/// A `TOC3.0` file whose variants are made of these base defines and are these filtered defines,
/// each written as the client's log writes it.
pub fn toc(base: &[(&str, &str)], variants: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    text(&mut out, "TOC3.0");
    // The count of variants, of base defines, of bundles, and the stage.
    for n in [variants.len(), base.len(), variants.len(), 0] {
        out.extend(count(n));
    }
    text(&mut out, "baseDefines");
    for (name, value) in base {
        text(&mut out, name);
        text(&mut out, value);
    }
    text(&mut out, "shaders");
    for filtered in variants {
        out.extend(variant_key(filtered).to_le_bytes());
    }
    for id in 0..variants.len() {
        out.extend(count(id));
    }
    out
}

fn text(out: &mut Vec<u8>, text: &str) {
    out.extend(count(text.len()));
    out.extend(text.as_bytes());
}

fn count(n: usize) -> [u8; 4] {
    let n = u32::try_from(n).unwrap_or_else(|_| panic!("{n} does not fit a TOC3.0 count"));
    n.to_le_bytes()
}
