//! Retype: each conversion of the catalogue alone, the value with no conversion, and the
//! value no build since the mod's own read.

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_hash::{HashAlgorithm, HashValue, HashWidth, Hasher};
use ltk_hashdb::{Compression, HashDbWriter};
use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};
use ltk_mimir_cache::{CommitItem, HashStore, Table};
use reforge::diag::Stage;
use reforge::env::Env;
use reforge::fixups::conform::{Conversion, Retype};
use reforge::fixups::{
    After, Change, Changed, FixError, FixInputs, FixupSet, ObjectSet, Outcome, Removed,
    Unconvertible,
};
use reforge::game::{NameHashError, Names, Schema};
use reforge::load::{Dating, LoadOptions};
use reforge::model::{BinHash, Record, WadHash, name_hash, path_hash};
use reforge_testkit::bins::{
    bin, blob, embedded, field_of, flag, float, link, object, pointer, text,
};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{
    embed, field, kind, list, list_of, list2, map, option, pointer as to,
};
use reforge_testkit::{FixRun, LoadRun, MemoryHistory, ModArchive, SchemaRows, skins};
use tempfile::TempDir;

const THING: &str = "Thing";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";
/// A texture the mod ships.
const OWN: &str = "assets/mod/a/own.tex";
/// A texture of the game outside the mod's characters' folders, which a name table spells.
const SHARED: &str = "assets/shared/particles/smoke_clouds.tex";
/// Two paths that share one name hash, `623bc5be`: one under the folder of the mod's
/// character and one outside it.
const TWIN_OWN: &str = "assets/characters/test/particles/xtexfkv.tex";
const TWIN_SHARED: &str = "assets/shared/particles/ajzfolr.tex";

/// One class with a field for each case. A field with `Was` in its name had another type up
/// to build 199. The others have had their type since 100, the schema's first build.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            THING,
            &[
                // retag
                field("asPointer", to("Part")),
                field("asEmbed", embed("Part")),
                field("asList2", list2("U8")),
                field("asList", list("U8")),
                field("asFlag", kind("Flag")),
                field("asBool", kind("Bool")),
                field("asHash", kind("Hash")),
                field("asLink", kind("Link")),
                // hash-value, rehash
                field("file", kind("File")),
                field("files", list("File")),
                field("maybeFile", option("File")),
                field("fileByName", map("Hash", "File")),
                field("fileWasString", kind("String")).then(200, kind("File")),
                // hash-key
                field("byFile", map("File", "File")),
                // widen, narrow
                field("u32", kind("U32")),
                field("i16", kind("I16")),
                field("i64", kind("I64")),
                field("u8", kind("U8")),
                field("i8", kind("I8")),
                // optional
                field("maybeFloat", option("F32")),
                // the rest
                field("text", kind("String")),
                field("parts", list_of("Part")),
            ],
        )
        .class("Part", &[field("name", kind("File"))])
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// When the game first shipped the mod's dated file, among the builds 50, 150 and 300.
#[derive(Clone, Copy)]
enum Made {
    Undated,
    BeforeTheSchema,
    At150,
}

fn env_with(made: Made, names: Names, game: reforge_testkit::MemoryGame) -> Env {
    let env = Env::builder().game(game).schema(schema()).names(names);
    let history = |first| {
        let history = MemoryHistory::new(&["1.0.50", "1.0.150", "16.19.300"]);
        Box::new(history.with_content(DATED, &dated_bytes(), first..=2))
    };
    let env = match made {
        Made::Undated => env,
        Made::BeforeTheSchema => env.history(history(0)),
        Made::At150 => env.history(history(1)),
    };
    env.build().expect("an environment")
}

/// The target at build 300, with no name table.
fn env(made: Made) -> Env {
    env_with(made, Names::default(), skins::game_at("16.19.300"))
}

/// Name tables in a scratch directory: paths, and plaintexts of hash values.
fn tables(paths: &[&str], hashes: &[&str]) -> (TempDir, Names) {
    let keep = tempfile::tempdir().unwrap();
    let dir = Utf8PathBuf::from_path_buf(keep.path().to_path_buf()).expect("a UTF-8 directory");
    let paths: String = paths
        .iter()
        .map(|path| format!("{:016x} {path}\n", path_hash(path).0))
        .collect();
    let hashes: String = hashes
        .iter()
        .map(|name| format!("{:08x} {name}\n", name_hash(name).0))
        .collect();
    fs::write(dir.join("hashes.game.txt"), paths).unwrap();
    fs::write(dir.join("hashes.binhashes.txt"), hashes).unwrap();
    let names = Names::builder().text_tables(&dir).keep_text_paths(|_| true);
    (keep, names.build().expect("name tables"))
}

