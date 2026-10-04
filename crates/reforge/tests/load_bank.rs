//! `bank-id`: a bank whose header id is zero, given the id its file name hashes to, over mods
//! and a game built here.

use reforge::archive::file_kind;
use reforge::diag::{Channel, Diag, Diagnostics, Record, Severity, Stage};
use reforge::env::Env;
use reforge::fixups::{FixupSelection, FixupSet};
use reforge::game::{NameBook, Names};
use reforge::history::NoHistory;
use reforge::load::bank::{BankId, BankIdSet, BankUnnamed, CODES, Named};
use reforge::load::files::{FileFixCx, FileFixup, FileStep, NewFile, one_record};
use reforge::load::{LoadOptions, Loader};
use reforge::model::entity::{FileKind, FileSource, FileUse, MadeBy};
use reforge::model::{ChunkPath, ContentId, Mod, Phase, path_hash};
use reforge_testkit::bins::{bin, entry, object, text};
use reforge_testkit::skins::{self, WAD, base_skin, skin_bin};
use reforge_testkit::{LoadRun, MemoryGame, ModArchive};
use reforge_wwise::{Bank, id_of};

const ENTRY: &str = "Characters/Test/Skins/Skin0";
/// A bank an object of the mod names.
const BANK: &str = "assets/sounds/wwise2016/sfx/characters/test/skins/base/test_base_sfx_audio.bnk";
/// A bank nothing names, at a path the game does not ship.
const LOOSE: &str = "assets/mod/a/left_audio.bnk";

fn section(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A bank of a header alone: the version, the id, and eight more bytes.
fn bank_bytes(id: u32) -> Vec<u8> {
    let mut header = 145u32.to_le_bytes().to_vec();
    header.extend(id.to_le_bytes());
    header.extend([0; 8]);
    section(b"BKHD", &header)
}

/// The header id of a bank's bytes.
fn header_id(bytes: &[u8]) -> u32 {
    Bank::read(bytes).expect("a bank").id()
}

fn env_of(game: MemoryGame) -> Env {
    let sizes = game.sizes();
    let env = Env::builder().game(game).sizes(sizes);
    env.build().expect("an environment")
}

fn env() -> Env {
    env_of(skins::game())
}

/// A skin bin whose entry is the base skin's, with an object naming each of `named`.
fn skin_naming(named: &[&str]) -> Vec<u8> {
    let names: Vec<String> = (0..named.len()).map(|n| format!("bank{n}")).collect();
    let fields = names.iter().zip(named);
    let fields = fields.map(|(name, path)| (name.as_str(), text(path)));
    let fields = fields.collect();
    bin(vec![
        entry(ENTRY, &base_skin()),
        object("Unknown/Object", "NotInSchema", fields),
    ])
}

/// A mod whose skin bin names `BANK`, shipping it with a header id.
fn mod_with(id: u32) -> ModArchive {
    ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&[BANK]))
        .file(WAD, BANK, bank_bytes(id))
}

#[test]
fn a_bank_with_header_id_zero_gets_the_id_of_its_file_name() {
    // The game's own bank of this name carries this id at 16.19.
    assert_eq!(id_of("braum_base_sfx_audio"), 3_580_963_463);

    let env = env();
    let run = LoadRun::of(&env, LoadOptions::new(), &mod_with(0));
    let (file, node) = run.file(BANK);
    let id = id_of("test_base_sfx_audio");
    assert_eq!(node.used, FileUse::Referenced);
    assert_eq!(header_id(&node.bytes), id);

    // The file is the archive's, at its path, with the bytes the fixup wrote.
    let shipped = bank_bytes(0);
    assert_eq!(node.bytes[..], bank_bytes(id)[..]);
    assert_eq!(node.size, shipped.len() as u64);
    assert_eq!(node.content, ContentId::of(&node.bytes));
    assert_eq!(node.kind, FileKind::Bnk);
    assert_eq!(
        node.source,
        FileSource::Rewritten {
            by: MadeBy::BankId,
            shipped: ContentId::of(&shipped)
        }
    );
    assert_eq!(run.module().origin_of(file), file);

    let set: &BankIdSet = run.only();
    assert_eq!(set.wad, node.wad);
    assert_eq!(set.banks, [Named { file, id }]);
    let raised = run.log.iter().find(|d| d.code == BankIdSet::CODE);
    let raised = raised.expect("the finding");
    assert_eq!(raised.stage, Stage::Load);
    assert_eq!(raised.by.as_deref(), Some("bank-id"));
    assert_eq!(raised.record, Some(Record::mod_artifact(23)));
    assert!(
        run.messages().contains(&format!(
            "audio.bank-id-set 1 bank with header id 0, given the id of the file name: {BANK} \
             ({id})"
        )),
        "{:?}",
        run.messages()
    );
}

