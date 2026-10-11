//! Names: the tables, the names a run learns, and the speller that reads both in one order for
//! every kind of hash. A name that does not hash back to its hash is refused.

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_hashdb::{Compression, HashDbWriter};
use ltk_meta::path::{FieldNames, ValuePath, ValueSegment};
use ltk_mimir_cache::{CommitItem, HashStore, Table};
use reforge::diag::{Channel, Diagnostics, Input, Render, Severity, Stage, Subject};
use reforge::game::{
    CachedTable, MimirAbsent, NameBook, NameHashSlot, NameSource, NameTable, Names, NamesError,
    NamesStamp, Speller,
};
use reforge::model::{BinHash, ContentId, Spell, WadHash, name_hash, path_hash};
use tempfile::TempDir;

/// Two paths that share one name hash, `9ce00afc`.
const TWIN_A: &str = "assets/characters/test/particles/ibyfnlh.tex";
const TWIN_B: &str = "assets/characters/test/particles/mayaenu.tex";

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path =
        Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("the directory is UTF-8");
    (dir, path)
}

/// One table of a hash cache: names under their own hashes, and names under the hashes given.
#[derive(Clone, Copy)]
struct CacheTable<'a> {
    table: Table,
    names: &'a [&'a str],
    under: &'a [(u64, &'a str)],
}