fn archive(fields: Vec<(&str, V)>) -> ModArchive {
    skin_archive(vec![object("T", THING, fields)])
        .file(skins::WAD, DATED, dated_bytes())
        .file(skins::WAD, OWN, blob(OWN))
}

fn retype() -> FixupSet {
    FixupSet::empty().with(Retype)
}

/// One object of the class, retyped. The run's names learn the fields the object holds, so
/// that a path of a change spells them.
fn retyped<'a>(env: &'a Env, fields: Vec<(&str, V)>) -> FixRun<'a> {
    let names: Vec<String> = fields.iter().map(|(name, _)| name.to_string()).collect();
    let mut run = FixRun::of(env, &retype(), &archive(fields));
    for name in names.iter().map(String::as_str).chain(["name"]) {
        run.names.learn_field(name);
    }
    run
}

fn list_of_kind(item: Kind, items: Vec<V>) -> values::Container {
    values::Container::new(item, items).expect("a list")
}

fn some(item: Kind, value: V) -> V {
    V::Optional(values::Optional::new(item, Some(value)).expect("an option"))
}

fn no(item: Kind) -> V {
    V::Optional(values::Optional::empty(item).expect("an option"))
}

fn map_of(key: Kind, item: Kind, entries: Vec<(V, V)>) -> V {
    V::Map(values::Map::new(key, item, entries).expect("a map"))
}

fn file(path: &str) -> V {
    V::WadChunkLink(values::WadChunkLink::new(path_hash(path)))
}

fn no_file() -> V {
    V::WadChunkLink(values::WadChunkLink::new(WadHash(0)))
}

/// The name hash of a path, as builds before the file type took an asset.
fn hash(name: &str) -> V {
    V::Hash(values::Hash::new(name_hash(name)))
}

fn u8s(items: &[u8]) -> values::Container {
    let items = items.iter().map(|x| values::U8::new(*x).into()).collect();
    list_of_kind(Kind::U8, items)
}

/// What a run did, as the field, the kind before and what is there after.
fn rows(changes: &[Change], run: &FixRun<'_>) -> Vec<(String, &'static str, String)> {
    let speller = run.speller();
    changes
        .iter()
        .map(|change| {
            let before = change.before.as_ref().map_or("-", |before| before.kind);
            let after = match &change.after {
                After::Value(after) => format!("{} {}", after.kind, after.text),
                After::Moved { to } => format!("at {}", to.spelled(&speller)),
                After::Removed => "removed".to_string(),
                After::Lost => "lost".to_string(),
                After::Unconvertible { wants } => format!("left for {wants}"),
            };
            (change.path.spelled(&speller), before, after)
        })
        .collect()
}

#[test]
fn the_catalogue_lists_each_conversion_with_the_records_it_follows() {
    let rows: Vec<(Conversion, String, Vec<String>)> = Conversion::ALL
        .iter()
        .map(|conversion| {
            let records = conversion.records().iter().map(Record::to_string);
            (*conversion, conversion.to_string(), records.collect())
        })
        .collect();
    let row = |conversion: Conversion, name: &str, records: &[&str]| {
        let records = records.iter().map(ToString::to_string).collect();
        (conversion, name.to_string(), records)
    };
    assert_eq!(
        rows,
        [
            row(Conversion::Retag, "retag", &["#GAME-CHANGE-0019"]),
            row(Conversion::HashValue, "hash-value", &["#GAME-CHANGE-0013"]),
            row(Conversion::Rehash, "rehash", &["#GAME-CHANGE-0013"]),
            row(Conversion::WideHash, "wide-hash", &["#GAME-CHANGE-0028"]),
            row(Conversion::HashKey, "hash-key", &["#GAME-CHANGE-0013"]),
            row(Conversion::Widen, "widen", &[]),
            row(
                Conversion::Narrow,
                "narrow",
                &["#GAME-CHANGE-0020", "#MOD-ARTIFACT-0021"]
            ),
            row(Conversion::Optional, "optional", &[]),
            row(Conversion::NullPointer, "null-pointer", &[]),
            row(Conversion::EmptyOption, "empty-option", &[]),
        ]
    );
    for conversion in Conversion::ALL {
        assert!(!conversion.takes().is_empty() && !conversion.how().is_empty());
        let written = serde_json::to_value(conversion).unwrap();
        assert_eq!(written, serde_json::json!(conversion.name()));
    }
    // Every record a conversion follows is one the fixup names.
    use reforge::fixups::BinFixup as _;
    let named = &Retype.info().records;
    let followed = Conversion::ALL.iter().flat_map(|c| c.records());
    assert!(followed.clone().all(|record| named.contains(record)));
    assert_eq!(Retype.info().one_record(), None);
}

