//! The questions every history answers from its builds alone, asked of a history in memory and
//! of no history: the version at a position, the first build of a patch, whether a life
//! reaches the head.

use reforge::history::{ClipJoints, EventsBank, History, Life, NoHistory, Position};
use reforge::model::{ClipKey, NormKey, ObjectKey, name_hash, path_hash};

use reforge_testkit::history::{MemoryHistory, file_key};

const SKIN: &str = "assets/characters/lux/skins/base/lux.skn";
const OLD_SKIN: &str = "assets/characters/lux/lux.skn";

/// Five builds in the order they arrived: a hotfix of 16.18 lands after the first build of
/// 16.19.
fn builds() -> MemoryHistory {
    MemoryHistory::new(&[
        "16.17.100",
        "16.18.200",
        "16.19.300",
        "16.18.250",
        "16.19.400",
    ])
}

#[test]
fn a_position_gives_the_version_of_its_build() {
    let history = builds();
    let version = |at: u16| history.version(Position(at)).map(ToString::to_string);
    assert_eq!(version(0).as_deref(), Some("16.17.100"));
    assert_eq!(version(3).as_deref(), Some("16.18.250"));
    assert_eq!(version(5), None);
    assert_eq!(history.build(Position(4)).unwrap().commit, "c4");
}

#[test]
fn a_patch_starts_at_the_first_build_that_arrived_of_it() {
    let history = builds();
    assert_eq!(history.first_of_patch("16.18"), Some(Position(1)));
    assert_eq!(history.first_of_patch("16.19"), Some(Position(2)));
    assert_eq!(history.first_of_patch("16.20"), None);
    // `16.1` is no prefix of `16.17`: a patch ends at its dot.
    assert_eq!(history.first_of_patch("16.1"), None);
}

#[test]
fn a_life_reaches_the_head_when_it_lasts_to_the_build_appended_last() {
    let history = builds();
    assert_eq!(history.head(), Some(Position(4)));
    assert!(history.reaches_head(Life::new(1, 4)));
    assert!(!history.reaches_head(Life::new(1, 3)));
    // The head is the build appended last, the newest build the highest version.
    let stamp = history.stamp().unwrap();
    assert_eq!(stamp.head, "c4");
    assert_eq!(stamp.newest_build.as_str(), "16.19.400");
}

#[test]
fn the_head_of_a_history_whose_last_build_is_a_hotfix_is_not_its_newest_build() {
    let history = MemoryHistory::new(&["16.18.200", "16.19.300", "16.18.250"]);
    assert_eq!(history.head(), Some(Position(2)));
    assert_eq!(history.stamp().unwrap().newest_build.as_str(), "16.19.300");
}

#[test]
fn a_life_is_spelled_as_versions() {
    let history = builds();
    assert_eq!(history.span(Life::new(0, 2)), "16.17.100 .. 16.19.300");
    assert_eq!(history.span(Life::new(1, 1)), "16.18.200");
    assert_eq!(history.span(Life::new(4, 9)), "16.19.400 .. #9");
}

#[test]
fn a_history_in_memory_answers_from_its_facts() {
    let history = builds()
        .with_content(OLD_SKIN, b"mesh", 0..=1)
        .with_content(SKIN, b"mesh", 2..=4)
        .with_content(SKIN, b"older mesh", 0..=1)
        .with_path("assets/characters/lux/lux.skl", 0..=4)
        .with_object(
            ObjectKey([1; 16]),
            NormKey(7),
            (
                "Characters/Lux/Skins/Skin0",
                "SkinCharacterDataProperties",
                "data/characters/lux/skins/skin0.bin",
            ),
            1..=4,
        )
        .with_clip(ClipKey(9), 3..=3);
    let mesh = file_key(b"mesh");

    let file = history.file(mesh).unwrap().unwrap();
    assert_eq!(file.life, Life::new(0, 4));
    assert_eq!(file.path, path_hash(OLD_SKIN));
    assert!(history.reaches_head(file.life));
    assert_eq!(history.file(file_key(b"own")).unwrap(), None);

    assert_eq!(
        history.held(path_hash(SKIN), mesh).unwrap(),
        Some(Life::new(2, 4))
    );
    assert_eq!(
        history
            .held(path_hash(SKIN), file_key(b"older mesh"))
            .unwrap(),
        Some(Life::new(0, 1))
    );
    assert_eq!(
        history.held(path_hash(SKIN), file_key(b"own")).unwrap(),
        None
    );
    let paths: Vec<_> = history.paths_of(mesh).unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|at| at.path == path_hash(SKIN)));

    let path = history.path(path_hash(SKIN)).unwrap().unwrap();
    assert_eq!(path.life, Life::new(0, 4));
    let former = history.path(path_hash(OLD_SKIN)).unwrap().unwrap();
    assert!(!history.reaches_head(former.life));
    let skeleton = path_hash("assets/characters/lux/lux.skl");
    assert_eq!(
        history.path(skeleton).unwrap().unwrap().life,
        Life::new(0, 4)
    );

    let object = history.object(ObjectKey([1; 16])).unwrap().unwrap();
    assert_eq!(object.entry, name_hash("Characters/Lux/Skins/Skin0"));
    assert_eq!(object.life, Life::new(1, 4));
    assert_eq!(history.objects_by_norm(NormKey(7)).unwrap(), [object]);
    assert!(history.objects_by_norm(NormKey(8)).unwrap().is_empty());
    assert_eq!(history.clip(ClipKey(9)).unwrap(), Some(Life::new(3, 3)));
    assert_eq!(history.clip(ClipKey(8)).unwrap(), None);
}

