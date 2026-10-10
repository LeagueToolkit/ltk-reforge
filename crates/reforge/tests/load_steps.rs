//! The stage `load` step by step: what each step leaves in the mod, the file fixups it runs,
//! its dump, and its codes. The game is `reforge_testkit::skins`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fs_err as fs;
use reforge::archive::{self, Caps, RawSnapshot};
use reforge::diag::{Cancel, Cancelled, Channel, Diagnostics, Record, Render, Stage};
use reforge::env::Env;
use reforge::game::{GameSource, NameBook};
use reforge::history::HistoryError;
use reforge::load::{
    self, CODES, FileFixCx, FileFixup, LoadError, LoadOptions, Loader, NewFile, against,
};
use reforge::model::entity::{FileKind, FileSource, FileUse, MadeBy, WadRole};
use reforge::model::{Blobs, ChunkPath, Dump, DumpCx, FixupInfo, NoNames, Phase, Spell, name_hash};
use reforge::store::MemoryStore;
use reforge_testkit::bins::{Skin, bin, blob, entry, linked_bin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::skins::{self, BASE_TEX, WAD, base_skin, skin_bin};
use reforge_testkit::{LoadRun, MemoryHistory, ModArchive};

const ENTRY: &str = "Characters/Test/Skins/Skin0";
const OWN_MESH: &str = "assets/mod/a/mesh.skn";

fn env() -> Env {
    let game = skins::game();
    let sizes = game.sizes();
    let env = Env::builder().game(game).sizes(sizes);
    env.build().expect("an environment")
}

fn own_skin() -> Skin {
    Skin {
        mesh: OWN_MESH.into(),
        ..base_skin()
    }
}

/// A mod of one skin bin naming its own mesh, and the base texture repainted.
fn small_mod() -> ModArchive {
    ModArchive::new()
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry(ENTRY, &own_skin())]),
        )
        .file(WAD, OWN_MESH, blob(OWN_MESH))
        .file(WAD, BASE_TEX, blob("repainted"))
}

/// An archive read into a store of its own.
fn read(built: &ModArchive, log: &mut Diagnostics) -> (RawSnapshot, Blobs) {
    let bytes = built.build();
    let store = Arc::new(MemoryStore::new());
    let caps = Caps::default();
    let raw = archive::read(&bytes, FILE_NAME, &caps, &*store, &mut log.at(Stage::Read));
    (raw.expect("it reads"), Blobs::new(store))
}

#[test]
fn each_step_leaves_the_mod_as_far_as_it_goes() {
    let env = env();
    let mut log = Diagnostics::new();
    let (raw, store) = read(&small_mod(), &mut log);
    let mut loader = Loader::new(&env).options(LoadOptions::new());
    let mut sink = log.at(Stage::Load);

    let mut m = loader.open(&raw, &store).expect("the store answers");
    assert_eq!((m.wads.len(), m.files.len()), (1, 3));
    assert!(m.wads.values().all(|wad| wad.role == WadRole::Other));
    assert!(
        m.files
            .values()
            .all(|f| f.used == FileUse::Unused && f.source == FileSource::Archive)
    );
    assert!(m.characters.is_empty() && m.bins.is_empty());
    assert_eq!(m.meta.name.as_ref().map(|n| n.raw()), Some("Synthetic"));

    loader
        .fix_files(&mut m, &mut sink)
        .expect("no history is asked");
    loader
        .take_game_bins(&mut m, &mut sink)
        .expect("no history is asked");
    assert_eq!(
        m.files.len(),
        3,
        "a WAD with a skin bin takes none of the game's"
    );

    loader.survey(&mut m, &mut sink).expect("a skin mod");
    assert_eq!((m.characters.len(), m.units.len()), (1, 1));
    assert!(m.bins.is_empty(), "the survey reads no bin");
    assert!(
        m.units
            .values()
            .all(|unit| unit.bin.is_none() && unit.entry.is_none())
    );

    loader
        .resolve(&mut m, &mut sink)
        .expect("no history is asked");
    assert_eq!((m.bins.len(), m.objects.len()), (1, 1));
    let unit = m.units.values().next().expect("the unit");
    assert_eq!(unit.loads.len(), 1);
    assert!(unit.entry.is_some());
    assert!(log.is_empty(), "{:?}", log.iter().collect::<Vec<_>>());
}

