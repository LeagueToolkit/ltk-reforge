//! The in-game tests of which of two entries of one hash in a bin the client keeps
//! (#GAME-BEHAVIOR-0020). For one champion's base skin it writes one mod per case: the game's
//! `skin0.bin`, its skin entry listed once or twice, as a `.fantome`.
//!
//! The changed entry draws the whole model with another texture: the one the bin's first
//! emitter draws, a file of the champion's own WAD. The other entry is the game's.
//!
//! | case | the bin's skin entries, in file order |
//! |---|---|
//! | `control` | the changed one |
//! | `changed-first` | the changed one, then the game's |
//! | `changed-last` | the game's, then the changed one |
//!
//! `cargo run -p reforge --example entry_twice_tests -- <game> <champion> <out>` writes
//! `<out>/entry-twice-test-<champion>-<case>.fantome`.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::game::Game;
use reforge::model::value::as_str;
use reforge::model::walk::{structs, structs_mut};
use reforge::model::{bin, name_hash, path_hash};
use reforge_testkit::ingame::{self, BaseSkin, GameBin};

/// Where the changed entry sits among the bin's skin entries.
#[derive(Debug, Clone, Copy)]
enum Order {
    /// The changed entry alone.
    Alone,
    /// The changed entry, then the game's.
    ChangedFirst,
    /// The game's entry, then the changed one.
    ChangedLast,
}

/// One mod: the order of its skin entries, and what it asks.
struct Case {
    name: &'static str,
    order: Order,
    asks: &'static str,
}

const CASES: [Case; 3] = [
    Case {
        name: "control",
        order: Order::Alone,
        asks: "The control: the skin entry once, drawing the model with another texture.",
    },
    Case {
        name: "changed-first",
        order: Order::ChangedFirst,
        asks: "The skin entry twice: first with another texture, then the game's.",
    },
    Case {
        name: "changed-last",
        order: Order::ChangedLast,
        asks: "The skin entry twice: first the game's, then with another texture.",
    },
];

/// The entry name the second entry is written under, then renamed in the bytes: `ltk_meta`
/// holds one object per entry hash and writes no bin that lists one twice.
const PLACEHOLDER: &str = "Reforge/Test/Entry_Twice_Placeholder";

/// The texture the bin's first emitter draws.
fn first_emitter_texture(skin: &GameBin) -> String {
    let class = name_hash("VfxEmitterDefinitionData");
    let mut found = None;
    for object in &skin.objects {
        structs(object, |s| {
            if found.is_none() && s.class() == class {
                found = s
                    .get(name_hash("texture"))
                    .and_then(as_str)
                    .map(str::to_owned);
            }
        });
    }
    found.unwrap_or_else(|| panic!("no emitter of the bin names a texture"))
}

/// The skin entry with its mesh drawing `texture`, in the value type the game's entry holds.
fn retextured(entry: &BinObject, texture: &str) -> BinObject {
    let mut entry = entry.clone();
    let (mesh, field) = (name_hash("SkinMeshDataProperties"), name_hash("texture"));
    let mut changed = 0;
    structs_mut(&mut entry, |mut s| {
        if s.class() != mesh {
            return;
        }
        let value = match s.fields().get(&field) {
            Some(V::WadChunkLink(_)) => {
                V::WadChunkLink(values::WadChunkLink::new(path_hash(texture)))
            }
            Some(V::String(_)) => V::String(values::String::from(texture)),
            other => panic!("the mesh's texture is neither a file nor a string: {other:?}"),
        };
        s.fields_mut().insert(field, value);
        changed += 1;
    });
    assert_eq!(changed, 1, "the skin entry holds {changed} meshes");
    entry
}

/// The bin's bytes with each listing of `from` renamed `to`.
fn renamed(mut bytes: Vec<u8>, from: &str, to: &str) -> Vec<u8> {
    let (from, to) = (
        name_hash(from).0.to_le_bytes(),
        name_hash(to).0.to_le_bytes(),
    );
    let at: Vec<usize> = bytes
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == from)
        .map(|(i, _)| i)
        .collect();
    let [at] = at.as_slice() else {
        panic!("the bin holds the placeholder's hash at {at:?}, not once");
    };
    bytes[*at..at + 4].copy_from_slice(&to);
    bytes
}

impl Case {
    /// The skin bin of this case: the game's, its entry changed, listed as the case orders.
    fn bin(&self, skin: &BaseSkin, changed: &BinObject) -> Vec<u8> {
        let entry_name = skin.entry_name();
        let game = skin.bin.object(&entry_name);
        let (first, second) = match self.order {
            Order::Alone => (changed, None),
            Order::ChangedFirst => (changed, Some(game)),
            Order::ChangedLast => (game, Some(changed)),
        };
        let mut bin = skin.bin.clone();
        *bin.object_mut(&entry_name) = first.clone();
        let Some(second) = second else {
            return bin.to_bytes();
        };
        let mut placed = second.clone();
        placed.path_hash = name_hash(PLACEHOLDER);
        bin.objects.push(placed);
        let bytes = renamed(bin.to_bytes(), PLACEHOLDER, &entry_name);
        let parsed = bin::parse(&bytes)
            .unwrap_or_else(|error| panic!("the bin of {} does not parse: {error}", self.name));
        assert_eq!(
            parsed.duplicates,
            [name_hash(&entry_name)],
            "the bin of {} lists the skin entry once",
            self.name
        );
        bytes
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (install, skin, out) = ingame::open("entry_twice_tests", &args);
    let champion = &skin.champion;

    let texture = first_emitter_texture(&skin.bin);
    // #GAME-BEHAVIOR-0007: a file of another WAD is there only when that WAD is mounted.
    assert!(
        install.wads(path_hash(&texture)).contains(&skin.wad),
        "{texture} is not in {}",
        skin.wad
    );
    let changed = retextured(skin.bin.object(&skin.entry_name()), &texture);
    println!("the changed entry draws {texture}");

    for case in &CASES {
        let name = format!("Entry twice test: {} ({champion})", case.name);
        let stem = format!("entry-twice-test-{champion}-{}", case.name);
        skin.write_mod(&out, &stem, &name, case.asks, &case.bin(&skin, &changed));
    }
}