#[test]
fn the_conversion_between_two_kinds_is_read_off_the_kinds() {
    let between = Conversion::between;
    assert_eq!(
        between(Kind::String, Kind::WadChunkLink),
        Some(Conversion::HashValue)
    );
    assert_eq!(
        between(Kind::Hash, Kind::WadChunkLink),
        Some(Conversion::Rehash)
    );
    assert_eq!(
        between(Kind::Hash, Kind::ObjectLink),
        Some(Conversion::Retag)
    );
    assert_eq!(
        between(Kind::String, Kind::Hash),
        Some(Conversion::WideHash)
    );
    assert_eq!(
        between(Kind::Container, Kind::UnorderedContainer),
        Some(Conversion::Retag)
    );
    // #GAME-CHANGE-0021: no value goes into an option.
    assert_eq!(between(Kind::F32, Kind::Optional), None);
    assert_eq!(
        between(Kind::None, Kind::Struct),
        Some(Conversion::NullPointer)
    );
    // Wider, and every value of the old kind fits: a widening. Any other pair of integers
    // is a narrowing, the same width under another sign included.
    assert_eq!(between(Kind::U8, Kind::U32), Some(Conversion::Widen));
    assert_eq!(between(Kind::U8, Kind::I16), Some(Conversion::Widen));
    assert_eq!(between(Kind::I8, Kind::U16), Some(Conversion::Narrow));
    assert_eq!(between(Kind::U8, Kind::I8), Some(Conversion::Narrow));
    assert_eq!(between(Kind::U32, Kind::U8), Some(Conversion::Narrow));
    assert_eq!(between(Kind::F32, Kind::String), None);
    assert_eq!(between(Kind::String, Kind::Struct), None);
    assert_eq!(between(Kind::F32, Kind::U8), None);
    // A value of the kind read is not asked here: what it holds is converted, a map by
    // `HashKey`, a present option by `Optional`, an empty one by `EmptyOption`.
    assert_eq!(between(Kind::Map, Kind::Map), None);
    assert_eq!(between(Kind::Optional, Kind::Optional), None);

    // #MOD-ARTIFACT-0022: what the manager makes at install.
    let at_install = |from, to| between(from, to).is_some_and(|c| c.at_install(from));
    assert!(at_install(Kind::String, Kind::WadChunkLink));
    assert!(at_install(Kind::Hash, Kind::WadChunkLink));
    assert!(at_install(Kind::Embedded, Kind::Struct));
    assert!(at_install(Kind::Bool, Kind::BitBool));
    assert!(at_install(Kind::U8, Kind::U32));
    assert!(!at_install(Kind::U32, Kind::U8));
    assert!(!at_install(Kind::ObjectLink, Kind::Hash));
    assert!(!at_install(Kind::Hash, Kind::ObjectLink));
    assert!(!at_install(Kind::String, Kind::Hash));
    assert!(!at_install(Kind::F32, Kind::Optional));
}

#[test]
fn retag_keeps_the_encoding_under_the_tag_the_build_reads() {
    let env = env(Made::Undated);
    let part = |name: &str| vec![("name", file(name))];
    let run = retyped(
        &env,
        vec![
            ("asPointer", embedded("Part", part("a.tex"))),
            ("asEmbed", pointer("Part", part("b.tex"))),
            ("asList2", V::Container(u8s(&[1, 2]))),
            (
                "asList",
                V::UnorderedContainer(values::UnorderedContainer(u8s(&[3]))),
            ),
            ("asFlag", flag(true)),
            ("asBool", V::BitBool(values::BitBool::new(true))),
            ("asHash", link("Queue/One")),
            ("asLink", hash("Queue/Two")),
        ],
    );
    let get = |name: &str| run.field("T", name).cloned();
    assert_eq!(get("asPointer"), Some(pointer("Part", part("a.tex"))));
    assert_eq!(get("asEmbed"), Some(embedded("Part", part("b.tex"))));
    assert_eq!(
        get("asList2"),
        Some(V::UnorderedContainer(values::UnorderedContainer(u8s(&[
            1, 2
        ]))))
    );
    assert_eq!(get("asList"), Some(V::Container(u8s(&[3]))));
    assert_eq!(get("asFlag"), Some(V::BitBool(values::BitBool::new(true))));
    assert_eq!(get("asBool"), Some(flag(true)));
    // #GAME-CHANGE-0019: an object's name either way.
    assert_eq!(get("asHash"), Some(hash("Queue/One")));
    assert_eq!(get("asLink"), Some(link("Queue/Two")));
    assert_eq!(run.changes.of("retype").len(), 8);
    assert_eq!(run.changes.counts().changed, 8);
}

