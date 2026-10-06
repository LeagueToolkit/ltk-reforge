//! The in-game tests of whether the client draws an emitter by its `LegacySimple`
//! (#GAME-CHANGE-0004, #GAME-BEHAVIOR-0024). For one champion's base skin it writes one mod per
//! case: the game's `skin0.bin` with at most one change to its emitters, as a `.fantome`.
//!
//! A system lists each emitter as complex or simple. Riot's simple emitters hold a
//! `LegacySimple` and its complex ones hold none. The emitters are ones a replay of the base skin
//! shows plainly:
//!
//! | champion | system | emitter | kind | shown |
//! |---|---|---|---|---|
//! | Zac | `Zac_Base_W_cas` | `Splat` | complex, no `LegacySimple` | the green goo bursting up at his W |
//! | Zac | `Zac_Base_W_cas` | `Shadow` | simple, a `LegacySimple` | the dark swirl on the ground at his W |
//! | Vi | `Vi_Base_Passive_BackPack` | `glow` | simple, a `LegacySimple` | the ring on her back while her passive is ready |
//! | Ziggs | `Ziggs_Base_Q_mis_01` to `_03`, `Ziggs_Base_BA_mis` | each | complex, meshes among them, no `LegacySimple` | the bomb of his Q and of his basic attack |
//!
//! | case | champion | the change |
//! |---|---|---|
//! | `control` | Zac | none |
//! | `simple-scaled` | Zac | `Splat` given a `LegacySimple`: `birthScale` its own birth size, `scale` 4 |
//! | `simple-zeroed` | Zac | `Splat` given a `LegacySimple`: `birthScale` 0, `scale` 0 |
//! | `kept-scaled` | Zac | the `scale` of the `LegacySimple` of `Shadow` four times the game's |
//! | `kept-removed` | Zac | the `LegacySimple` of `Shadow` removed |
//! | `kept-control` | Vi | none |
//! | `kept-scaled` | Vi | the `scale` of the `LegacySimple` of `glow` four times the game's |
//! | `kept-removed` | Vi | the `LegacySimple` of `glow` removed |
//! | `control` | Ziggs | none |
//! | `complex-zeroed` | Ziggs | each emitter of the bombs' systems given a `LegacySimple`: `birthScale` 0, `scale` 0 |
//!
//! `Splat` is born at a random rotation, so no case turns it.
//!
//! `cargo run -p reforge --example legacy_simple_tests -- <game> <champion> <out>` writes
//! `<out>/legacy-simple-test-<champion>-<case>.fantome` for the champion's cases.

use camino::{Utf8Path, Utf8PathBuf};
use ltk_meta::PropertyValueEnum as V;
use reforge::game::Install;
use reforge::model::name_hash;
use reforge::model::value::{each_mut, field_of};
use reforge::model::walk::Fields;
use reforge_testkit::bins::{embedded, float, pointer};
use reforge_testkit::ingame::{BaseSkin, GameBin};

const COMPLEX: &str = "complexEmitterDefinitionData";
const SIMPLE: &str = "simpleEmitterDefinitionData";

/// The emitters a case changes.
#[derive(Debug, Clone, Copy)]
enum Target {
    /// One emitter of a system, by its name.
    Emitter {
        system: &'static str,
        name: &'static str,
    },
    /// Every emitter of these systems, each of which lists only complex emitters.
    Complex(&'static [&'static str]),
}

/// What a case does to each emitter it changes.
#[derive(Debug, Clone, Copy)]
enum Change {
    /// A `LegacySimple` added: its `birthScale` the emitter's own birth size, and this `scale`.
    Added { scale: f32 },
    /// A `LegacySimple` added with `birthScale` 0 and `scale` 0.
    Zeroed,
    /// The `scale` of the emitter's `LegacySimple` multiplied: its constant and its curve.
    Scaled(f32),
    /// The emitter's `LegacySimple` removed.
    Removed,
}

/// One mod: its name, the emitters it changes and how, none for a control, and what it asks.
struct Case {
    name: &'static str,
    change: Option<(Target, Change)>,
    asks: &'static str,
}