#[test]
fn the_whole_stage_is_the_steps_in_order_and_the_same_for_the_same_archive() {
    let env = env();
    let first = LoadRun::of(&env, LoadOptions::new(), &small_mod());
    let second = LoadRun::of(&env, LoadOptions::new(), &small_mod());
    assert_eq!(first.dump(), second.dump());
    let json = |run: &LoadRun<'_>| -> Vec<String> {
        let cx = Render::plain();
        run.log.iter().map(|d| d.to_json(&cx).to_string()).collect()
    };
    assert_eq!(json(&first), json(&second));
}

#[test]
fn the_dump_lists_the_mod_in_creation_order() {
    let env = env();
    let run = LoadRun::of(&env, LoadOptions::new(), &small_mod());
    let speller = run.speller();
    let dump = run.module().dump(&DumpCx::new(&speller));

    // Files are in path-hash order, which is the hash function's: the ids come from the mod.
    let file = |path: &str, kind: &str, used: &str| {
        let (id, node) = run.file(path);
        let (hash, sha) = (node.hash.0, &node.content.hex()[..16]);
        let line = format!(
            "file {id} %w0 {hash:016x} {kind} {used} {}B sha={sha}",
            node.size
        );
        (id, format!("{line}\n     {path}\n"))
    };
    let (skin, skin_lines) = file(&skin_bin("test", 0), "bin", "loaded");
    let mod_id = &run.module().meta.mod_id;
    let head = format!(
        "; reforge mod={mod_id} archive=\"synthetic.fantome\"\n\
         wad %w0 \"Test.wad.client\" champion(test) loose files=3\n"
    );
    let tail = format!(
        "bin %b0 {skin} objects=1 links=0\n\
         char %c0 test (test) champion units=[%u0]\n\
         unit %u0 %c0 slot=0 file={skin} bin=%b0 loads=1 entry=%o0 \"Characters/test/Skins/Skin0\"\n"
    );
    assert_eq!(dump, format!("{head}{skin_lines}{tail}"));

    // A file whose bytes are of no kind a rig is made of is listed on demand.
    let mut files = [
        (skin, skin_lines),
        file(OWN_MESH, "other", "referenced"),
        file(BASE_TEX, "other", "referenced"),
    ];
    files.sort();
    let files: String = files.iter().map(|(_, lines)| lines.as_str()).collect();
    assert_eq!(run.dump(), format!("{head}{files}{tail}"));
}

#[test]
fn text_a_mod_spells_is_cleaned_in_the_dump() {
    let env = env();
    let link = "DATA/\u{1b}[31mMod.bin";
    let archive = ModArchive::new().file(
        WAD,
        &skin_bin("test", 0),
        linked_bin(vec![entry(ENTRY, &own_skin())], &[link]),
    );
    let mut log = Diagnostics::new();
    let bytes = archive.build();
    let name = "evil\u{202e}emotnaf.zip";
    let store = Arc::new(MemoryStore::new());
    let caps = Caps::default();
    let raw = archive::read(&bytes, name, &caps, &*store, &mut log.at(Stage::Read));
    let mut loader = Loader::new(&env);
    let m = loader
        .load(
            &raw.expect("it reads"),
            &Blobs::new(store),
            &mut log.at(Stage::Load),
        )
        .expect("a mod");
    let dump = m.dump(&DumpCx::new(&NoNames).every_file());
    assert!(
        !dump.contains('\u{1b}') && !dump.contains('\u{202e}'),
        "{dump}"
    );
    assert!(dump.contains("\\u{1b}[31mMod.bin -> missing"), "{dump}");
    assert!(
        dump.contains("archive=\"evil\\u{202e}emotnaf.zip\""),
        "{dump}"
    );
    let cx = Render {
        names: &NoNames,
        module: Some(&m),
    };
    let messages: Vec<String> = log.iter().map(|d| d.message(&cx)).collect();
    assert!(
        messages.iter().all(|m| !m.contains('\u{1b}')),
        "{messages:?}"
    );
}