#[test]
fn hash_value_names_a_path_by_its_chunk_hash_and_the_names_learn_the_path() {
    let env = env(Made::Undated);
    let spelled = "ASSETS/Mod/A/Own.tex";
    let run = retyped(
        &env,
        vec![
            ("file", text(spelled)),
            (
                "files",
                V::Container(list_of_kind(Kind::String, vec![text(OWN), text(SHARED)])),
            ),
            ("maybeFile", some(Kind::String, text(SHARED))),
            (
                "fileByName",
                map_of(Kind::Hash, Kind::String, vec![(hash("slot"), text(OWN))]),
            ),
            ("fileWasString", text("")),
        ],
    );
    let get = |name: &str| run.field("T", name).cloned();
    assert_eq!(get("file"), Some(file(OWN)));
    assert_eq!(
        get("files"),
        Some(V::Container(list_of_kind(
            Kind::WadChunkLink,
            vec![file(OWN), file(SHARED)]
        )))
    );
    assert_eq!(
        get("maybeFile"),
        Some(some(Kind::WadChunkLink, file(SHARED)))
    );
    assert_eq!(
        get("fileByName"),
        Some(map_of(
            Kind::Hash,
            Kind::WadChunkLink,
            vec![(hash("slot"), file(OWN))]
        ))
    );
    // An empty string names nothing.
    assert_eq!(get("fileWasString"), Some(no_file()));
    assert_eq!(run.names.path(path_hash(SHARED)), Some(SHARED));
    assert_eq!(run.names.path(path_hash(spelled)), Some(OWN));

    // One change per property, whatever it holds.
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("file".into(), "String", format!("File {OWN}")),
            ("files".into(), "List", "List [2 File]".into()),
            (
                "maybeFile".into(),
                "Option",
                format!("Option some({SHARED})")
            ),
            ("fileByName".into(), "Map", "Map {1 Hash to File}".into()),
            (
                "fileWasString".into(),
                "String",
                "File 0000000000000000".into()
            ),
        ]
    );
    let [changed] = run.raised::<Changed>("retype")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (5, 1));
}

#[test]
fn rehash_finds_the_path_a_name_hash_is_of_and_hashes_it_as_a_file() {
    let (_keep, names) = tables(&[SHARED], &["assets/named/by_the_hashes_table.tex"]);
    let env = env_with(Made::Undated, names, skins::game_at("16.19.300"));
    let run = retyped(
        &env,
        vec![
            // A path the archive ships.
            ("file", hash(OWN)),
            (
                "files",
                V::Container(list_of_kind(
                    Kind::Hash,
                    vec![
                        // A path a name table spells, outside the characters' folders.
                        hash(SHARED),
                        // A plaintext the table of hash values holds.
                        hash("assets/named/by_the_hashes_table.tex"),
                    ],
                )),
            ),
            // The null hash names nothing.
            ("fileWasString", V::Hash(values::Hash::new(BinHash(0)))),
        ],
    );
    assert_eq!(run.field("T", "file"), Some(&file(OWN)));
    assert_eq!(
        run.field("T", "files"),
        Some(&V::Container(list_of_kind(
            Kind::WadChunkLink,
            vec![file(SHARED), file("assets/named/by_the_hashes_table.tex")]
        )))
    );
    assert_eq!(run.field("T", "fileWasString"), Some(&no_file()));
    assert_eq!(run.changes.counts().changed, 3);
}

#[test]
fn rehash_refuses_a_hash_no_name_spells() {
    let env = env(Made::Undated);
    let unknown = hash("assets/nobody/knows.tex");
    let listed = V::Container(list_of_kind(Kind::Hash, vec![hash(OWN), unknown.clone()]));
    let run = retyped(
        &env,
        vec![("file", unknown.clone()), ("files", listed.clone())],
    );
    assert_eq!(run.field("T", "file"), Some(&unknown));
    // One item with no conversion leaves the whole list.
    assert_eq!(run.field("T", "files"), Some(&listed));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("file".into(), "Hash", "left for File".into()),
            ("files".into(), "List", "left for File".into()),
        ]
    );
    assert_eq!(run.raised::<Unconvertible>("retype").len(), 1);
    assert!(!run.changes.edits(run.id("T")));
}

