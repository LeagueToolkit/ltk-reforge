//! The report: a run's results as rows, serialised. The game is `reforge_testkit::skins`.

use reforge::archive::{Archive, Caps};
use reforge::diag::{Diagnostics, Render, Stage};
use reforge::env::Env;
use reforge::fixups::{FixupSet, Rule};
use reforge::game::Build;
use reforge::history::Position;
use reforge::load::dating::Bound;
use reforge::load::{Dating, FileProvenance, Inventory, LoadError, LoadOptions, Loader, against};
use reforge::model::entity::{FileSource, FileUse};
use reforge::model::{ContentId, hash, value};
use reforge::report::{AgainRow, Report, SCHEMA};
use reforge_testkit::bins::{Skin, bin, blob, entry, float, linked_bin, object};
use reforge_testkit::skins::{self, BASE_TEX, WAD, base_skin, skin_bin};
use reforge_testkit::{LoadRun, MemoryHistory, ModArchive, PackRun};
use serde_json::{Value, json};

const ENTRY: &str = "Characters/Test/Skins/Skin0";
const OWN_MESH: &str = "assets/mod/a/mesh.skn";
/// The base texture as the game shipped it at the first build of the history.
const OLD_TEX: &[u8] = b"the base texture at 16.18";

fn env() -> Env {
    let game = skins::game();
    let sizes = game.sizes();
    let history =
        MemoryHistory::new(&["16.18.1", "16.19.1"]).with_content(BASE_TEX, OLD_TEX, 0..=0);
    let env = Env::builder()
        .game(game)
        .sizes(sizes)
        .history(Box::new(history));
    env.build().expect("an environment")
}

fn own_skin() -> Skin {
    Skin {
        mesh: OWN_MESH.into(),
        ..base_skin()
    }
}

/// A skin bin naming its own mesh and linking a bin nothing holds, the mesh, a stale copy of
/// the base texture, and a readme outside the WAD.
fn small_mod() -> ModArchive {
    let skin = linked_bin(vec![entry(ENTRY, &own_skin())], &["DATA/Nowhere.bin"]);
    ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin)
        .file(WAD, OWN_MESH, blob(OWN_MESH))
        .file(WAD, BASE_TEX, OLD_TEX)
        .extra("README.txt", b"read me".to_vec())
}

/// The report of a loaded archive, with the three results on its files.
fn report_of(env: &Env, archive: &ModArchive) -> Report {
    let run = LoadRun::of(env, LoadOptions::new(), archive);
    let speller = run.speller();
    let m = run.module();
    let dating = Dating::find(m, env.history()).expect("the history answers");
    let inventory = Inventory::take(m, &speller);
    let provenance = FileProvenance::find(m, &against(env, &speller)).expect("it answers");
    let cx = Render {
        names: &speller,
        module: Some(m),
    };
    Report::builder()
        .env(env)
        .module(m)
        .dating(&dating, m)
        .inventory(&inventory)
        .provenance(&provenance)
        .diagnostics(&run.log, &cx)
        .build()
}

fn parsed(report: &Report) -> Value {
    serde_json::from_str(&report.to_json()).expect("a report is JSON")
}

#[test]
fn a_report_with_no_part_holds_its_schema_and_nothing_else() {
    let report = parsed(&Report::builder().build());
    assert_eq!(report["schema"], SCHEMA);
    assert_eq!(report["mod"], Value::Null);
    assert_eq!(report["inputs"], Value::Null);
    assert_eq!(report["halted"], Value::Null);
    assert_eq!(report["dating"], Value::Null);
    assert_eq!(report["package"], Value::Null);
    for table in ["wads", "files", "characters", "units", "diagnostics"] {
        assert_eq!(report[table], json!([]), "{table}");
    }
}

