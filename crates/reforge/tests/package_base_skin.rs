//! The check `base-skin` on a port's package (#ADR-0062): a champion's patched base skin that
//! names another skin of the champion is reported, and one that names none is not. The game is
//! `reforge_testkit::skins`.

use reforge::history::NoHistory;
use reforge::package::{Coverage, Done, OtherSkin, OtherSkinName, SkinValue};
use reforge::port::{AnimReuse, Policy, PortOptions, TargetPolicy};
use reforge_testkit::PortRun;
use reforge_testkit::bins::Skin;
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, S3_SKEL};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// A skin0 mod on a mesh of its own and this skeleton.
fn on_skeleton(skeleton: Option<&str>) -> reforge_testkit::ModArchive {
    let mut files = own_files("a");
    let mut skin = own_skin("a", GRAPH0, RESOURCES0);
    if let Some(skeleton) = skeleton {
        skin = Skin {
            skeleton: skeleton.into(),
            ..skin
        };
    }
    let objects = vec![own_graph("a", GRAPH0)];
    files.push(mod_skin("test", 0, SKIN0, &skin, objects));
    archive(&files)
}

/// How many patched base skins the check read.
fn read(run: &PortRun<'_>) -> Vec<usize> {
    let done = run.found::<Done>().into_iter();
    done.filter_map(|done| match done.0 {
        Coverage::BaseSkin { entries } => Some(entries),
        _ => None,
    })
    .collect()
}

#[test]
fn a_base_skin_on_the_mods_own_rig_names_no_other_skin() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &on_skeleton(None));
    assert!(run.found::<OtherSkin>().is_empty(), "{:#?}", run.messages());
    assert_eq!(read(&run), [1]);
}

#[test]
fn a_base_skin_forced_onto_a_premium_skins_skeleton_names_that_skin() {
    let env = env(skins::game(), NoHistory);
    // Left to the facts the unit goes to skin 3. Forced to the base, its entry names skin 3's
    // skeleton at the game's path.
    let forced = Policy {
        target: TargetPolicy::Base,
        anim_reuse: AnimReuse::Block,
    };
    let mods = on_skeleton(Some(S3_SKEL));
    let run = PortRun::under(&env, &mods, forced, PortOptions::default());
    let found = run.found::<OtherSkin>();
    let values: Vec<&OtherSkinName> = found.iter().flat_map(|found| &found.values).collect();
    assert_eq!(values.len(), 1, "{:#?}", run.messages());
    assert_eq!(values[0].value, SkinValue::Skeleton);
    assert!(values[0].names.eq_ignore_ascii_case(S3_SKEL));
    assert_eq!(read(&run), [1]);
}