/// A champion and its cases.
struct Champion {
    name: &'static str,
    cases: &'static [Case],
}

const ZAC_SPLAT: Target = Target::Emitter {
    system: "Characters/Zac/Skins/Skin0/Particles/Zac_Base_W_cas",
    name: "Splat",
};
const ZAC_SHADOW: Target = Target::Emitter {
    system: "Characters/Zac/Skins/Skin0/Particles/Zac_Base_W_cas",
    name: "Shadow",
};
const VI_GLOW: Target = Target::Emitter {
    system: "Characters/Vi/Skins/Skin0/Particles/Vi_Base_Passive_BackPack",
    name: "glow",
};
const ZIGGS_BOMBS: Target = Target::Complex(&[
    "Characters/Ziggs/Skins/Skin0/Particles/Ziggs_Base_Q_mis_01",
    "Characters/Ziggs/Skins/Skin0/Particles/Ziggs_Base_Q_mis_02",
    "Characters/Ziggs/Skins/Skin0/Particles/Ziggs_Base_Q_mis_03",
    "Characters/Ziggs/Skins/Skin0/Particles/Ziggs_Base_BA_mis",
]);

const CHAMPIONS: [Champion; 3] = [
    Champion {
        name: "zac",
        cases: &[
            Case {
                name: "control",
                change: None,
                asks: "The control: the skin bin as the game holds it.",
            },
            Case {
                name: "simple-scaled",
                change: Some((ZAC_SPLAT, Change::Added { scale: 4.0 })),
                asks: "The complex emitter Splat given a LegacySimple: birthScale its own birth \
                       size, scale 4. Drawn by it, the green goo of W is four times the size.",
            },
            Case {
                name: "simple-zeroed",
                change: Some((ZAC_SPLAT, Change::Zeroed)),
                asks: "The complex emitter Splat given a LegacySimple: birthScale 0, scale 0. \
                       Drawn by it, the green goo of W is not seen.",
            },
            Case {
                name: "kept-scaled",
                change: Some((ZAC_SHADOW, Change::Scaled(4.0))),
                asks: "The simple emitter Shadow with the scale of its LegacySimple four times \
                       the game's.",
            },
            Case {
                name: "kept-removed",
                change: Some((ZAC_SHADOW, Change::Removed)),
                asks: "The simple emitter Shadow with its LegacySimple removed.",
            },
        ],
    },
    Champion {
        name: "vi",
        cases: &[
            Case {
                name: "kept-control",
                change: None,
                asks: "The control: the skin bin as the game holds it.",
            },
            Case {
                name: "kept-scaled",
                change: Some((VI_GLOW, Change::Scaled(4.0))),
                asks: "The simple emitter glow with the scale of its LegacySimple four times the \
                       game's.",
            },
            Case {
                name: "kept-removed",
                change: Some((VI_GLOW, Change::Removed)),
                asks: "The simple emitter glow with its LegacySimple removed.",
            },
        ],
    },
    Champion {
        name: "ziggs",
        cases: &[
            Case {
                name: "control",
                change: None,
                asks: "The control: the skin bin as the game holds it.",
            },
            Case {
                name: "complex-zeroed",
                change: Some((ZIGGS_BOMBS, Change::Zeroed)),
                asks: "Each complex emitter of the bombs of Q and of the basic attack given a \
                       LegacySimple: birthScale 0, scale 0. Drawn by it, the bombs are not seen.",
            },
        ],
    },
];

const LEGACY_SIMPLE: &str = "LegacySimple";

/// A `ValueFloat` of a constant.
fn constant(value: f32) -> V {
    embedded("ValueFloat", vec![("constantValue", float(value))])
}

/// The largest axis of the emitter's constant `birthScale0`: its birth size.
fn birth_size(emitter: &Fields) -> f32 {
    let size = emitter
        .get(&name_hash("birthScale0"))
        .and_then(|scale| field_of(scale, "constantValue"));
    match size {
        Some(V::Vector3(size)) => size.value.max_element(),
        other => panic!("the emitter holds no constant birthScale0: {other:?}"),
    }
}