#[test]
fn rehash_ends_the_run_with_a_fault_when_the_name_hash_index_does_not_read() {
    // A hash cache whose path table spells one path, and its name-hash index on disk.
    let keep = tempfile::tempdir().unwrap();
    let dir = Utf8PathBuf::from_path_buf(keep.path().to_path_buf()).expect("a UTF-8 directory");
    let (cache, staging, index) = (dir.join("cache"), dir.join("staging"), dir.join("index"));
    fs::create_dir_all(&staging).unwrap();
    let config = Table::Game.key_config();
    let mut writer = HashDbWriter::with_key_config(config, Compression::None);
    writer.insert(config.hash(SHARED), SHARED);
    let table = staging.join("game.lhdb");
    writer.build(fs::File::create(&table).unwrap()).unwrap();
    let item = CommitItem::new(Table::Game, "1", table);
    HashStore::at(&cache).commit(&[item], None).unwrap();
    let names = Names::builder().mimir(&cache).name_hash_index(&index);
    let names = names.build().expect("name tables");

    let env = env_with(Made::Undated, names, skins::game_at("16.19.300"));
    let archive = archive(vec![("file", hash("assets/nobody/knows.tex"))]);
    let run = LoadRun::of(&env, LoadOptions::default(), &archive);
    let LoadRun {
        result,
        mut log,
        mut names,
        ..
    } = run;
    let m = result.expect("a mod");

    // The index loses its one record behind the handle the names hold open.
    let file = index.join("name-hash.idx");
    let whole = fs::metadata(&file).unwrap().len();
    let cut = fs::OpenOptions::new().write(true).open(&file).unwrap();
    cut.set_len(whole - 12).unwrap();
    drop(cut);

    let dating = Dating::default();
    let mut inputs = FixInputs {
        env: &env,
        dating: &dating,
        names: &mut names,
    };
    let applied = retype().apply(m, &ObjectSet::every(), &mut inputs, &mut log.at(Stage::Fix));
    assert!(
        matches!(applied, Err(FixError::Names(NameHashError::Io(_)))),
        "{:?}",
        applied.err()
    );
    assert!(log.iter().all(|d| d.stage != Stage::Fix));
}

#[test]
fn rehash_tells_two_paths_of_one_name_hash_apart_by_the_characters_folders() {
    assert_eq!(name_hash(TWIN_OWN), name_hash(TWIN_SHARED));
    let (_keep, names) = tables(&[TWIN_OWN, TWIN_SHARED], &[]);
    let env = env_with(Made::Undated, names, skins::game_at("16.19.300"));
    let run = retyped(&env, vec![("file", hash(TWIN_SHARED))]);
    // The mod's one character is `test`: the path under its folder is the one meant.
    assert_eq!(run.field("T", "file"), Some(&file(TWIN_OWN)));
}

#[test]
fn hash_key_converts_a_maps_keys_and_values_or_leaves_the_whole_map() {
    let env = env(Made::Undated);
    let whole = map_of(
        Kind::Hash,
        Kind::String,
        vec![(hash(OWN), text(OWN)), (hash(DATED), text(DATED))],
    );
    let run = retyped(&env, vec![("byFile", whole)]);
    assert_eq!(
        run.field("T", "byFile"),
        Some(&map_of(
            Kind::WadChunkLink,
            Kind::WadChunkLink,
            vec![(file(OWN), file(OWN)), (file(DATED), file(DATED))]
        ))
    );
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [("byFile".into(), "Map", "Map {2 File to File}".into())]
    );

    // One key no name spells refuses the map: a map read under two hash functions is worse
    // than the type it had.
    let broken = map_of(
        Kind::Hash,
        Kind::String,
        vec![
            (hash(OWN), text(OWN)),
            (hash("assets/nobody/knows.tex"), text(DATED)),
        ],
    );
    let run = retyped(&env, vec![("byFile", broken.clone())]);
    assert_eq!(run.field("T", "byFile"), Some(&broken));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [("byFile".into(), "Map", "left for File".into())]
    );
}

#[test]
fn widen_puts_an_integer_in_a_wider_kind() {
    let env = env(Made::Undated);
    let run = retyped(
        &env,
        vec![
            ("u32", values::U8::new(7).into()),
            ("i16", values::U8::new(200).into()),
            ("i64", values::I32::new(-5).into()),
        ],
    );
    assert_eq!(run.field("T", "u32"), Some(&values::U32::new(7).into()));
    assert_eq!(run.field("T", "i16"), Some(&values::I16::new(200).into()));
    assert_eq!(run.field("T", "i64"), Some(&values::I64::new(-5).into()));
    assert_eq!(run.changes.counts().changed, 3);
}