#[test]
fn a_report_holds_the_inputs_and_the_mod_as_rows_joined_by_ids() {
    let env = env();
    let report = parsed(&report_of(&env, &small_mod()));

    assert_eq!(report["mod"]["name"], "Synthetic");
    assert_eq!(report["mod"]["mod_id"].as_str().map(str::len), Some(12));
    assert_eq!(report["inputs"]["install"], "memory");
    assert_eq!(report["inputs"]["build"], "16.19.1");
    let history = json!({ "head": "c1", "newest_build": "16.19.1", "builds": 2 });
    assert_eq!(report["inputs"]["history"], history);

    let wad = &report["wads"][0];
    let expected = json!({
        "id": "%w0", "name": WAD, "role": "champion", "champion": "test", "layout": "loose",
        "files": 3, "named": 3, "restoration": false,
    });
    assert_eq!(wad, &expected);

    let files = report["files"].as_array().expect("rows");
    assert_eq!(files.len(), 3);
    let file = |path: &str| {
        let found = files.iter().find(|f| f["path"] == path);
        found.unwrap_or_else(|| panic!("no row of {path}")).clone()
    };
    let texture = file(BASE_TEX);
    assert_eq!(texture["use"], "stale");
    assert_eq!(texture["source"], "archive");
    assert_eq!(texture["wad"], "%w0");
    assert_eq!(texture["size"], OLD_TEX.len());
    assert_eq!(texture["hash"].as_str().map(str::len), Some(16));
    assert_eq!(texture["content"].as_str().map(str::len), Some(64));
    let skin = file(&skin_bin("test", 0));
    assert_eq!(
        (&skin["kind"], &skin["use"]),
        (&json!("bin"), &json!("loaded"))
    );

    let character = json!({
        "id": "%c0", "name": "test", "spelled": "test", "kind": "champion", "wad": "%w0",
        "units": ["%u0"], "root_bin": null, "equal_to_game": false,
    });
    assert_eq!(report["characters"], json!([character]));
    let unit = json!({
        "id": "%u0", "character": "%c0", "character_name": "test", "slot": 0,
        "file": skin["id"], "bin": "%b0", "loads": [skin["id"]], "entry": "%o0",
        "entry_name_slot": 0,
    });
    assert_eq!(report["units"], json!([unit]));
}

#[test]
fn a_report_holds_the_results_on_the_files() {
    let env = env();
    let report = parsed(&report_of(&env, &small_mod()));

    // Nothing of the mod is large enough to date it.
    let dating = json!({ "not_before": null, "evidence": null, "dated": 0, "at_bound": 0 });
    assert_eq!(report["dating"], dating);
    assert_eq!(report["inventory"]["by_kind"]["property-bin"]["files"], 1);
    assert_eq!(report["inventory"]["items"], json!([]));

    let rows = report["provenance"].as_array().expect("rows");
    let texture = report["files"].as_array().expect("rows").iter();
    let texture = texture
        .clone()
        .find(|f| f["path"] == BASE_TEX)
        .expect("the texture");
    let fact = rows
        .iter()
        .find(|row| row["file"] == texture["id"])
        .expect("its fact");
    assert_eq!(fact["at_path"], "other-bytes");
    assert_eq!(fact["origin"], "history");
    assert_eq!(fact["life"], json!({ "first": 0, "last": 0 }));
    assert_eq!(fact["path"], texture["hash"]);
    assert_eq!(fact["still_shipped"], false);
}

#[test]
fn a_report_holds_every_diagnostic_as_the_log_holds_it() {
    let env = env();
    let report = parsed(&report_of(&env, &small_mod()));
    let rows = report["diagnostics"].as_array().expect("rows");
    let codes: Vec<&str> = rows.iter().filter_map(|d| d["code"].as_str()).collect();
    assert_eq!(codes, ["file.stale", "bin.link-missing"]);
    let ids: Vec<u64> = rows.iter().filter_map(|d| d["id"].as_u64()).collect();
    assert_eq!(ids, [0, 1]);

    let stale = &rows[0];
    assert_eq!(stale["severity"], "note");
    assert_eq!(stale["channel"], "input");
    assert_eq!(stale["stage"], "load");
    let subject = json!({ "kind": "wad", "id": "%w0", "name": WAD });
    assert_eq!(stale["subject"], subject);
    assert_eq!(stale["record"], "#MOD-ARTIFACT-0004");
    assert!(stale.get("by").is_none(), "no fixup raised it");
    assert_eq!(stale["data"]["bytes"], OLD_TEX.len());
    let message = stale["message"].as_str().expect("a line");
    assert!(
        message.starts_with("1 file (0.0 MiB) at the game's paths"),
        "{message}"
    );

    let missing = &rows[1];
    assert_eq!(missing["subject"]["kind"], "bin");
    assert_eq!(missing["subject"]["name"], skin_bin("test", 0));
    assert_eq!(missing["data"]["links"][0]["path"], "DATA/Nowhere.bin");
}