#[test]
fn a_bank_with_an_id_and_a_bank_nothing_loads_are_left() {
    let env = env();
    let run = LoadRun::of(&env, LoadOptions::new(), &mod_with(7));
    let (_, node) = run.file(BANK);
    assert_eq!(node.bytes[..], bank_bytes(7)[..]);
    assert_eq!(node.source, FileSource::Archive);
    assert_eq!(run.log.of::<BankIdSet>().count(), 0);

    // Nothing names it and the game ships nothing at its path.
    let unused = mod_with(7).file(WAD, LOOSE, bank_bytes(0));
    let run = LoadRun::of(&env, LoadOptions::new(), &unused);
    let (_, node) = run.file(LOOSE);
    assert_eq!(node.used, FileUse::Unused);
    assert_eq!(header_id(&node.bytes), 0);
    assert_eq!(node.source, FileSource::Archive);
    assert_eq!(run.log.of::<BankIdSet>().count(), 0);
    assert_eq!(run.log.of::<BankUnnamed>().count(), 0);
}

#[test]
fn a_bank_at_a_path_the_game_ships_is_one_the_client_loads() {
    let mut game = skins::game();
    game.insert(BANK, bank_bytes(id_of("test_base_sfx_audio")));
    let env = env_of(game);
    // No object names it: the game's own bins may.
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&[]))
        .file(WAD, BANK, bank_bytes(0));
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let (_, node) = run.file(BANK);
    assert_eq!(node.used, FileUse::Override);
    assert_eq!(header_id(&node.bytes), id_of("test_base_sfx_audio"));
}

#[test]
fn a_bank_named_by_its_hash_takes_the_path_a_bin_of_the_mod_spells() {
    let mut game = skins::game();
    game.insert(BANK, bank_bytes(1));
    let hashed = format!("{:016x}.bnk", path_hash(BANK).0);

    // The archive spells no path and no bin names the bank: its name is not known.
    let env = env_of(game);
    let unnamed = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&[]))
        .file(WAD, &hashed, bank_bytes(0));
    let run = LoadRun::of(&env, LoadOptions::new(), &unnamed);
    let (file, node) = run.file(BANK);
    assert_eq!((node.used, node.path.as_ref()), (FileUse::Override, None));
    assert_eq!(header_id(&node.bytes), 0);
    assert_eq!(node.source, FileSource::Archive);
    let finding: &BankUnnamed = run.only();
    assert_eq!((finding.wad, &finding.banks[..]), (node.wad, &[file][..]));
    assert_eq!(BankUnnamed::CODE.severity, Severity::Warning);
    assert_eq!(run.log.of::<BankIdSet>().count(), 0);

    // The skin bin spells the path: the fixup runs once the bins are read.
    let named = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&[BANK]))
        .file(WAD, &hashed, bank_bytes(0));
    let run = LoadRun::of(&env, LoadOptions::new(), &named);
    let (_, node) = run.file(BANK);
    assert_eq!(header_id(&node.bytes), id_of("test_base_sfx_audio"));
    assert_eq!(run.log.of::<BankUnnamed>().count(), 0);
}

