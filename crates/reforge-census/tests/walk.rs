//! The walk over a history in memory: lives are hulls or spans, families filter, texts that
//! do not parse are counted.

mod common;

use std::collections::BTreeMap;

use reforge::history::tables::{
    Animation, BankEvent, BankFacts, JointList, Naming, Recorded, UnitEvent,
};
use reforge::history::{Families, Life};
use reforge::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, path_hash};
use reforge_census::{Walked, walk};

use self::common::{A, B, HEADER, JOINTS, Memory, NAMED, bank, build, events_bank, memory};

fn walked(families: Families) -> Walked {
    walk(&memory(), families, &mut |_| {}).unwrap()
}

#[test]
fn lives_are_hulls() {
    let walked = walked(Families::ALL);
    let head = walked.head();
    assert_eq!(walked.builds.len(), 3);
    assert_eq!(head.0, 2);
    let files = &walked.folds.files;
    let file = |key: &str| files.life(&FileKey::parse(key).unwrap(), head);
    assert_eq!(file(A), Some(Life::new(0, 2)));
    assert_eq!(file(B), Some(Life::new(1, 1)));
    let clips = &walked.folds.clips;
    assert_eq!(clips.life(&ClipKey(1), head), Some(Life::new(0, 0)));
    assert_eq!(clips.life(&ClipKey(2), head), Some(Life::new(1, 2)));
    assert_eq!(walked.unparsed_texts, 0);

    let objects = walked.folds.objects.rows(head);
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].key.key, ObjectKey::parse(A).unwrap());
    assert_eq!(objects[0].key.class, BinHash(2));
    assert_eq!(objects[0].life, Life::new(0, 2));
    assert_eq!(objects[0].alive, 1);
    let facts = objects[0].facts;
    assert_eq!((facts.bin, facts.norm), (WadHash(2), NormKey(7)));
    assert_eq!(facts.entry, BinHash(1));
}

#[test]
fn a_row_counts_the_records_that_hold_its_key_at_the_head() {
    let walked = walked(Families::ALL);
    let head = walked.head();
    // Bytes A are at path 1 and path 3 in the last build, bytes B at none.
    let files = walked.folds.files.rows(head);
    let alive: Vec<(String, u32)> = files
        .iter()
        .map(|row| (row.key.to_string(), row.alive))
        .collect();
    assert!(alive.contains(&(A[..32].to_string(), 2)), "{alive:?}");
    assert!(alive.contains(&(B[..32].to_string(), 0)), "{alive:?}");
}

#[test]
fn an_objects_text_gives_its_references_tags_and_units() {
    let walked = walked(Families::ALL);
    let head = walked.head();
    let refs = walked.folds.refs.rows(head);
    let named: Vec<Naming> = refs.iter().map(|row| row.key).collect();
    let mut paths = vec![WadHash(1), path_hash(NAMED), bank()];
    paths.sort_unstable();
    let naming = |path| Naming {
        path,
        bin: WadHash(2),
        entry: BinHash(1),
    };
    assert_eq!(named, paths.into_iter().map(naming).collect::<Vec<_>>());

    let tags = walked.folds.tags.rows(head);
    let tags: Vec<&str> = tags.iter().map(|row| row.key.tag.as_str()).collect();
    assert_eq!(tags, ["Test"]);
    let units = walked.folds.units.rows(head);
    assert_eq!(units.len(), 1);
    let unit = &units[0].key;
    assert_eq!((unit.unit.as_str(), unit.voice), ("Test_Base_VO", true));
    assert_eq!(walked.folds.unit_banks.len(), 1);
    let events = walked.folds.unit_events.rows(head);
    let expected = UnitEvent {
        bin: WadHash(2),
        entry: BinHash(1),
        unit: "Test_Base_VO".into(),
        event: reforge_wwise::id_of("Play_vo_Test"),
    };
    let events: Vec<&UnitEvent> = events.iter().map(|row| &row.key).collect();
    assert_eq!(events, [&expected]);
}