#[test]
fn narrow_puts_an_integer_in_another_kind_only_where_it_fits() {
    let env = env(Made::Undated);
    let run = retyped(
        &env,
        vec![
            ("u8", values::U16::new(5).into()),
            ("i8", values::U8::new(100).into()),
            ("i16", values::U32::new(700).into()),
        ],
    );
    assert_eq!(run.field("T", "u8"), Some(&values::U8::new(5).into()));
    assert_eq!(run.field("T", "i8"), Some(&values::I8::new(100).into()));
    assert_eq!(run.field("T", "i16"), Some(&values::I16::new(700).into()));

    let run = retyped(
        &env,
        vec![
            ("u8", values::U32::new(700).into()),
            ("i8", values::U8::new(200).into()),
            ("u32", values::I8::new(-1).into()),
        ],
    );
    assert_eq!(run.field("T", "u8"), Some(&values::U32::new(700).into()));
    assert_eq!(run.field("T", "i8"), Some(&values::U8::new(200).into()));
    assert_eq!(run.field("T", "u32"), Some(&values::I8::new(-1).into()));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("u8".into(), "U32", "left for U8".into()),
            ("i8".into(), "U8", "left for I8".into()),
            ("u32".into(), "I8", "left for U32".into()),
        ]
    );
}

#[test]
fn optional_converts_the_value_a_present_option_holds() {
    let env = env(Made::Undated);
    let run = retyped(&env, vec![("maybeFile", some(Kind::String, text(OWN)))]);
    assert_eq!(
        run.field("T", "maybeFile"),
        Some(&some(Kind::WadChunkLink, file(OWN)))
    );
    assert_eq!(run.changes.counts().changed, 1);
}

#[test]
fn a_value_where_the_build_reads_an_option_is_left_and_reported() {
    // #GAME-CHANGE-0021: Riot's own objects lost the value when the field became an option.
    let env = env(Made::Undated);
    let fields = vec![("maybeFloat", float(800.0)), ("maybeFile", text(OWN))];
    let run = retyped(&env, fields.clone());
    assert_eq!(run.object("T"), &object("T", THING, fields));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("maybeFloat".into(), "F32", "left for Option".into()),
            ("maybeFile".into(), "String", "left for Option".into()),
        ]
    );
    let [left] = run.raised::<Unconvertible>("retype")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(left.0.values, 2);
}

#[test]
fn null_pointer_reads_no_value_as_a_pointer_to_nothing() {
    let env = env(Made::Undated);
    let run = retyped(&env, vec![("asPointer", V::None(values::None))]);
    assert_eq!(
        run.field("T", "asPointer"),
        Some(&V::Struct(values::Struct::default()))
    );
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [("asPointer".into(), "None", "Pointer null".into())]
    );
}

#[test]
fn empty_option_takes_the_item_kind_the_build_reads() {
    let env = env(Made::Undated);
    let run = retyped(&env, vec![("maybeFile", no(Kind::String))]);
    assert_eq!(run.field("T", "maybeFile"), Some(&no(Kind::WadChunkLink)));
    assert_eq!(run.changes.counts().changed, 1);
    // An empty list takes its item kind the same way.
    let empty = |item| V::Container(list_of_kind(item, Vec::new()));
    let run = retyped(&env, vec![("files", empty(Kind::String))]);
    assert_eq!(run.field("T", "files"), Some(&empty(Kind::WadChunkLink)));
}

#[test]
fn a_value_with_no_conversion_is_left_and_reported() {
    let env = env(Made::Undated);
    let fields = vec![("text", float(1.5)), ("asPointer", text("a/b.tex"))];
    let run = retyped(&env, fields.clone());
    assert_eq!(run.object("T"), &object("T", THING, fields));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("text".into(), "F32", "left for String".into()),
            ("asPointer".into(), "String", "left for Pointer".into()),
        ]
    );
    let [left] = run.raised::<Unconvertible>("retype")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(left.0.values, 2);
    let outcomes: Vec<Outcome> = run.changes.iter().map(Change::outcome).collect();
    assert_eq!(outcomes, [Outcome::Unconvertible, Outcome::Unconvertible]);
    assert!(run.changes.edited().is_empty());
}

#[test]
fn a_struct_under_a_value_is_typed_in_its_turn() {
    let env = env(Made::Undated);
    let part = |name: V| pointer("Part", vec![("name", name)]);
    let parts = |a: V, b: V| V::Container(list_of_kind(Kind::Struct, vec![part(a), part(b)]));
    let run = retyped(
        &env,
        vec![
            ("parts", parts(text(OWN), text(SHARED))),
            // An embed retagged is entered as the pointer it became.
            ("asPointer", embedded("Part", vec![("name", text(OWN))])),
        ],
    );
    assert_eq!(
        run.field("T", "parts"),
        Some(&parts(file(OWN), file(SHARED)))
    );
    let made = run.field("T", "asPointer").expect("the pointer");
    assert_eq!(field_of(made, "name"), Some(&file(OWN)));
    let paths: Vec<String> = rows(run.changes.of("retype"), &run)
        .into_iter()
        .map(|(path, _, _)| path)
        .collect();
    assert_eq!(
        paths,
        [
            "asPointer",
            "parts[0].name",
            "parts[1].name",
            "asPointer.name"
        ]
    );
}