#[test]
fn a_cancel_set_before_load_ends_it_before_the_mod_is_opened() {
    // #ADR-0097: the log carries the cancel, and the stage reads it before each step.
    let env = env();
    let mut log = Diagnostics::new();
    let (raw, store) = read(&small_mod(), &mut log);
    let cancel = Cancel::new();
    cancel.cancel();
    log.cancel_on(cancel);
    let loaded = Loader::new(&env).load(&raw, &store, &mut log.at(Stage::Load));
    let Err(error) = loaded else {
        panic!("a cancelled load gives no mod");
    };
    assert!(
        matches!(error, LoadError::Cancelled(Cancelled(Stage::Load))),
        "{error:?}"
    );
    assert_eq!(log.cancelled(), Some(Cancelled(Stage::Load)));
    assert!(
        log.iter().all(|d| d.stage != Stage::Load),
        "a cancel is no diagnostic"
    );
}

#[test]
fn a_snapshot_naming_bytes_the_store_lacks_is_a_fault_of_the_store() {
    // #ADR-0093: the snapshot names bytes by id. A store that lacks them does not answer as
    // if the file held nothing.
    let env = env();
    let mut log = Diagnostics::new();
    let (raw, _store) = read(&small_mod(), &mut log);
    let mut loader = Loader::new(&env);
    let empty = Blobs::new(Arc::new(MemoryStore::new()));
    let Err(LoadError::Store(id)) = loader.open(&raw, &empty) else {
        panic!("an empty store is a fault");
    };
    let named: Vec<_> = raw
        .wads
        .iter()
        .flat_map(|wad| wad.files.values().map(|file| file.content))
        .chain(raw.extras.iter().map(|extra| extra.head))
        .collect();
    assert!(named.contains(&id), "{id:?} is one the snapshot names");
}

#[test]
fn the_paths_a_bin_spells_name_the_chunks_of_a_packed_wad() {
    // A packed WAD spells no path. The skin bin's is the survey's, the mesh's the entry's.
    let skin = bin(vec![entry(ENTRY, &own_skin())]);
    let mesh = blob(OWN_MESH);
    let skin_path = skin_bin("test", 0);
    let chunks: [(&str, &[u8]); 3] = [
        (&skin_path, &skin),
        (OWN_MESH, &mesh),
        ("assets/mod/a/unnamed.tex", b"named by nothing"),
    ];
    let archive = ModArchive::new().packed(WAD, &chunks);
    let env = env();
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let path = |path: &str| run.file(path).1.path.as_ref().map(ToString::to_string);
    assert_eq!(path(OWN_MESH).as_deref(), Some(OWN_MESH));
    assert_eq!(path(&skin_bin("test", 0)), Some(skin_bin("test", 0)));
    assert_eq!(path("assets/mod/a/unnamed.tex"), None);
    // The archive spells none of them: what a path of the mod's own says is not read off these.
    let m = run.module();
    assert_eq!(m.spelled_path(run.file(OWN_MESH).0), None);
    assert!(m.named_by_table.contains(&run.file(OWN_MESH).0));
    assert!(
        !m.named_by_table
            .contains(&run.file("assets/mod/a/unnamed.tex").0)
    );
    assert_eq!(
        run.speller().path(run.file(OWN_MESH).1.hash).as_deref(),
        Some(OWN_MESH)
    );
}

#[test]
fn a_character_is_spelled_as_its_skin_entry_spells_it_and_its_names_are_learnt() {
    let env = env();
    let mut log = Diagnostics::new();
    let (raw, store) = read(&small_mod(), &mut log);
    let mut book = NameBook::new();
    book.learn_entry(ENTRY);
    let mut loader = Loader::new(&env).book(book);
    let m = loader
        .load(&raw, &store, &mut log.at(Stage::Load))
        .expect("a mod");
    assert_eq!(m.characters.values().next().expect("test").spelled, "Test");
    let names = loader.into_names();
    let learnt = name_hash("Characters/Test/Animations/Skin7");
    assert_eq!(
        names.entry(learnt),
        Some("Characters/Test/Animations/Skin7")
    );
    assert_eq!(
        names.field(name_hash("skinMeshProperties")),
        Some("skinMeshProperties")
    );
}