#[test]
fn a_fixups_diagnostic_names_the_fixup() {
    // The base texture in the format the game dropped: the twin is made, by `legacy-formats`.
    let dds = [b"DDS ".as_slice(), &[0u8; 124]].concat();
    let old = "assets/characters/test/skins/base/test.dds";
    let archive = ModArchive::new()
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry(ENTRY, &base_skin())]),
        )
        .file(WAD, old, dds);
    let env = env();
    let report = parsed(&report_of(&env, &archive));
    let made = &report["diagnostics"][0];
    assert_eq!(made["code"], "file.format-converted");
    assert_eq!(made["by"], "legacy-formats");
    assert_eq!(made["channel"], "output");
    let files = report["files"].as_array().expect("rows");
    let twin = files
        .iter()
        .find(|f| f["source"] == "made")
        .expect("the twin");
    assert_eq!(twin["by"], "legacy-format");
    let from = files
        .iter()
        .find(|f| f["id"] == twin["from"])
        .expect("its source");
    assert_eq!(from["path"], old);
}

#[test]
fn a_halted_run_reports_the_mod_as_far_as_it_was_loaded() {
    // The steps one by one, as a caller that keeps the mod of a halted load runs them.
    let env = env();
    let archive = ModArchive::new().file("Map11.wad.client", "data/maps/x.bin", bin(vec![]));
    let mut log = Diagnostics::new();
    let bytes = archive.build();
    let read = Archive::read(
        &bytes,
        "m.fantome",
        &Caps::default(),
        &mut log.at(Stage::Read),
    );
    let mut loader = Loader::new(&env);
    let mut m = loader.open(read.expect("it reads"));
    let mut sink = log.at(Stage::Load);
    loader.fix_files(&mut m, &mut sink).expect("it answers");
    loader
        .take_game_bins(&mut m, &mut sink)
        .expect("it answers");
    let Err(LoadError::Halted(halted)) = loader.survey(&mut m, &mut sink) else {
        panic!("a mod with no skin ends the load at a fatal diagnostic");
    };

    let names = loader.into_names();
    let speller = reforge::game::Speller::new(&names, env.names());
    let cx = Render {
        names: &speller,
        module: Some(&m),
    };
    let report = Report::builder().module(&m).diagnostics(&log, &cx).build();
    assert_eq!(report.halted, Some(halted.0.0));
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].used, FileUse::Unused);
    assert_eq!(report.files[0].source, FileSource::Archive);
    assert!(report.units.is_empty() && report.dating.is_none() && report.inputs.is_none());
    let last = report.diagnostics.last().expect("the fatal one");
    assert_eq!(
        (last.code, last.severity, last.id),
        ("archive.no-skin", "fatal", halted.0.0)
    );
}

#[test]
fn two_runs_of_one_archive_give_the_same_report() {
    let env = env();
    let first = report_of(&env, &small_mod()).to_json();
    let second = report_of(&env, &small_mod()).to_json();
    assert_eq!(first, second);
}

#[test]
fn text_a_mod_spells_is_escaped_in_the_json_and_reads_back_as_written() {
    let link = "DATA/\u{1b}[31mMod.bin";
    let skin = linked_bin(vec![entry(ENTRY, &own_skin())], &[link]);
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), skin);
    let env = env();
    let report = report_of(&env, &archive);
    let json = report.to_json();
    assert!(!json.contains('\u{1b}'), "no character a terminal acts on");
    assert!(json.contains("\\u001b[31mMod.bin"));
    let read: Value = serde_json::from_str(&json).expect("JSON");
    assert_eq!(read["diagnostics"][0]["data"]["links"][0]["path"], link);
}

#[test]
fn a_run_with_no_history_says_it_was_judged_against_the_install_alone() {
    let env = Env::builder()
        .game(skins::game())
        .build()
        .expect("an environment");
    let report = parsed(&Report::builder().env(&env).build());
    assert_eq!(report["inputs"]["build"], "16.19.1");
    assert_eq!(report["inputs"]["history"], Value::Null);
}