/// A string where the build has read a pointer since before the mod was made, a string where
/// it read a string when the mod was made, and values of kinds no build since read.
fn stale() -> Vec<(&'static str, V)> {
    vec![
        ("fileWasString", text(OWN)),
        ("asPointer", text("assets/mod/a/mult.tex")),
        ("file", text(OWN)),
        ("u32", values::U8::new(7).into()),
        ("u8", values::U16::new(5).into()),
        ("asHash", link("Queue/One")),
        ("maybeFloat", float(800.0)),
    ]
}

#[test]
fn a_value_no_build_since_the_mods_own_read_is_removed() {
    let env = env(Made::At150);
    let run = retyped(&env, stale());
    assert_eq!(run.dating.build(), Some(150));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            // A string at 150 and a file at 300: converted.
            ("fileWasString".into(), "String", format!("File {OWN}")),
            // A pointer since before 150: the string never showed.
            ("asPointer".into(), "String", "removed".into()),
            // #MOD-ARTIFACT-0022: the manager hashes a path and widens an integer at
            // install, so the author may have seen these.
            ("file".into(), "String", format!("File {OWN}")),
            ("u32".into(), "U8", "U32 7".into()),
            // A value is not brought back by a conversion the manager does not make.
            ("u8".into(), "U16", "removed".into()),
            ("asHash".into(), "Link", "removed".into()),
            ("maybeFloat".into(), "F32", "removed".into()),
        ]
    );
    for gone in ["asPointer", "u8", "asHash", "maybeFloat"] {
        assert_eq!(run.field("T", gone), None, "{gone} stays");
    }
    let [removed] = run.raised::<Removed>("retype")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(removed.0.values, 4);
    assert!(run.raised::<Unconvertible>("retype").is_empty());
}

#[test]
fn a_list_whose_items_have_no_conversion_stays_under_either_tag() {
    // Made at 150, when both fields read lists of bytes: a list of the other tag never
    // showed, and one whose items convert is retagged all the same. This one is left.
    let env = env(Made::At150);
    let strings = V::Container(list_of_kind(Kind::String, vec![text("a")]));
    let fields = vec![("asList2", strings.clone()), ("asList", strings)];
    let run = retyped(&env, fields.clone());
    assert_eq!(run.object("T"), &object("T", THING, fields));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("asList2".into(), "List", "left for U8".into()),
            ("asList".into(), "List", "left for U8".into()),
        ]
    );
}

#[test]
fn a_mod_with_no_date_or_one_before_the_schema_is_not_judged() {
    for made in [Made::Undated, Made::BeforeTheSchema] {
        let env = env(made);
        let run = retyped(&env, stale());
        assert_eq!(
            rows(run.changes.of("retype"), &run),
            [
                ("fileWasString".into(), "String", format!("File {OWN}")),
                ("asPointer".into(), "String", "left for Pointer".into()),
                ("file".into(), "String", format!("File {OWN}")),
                ("u32".into(), "U8", "U32 7".into()),
                ("u8".into(), "U16", "U8 5".into()),
                (
                    "asHash".into(),
                    "Link",
                    format!("Hash {:08x}", name_hash("Queue/One").0)
                ),
                ("maybeFloat".into(), "F32", "left for Option".into()),
            ]
        );
        assert!(run.raised::<Removed>("retype").is_empty());
    }
}