/// A fixup that makes one file from the base texture and records that it ran.
struct Copies {
    id: &'static str,
    info: FixupInfo,
    to: &'static str,
    ran: Arc<Mutex<Vec<&'static str>>>,
}

impl FileFixup for Copies {
    fn info(&self) -> &FixupInfo {
        &self.info
    }

    fn fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError> {
        self.ran
            .lock()
            .expect("no test panics with it")
            .push(self.id);
        let from = cx
            .module
            .files
            .iter()
            .find(|(_, f)| f.path.as_ref().is_some_and(|p| p.as_str() == BASE_TEX));
        let (from, node) = from.expect("the base texture");
        let bytes = node.bytes(cx.module.blobs()).view().clone();
        let to = ChunkPath::parse(self.to).expect("a path");
        let made = cx
            .add(NewFile {
                from,
                by: MadeBy::MovedAsset,
                to,
                bytes,
                kind: FileKind::Other,
            })
            .expect("the path is free");
        cx.sink.raise(load::WadNotCarried {
            wad: cx.module.files[made].wad,
        });
        Ok(())
    }

    fn destinations(&self, _: &ChunkPath, _: &dyn GameSource) -> Vec<ChunkPath> {
        Vec::new()
    }
}

#[test]
fn the_file_fixups_run_in_the_order_of_the_options_each_through_its_own_sink() {
    const ONE: &[Record] = &[Record::game_change(7)];
    const TWO: &[Record] = &[Record::game_change(9), Record::game_change(10)];
    let ran = Arc::new(Mutex::new(Vec::new()));
    let fixup = |id, to, records| Copies {
        id,
        info: FixupInfo::built_in(id, "A copy of the base texture", records, Phase::Files),
        to,
        ran: Arc::clone(&ran),
    };
    let options = LoadOptions::new()
        .without_fixups()
        .fixup(fixup("second-in-name", "assets/made/a.tex", ONE))
        .fixup(fixup("first-in-name", "assets/made/b.tex", TWO));
    let env = env();
    let run = LoadRun::of(&env, options, &small_mod());

    assert_eq!(
        *ran.lock().expect("unpoisoned"),
        ["second-in-name", "first-in-name"]
    );
    let by_a_fixup = run.log.iter().filter(|d| d.by.is_some());
    let raised: Vec<_> = by_a_fixup.map(|d| (d.by.as_deref(), d.record)).collect();
    // The record of a fixup of one record. A fixup of several leaves it to each finding.
    assert_eq!(
        raised,
        [
            (Some("second-in-name"), Some(ONE[0])),
            (Some("first-in-name"), None)
        ]
    );
    let (_, made) = run.file("assets/made/a.tex");
    assert!(matches!(
        made.source,
        FileSource::Made {
            by: MadeBy::MovedAsset,
            ..
        }
    ));
    assert_eq!(made.content, run.file(BASE_TEX).1.content);
    assert_eq!(
        run.speller().path(made.hash).as_deref(),
        Some("assets/made/a.tex")
    );
}

#[test]
fn the_standard_file_fixups_are_legacy_formats_then_moved_assets_then_bank_id() {
    let ids = |options: &LoadOptions| -> Vec<String> {
        let fixups = options.fixups.iter();
        fixups.map(|fixup| fixup.id().to_string()).collect()
    };
    assert_eq!(
        ids(&LoadOptions::new()),
        ["legacy-formats", "moved-assets", "bank-id"]
    );
    assert_eq!(
        ids(&LoadOptions::new().without_fixup("legacy-formats")),
        ["moved-assets", "bank-id"]
    );
    assert_eq!(ids(&LoadOptions::new().without_fixup("no-such")).len(), 3);
    assert!(ids(&LoadOptions::new().without_fixups()).is_empty());
    assert!(!LoadOptions::new().keep_stale);
}

