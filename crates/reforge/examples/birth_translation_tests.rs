//! The in-game tests of where the client draws an emitter whose offset is a `birthTranslation`
//! inside its `VfxShapeLegacy` (#MOD-ARTIFACT-0019). For one champion's base skin it writes one
//! mod per case: the game's `skin0.bin` with one emitter changed, as a `.fantome`.
//!
//! The emitter is one a replay of the base skin shows plainly, and it holds no `EmitterPosition`
//! and no `SpawnShape`. Each case gives it an empty `VfxShapeLegacy`, so the cases differ only in
//! where the offset is:
//!
//! | champion | system | emitter | shown |
//! |---|---|---|---|
//! | Zac | `Zac_Base_W_cas` | `Splat` | the green goo bursting up at his W |
//!
//! | case | `EmitterPosition` | `birthTranslation` in the shape |
//! |---|---|---|
//! | `control` | 0, 300, 0 | none |
//! | `misplaced` | none | 0, 300, 0 |
//! | `both` | 0, 300, 0 | 0, 150, 0 |
//!
//! `cargo run -p reforge --example birth_translation_tests -- <game> <champion> <out>` writes
//! `<out>/birth-translation-test-<champion>-<case>.fantome`.

use glam::Vec3;
use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;
use reforge::model::name_hash;
use reforge::model::walk::Fields;
use reforge_testkit::bins::{embedded, pointer};
use reforge_testkit::ingame;

const CHAMPION: &str = "zac";
const SYSTEM: &str = "Characters/Zac/Skins/Skin0/Particles/Zac_Base_W_cas";
const EMITTER: &str = "Splat";

/// One mod: the offsets its emitter holds, and what it asks.
struct Case {
    name: &'static str,
    /// The emitter's `EmitterPosition`.
    position: Option<Vec3>,
    /// The `birthTranslation` inside the emitter's `VfxShapeLegacy`.
    birth_translation: Option<Vec3>,
    asks: &'static str,
}

const CASES: [Case; 3] = [
    Case {
        name: "control",
        position: Some(Vec3::new(0.0, 300.0, 0.0)),
        birth_translation: None,
        asks: "The control: the emitter's EmitterPosition 300 units up.",
    },
    Case {
        name: "misplaced",
        position: None,
        birth_translation: Some(Vec3::new(0.0, 300.0, 0.0)),
        asks: "The same offset as a birthTranslation inside the emitter's VfxShapeLegacy, and no \
               EmitterPosition.",
    },
    Case {
        name: "both",
        position: Some(Vec3::new(0.0, 300.0, 0.0)),
        birth_translation: Some(Vec3::new(0.0, 150.0, 0.0)),
        asks: "EmitterPosition 300 units up, and a birthTranslation of 150 up inside the \
               VfxShapeLegacy.",
    },
];

/// A `ValueVector3` of a constant.
fn constant(value: Vec3) -> V {
    let value = V::Vector3(values::Vector3::new(value));
    embedded("ValueVector3", vec![("constantValue", value)])
}

impl Case {
    fn apply(&self, emitter: &mut Fields) {
        let (position, shape) = (name_hash("EmitterPosition"), name_hash("SpawnShape"));
        assert!(
            !emitter.contains_key(&position) && !emitter.contains_key(&shape),
            "the emitter holds an EmitterPosition or a SpawnShape of its own"
        );
        emitter.insert(shape, pointer("VfxShapeLegacy", vec![]));
        let Some(V::Struct(shape)) = emitter.get_mut(&shape) else {
            unreachable!("the shape was inserted as a pointer");
        };
        if let Some(offset) = self.birth_translation {
            shape
                .properties
                .insert(name_hash("birthTranslation"), constant(offset));
        }
        if let Some(offset) = self.position {
            emitter.insert(position, constant(offset));
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (_install, skin, out) = ingame::open("birth_translation_tests", &args);
    let champion = &skin.champion;
    assert_eq!(champion, CHAMPION, "the cases are for {CHAMPION}");

    for case in &CASES {
        let mut bin = skin.bin.clone();
        bin.change_emitter(SYSTEM, EMITTER, |fields| case.apply(fields));
        let name = format!("Birth translation test: {} ({champion})", case.name);
        let stem = format!("birth-translation-test-{champion}-{}", case.name);
        skin.write_mod(&out, &stem, &name, case.asks, &bin.to_bytes());
    }
}