/// #GAME-CHANGE-0028: a string where the build reads an 8-byte hash, in a field and in a
/// list, becomes the XXH3-64 of the lowercased string. The schema knows the kind and the
/// install's bin the width. A string where nothing shows the width is left, and so is an
/// empty string.
#[test]
fn wide_hash_puts_a_string_in_the_eight_byte_hash_the_installs_bins_hold() {
    let schema = SchemaRows::new(300)
        .class(
            "Material",
            &[
                field("name", kind("String")).then(200, kind("Hash")),
                field("names", list("String")).then(200, list("Hash")),
                field("label", kind("String")).then(200, kind("Hash")),
                field("blank", kind("String")).then(200, kind("Hash")),
            ],
        )
        .build();
    let wide = |hash: u64| V::Hash(values::Hash::new(HashValue::wide(hash)));
    let mut game = skins::game_at("16.21.300");
    let held = object(
        "G",
        "Material",
        vec![
            ("name", wide(0x2a25_98c3_55f5_7ed7)),
            (
                "names",
                V::Container(list_of_kind(Kind::Hash, vec![wide(0x0102_0304_0506_0708)])),
            ),
            ("blank", wide(0x1)),
        ],
    );
    game.insert(&skins::skin_bin("test", 5), bin(vec![held]));
    let history = MemoryHistory::new(&["1.0.50", "1.0.150", "16.21.300"]);
    let env = Env::builder()
        .game(game)
        .schema(schema)
        .history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)))
        .build()
        .expect("an environment");
    let path = "Characters/Test/Skins/Skin0/Materials/Body";
    let mine = object(
        "M",
        "Material",
        vec![
            ("name", text(path)),
            (
                "names",
                V::Container(list_of_kind(Kind::String, vec![text(path)])),
            ),
            ("label", text(path)),
            ("blank", text("")),
        ],
    );
    let archive = skin_archive(vec![mine]).file(skins::WAD, DATED, dated_bytes());
    let mut run = FixRun::of(&env, &retype(), &archive);
    for name in ["name", "names", "label", "blank"] {
        run.names.learn_field(name);
    }
    let hasher = Hasher {
        width: HashWidth::W8,
        algorithm: HashAlgorithm::Xxh3,
        lowercased: true,
    };
    let hashed = hasher.hash_str(path);
    assert_eq!(hashed.width(), HashWidth::W8);
    assert_eq!(hashed, hasher.hash_str(&path.to_ascii_lowercase()));
    let get = |name: &str| run.field("M", name).cloned();
    assert_eq!(get("name"), Some(V::Hash(values::Hash::new(hashed))));
    assert_eq!(
        get("names"),
        Some(V::Container(list_of_kind(
            Kind::Hash,
            vec![V::Hash(values::Hash::new(hashed))]
        )))
    );
    assert_eq!(get("label"), Some(text(path)));
    assert_eq!(get("blank"), Some(text("")));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [
            ("name".into(), "String", format!("Hash {hashed:x}")),
            ("names".into(), "List", "List [1 Hash]".into()),
            ("label".into(), "String", "left for Hash".into()),
            ("blank".into(), "String", "left for Hash".into()),
        ]
    );
}

#[test]
fn a_type_only_the_installs_bins_witness_is_not_judged_by_the_history() {
    // The schema declares `seen` up to 199 as a float. The game's own bin holds it at the
    // target build as a string: that is the type read, and the schema says nothing of
    // which kinds showed under it.
    let schema = SchemaRows::new(300)
        .class("Witnessed", &[field("seen", kind("F32")).until(199)])
        .build();
    let mut game = skins::game_at("16.19.300");
    let held = object("G", "Witnessed", vec![("seen", text("a"))]);
    game.insert(&skins::skin_bin("test", 5), bin(vec![held]));
    let history = MemoryHistory::new(&["1.0.50", "1.0.150", "16.19.300"]);
    let env = Env::builder()
        .game(game)
        .schema(schema)
        .history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)))
        .build()
        .expect("an environment");
    let value: V = values::U32::new(1).into();
    let mine = object("M", "Witnessed", vec![("seen", value.clone())]);
    let archive = skin_archive(vec![mine]).file(skins::WAD, DATED, dated_bytes());
    let mut run = FixRun::of(&env, &retype(), &archive);
    run.names.learn_field("seen");
    assert_eq!(run.dating.build(), Some(150));
    assert_eq!(run.field("M", "seen"), Some(&value));
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [("seen".into(), "U32", "left for String".into())]
    );
}

#[test]
fn retype_applied_twice_changes_nothing_the_second_time() {
    let part = |name: V| pointer("Part", vec![("name", name)]);
    let mut fields = stale();
    fields.extend([
        ("asEmbed", part(text(OWN))),
        (
            "parts",
            V::Container(list_of_kind(Kind::Struct, vec![part(text(SHARED))])),
        ),
        ("maybeFile", no(Kind::String)),
        (
            "byFile",
            map_of(Kind::Hash, Kind::String, vec![(hash(OWN), text(OWN))]),
        ),
        ("text", float(1.5)),
    ]);
    for made in [Made::Undated, Made::At150] {
        let env = env(made);
        let set = retype();
        let once = FixRun::of(&env, &set, &archive(fields.clone()));
        assert!(once.changes.counts().edits() > 0);
        let after = once.object("T").clone();
        let left = once.changes.counts().unconvertible;
        let twice = once.again(&set);
        // What has no conversion is said again, and nothing else is.
        assert_eq!(twice.changes.counts().edits(), 0, "{:?}", twice.changes);
        assert_eq!(twice.changes.counts().unconvertible, left);
        assert_eq!(twice.object("T"), &after);
    }
}