/// A fixup whose history does not answer.
struct Faulty;

impl FileFixup for Faulty {
    fn info(&self) -> &FixupInfo {
        static INFO: FixupInfo = FixupInfo::built_in(
            "faulty",
            "A history that does not answer",
            &[],
            Phase::Files,
        );
        &INFO
    }

    fn fix(&self, _: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError> {
        Err(HistoryError::UnknownFamily("none".into()))
    }
}

#[test]
fn a_fault_of_a_file_fixup_ends_the_load() {
    let env = env();
    let options = LoadOptions::new().without_fixups().fixup(Faulty);
    let run = LoadRun::of(&env, options, &small_mod());
    assert!(matches!(
        run.result,
        Err(LoadError::History(HistoryError::UnknownFamily(_)))
    ));
    assert!(run.log.is_empty());

    // The step gives the fault as it is, and the stage makes it its own error.
    let mut log = Diagnostics::new();
    let (raw, store) = read(&small_mod(), &mut log);
    let options = LoadOptions::new().without_fixups().fixup(Faulty);
    let mut loader = Loader::new(&env).options(options);
    let mut m = loader.open(&raw, &store).expect("the store answers");
    let fixed = loader.fix_files(&mut m, &mut log.at(Stage::Load));
    assert!(matches!(
        fixed,
        Err(load::Fault::History(HistoryError::UnknownFamily(_)))
    ));
    assert_eq!(m.files.len(), 3, "the files the archive ships stay");
}

#[test]
fn the_results_on_the_files_are_gathered_on_the_mod_the_stage_returns() {
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]);
    let game = skins::game();
    let sizes = game.sizes();
    let env = Env::builder()
        .game(game)
        .sizes(sizes)
        .history(Box::new(history));
    let env = env.build().expect("an environment");
    let run = LoadRun::of(&env, LoadOptions::new(), &small_mod());
    let speller = run.speller();
    let m = run.module();
    let against = against(&env, &speller);
    let provenance = load::FileProvenance::find(m, &against).expect("the history answers");
    assert!(provenance.fact(run.file(OWN_MESH).0).is_some());
    let dating = load::Dating::find(m, env.history()).expect("the history answers");
    assert_eq!(dating.position(), None);
    let inventory = load::Inventory::take(m, &speller);
    assert!(!inventory.by_kind.is_empty());
}

/// The code, severity and channel of every row of the tables in `CODES.md`.
fn documented() -> BTreeMap<String, String> {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/CODES.md");
    let text = fs::read_to_string(file).expect("CODES.md reads");
    text.lines()
        .filter_map(|line| {
            let mut cells = line.strip_prefix("| `")?.split('|');
            let code = cells.next()?.trim().trim_end_matches('`');
            Some((code.to_string(), cells.next()?.trim().to_string()))
        })
        .collect()
}

#[test]
fn the_codes_of_the_stage_are_the_ones_the_codes_table_lists() {
    let documented = documented();
    for code in CODES {
        assert_eq!(
            documented.get(code.name),
            Some(&code.severity.as_str().to_string()),
            "{code}"
        );
        assert_eq!(
            code.channel,
            Channel::Input,
            "{code}: the archive as shipped"
        );
    }
    // Every code of a WAD, a character, a bin and a rework is raised here, and of the files
    // the three uses, and the one code of the archive only `load` can tell.
    let mut ours: Vec<&str> = CODES.iter().map(|code| code.name).collect();
    ours.sort_unstable();
    let here = |code: &&str| {
        let area = code.split('.').next().unwrap_or_default();
        matches!(area, "wad" | "character" | "bin" | "rework")
            || matches!(
                *code,
                "file.unused" | "file.stale" | "file.former-path" | "archive.no-skin"
            )
    };
    let theirs: Vec<&str> = documented.keys().map(String::as_str).filter(here).collect();
    assert_eq!(ours, theirs);
}
