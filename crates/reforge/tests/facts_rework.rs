//! A reworked champion's mod judged against the rework (#ADR-0019): made for the model before
//! it, after it, or nothing says. The game is `reforge_testkit::skins`, and the history holds
//! one build before the rework and one from it.

use reforge::facts::{Judged, Made};
use reforge::history::Position;
use reforge::load::ReworkTable;
use reforge_testkit::bins::blob;
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::{FactRun, MemoryHistory, skins};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OLD_TEX: &str = "assets/characters/test/skins/base/test_old.tex";
const SHARED_TEX: &str = "assets/shared/particles/glow.tex";

/// A history of two builds in which `test` is reworked at the second: the old texture's path
/// is dropped at it.
fn reworked() -> (MemoryHistory, ReworkTable) {
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]).with_path(OLD_TEX, 0..=0);
    (history, ReworkTable::new([("test", "16.19")]))
}

/// A skin0 mod on a model of its own, under paths that are not the champion's.
fn own_mod() -> Vec<(String, Vec<u8>)> {
    let mut files = own_files("a");
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    let graph = own_graph("a", GRAPH0);
    files.push(mod_skin("test", 0, SKIN0, &skin, vec![graph]));
    files
}

fn judged(run: &FactRun<'_>) -> Judged {
    let mut judged = run.facts.reworks.iter();
    let (Some(first), None) = (judged.next(), judged.next()) else {
        panic!("one champion is judged\n{}", run.dump());
    };
    first.clone()
}

#[test]
fn a_mod_with_no_file_around_the_rework_and_no_dating_is_not_judged() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    let run = FactRun::with_reworks(&env, &archive(&own_mod()), &table);
    let judged = judged(&run);
    assert_eq!(
        (judged.made, judged.older, judged.newer, judged.example),
        (Made::Unknown, 0, 0, None),
        "{}",
        run.dump()
    );
    assert_eq!(judged.rework, "16.19");
    assert!(run.facts.reworks.before(judged.character).is_none());
}

#[test]
fn a_mod_dated_from_the_rework_on_is_newer_whatever_old_paths_it_ships() {
    let (history, table) = reworked();
    // Bytes the game first shipped at the build of the rework date the mod there.
    let glow = vec![7; 6000];
    let history = history.with_content(SHARED_TEX, &glow, 1..=1);
    let env = env(skins::game(), history);
    let mut files = own_mod();
    files.push((OLD_TEX.into(), blob(OLD_TEX)));
    files.push(("assets/mod/a/glow.tex".into(), glow));
    let run = FactRun::with_reworks(&env, &archive(&files), &table);
    assert_eq!(
        run.fix.dating.position(),
        Some(Position(1)),
        "{}",
        run.dump()
    );
    let judged = judged(&run);
    assert_eq!(
        (judged.made, judged.older, judged.newer),
        (Made::After, 1, 0),
        "{}",
        run.dump()
    );
    assert!(run.facts.reworks.before(judged.character).is_none());
}