#[test]
fn only_the_id_changes_in_a_bank_a_tool_laid_out() {
    // A clip three bytes into the data, where the writer of a whole bank would align it, and
    // a section of an unknown tag after it.
    let mut index = 5u32.to_le_bytes().to_vec();
    index.extend(3u32.to_le_bytes());
    index.extend(4u32.to_le_bytes());
    let mut shipped = bank_bytes(0);
    shipped.extend(section(b"DIDX", &index));
    shipped.extend(section(b"DATA", b"...clip.."));
    shipped.extend(section(b"TOOL", b"left by a tool"));
    let read = Bank::read(&shipped).expect("a bank");
    assert_eq!(read.media[0].bytes, b"clip");
    assert_ne!(read.write(), shipped, "the writer lays the clip out again");

    let env = env();
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&[BANK]))
        .file(WAD, BANK, shipped.clone());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let (_, node) = run.file(BANK);
    let id = id_of("test_base_sfx_audio");
    assert_eq!(header_id(&node.bytes), id);
    let mut expected = shipped;
    expected[12..16].copy_from_slice(&id.to_le_bytes());
    assert_eq!(node.bytes[..], expected[..]);
}

#[test]
fn what_is_no_readable_sound_bank_is_left() {
    // A header that states more bytes than the file holds, a header too short to hold an id,
    // and a package, each where the client loads it.
    let mut cut = bank_bytes(0);
    cut[4..8].copy_from_slice(&64u32.to_le_bytes());
    let short = section(b"BKHD", &145u32.to_le_bytes());
    let mut package = b"r3d2".to_vec();
    package.extend(1u32.to_le_bytes());
    package.extend(0u32.to_le_bytes());
    assert_eq!(file_kind(&cut), FileKind::Bnk);
    assert!(Bank::read(&cut).is_err() && Bank::read(&short).is_err());

    let paths = [
        "assets/mod/a/cut_audio.bnk",
        "assets/mod/a/short_audio.bnk",
        "assets/mod/a/clips_audio.wpk",
    ];
    let env = env();
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&paths))
        .file(WAD, paths[0], cut.clone())
        .file(WAD, paths[1], short.clone())
        .file(WAD, paths[2], package.clone());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    for (path, shipped) in paths.iter().zip([cut, short, package]) {
        let (_, node) = run.file(path);
        assert_eq!(node.used, FileUse::Referenced, "{path}");
        assert_eq!(node.bytes[..], shipped[..], "{path}");
        assert_eq!(node.source, FileSource::Archive, "{path}");
    }
    let of_banks = run.log.iter().filter(|d| d.code.area() == "audio");
    assert_eq!(of_banks.count(), 0, "{:?}", run.messages());
}

#[test]
fn the_banks_of_a_wad_are_one_finding() {
    let other =
        "assets/sounds/wwise2016/sfx/characters/test/skins/skin03/test_skin03_sfx_audio.bnk";
    let env = env();
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin_naming(&[BANK, other]))
        .file(WAD, BANK, bank_bytes(0))
        .file(WAD, other, bank_bytes(0));
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let set: &BankIdSet = run.only();
    let mut ids: Vec<u32> = set.banks.iter().map(|bank| bank.id).collect();
    ids.sort_unstable();
    let mut expected = [id_of("test_base_sfx_audio"), id_of("test_skin03_sfx_audio")];
    expected.sort_unstable();
    assert_eq!(ids, expected);
    // In file order.
    assert!(set.banks.windows(2).all(|pair| pair[0].file < pair[1].file));
    let data = serde_json::to_value(set).expect("a finding writes");
    assert_eq!(data["banks"][0]["id"], set.banks[0].id);
    assert_eq!(data["banks"][0]["file"], set.banks[0].file.to_string());
}

