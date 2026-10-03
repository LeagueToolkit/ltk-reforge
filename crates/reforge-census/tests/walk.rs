//! The walk over a history in memory: lives are hulls, families filter, texts that do not parse
//! are counted.

mod common;

use reforge::history::tables::{BankEvent, Naming, UnitEvent};
use reforge::history::{Families, Life};
use reforge::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, path_hash};
use reforge_census::{Walked, walk};

use self::common::{A, B, NAMED, bank, memory};

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
    assert_eq!(objects[0].key, ObjectKey::parse(A).unwrap());
    assert_eq!(objects[0].life, Life::new(0, 2));
    assert_eq!(objects[0].alive, 1);
    let facts = objects[0].facts;
    assert_eq!((facts.bin, facts.norm), (WadHash(2), NormKey(7)));
    assert_eq!((facts.entry, facts.class), (BinHash(1), BinHash(2)));
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
fn families_filter() {
    let walked = walked(Families::parse("clips").unwrap());
    let folds = &walked.folds;
    assert!(folds.files.is_empty() && folds.objects.is_empty() && folds.refs.is_empty());
    assert!(folds.skeletons.is_empty() && folds.textures.is_empty());
    assert_eq!(folds.clips.len(), 2);
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