/// A `ValueFloat` multiplied: its constant and each value of its curve, not the curve's times.
fn multiply(value: &mut V, factor: f32) {
    let V::Embedded(float) = value else {
        panic!("the value is no ValueFloat: {value:?}");
    };
    let fields = &mut float.0.properties;
    if let Some(V::F32(constant)) = fields.get_mut(&name_hash("constantValue")) {
        constant.value *= factor;
    }
    if let Some(V::Struct(curve)) = fields.get_mut(&name_hash("dynamics"))
        && let Some(values) = curve.properties.get_mut(&name_hash("values"))
    {
        each_mut(values, &mut |item| {
            if let V::F32(item) = item {
                item.value *= factor;
            }
        });
    }
}

impl Change {
    fn apply(self, emitter: &mut Fields) {
        let legacy = name_hash(LEGACY_SIMPLE);
        let (size, scale) = match self {
            Change::Added { scale } => (birth_size(emitter), scale),
            Change::Zeroed => (0.0, 0.0),
            Change::Scaled(factor) => {
                let scale = match emitter.get_mut(&legacy) {
                    Some(V::Struct(simple)) => simple.properties.get_mut(&name_hash("scale")),
                    other => panic!("the emitter holds no LegacySimple to scale: {other:?}"),
                };
                let scale = scale.unwrap_or_else(|| panic!("the LegacySimple holds no scale"));
                multiply(scale, factor);
                return;
            }
            Change::Removed => {
                emitter
                    .shift_remove(&legacy)
                    .unwrap_or_else(|| panic!("the emitter holds no LegacySimple to remove"));
                return;
            }
        };
        assert!(
            !emitter.contains_key(&legacy),
            "the emitter holds a LegacySimple to add one to"
        );
        let fields = vec![("birthScale", constant(size)), ("scale", constant(scale))];
        emitter.insert(legacy, pointer("VfxEmitterLegacySimple", fields));
    }
}

impl Target {
    fn apply(self, bin: &mut GameBin, change: Change) {
        match self {
            Target::Emitter { system, name } => {
                bin.change_emitter(system, name, |fields| change.apply(fields));
            }
            Target::Complex(systems) => {
                for system in systems {
                    let simple = bin.change_emitters(system, SIMPLE, |_| {});
                    assert_eq!(simple, 0, "{system} lists {simple} simple emitters");
                    let complex =
                        bin.change_emitters(system, COMPLEX, |fields| change.apply(fields));
                    assert!(complex > 0, "{system} lists no complex emitter");
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [game, champion, out] = args.as_slice() else {
        panic!("usage: legacy_simple_tests <game> <champion> <out>; got {args:?}");
    };
    let install = Install::open(Utf8Path::new(game))
        .unwrap_or_else(|error| panic!("the install at {game} does not open: {error}"));
    let skin = BaseSkin::read(&install, champion);
    let champion = &skin.champion;
    let cases = CHAMPIONS
        .iter()
        .find(|known| known.name == champion)
        .unwrap_or_else(|| {
            let known: Vec<_> = CHAMPIONS.iter().map(|c| c.name).collect();
            panic!("no cases for {champion}; the cases are for {known:?}")
        })
        .cases;

    let out = Utf8PathBuf::from(out);
    fs_err::create_dir_all(&out).unwrap_or_else(|error| panic!("{out} is not made: {error}"));
    for case in cases {
        let mut bin = skin.bin.clone();
        if let Some((target, change)) = case.change {
            target.apply(&mut bin, change);
        }
        let name = format!("LegacySimple test: {} ({champion})", case.name);
        let written = skin.fantome(&name, case.asks, &bin.to_bytes());
        let file = out.join(format!(
            "legacy-simple-test-{champion}-{}.fantome",
            case.name
        ));
        fs_err::write(&file, written)
            .unwrap_or_else(|error| panic!("{file} is not written: {error}"));
        println!("{file}");
    }
}