#[test]
fn a_report_holds_the_package_with_the_digest_of_each_object_of_its_bins() {
    let env = env();
    let material = object("M", "Material", vec![("junk", float(1.0))]);
    let skin = bin(vec![entry(ENTRY, &own_skin()), material]);
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin)
        .file(WAD, OWN_MESH, blob(OWN_MESH));
    // One fixup: a field no bin of the game holds, removed.
    let set = FixupSet::empty().with(Rule::remove_fields("Material", ["junk"]).id("no-junk"));
    let run = PackRun::of(&env, &set, &archive);
    let built = Report::builder().package(run.package()).build();
    let report = parsed(&built);

    let package = &report["package"];
    assert_eq!(package["name"], "Synthetic-reforgetests");
    assert_eq!(package["version"], "1.0.0");
    let chunks = package["chunks"].as_array().expect("rows");
    assert_eq!(chunks.len(), 2);
    let mesh = &chunks[0];
    assert_eq!(mesh["path"], OWN_MESH);
    assert_eq!(mesh["origin"], "kept");
    assert_eq!(mesh["content"], ContentId::of(&blob(OWN_MESH)).hex());
    assert_eq!(mesh.get("objects"), None);
    assert_eq!(mesh.get("edited"), None);

    let skin = &chunks[1];
    let bytes = &run.chunk(&skin_bin("test", 0)).bytes;
    assert_eq!(skin["layer"], "base");
    assert_eq!(skin["wad"], WAD);
    assert_eq!(skin["path"], skin_bin("test", 0));
    assert_eq!(skin["origin"], "delta");
    assert_eq!(skin["size"], bytes.len());
    assert_eq!(skin["content"], ContentId::of(bytes).hex());
    let chunk = run.chunk(&skin_bin("test", 0));
    let file = chunk
        .origin
        .file()
        .expect("a chunk of a repair is a file of the mod");
    assert_eq!(skin["file"], file.to_string());
    // Each object by its entry hash, in file order, with the digest of the value written.
    let digest = |name: &str| hash::hex(&value::digest(run.fix.object(name)));
    let entry_of = |name: &str| format!("{:08x}", reforge::model::name_hash(name).0);
    assert_eq!(
        skin["objects"],
        json!({ entry_of(ENTRY): digest(ENTRY), entry_of("M"): digest("M") })
    );
    assert_eq!(skin["edited"], json!([entry_of("M")]));
    // The changed object again, with its fields in the order of their hashes: it holds none
    // after the fixup, so the two digests are one.
    assert_eq!(skin["values"], json!({ entry_of("M"): digest("M") }));
    let keys: Vec<&String> = skin["objects"].as_object().expect("a map").keys().collect();
    assert_eq!(keys, [&entry_of(ENTRY), &entry_of("M")]);
    assert_eq!(
        built.to_json(),
        Report::builder().package(run.package()).build().to_json()
    );
}

#[test]
fn a_report_says_what_repairing_the_repaired_mod_again_changed() {
    let env = env();
    let material = object("M", "Material", vec![("junk", float(1.0))]);
    let skin = bin(vec![entry(ENTRY, &own_skin()), material]);
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin)
        .file(WAD, OWN_MESH, blob(OWN_MESH));
    let set = FixupSet::empty().with(Rule::remove_fields("Material", ["junk"]).id("no-junk"));
    let first = PackRun::of(&env, &set, &archive);
    let second = PackRun::of(&env, &set, &first.archive());

    // The package repaired again: no value changes, and every chunk is the same.
    let again = AgainRow::of(
        first.package(),
        Some(second.package()),
        Some(&second.fix.changes),
    );
    assert!(again.same(), "{again:?}");
    let report = parsed(&Report::builder().again(again.clone()).build());
    assert_eq!(
        report["again"],
        json!({ "built": true, "fixups": [], "chunks_differing": 0, "chunks": [] })
    );
    // The build the repaired mod dates at by itself is beside it, where the caller gave it.
    let dating = Dating {
        not_before: Some(Bound {
            at: Position(0),
            build: Build::parse("16.18.1").expect("a build"),
        }),
        ..Dating::default()
    };
    let report = parsed(&Report::builder().again(again.redated(&dating)).build());
    assert_eq!(report["again"]["redated"]["build"], "16.18.1");

    // Against the archive's own run, which changed the material: the fixup and the bin.
    let unrepaired = PackRun::of(&env, &FixupSet::empty(), &archive);
    let again = AgainRow::of(
        unrepaired.package(),
        Some(first.package()),
        Some(&first.fix.changes),
    );
    assert!(!again.same());
    let row = serde_json::to_value(&again).expect("a row is data");
    assert_eq!(row["fixups"], json!([{ "fixup": "no-junk", "values": 1 }]));
    assert_eq!(row["chunks_differing"], 1);
    assert_eq!(row["chunks"][0]["wad"], WAD);
    let shipped = &unrepaired.chunk(&skin_bin("test", 0)).bytes;
    let repaired = &first.chunk(&skin_bin("test", 0)).bytes;
    assert_eq!(row["chunks"][0]["first"], ContentId::of(shipped).hex());
    assert_eq!(row["chunks"][0]["second"], ContentId::of(repaired).hex());

    // A second run that builds nothing is no repair that changes nothing.
    assert!(!AgainRow::of(first.package(), None, None).same());
    // A report nobody asked a second run of holds no such part.
    assert_eq!(parsed(&Report::builder().build()).get("again"), None);
}