/// Runs the fixup on a loaded mod, as `load` runs it.
fn run_again(env: &Env, module: &mut Mod, book: &mut NameBook, log: &mut Diagnostics) {
    let fixup = BankId;
    let mut stage = log.at(Stage::Load);
    let mut sink = stage.by(fixup.id(), one_record(&fixup));
    fixup
        .fix(&mut FileFixCx {
            module,
            game: env.game(),
            history: &NoHistory,
            names: &Names::default(),
            book,
            sink: &mut sink,
        })
        .expect("the fixup asks the history nothing");
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let env = env();
    let LoadRun {
        result,
        mut log,
        mut names,
        ..
    } = LoadRun::of(&env, LoadOptions::new(), &mod_with(0));
    let mut module = result.expect("a mod");
    let file = module.file_at(module.wads.ids().next().unwrap(), path_hash(BANK));
    let file = file.expect("the bank");
    let after = module.files[file].clone();
    let raised = log.len();

    run_again(&env, &mut module, &mut names, &mut log);
    let again = &module.files[file];
    assert_eq!(again.bytes[..], after.bytes[..]);
    assert_eq!(
        (again.content, again.size, again.source),
        (after.content, after.size, after.source)
    );
    assert_eq!(log.len(), raised, "the second run raises nothing");
}

#[test]
fn a_rewrite_keeps_what_the_archive_shipped_and_a_made_file_stays_made() {
    let env = env();
    let LoadRun {
        result,
        mut log,
        mut names,
        ..
    } = LoadRun::of(&env, LoadOptions::new().without_fixups(), &mod_with(0));
    let mut module = result.expect("a mod");
    let wad = module.wads.ids().next().unwrap();
    let bank = module.file_at(wad, path_hash(BANK)).expect("the bank");
    let shipped = module.files[bank].content;
    assert_eq!(header_id(&module.files[bank].bytes), 0, "no fixup ran");

    let mut stage = log.at(Stage::Load);
    let mut sink = stage.by("test", None);
    let mut cx = FileFixCx {
        module: &mut module,
        game: env.game(),
        history: &NoHistory,
        names: &Names::default(),
        book: &mut names,
        sink: &mut sink,
    };
    let copy = cx
        .add(NewFile {
            from: bank,
            by: MadeBy::MovedAsset,
            to: ChunkPath::parse("assets/mod/a/copy_audio.bnk").unwrap(),
            bytes: bank_bytes(0).into(),
            kind: FileKind::Bnk,
        })
        .expect("the path is free");
    for round in [1u32, 2] {
        cx.rewrite(bank, bank_bytes(round).into(), MadeBy::BankId);
        cx.rewrite(copy, bank_bytes(round).into(), MadeBy::BankId);
    }
    let made = FileSource::Made {
        from: bank,
        by: MadeBy::MovedAsset,
    };
    let rewritten = FileSource::Rewritten {
        by: MadeBy::BankId,
        shipped,
    };
    for (file, source) in [(bank, rewritten), (copy, made)] {
        let node = &module.files[file];
        assert_eq!(node.source, source);
        assert_eq!(node.bytes[..], bank_bytes(2)[..]);
        assert_eq!(node.content, ContentId::of(&bank_bytes(2)));
        assert_eq!(node.size, bank_bytes(2).len() as u64);
    }
    assert_eq!(
        serde_json::to_value(rewritten).expect("a source writes"),
        serde_json::json!({ "source": "rewritten", "by": "bank-id", "shipped": shipped.hex() })
    );
}