#[test]
fn an_entrys_file_gives_its_events_rig_and_texture() {
    let walked = walked(Families::ALL);
    let head = walked.head();
    let event = BankEvent {
        bank: bank(),
        event: 0xe1,
    };
    assert_eq!(
        walked.folds.events.life(&event, head),
        Some(Life::new(0, 2))
    );
    assert_eq!(walked.folds.event_wems.len(), 1);
    let skeletons = walked.folds.skeletons.rows(head);
    let mut shape: Vec<(Life, usize)> = skeletons
        .iter()
        .map(|row| (row.life, row.key.record.joints.len()))
        .collect();
    shape.sort_by_key(|(life, _)| life.first);
    assert_eq!(shape, [(Life::new(0, 1), 1), (Life::new(2, 2), 2)]);
    assert!(walked.folds.meshes.is_empty());
    // One texture record at path 1 through bytes A, B and A, and the same at path 3.
    let textures = walked.folds.textures.rows(head);
    let lives: Vec<(WadHash, Life)> = textures
        .iter()
        .map(|row| (row.key.path, row.life))
        .collect();
    assert_eq!(
        lives,
        [(WadHash(1), Life::new(0, 2)), (WadHash(3), Life::new(2, 2))]
    );
}

#[test]
fn an_animation_clips_joints_are_one_list_for_every_clip_that_animates_them() {
    let walked = walked(Families::ALL);
    let head = walked.head();
    let list = JointList(JOINTS.into());
    // The clip's bytes change in the last build and its joints stay.
    let lists = walked.folds.joint_lists.rows(head);
    assert_eq!(lists.len(), 1);
    assert_eq!((&lists[0].key, lists[0].life), (&list, Life::new(0, 2)));
    assert_eq!(lists[0].alive, 1);
    let animations = walked.folds.animations.rows(head);
    let clip = |bytes: char| Animation {
        path: WadHash(7),
        key: FileKey::parse(&bytes.to_string().repeat(64)).unwrap(),
    };
    let rows: Vec<(Animation, Life, u64)> = animations
        .iter()
        .map(|row| (row.key, row.life, row.facts))
        .collect();
    let expected = [
        (clip('5'), Life::new(0, 1), list.digest()),
        (clip('6'), Life::new(2, 2), list.digest()),
    ];
    assert_eq!(rows, expected);
}

#[test]
fn an_events_bank_is_its_header_and_the_records_of_its_objects() {
    let walked = walked(Families::ALL);
    let head = walked.head();
    // The sound changes with the clip after the first build, and the event stays.
    let objects = walked.folds.bank_objects.rows(head);
    let mut held: Vec<(u8, u32, &[u8], Life)> = objects
        .iter()
        .map(|row| {
            let object = &row.facts;
            assert_eq!(row.key, object.digest());
            (object.kind, object.id, object.body.as_slice(), row.life)
        })
        .collect();
    held.sort_unstable_by_key(|(kind, id, body, _)| (*kind, *id, *body));
    let expected: [(u8, u32, &[u8], Life); 3] = [
        (2, 1, &[1], Life::new(0, 0)),
        (2, 1, &[2], Life::new(1, 2)),
        (4, 0xe1, &[1, 1, 0, 0, 0], Life::new(0, 2)),
    ];
    assert_eq!(held, expected);

    let mut banks = walked.folds.event_banks.rows(head);
    banks.sort_by_key(|row| row.life.first);
    let lives: Vec<Life> = banks.iter().map(|row| row.life).collect();
    assert_eq!(lives, [Life::new(0, 0), Life::new(1, 2)]);
    let record = |kind, id, body: &[u8]| {
        let found = objects.iter().find(|row| {
            let object = &row.facts;
            (object.kind, object.id, object.body.as_slice()) == (kind, id, body)
        });
        found.unwrap().key
    };
    for (row, sound) in banks.iter().zip([1u8, 2]) {
        assert_eq!(row.key.path, bank());
        assert_eq!(row.facts.header[..], HEADER);
        let records: Vec<u64> = BankFacts::records(&row.facts.objects).collect();
        let stored = [record(2, 1, &[sound]), record(4, 0xe1, &[1, 1, 0, 0, 0])];
        assert_eq!(records, stored);
    }
    assert_ne!(banks[0].facts.record, banks[1].facts.record);
}