/// Publishes a hash cache in `dir` as Mimir's updater does.
fn hash_cache(dir: &Utf8Path, version: &str, tables: &[CacheTable<'_>]) {
    let staging = dir.join("staging");
    fs::create_dir_all(&staging).unwrap();
    let mut items = Vec::new();
    for CacheTable {
        table,
        names,
        under,
    } in tables
    {
        let config = table.key_config();
        let mut writer = HashDbWriter::with_key_config(config, Compression::None);
        for name in *names {
            writer.insert(config.hash(name), name);
        }
        for (hash, name) in *under {
            writer.insert(*hash, name);
        }
        let file = staging.join(format!("{table}.lhdb"));
        writer.build(fs::File::create(&file).unwrap()).unwrap();
        items.push(CommitItem::new(*table, version, file));
    }
    HashStore::at(dir).commit(&items, None).unwrap();
}

fn text_table(dir: &Utf8Path, file: &str, lines: &[(u64, &str)]) {
    let text: String = lines
        .iter()
        .map(|(hash, name)| format!("{hash:x} {name}\n"))
        .collect();
    fs::write(dir.join(file), text).unwrap();
}

fn raised(names: &Names) -> Diagnostics {
    let mut log = Diagnostics::new();
    names.report(&mut log.at(Stage::Read));
    log
}

#[test]
fn a_run_spells_the_names_it_learnt_without_any_table() {
    let names = Names::default();
    let mut book = NameBook::new();
    let field = book.learn_field("skinMeshProperties");
    assert_eq!(field, name_hash("skinMeshProperties"));
    book.learn_field("animationGraphData");
    let speller = Speller::new(&book, &names);
    assert_eq!(
        Spell::field(&speller, field).as_deref(),
        Some("skinMeshProperties")
    );
    assert_eq!(
        Spell::field(&speller, name_hash("animationGraphData")).as_deref(),
        Some("animationGraphData")
    );
    assert_eq!(
        Spell::field(&speller, name_hash("someFieldNobodyNamed")),
        None
    );
    assert_eq!(
        speller.field_or_hex(BinHash(0x1234_5678)),
        "12345678",
        "a hash no source names prints as hex"
    );
    // A field name is no entry name: each kind of hash has its own names.
    assert_eq!(speller.entry(field), None);
    assert_eq!(speller.class(field), None);
    assert_eq!(speller.hash(field), None);
}

#[test]
fn a_name_in_a_text_table_must_hash_back_to_its_hash() {
    let (_keep, dir) = scratch();
    let good = u64::from(name_hash("idleParticlesEffects").0);
    let later = u64::from(name_hash("skeleton").0);
    text_table(
        &dir,
        "hashes.binfields.txt",
        &[
            (good, "idleParticlesEffects"),
            (0x1234_5678, "notThisOne"),
            // A wrong name does not take the place of the right one after it.
            (later, "notTheSkeleton"),
            (later, "skeleton"),
        ],
    );
    let names = Names::builder().text_tables(&dir).build().unwrap();
    assert_eq!(
        names.field(name_hash("idleParticlesEffects")).as_deref(),
        Some("idleParticlesEffects")
    );
    assert_eq!(names.field(BinHash(0x1234_5678)), None);
    assert_eq!(
        names.field(name_hash("skeleton")).as_deref(),
        Some("skeleton")
    );
}

#[test]
fn a_name_in_the_hash_cache_must_hash_back_to_its_hash() {
    let (_keep, dir) = scratch();
    let cache = dir.join("cache");
    let text = dir.join("text");
    fs::create_dir_all(&text).unwrap();
    let field = name_hash("skeleton");
    let path = path_hash("data/characters/lux/lux.bin");
    hash_cache(
        &cache,
        "1",
        &[
            CacheTable {
                table: Table::BinFields,
                names: &[],
                under: &[
                    (u64::from(field.0), "notTheSkeleton"),
                    (0x1234_5678, "alsoWrong"),
                ],
            },
            CacheTable {
                table: Table::Game,
                names: &[],
                under: &[(path.0, "data/characters/lux/wrong.bin")],
            },
        ],
    );
    text_table(
        &text,
        "hashes.binfields.txt",
        &[(u64::from(field.0), "skeleton")],
    );
    let names = Names::builder()
        .mimir(&cache)
        .text_tables(&text)
        .build()
        .unwrap();
    assert_eq!(
        names.field(field).as_deref(),
        Some("skeleton"),
        "the text table's name stands where the cache's is wrong"
    );
    assert_eq!(names.field(BinHash(0x1234_5678)), None);
    assert_eq!(names.path(path), None);
    assert!(names.paths_with_prefix("data/characters/lux/").is_empty());
}

#[test]
fn the_speller_spells_a_record_path() {
    let names = Names::default();
    let mut book = NameBook::new();
    for field in ["skinAnimationProperties", "animationGraphData", "skeleton"] {
        book.learn_field(field);
    }
    book.learn_hash("Idle1");
    let speller = Speller::new(&book, &names);
    let path: ValuePath = [
        ValueSegment::Field(name_hash("skinAnimationProperties")),
        ValueSegment::Field(name_hash("animationGraphData")),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        path.to_property_path(&speller).unwrap().as_str(),
        "skinAnimationProperties.animationGraphData"
    );
    // A field no source names cannot be spelled, so no record can address it.
    let unnamed: ValuePath = [ValueSegment::Field(name_hash("someFieldNobodyNamed"))]
        .into_iter()
        .collect();
    assert!(unnamed.to_property_path(&speller).is_err());
    assert!(FieldNames::field(&speller, name_hash("skeleton"), None).is_some());
    assert_eq!(
        FieldNames::hash(&speller, name_hash("Idle1")).as_deref(),
        Some("Idle1")
    );
}

/// A cache and a text directory that spell one name of each kind, each in a case of its own, so
/// the answer says which source gave it.
struct ThreeSources {
    _keep: TempDir,
    cache: Utf8PathBuf,
    text: Utf8PathBuf,
}

const PATH: &str = "assets/characters/lux/skins/base/lux.skn";
const ENTRY: &str = "Characters/Lux/Skins/Skin0";
const FIELD: &str = "skinMeshProperties";
const CLASS: &str = "SkinCharacterDataProperties";
const HASH: &str = "Lux_Base_Q_Tar";

impl ThreeSources {
    fn new() -> Self {
        let (keep, dir) = scratch();
        let cache = dir.join("cache");
        let text = dir.join("text");
        fs::create_dir_all(&text).unwrap();
        let mixed = |name: &str| -> String {
            name.chars()
                .enumerate()
                .map(|(i, c)| {
                    if i % 2 == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect()
        };
        let cached = [PATH, ENTRY, FIELD, CLASS, HASH].map(mixed);
        let tables = [
            Table::Game,
            Table::BinEntries,
            Table::BinFields,
            Table::BinTypes,
            Table::BinHashes,
        ];
        let names: Vec<[&str; 1]> = cached.iter().map(|name| [name.as_str()]).collect();
        let tables: Vec<CacheTable<'_>> = tables
            .iter()
            .zip(&names)
            .map(|(table, names)| CacheTable {
                table: *table,
                names,
                under: &[],
            })
            .collect();
        hash_cache(&cache, "1", &tables);
        for (file, name) in [
            ("hashes.game.txt", PATH),
            ("hashes.binentries.txt", ENTRY),
            ("hashes.binfields.txt", FIELD),
            ("hashes.bintypes.txt", CLASS),
            ("hashes.binhashes.txt", HASH),
        ] {
            let upper = name.to_ascii_uppercase();
            let hash = if file == "hashes.game.txt" {
                path_hash(name).0
            } else {
                u64::from(name_hash(name).0)
            };
            text_table(&text, file, &[(hash, &upper)]);
        }
        Self {
            _keep: keep,
            cache,
            text,
        }
    }

    fn both(&self) -> Names {
        Names::builder()
            .mimir(&self.cache)
            .text_tables(&self.text)
            .keep_text_paths(|_| true)
            .build()
            .unwrap()
    }

    fn text_only(&self) -> Names {
        Names::builder()
            .text_tables(&self.text)
            .keep_text_paths(|_| true)
            .build()
            .unwrap()
    }
}

/// What a speller answers for the five names, in the order path, entry, field, class, hash.
fn spelled(speller: &Speller<'_>) -> [Option<String>; 5] {
    [
        speller.path(path_hash(PATH)),
        speller.entry(name_hash(ENTRY)),
        Spell::field(speller, name_hash(FIELD)),
        speller.class(name_hash(CLASS)),
        speller.hash(name_hash(HASH)),
    ]
}

fn learnt() -> NameBook {
    let mut book = NameBook::new();
    book.learn_path(PATH);
    book.learn_entry(ENTRY);
    book.learn_field(FIELD);
    book.learn_class(CLASS);
    book.learn_hash(HASH);
    book
}

#[test]
fn every_kind_of_hash_is_spelled_from_the_run_then_the_cache_then_the_text_tables() {
    let sources = ThreeSources::new();
    let (both, text_only) = (sources.both(), sources.text_only());
    let nothing = NameBook::new();
    let book = learnt();
    let own = [PATH, ENTRY, FIELD, CLASS, HASH].map(|name| Some(name.to_string()));

    // The run's names come first, whatever the tables hold.
    assert_eq!(spelled(&Speller::new(&book, &both)), own);
    assert_eq!(spelled(&Speller::new(&book, &text_only)), own);
    assert_eq!(spelled(&Speller::new(&book, &Names::default())), own);

    // Then the hash cache, which spells every other letter in capitals here.
    let cached = spelled(&Speller::new(&nothing, &both));
    assert_eq!(
        cached.each_ref().map(|name| name.as_deref()),
        [
            Some("AsSeTs/cHaRaCtErS/LuX/SkInS/BaSe/lUx.sKn"),
            Some("ChArAcTeRs/lUx/sKiNs/sKiN0"),
            Some("SkInMeShPrOpErTiEs"),
            Some("SkInChArAcTeRdAtApRoPeRtIeS"),
            Some("LuX_BaSe_q_tAr"),
        ]
    );

    // Then the text tables, which spell in capitals here.
    let from_text = spelled(&Speller::new(&nothing, &text_only));
    let capitals = [PATH, ENTRY, FIELD, CLASS, HASH].map(|name| Some(name.to_ascii_uppercase()));
    assert_eq!(from_text, capitals);

    // The tables alone answer in the same order.
    assert_eq!(both.path(path_hash(PATH)), cached[0]);
    assert_eq!(both.entry(name_hash(ENTRY)), cached[1]);
    assert_eq!(both.field(name_hash(FIELD)), cached[2]);
    assert_eq!(both.class(name_hash(CLASS)), cached[3]);
    assert_eq!(both.hash(name_hash(HASH)), cached[4]);

    assert_eq!(
        both.sources(),
        [
            NameSource::Mimir {
                dir: sources.cache.clone(),
                tables: NameTable::ALL.to_vec(),
            },
            NameSource::TextTables {
                dir: sources.text.clone(),
                files: 5,
            },
        ]
    );
    assert!(both.has_mimir() && !text_only.has_mimir());
    assert!(raised(&both).is_empty() && raised(&text_only).is_empty());
}

#[test]
fn the_stamp_names_each_cache_table_by_its_manifest_entry_and_each_text_file_by_its_digest() {
    let sources = ThreeSources::new();
    let both = sources.both();
    let manifest = HashStore::at(&sources.cache).manifest().unwrap();
    let cache: Vec<CachedTable> = NameTable::ALL
        .iter()
        .zip([
            Table::Game,
            Table::BinEntries,
            Table::BinFields,
            Table::BinTypes,
            Table::BinHashes,
        ])
        .map(|(&table, mimir)| {
            let entry = manifest.entry(mimir).unwrap();
            assert_eq!(entry.version, "1");
            CachedTable {
                table,
                version: entry.version.clone(),
                sha256: entry.sha256.clone(),
            }
        })
        .collect();
    assert_eq!(both.stamp().cache, cache);

    let text = both.stamp().text.iter();
    let files: Vec<(NameTable, &str)> = text.map(|f| (f.table, f.file.as_str())).collect();
    assert_eq!(
        files,
        [
            (NameTable::Entries, "hashes.binentries.txt"),
            (NameTable::Fields, "hashes.binfields.txt"),
            (NameTable::Hashes, "hashes.binhashes.txt"),
            (NameTable::Classes, "hashes.bintypes.txt"),
            (NameTable::Paths, "hashes.game.txt"),
        ]
    );
    for file in &both.stamp().text {
        let bytes = fs::read(sources.text.join(&file.file)).unwrap();
        assert_eq!(file.sha256, ContentId::of(&bytes), "{}", file.file);
    }
    assert!(sources.text_only().stamp().cache.is_empty());
    assert_eq!(Names::default().stamp(), &NamesStamp::default());
}

#[test]
fn the_first_name_learnt_for_a_hash_stays() {
    let mut book = NameBook::new();
    let first = book.learn_entry("Characters/Lux/Skins/Skin0");
    let second = book.learn_entry("characters/lux/skins/skin0");
    assert_eq!(first, second);
    assert_eq!(book.entry(first), Some("Characters/Lux/Skins/Skin0"));
    book.learn_field("mField");
    book.learn_field("MFIELD");
    assert_eq!(book.field(name_hash("mfield")), Some("mField"));
    book.learn_class("VfxSystemDefinitionData");
    book.learn_class("vfxsystemdefinitiondata");
    assert_eq!(
        book.class(name_hash("VFXSYSTEMDEFINITIONDATA")),
        Some("VfxSystemDefinitionData")
    );
    book.learn_hash("Lux_Q");
    book.learn_hash("lux_q");
    assert_eq!(book.hash(name_hash("lux_q")), Some("Lux_Q"));

    // Of two paths with one name hash, the first learnt is the one the name hash finds.
    assert_eq!(name_hash(TWIN_A), name_hash(TWIN_B));
    let a = book.learn_path(TWIN_A);
    let b = book.learn_path(TWIN_B);
    assert_ne!(a, b);
    assert_eq!(book.path(a), Some(TWIN_A));
    assert_eq!(book.path(b), Some(TWIN_B));
    assert_eq!(book.path_by_name_hash(name_hash(TWIN_B)), Some(TWIN_A));
}

#[test]
fn a_path_is_learnt_lowercase() {
    let mut book = NameBook::new();
    let hash = book.learn_path("ASSETS/Characters/Lux/Skins/Base/Lux.skn");
    assert_eq!(hash, path_hash("assets/characters/lux/skins/base/lux.skn"));
    assert_eq!(
        book.path(hash),
        Some("assets/characters/lux/skins/base/lux.skn")
    );
    assert_eq!(book.path(WadHash(1)), None);
}

#[test]
fn a_book_lists_what_it_learnt_in_hash_order_whatever_the_order_learnt() {
    let paths = ["assets/c.tex", "assets/a.tex", "assets/b.tex"];
    let entries = ["Entry/C", "Entry/A", "Entry/B"];
    let mut forward = NameBook::new();
    let mut backward = NameBook::new();
    for i in 0..3 {
        forward.learn_path(paths[i]);
        forward.learn_entry(entries[i]);
        forward.learn_field(entries[i]);
        forward.learn_class(entries[i]);
        forward.learn_hash(entries[i]);
        backward.learn_path(paths[2 - i]);
        backward.learn_entry(entries[2 - i]);
        backward.learn_field(entries[2 - i]);
        backward.learn_class(entries[2 - i]);
        backward.learn_hash(entries[2 - i]);
    }
    let listed: Vec<(WadHash, &str)> = forward.paths().collect();
    assert_eq!(listed, backward.paths().collect::<Vec<_>>());
    assert!(listed.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert_eq!(listed.len(), 3);

    let listed: Vec<(BinHash, &str)> = forward.entries().collect();
    assert!(listed.windows(2).all(|pair| pair[0].0 < pair[1].0));
    for (ours, theirs) in [
        (listed.clone(), backward.entries().collect::<Vec<_>>()),
        (forward.fields().collect(), backward.fields().collect()),
        (forward.classes().collect(), backward.classes().collect()),
        (forward.hashes().collect(), backward.hashes().collect()),
    ] {
        assert_eq!(ours, theirs);
        assert_eq!(ours, listed);
    }
}

#[test]
fn a_hash_cache_that_is_not_there_is_reported_once_with_its_tables() {
    let (_keep, dir) = scratch();
    let names = Names::builder().mimir(dir.join("nowhere")).build().unwrap();
    assert!(!names.has_mimir());
    assert!(names.sources().is_empty());

    let log = raised(&names);
    let found: Vec<&MimirAbsent> = log.of::<MimirAbsent>().collect();
    assert_eq!(found.len(), 1);
    let tables: Vec<NameTable> = found[0].tables.iter().map(|absent| absent.table).collect();
    assert_eq!(tables, NameTable::ALL);
    assert!(
        found[0]
            .tables
            .iter()
            .all(|absent| !absent.reason.is_empty())
    );

    let diagnostic = log.iter().next().unwrap();
    assert_eq!(diagnostic.code.name, "names.mimir-absent");
    assert_eq!(diagnostic.severity(), Severity::Note);
    assert_eq!(diagnostic.channel(), Channel::Run);
    assert_eq!(diagnostic.subject, Subject::Input(Input::Names));
    assert_eq!(
        diagnostic.message(&Render::plain()),
        "the hash cache was not found; names come from the text tables only"
    );
    assert_eq!(diagnostic.data()["tables"][0]["table"], "paths");
}

#[test]
fn a_hash_cache_with_tables_missing_names_them() {
    let (_keep, dir) = scratch();
    hash_cache(
        &dir,
        "1",
        &[
            CacheTable {
                table: Table::Game,
                names: &[PATH],
                under: &[],
            },
            CacheTable {
                table: Table::BinEntries,
                names: &[ENTRY],
                under: &[],
            },
        ],
    );
    let names = Names::builder().mimir(&dir).build().unwrap();
    assert!(names.has_mimir());
    assert_eq!(names.path(path_hash(PATH)).as_deref(), Some(PATH));
    let log = raised(&names);
    let absent = log.of::<MimirAbsent>().next().unwrap();
    let tables: Vec<NameTable> = absent.tables.iter().map(|absent| absent.table).collect();
    assert_eq!(
        tables,
        [NameTable::Fields, NameTable::Classes, NameTable::Hashes]
    );
    assert_eq!(
        log.iter().next().unwrap().message(&Render::plain()),
        "3 tables of the hash cache not found: fields, classes, hashes"
    );
}

#[test]
fn a_machine_with_no_cache_directory_has_the_same_finding() {
    let mut log = Diagnostics::new();
    log.at(Stage::Read).raise(MimirAbsent::no_directory());
    let absent = log.of::<MimirAbsent>().next().unwrap();
    let tables: Vec<NameTable> = absent.tables.iter().map(|absent| absent.table).collect();
    assert_eq!(tables, NameTable::ALL);
    assert_eq!(
        log.iter().next().unwrap().message(&Render::plain()),
        "the hash cache was not found; names come from the text tables only"
    );
}

#[test]
fn names_built_without_a_hash_cache_report_nothing() {
    assert!(raised(&Names::default()).is_empty());
    assert!(raised(&Names::builder().build().unwrap()).is_empty());
}

#[test]
fn a_text_table_directory_that_does_not_read_is_an_error() {
    let (_keep, dir) = scratch();
    let error = Names::builder()
        .text_tables(dir.join("nowhere"))
        .build()
        .unwrap_err();
    assert!(matches!(error, NamesError::Io(_)), "{error}");
}

#[test]
fn text_tables_keep_the_paths_of_characters_unless_told_otherwise() {
    let (_keep, dir) = scratch();
    let character = "assets/characters/lux/skins/base/lux.skn";
    let map = "assets/maps/particles/glow.tex";
    // The path table comes in parts.
    text_table(
        &dir,
        "hashes.game.txt.0",
        &[(path_hash(character).0, character)],
    );
    text_table(&dir, "hashes.game.txt.1", &[(path_hash(map).0, map)]);
    fs::write(dir.join("readme.txt"), "not a table").unwrap();

    let names = Names::builder().text_tables(&dir).build().unwrap();
    assert_eq!(names.path(path_hash(character)).as_deref(), Some(character));
    assert_eq!(names.path(path_hash(map)), None);
    assert_eq!(
        names.sources(),
        [NameSource::TextTables {
            dir: dir.clone(),
            files: 2
        }]
    );

    let all = Names::builder()
        .text_tables(&dir)
        .keep_text_paths(|_| true)
        .build()
        .unwrap();
    assert_eq!(all.path(path_hash(map)).as_deref(), Some(map));
}

#[test]
fn characters_are_found_by_a_champions_name_in_every_table() {
    let (_keep, dir) = scratch();
    let cache = dir.join("cache");
    let text = dir.join("text");
    fs::create_dir_all(&text).unwrap();
    hash_cache(
        &cache,
        "1",
        &[CacheTable {
            table: Table::Game,
            names: &[
                "data/characters/lux/lux.bin",
                "data/characters/lux/skins/skin0.bin",
                "data/characters/luxair/luxair.bin",
                "data/characters/lulu/lulu.bin",
                "assets/characters/lux/skins/base/lux.skn",
            ],
            under: &[],
        }],
    );
    let cannon = "data/characters/luxcannon/luxcannon.bin";
    text_table(&text, "hashes.game.txt", &[(path_hash(cannon).0, cannon)]);
    let names = Names::builder()
        .mimir(&cache)
        .text_tables(&text)
        .build()
        .unwrap();

    let characters: Vec<String> = names.characters_like("Lux").into_iter().collect();
    assert_eq!(characters, ["lux", "luxair", "luxcannon"]);
    let paths: Vec<String> = names
        .paths_with_prefix("data/characters/lux/")
        .into_iter()
        .collect();
    assert_eq!(
        paths,
        [
            "data/characters/lux/lux.bin",
            "data/characters/lux/skins/skin0.bin"
        ]
    );
    assert!(names.characters_like("Ahri").is_empty());
}

#[test]
fn the_first_paths_of_a_prefix_come_in_path_order_from_every_table() {
    let (_keep, dir) = scratch();
    let cache = dir.join("cache");
    let text = dir.join("text");
    fs::create_dir_all(&text).unwrap();
    hash_cache(
        &cache,
        "1",
        &[CacheTable {
            table: Table::Game,
            names: &[
                "data/characters/lux/skins/skin1.bin",
                "data/characters/lux/lux.bin",
                "data/characters/lux/skins/skin0.bin",
                "data/characters/lulu/lulu.bin",
            ],
            under: &[],
        }],
    );
    let animations = "data/characters/lux/animations/lux.bin";
    text_table(
        &text,
        "hashes.game.txt",
        &[(path_hash(animations).0, animations)],
    );
    let names = Names::builder()
        .mimir(&cache)
        .text_tables(&text)
        .build()
        .unwrap();

    assert_eq!(
        names.first_paths_with_prefix("data/characters/lux/", 2),
        [animations, "data/characters/lux/lux.bin"]
    );
    assert_eq!(
        names.first_paths_with_prefix("data/characters/lux/", 64),
        [
            animations,
            "data/characters/lux/lux.bin",
            "data/characters/lux/skins/skin0.bin",
            "data/characters/lux/skins/skin1.bin",
        ]
    );
    assert!(names.first_paths_with_prefix("data/maps/", 64).is_empty());
}

/// A cache whose path table holds one path alone under its name hash, and the two twins.
fn cache_with_twins(dir: &Utf8Path) {
    hash_cache(
        dir,
        "1",
        &[CacheTable {
            table: Table::Game,
            names: &[PATH, TWIN_A, TWIN_B],
            under: &[],
        }],
    );
}

#[test]
fn a_path_is_found_by_its_name_hash_in_the_cache_with_or_without_the_index_on_disk() {
    let (_keep, dir) = scratch();
    let cache = dir.join("cache");
    let index = dir.join("index");
    cache_with_twins(&cache);

    let in_memory = Names::builder().mimir(&cache).build().unwrap();
    let on_disk = Names::builder()
        .mimir(&cache)
        .name_hash_index(&index)
        .build()
        .unwrap();
    assert!(index.join("name-hash.idx").is_file());
    for names in [&in_memory, &on_disk] {
        assert_eq!(
            names.path_by_name_hash(name_hash(PATH)).unwrap().as_deref(),
            Some(PATH)
        );
        assert_eq!(
            names.path_by_name_hash(name_hash(TWIN_A)).unwrap(),
            None,
            "two paths share the name hash, so it names neither"
        );
        assert_eq!(
            names
                .path_by_name_hash(name_hash("assets/none.tex"))
                .unwrap(),
            None
        );
    }
}

#[test]
fn a_name_hash_the_cache_and_a_text_table_give_to_two_paths_names_neither() {
    let (_keep, dir) = scratch();
    let cache = dir.join("cache");
    let text = dir.join("text");
    fs::create_dir_all(&text).unwrap();
    hash_cache(
        &cache,
        "1",
        &[CacheTable {
            table: Table::Game,
            names: &[PATH, TWIN_A],
            under: &[],
        }],
    );
    let other = "assets/characters/lux/skins/skin1/lux_skin1.skn";
    text_table(
        &text,
        "hashes.game.txt",
        &[
            (path_hash(TWIN_B).0, TWIN_B),
            (path_hash(PATH).0, PATH),
            (path_hash(other).0, other),
        ],
    );
    for index in [None, Some(dir.join("index"))] {
        let mut builder = Names::builder().mimir(&cache).text_tables(&text);
        if let Some(index) = &index {
            builder = builder.name_hash_index(index);
        }
        let names = builder.build().unwrap();
        let found = |path: &str| names.path_by_name_hash(name_hash(path)).unwrap();
        assert_eq!(found(TWIN_A), None, "{index:?}");
        // Both sources spell this path, and they agree.
        assert_eq!(found(PATH).as_deref(), Some(PATH), "{index:?}");
        // Only the text table spells this one.
        assert_eq!(found(other).as_deref(), Some(other), "{index:?}");
    }
    assert_eq!(
        NameHashSlot::One(WadHash(1)).merge(NameHashSlot::One(WadHash(1))),
        NameHashSlot::One(WadHash(1))
    );
    assert_eq!(
        NameHashSlot::One(WadHash(1)).merge(NameHashSlot::One(WadHash(2))),
        NameHashSlot::Several
    );
    assert_eq!(
        NameHashSlot::One(WadHash(1)).merge(NameHashSlot::Several),
        NameHashSlot::Several
    );
}

#[test]
fn the_runs_path_comes_first_by_name_hash_too() {
    let (_keep, dir) = scratch();
    cache_with_twins(&dir);
    let names = Names::builder().mimir(&dir).build().unwrap();
    let mut book = NameBook::new();
    let nothing = NameBook::new();
    book.learn_path(TWIN_B);
    let hash = name_hash(TWIN_A);
    assert_eq!(
        Speller::new(&nothing, &names)
            .path_by_name_hash(hash)
            .unwrap(),
        None
    );
    assert_eq!(
        Speller::new(&book, &names)
            .path_by_name_hash(hash)
            .unwrap()
            .as_deref(),
        Some(TWIN_B),
        "the path the run learnt, though the tables spell two with the name hash"
    );
    assert_eq!(
        Speller::new(&book, &names)
            .path_by_name_hash(name_hash(PATH))
            .unwrap()
            .as_deref(),
        Some(PATH)
    );
}

#[test]
fn names_are_shared_between_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<Names>();
    shared::<NameBook>();
    shared::<Speller<'static>>();
}
