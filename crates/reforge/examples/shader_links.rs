//! The shaders the bins of mod archives name and define, for `scripts/corpus/shaders.py`
//! (#GAME-CHANGE-0024, #GAME-BEHAVIOR-0022, #GAME-BEHAVIOR-0023).
//!
//! `cargo run --release -p reforge --example shader_links -- <game> <list>` reads each archive
//! the list names, one path per line, in memory, and prints one TSV row per finding:
//!
//! | column | holds |
//! |---|---|
//! | `sha256` | the archive's SHA-256 |
//! | `wad` | the WAD's name |
//! | `chunk` | the path hash of the bin |
//! | `kind` | `def`, a `CustomShaderDef` of the bin, or `link`, a material pass's shader |
//! | `entry` | the definition's entry hash, or the material's |
//! | `shader` | the shader's entry hash: the definition's own, or the one the pass links |
//! | `in_game` | whether the game's shader bin defines that shader |

use camino::Utf8Path;
use ltk_meta::PropertyValueEnum as V;
use reforge::archive::{self, Caps};
use reforge::diag::{Diagnostics, Stage};
use reforge::game::Install;
use reforge::model::walk::structs;
use reforge::model::{BinHash, bin, name_hash};
use reforge::store::{BlobStore, MemoryStore};
use reforge_testkit::ingame::GameBin;

const SHADER_BIN: &str = "data/shaders/shaders.bin";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [game, list] = args.as_slice() else {
        panic!("usage: shader_links <game> <list>; got {args:?}");
    };
    let install = Install::open(Utf8Path::new(game))
        .unwrap_or_else(|error| panic!("the install at {game} does not open: {error}"));
    let game_shaders: Vec<BinHash> = GameBin::read(&install, SHADER_BIN)
        .objects
        .iter()
        .map(|object| object.path_hash)
        .collect();
    let (definition, pass) = (
        name_hash("CustomShaderDef"),
        name_hash("StaticMaterialPassDef"),
    );
    let shader_field = name_hash("shader");
    let text = fs_err::read_to_string(list)
        .unwrap_or_else(|error| panic!("{list} does not read: {error}"));
    println!("sha256\twad\tchunk\tkind\tentry\tshader\tin_game");
    for path in text.lines().filter(|line| !line.is_empty()) {
        let bytes = fs_err::read(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let mut log = Diagnostics::new();
        let store = MemoryStore::new();
        let caps = Caps::default();
        let read = archive::read(&bytes, path, &caps, &store, &mut log.at(Stage::Read));
        let Ok(raw) = read else {
            eprintln!("not read: {path}");
            continue;
        };
        let sha = raw.meta.sha256.hex();
        let files = raw
            .wads
            .iter()
            .flat_map(|wad| wad.files.iter().map(move |(hash, file)| (wad, *hash, file)));
        for (wad, hash, file) in files {
            let Some(bytes) = store.get(file.content) else {
                continue;
            };
            let Ok(parsed) = bin::parse(&bytes) else {
                continue;
            };
            let row = |kind: &str, entry: BinHash, shader: BinHash| {
                let in_game = game_shaders.contains(&shader);
                println!(
                    "{sha}\t{}\t{:016x}\t{kind}\t{:08x}\t{:08x}\t{in_game}",
                    wad.name, hash.0, entry.0, shader.0
                );
            };
            for object in parsed.objects.iter().map(|object| &object.value) {
                if object.class_hash == definition {
                    row("def", object.path_hash, object.path_hash);
                }
                structs(object, |s| {
                    if s.class() != pass {
                        return;
                    }
                    if let Some(V::ObjectLink(link)) = s.get(shader_field) {
                        row("link", s.entry(), link.value);
                    }
                });
            }
        }
    }
}