// #ADR-0081: every locale's WAD holds its own voice bank at one path.
#[test]
fn an_events_bank_is_a_row_per_file_and_one_for_a_file_two_wads_hold() {
    let in_wad = |wad: &str, text: String| {
        let path = bank().0;
        let file = format!(
            "files/data/final/champions/{wad}/{:02x}/{path:016x}.yaml",
            path >> 56
        );
        (file, text)
    };
    // Two locales' banks of the same objects in other bytes, and a third WAD with the first's.
    let tree = BTreeMap::from([
        in_wad("test.en_us.wad.client", events_bank('e', 1)),
        in_wad("test.fr_fr.wad.client", events_bank('f', 1)),
        in_wad("test2.en_us.wad.client", events_bank('e', 1)),
    ]);
    let history = Memory {
        commits: vec![(build("1.1.1"), tree)],
    };
    let walked = walk(&history, Families::ALL, &mut |_| {}).unwrap();
    let head = walked.head();
    let rows = walked.folds.event_banks.rows(head);
    let key = |digit: char| FileKey::parse(&digit.to_string().repeat(64)).unwrap();
    let held: Vec<(WadHash, FileKey, u32)> = rows
        .iter()
        .map(|row| (row.key.path, row.key.key, row.alive))
        .collect();
    assert_eq!(held, [(bank(), key('e'), 2), (bank(), key('f'), 1)]);
    assert_eq!(
        rows[0].facts, rows[1].facts,
        "one bank's header and objects"
    );
}

#[test]
fn families_filter() {
    let walked = walked(Families::parse("clips").unwrap());
    let folds = &walked.folds;
    assert!(folds.files.is_empty() && folds.objects.is_empty() && folds.refs.is_empty());
    assert!(folds.skeletons.is_empty() && folds.textures.is_empty());
    assert!(folds.joint_lists.is_empty() && folds.animations.is_empty());
    assert_eq!(folds.clips.len(), 2);
    assert_eq!((folds.bank_objects.len(), folds.event_banks.len()), (3, 2));

    let walked = self::walked(Families::parse("rigs").unwrap());
    let folds = &walked.folds;
    assert!(folds.bank_objects.is_empty() && folds.event_banks.is_empty());
    assert_eq!((folds.joint_lists.len(), folds.animations.len()), (1, 2));
}

#[test]
fn a_text_that_does_not_parse_names_nothing() {
    let mut history = memory();
    for (_, files) in &mut history.commits {
        for (path, text) in files.iter_mut() {
            if path.ends_with(".rito") {
                text.truncate(text.len() / 2);
            }
        }
    }
    let walked = walk(&history, Families::ALL, &mut |_| {}).unwrap();
    assert_eq!(walked.unparsed_texts, 1);
    assert!(walked.folds.refs.is_empty() && walked.folds.units.is_empty());
    assert_eq!(walked.folds.objects.len(), 1);
}

#[test]
fn a_walk_tells_each_build_as_it_is_folded() {
    let mut heard = Vec::new();
    walk(&memory(), Families::ALL, &mut |p| {
        heard.push((p.done, p.of, p.build.build.to_string()));
    })
    .unwrap();
    let expected = [(1, 3, "1.1.1"), (2, 3, "1.2.1"), (3, 3, "1.3.1")];
    let expected = expected.map(|(done, of, version)| (done, of, version.to_string()));
    assert_eq!(heard, expected);
}