#[test]
fn a_history_in_memory_answers_a_clips_joints_and_an_events_bank() {
    const CLIP: &str = "assets/characters/lux/skins/base/animations/lux_idle1.anm";
    const BANK: &str =
        "assets/sounds/wwise2016/sfx/characters/lux/skins/base/lux_base_sfx_events.bnk";
    let bank = |id: u8| EventsBank::new(vec![145, 0, 0, 0, id, 0, 0, 0], Vec::new()).unwrap();
    let history = builds()
        .with_animation(CLIP, b"idle", &[1, 2], 0..=1)
        .with_animation(CLIP, b"idle", &[1, 2], 3..=3)
        .with_animation(CLIP, b"new idle", &[1, 2, 3], 4..=4)
        .with_events_bank(BANK, bank(5), 0..=2)
        .with_events_bank(BANK, bank(6), 3..=4)
        .with_events_bank(BANK, bank(5), 4..=4);

    let clip = |bytes: &[u8], life, joints: &[u32]| ClipJoints {
        life,
        key: file_key(bytes),
        joints: joints.to_vec(),
    };
    let expected = [
        clip(b"idle", Life::new(0, 3), &[1, 2]),
        clip(b"new idle", Life::new(4, 4), &[1, 2, 3]),
    ];
    assert_eq!(history.clip_joints(path_hash(CLIP)).unwrap(), expected);
    assert!(history.clip_joints(path_hash(SKIN)).unwrap().is_empty());
    let joints = history.joints_of(file_key(b"new idle")).unwrap();
    assert_eq!(joints, Some(vec![1, 2, 3]));
    assert_eq!(history.joints_of(file_key(b"own")).unwrap(), None);
    // The clip's bytes are a content of the history too.
    let held = history.held(path_hash(CLIP), file_key(b"idle")).unwrap();
    assert_eq!(held, Some(Life::new(0, 3)));

    let banks = history.events_banks(path_hash(BANK)).unwrap();
    let ids: Vec<(u32, Life)> = banks
        .iter()
        .map(|hit| (hit.bank.bank_id(), hit.life))
        .collect();
    assert_eq!(ids, [(5, Life::new(0, 4)), (6, Life::new(3, 4))]);
    assert!(history.events_banks(path_hash(SKIN)).unwrap().is_empty());
}

#[test]
fn no_history_answers_nothing() {
    let history = NoHistory;
    assert!(history.stamp().is_none());
    assert_eq!(history.head(), None);
    assert_eq!(history.version(Position(0)), None);
    assert_eq!(history.first_of_patch("16.19"), None);
    assert!(!history.reaches_head(Life::new(0, 0)));
    assert_eq!(history.file(file_key(b"mesh")).unwrap(), None);
    assert_eq!(history.path(path_hash(SKIN)).unwrap(), None);
    assert_eq!(
        history.held(path_hash(SKIN), file_key(b"mesh")).unwrap(),
        None
    );
    assert_eq!(history.object(ObjectKey([1; 16])).unwrap(), None);
    assert_eq!(history.clip(ClipKey(9)).unwrap(), None);
    assert!(history.clip_joints(path_hash(SKIN)).unwrap().is_empty());
    assert_eq!(history.joints_of(file_key(b"mesh")).unwrap(), None);
    assert!(history.events_banks(path_hash(SKIN)).unwrap().is_empty());
    assert!(history.paths_of(file_key(b"mesh")).unwrap().is_empty());
    assert!(history.named_by(path_hash(SKIN)).unwrap().is_empty());
    assert!(history.skeletons(path_hash(SKIN)).unwrap().is_empty());
}