#[test]
fn the_fixup_is_a_standard_file_fixup_that_runs_once_each_file_has_its_use() {
    let fixup = BankId;
    let info = fixup.info();
    assert_eq!(info.id, "bank-id");
    assert_eq!(info.phase, Phase::Files);
    assert!(info.default);
    assert_eq!(info.one_record(), Some(Record::mod_artifact(23)));
    assert_eq!(fixup.step(), FileStep::AfterResolve);
    assert_eq!(
        CODES.iter().map(|code| code.name).collect::<Vec<_>>(),
        ["audio.bank-id-set", "audio.bank-unnamed"]
    );
    assert!(CODES.iter().all(|code| code.channel == Channel::Output));

    let steps: Vec<(String, FileStep)> = LoadOptions::standard_fixups()
        .iter()
        .map(|fixup| (fixup.id().to_string(), fixup.step()))
        .collect();
    let row = |id: &str, step| (id.to_string(), step);
    assert_eq!(
        steps,
        [
            row("legacy-formats", FileStep::BeforeSurvey),
            row("moved-assets", FileStep::BeforeSurvey),
            row("bank-id", FileStep::AfterResolve),
        ]
    );
    // A set and the catalog list it where it runs: after the file fixups of the earlier step.
    let of_files = ["legacy-formats", "moved-assets", "bank-id"];
    assert_eq!(FixupSet::standard().ids()[..3], of_files);
    let catalog = FixupSet::catalog();
    let listed: Vec<&str> = catalog.iter().map(|info| info.id.as_str()).collect();
    assert_eq!(listed[..3], of_files);

    // Until the bins are read no file has a use: the step that makes files leaves the bank.
    let env = env();
    let mut log = Diagnostics::new();
    let bytes = mod_with(0).build();
    let caps = reforge::archive::Caps::default();
    let archive = reforge::archive::Archive::read(
        &bytes,
        reforge_testkit::mods::FILE_NAME,
        &caps,
        &mut log.at(Stage::Read),
    )
    .expect("it reads");
    let mut loader = Loader::new(&env);
    let mut sink = log.at(Stage::Load);
    let mut m = loader.open(archive);
    let bank = m
        .file_at(m.wads.ids().next().unwrap(), path_hash(BANK))
        .unwrap();
    loader
        .fix_files(&mut m, &mut sink)
        .expect("no history is asked");
    loader
        .take_game_bins(&mut m, &mut sink)
        .expect("no history is asked");
    loader.survey(&mut m, &mut sink).expect("a skin mod");
    assert_eq!(header_id(&m.files[bank].bytes), 0);
    loader
        .resolve(&mut m, &mut sink)
        .expect("no history is asked");
    assert_eq!(
        header_id(&m.files[bank].bytes),
        id_of("test_base_sfx_audio")
    );
}

#[test]
fn the_fixup_is_turned_off_by_its_id() {
    let env = env();
    let options = LoadOptions::new().without_fixup("bank-id");
    let run = LoadRun::of(&env, options, &mod_with(0));
    assert_eq!(header_id(&run.file(BANK).1.bytes), 0);
    assert_eq!(run.log.of::<BankIdSet>().count(), 0);

    // A chosen set of fixups holds it, or leaves it out.
    let ids = |set: &FixupSet| -> Vec<String> {
        let fixups = set.file_fixups();
        fixups.iter().map(|f| f.id().to_string()).collect()
    };
    assert!(ids(&FixupSet::standard()).contains(&"bank-id".to_string()));
    let without = FixupSet::from(&FixupSelection::standard().without("bank-id"));
    assert!(!ids(&without).contains(&"bank-id".to_string()));
    let options = LoadOptions::new().file_fixups(without.file_fixups());
    let run = LoadRun::of(&env, options, &mod_with(0));
    assert_eq!(header_id(&run.file(BANK).1.bytes), 0);
}

#[test]
fn the_dump_marks_a_rewritten_file() {
    let env = env();
    let run = LoadRun::of(&env, LoadOptions::new(), &mod_with(0));
    let dump = run.dump();
    let line = dump.lines().find(|line| line.contains(" bnk "));
    let line = line.unwrap_or_else(|| panic!("no bank in:\n{dump}"));
    assert!(line.ends_with(" rewritten(bank-id)"), "{line}");
}
