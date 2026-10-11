//! The guided view's questions (#ADR-0090), from views built here: one per finding the library
//! decided by a default, in the order asked, each with the run's own choice chosen and every
//! choice a setting the library takes.

use std::collections::BTreeMap;

use reforge_app::guided::{self, Answer, ChoiceLabel, Question, QuestionKind};
use reforge_app::run::{
    CandidateView, CheckView, DecisionView, KeptFileView, OriginView, PolicyView, RunView,
    TargetsView, UnitTargetView,
};
use reforge_app::view::{Finding, Severity, Subject};
use serde_json::{Value, json};

fn finding(id: u32, code: &str, severity: Severity, subject: (&str, &str), data: Value) -> Finding {
    let (kind, subject_id) = subject;
    Finding {
        id,
        code: code.to_string(),
        severity,
        channel: "decision".to_string(),
        stage: "decide".to_string(),
        subject: Subject {
            kind: kind.to_string(),
            id: subject_id.to_string(),
            name: format!("the {kind} {subject_id}"),
        },
        by: None,
        record: None,
        message: format!("the message of {code}"),
        data,
    }
}

fn candidate(slot: u16, feasible: bool) -> CandidateView {
    let fit = if feasible { "fits" } else { "NO" };
    CandidateView {
        slot,
        why: "origin".to_string(),
        checks: vec![CheckView {
            aspect: "mesh".to_string(),
            fit: fit.to_string(),
            detail: "the mesh".to_string(),
        }],
        feasible,
    }
}

fn decision(
    unit: &str,
    character: (&str, &str),
    source: u16,
    chosen: Option<u16>,
    candidates: Vec<CandidateView>,
) -> DecisionView {
    DecisionView {
        unit: unit.to_string(),
        character: character.0.to_string(),
        character_name: character.1.to_string(),
        source,
        chosen,
        reason: "base".to_string(),
        origin: OriginView {
            verdict: "custom".to_string(),
            slot: None,
        },
        candidates,
        alternatives: Vec::new(),
    }
}

fn view(findings: Vec<Finding>, decisions: Vec<DecisionView>) -> RunView {
    let names: BTreeMap<String, String> = [
        ("%w0", "Test.wad.client"),
        ("%f1", "assets/characters/test/skins/base/test.tex"),
        ("%f2", "assets/characters/test/skins/base/test_2.tex"),
        ("%c0", "test"),
        ("%c1", "testpet"),
        ("%u0", "test skin0"),
        ("%u1", "test skin2"),
        ("%u2", "testpet skin6"),
        ("%u3", "testpet skin7"),
        ("%u4", "test skin3"),
    ]
    .into_iter()
    .map(|(id, name)| (id.to_string(), name.to_string()))
    .collect();
    RunView {
        stages: Vec::new(),
        meta: None,
        halted: None,
        findings,
        changes: Vec::new(),
        objects: Vec::new(),
        targets: Some(TargetsView {
            policy: PolicyView {
                target: "auto".to_string(),
                anim_reuse: "own-rig".to_string(),
            },
            decisions,
        }),
        package: None,
        written: None,
        saved: None,
        refused: false,
        fault: None,
        cancelled: None,
        millis: 0,
        names,
    }
}

const TEST: (&str, &str) = ("%c0", "test");
const PET: (&str, &str) = ("%c1", "testpet");

/// A port's view with one finding of each code the guided view asks about.
fn every_kind() -> RunView {
    use Severity::*;
    let findings = vec![
        finding(
            0,
            "history.absent",
            Warning,
            ("input", "history"),
            json!(null),
        ),
        finding(
            1,
            "file.stale",
            Note,
            ("wad", "%w0"),
            json!({ "wad": "%w0", "files": ["%f1", "%f2"], "bytes": 10 }),
        ),
        finding(
            2,
            "target.infeasible",
            Error,
            ("unit", "%u0"),
            json!({ "unit": "%u0", "origin": null }),
        ),
        finding(
            3,
            "target.unjudged",
            Warning,
            ("unit", "%u1"),
            json!({ "unit": "%u1", "slot": 2 }),
        ),
        finding(
            4,
            "target.conflict",
            Warning,
            ("character", "%c1"),
            json!({ "character": "%c1", "slot": 5, "units": ["%u2", "%u3"] }),
        ),
        finding(
            5,
            "unit.left",
            Note,
            ("character", "%c0"),
            json!({ "character": "%c0", "units": [
                { "unit": "%u4", "why": "off-target", "target": "%u0" },
            ] }),
        ),
        finding(
            6,
            "unit.left-with-work",
            Warning,
            ("character", "%c0"),
            json!({ "character": "%c0", "target": "%u0", "units": [
                { "unit": "%u4", "slot": 3, "kind": "shipped-by-option", "models": 2,
                  "skeletons": 0, "textures": 1, "files": 0, "objects": 0, "clips": 0 },
            ] }),
        ),
        finding(
            7,
            "audio.carried",
            Note,
            ("unit", "%u0"),
            json!({ "unit": "%u0", "once": [], "banks": [
                { "source": "test_base_sfx", "name": "mod_sfx", "clips": 4, "layer": "base",
                  "replaces": null,
                  "carry": { "shape": "own", "tag": "mod", "events": 3, "from_mod": true,
                             "shared": 1, "silent": 0, "unnamed": [] } },
            ] }),
        ),
        finding(
            8,
            "rework.restoration",
            Note,
            ("wad", "%w0"),
            json!({ "wad": "%w0", "champion": "test", "patch": "16.19", "older": 3, "newer": 1 }),
        ),
    ];
    let decisions = vec![
        decision(
            "%u0",
            TEST,
            0,
            Some(0),
            vec![candidate(0, false), candidate(3, true)],
        ),
        decision("%u1", TEST, 2, Some(2), Vec::new()),
        decision("%u2", PET, 6, None, vec![candidate(5, true)]),
        decision("%u3", PET, 7, None, vec![candidate(5, true)]),
        decision("%u4", TEST, 3, Some(0), vec![candidate(0, true)]),
    ];
    view(findings, decisions)
}

fn ids(questions: &[Question]) -> Vec<&str> {
    questions.iter().map(|q| q.id.as_str()).collect()
}

fn question<'q>(questions: &'q [Question], id: &str) -> &'q Question {
    let found = questions.iter().find(|q| q.id == id);
    found.unwrap_or_else(|| panic!("no question {id} in {:?}", ids(questions)))
}

fn answer_of(question: &Question, choice: &str) -> Answer {
    let found = question.choices.iter().find(|c| c.id == choice);
    let found = found.unwrap_or_else(|| panic!("no choice {choice} in {}", question.id));
    found.answer.clone()
}

fn targets(answer: &Answer) -> Vec<UnitTargetView> {
    answer.unit_targets.clone().unwrap_or_default()
}

fn given(character: &str, slot: u16, target: Option<u16>) -> UnitTargetView {
    UnitTargetView {
        character: character.to_string(),
        slot,
        target,
    }
}

#[test]
fn each_finding_decided_by_a_default_is_asked_in_order() {
    let questions = guided::questions(&every_kind());

    assert_eq!(
        ids(&questions),
        [
            "stale-files:Test.wad.client",
            "unit-target:%u0",
            "unit-target:%u1",
            "slot-conflict:%c1:5",
            "other-slots:%c0",
            "events-bank:%u0",
            "several-skins:%w0",
            "blocking-error:target.infeasible:%u0",
        ]
    );
    let kinds: Vec<QuestionKind> = questions.iter().map(|q| q.kind).collect();
    use QuestionKind::*;
    assert_eq!(
        kinds,
        [
            StaleFiles,
            UnitTarget,
            UnitTarget,
            SlotConflict,
            OtherSlots,
            EventsBank,
            SeveralSkins,
            BlockingError
        ]
    );
    let findings: Vec<u32> = questions.iter().map(|q| q.finding).collect();
    assert_eq!(findings, [1, 2, 3, 4, 5, 7, 8, 2]);
    for question in &questions {
        assert_eq!(
            question.facts.first(),
            Some(&format!(
                "the message of {}",
                every_kind().findings[question.finding as usize].code
            )),
            "{}",
            question.id
        );
    }
}

#[test]
fn the_run_s_own_choice_is_chosen_and_gives_no_setting_but_the_block() {
    let questions = guided::questions(&every_kind());

    for question in &questions {
        let chosen = &question.choices[question.chosen];
        let block = match question.kind {
            QuestionKind::BlockingError => Some(vec!["target.infeasible,subject=%u0".to_string()]),
            _ => None,
        };
        assert_eq!(
            chosen.answer,
            Answer {
                block,
                ..Answer::default()
            },
            "{}",
            question.id
        );
    }
    let chosen = |id: &str| {
        let q = question(&questions, id);
        q.choices[q.chosen].id.clone()
    };
    assert_eq!(chosen("stale-files:Test.wad.client"), "drop");
    assert_eq!(chosen("unit-target:%u0"), "target:0");
    assert_eq!(chosen("unit-target:%u1"), "keep-at:2");
    assert_eq!(chosen("slot-conflict:%c1:5"), "leave");
    assert_eq!(chosen("other-slots:%c0"), "target-only");
    assert_eq!(chosen("events-bank:%u0"), "keep-mod-bank");
    assert_eq!(chosen("several-skins:%w0"), "several");
    assert_eq!(chosen("blocking-error:target.infeasible:%u0"), "block");
}

#[test]
fn a_stale_question_keeps_every_file_or_one_by_its_wad_and_path() {
    let questions = guided::questions(&every_kind());
    let stale = question(&questions, "stale-files:Test.wad.client");

    let choices: Vec<&str> = stale.choices.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        choices,
        [
            "drop",
            "keep-all",
            "keep:assets/characters/test/skins/base/test.tex",
            "keep:assets/characters/test/skins/base/test_2.tex",
        ]
    );
    assert_eq!(answer_of(stale, "keep-all").keep_stale, Some(true));
    let file = KeptFileView {
        wad: "Test.wad.client".to_string(),
        path: "assets/characters/test/skins/base/test.tex".to_string(),
    };
    let one = answer_of(stale, "keep:assets/characters/test/skins/base/test.tex");
    assert_eq!(one.kept_files, Some(vec![file.clone()]));
    assert_eq!(
        stale.choices[2].label,
        ChoiceLabel::KeepThese { files: vec![file] }
    );
    assert!(
        stale
            .facts
            .contains(&"assets/characters/test/skins/base/test_2.tex".to_string())
    );
}

#[test]
fn a_stale_question_of_many_files_offers_the_whole_mod_choices_alone() {
    let files: Vec<String> = (0..17).map(|n| format!("%f{n}")).collect();
    let stale = finding(
        0,
        "file.stale",
        Severity::Note,
        ("wad", "%w0"),
        json!({ "wad": "%w0", "files": files, "bytes": 10 }),
    );

    let questions = guided::questions(&view(vec![stale], Vec::new()));

    let choices: Vec<&str> = questions[0].choices.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(choices, ["drop", "keep-all"]);
    assert_eq!(
        questions[0].facts.last().map(String::as_str),
        Some("and 1 more")
    );
}

#[test]
fn a_unit_question_offers_each_slot_it_fits_and_the_game_s_skin() {
    let questions = guided::questions(&every_kind());
    let unit = question(&questions, "unit-target:%u0");

    let choices: Vec<(&str, &ChoiceLabel)> = unit
        .choices
        .iter()
        .map(|c| (c.id.as_str(), &c.label))
        .collect();
    assert_eq!(
        choices,
        [
            ("target:3", &ChoiceLabel::Target { slot: 3 }),
            ("target:0", &ChoiceLabel::Target { slot: 0 }),
            ("leave", &ChoiceLabel::LeaveToGame),
        ]
    );
    assert_eq!(
        targets(&answer_of(unit, "target:3")),
        [given("test", 0, Some(3))]
    );
    assert_eq!(targets(&answer_of(unit, "leave")), [given("test", 0, None)]);
    // The candidates are the decision's, the slot it does not fit among them, and no fact.
    assert_eq!(unit.candidates, [candidate(0, false), candidate(3, true)]);
    assert_eq!(unit.chosen_slot, Some(0));
    assert_eq!(unit.facts, ["the message of target.infeasible"]);
    let unjudged = question(&questions, "unit-target:%u1");
    assert_eq!(unjudged.candidates, []);
    assert_eq!(unjudged.chosen_slot, Some(2));
    assert_eq!(
        unjudged.choices[0].label,
        ChoiceLabel::KeepAtSlot { slot: 2 }
    );
    assert_eq!(
        targets(&answer_of(unjudged, "leave")),
        [given("test", 2, None)]
    );
}

#[test]
fn a_conflict_question_gives_the_slot_to_one_unit_and_leaves_the_others() {
    let questions = guided::questions(&every_kind());
    let conflict = question(&questions, "slot-conflict:%c1:5");

    assert_eq!(conflict.subject, "testpet");
    assert_eq!(
        conflict.choices[0].label,
        ChoiceLabel::TakesSlot {
            unit: "testpet skin6".to_string(),
            slot: 5
        }
    );
    assert_eq!(
        targets(&answer_of(conflict, "takes:%u3")),
        [given("testpet", 6, None), given("testpet", 7, Some(5))]
    );
    // Each unit's candidate at the slot, in the order of its choice.
    assert_eq!(
        conflict.candidates,
        [candidate(5, true), candidate(5, true)]
    );
    assert_eq!(conflict.chosen_slot, None);
}

#[test]
fn a_conflict_question_lists_no_candidate_when_a_unit_weighed_none_at_the_slot() {
    let mut run = every_kind();
    let targets = run.targets.as_mut().unwrap();
    targets.decisions[3].candidates = vec![candidate(7, true)];

    let questions = guided::questions(&run);

    let conflict = question(&questions, "slot-conflict:%c1:5");
    assert_eq!(conflict.candidates, []);
    assert_eq!(conflict.choices.len(), 3);
}

#[test]
fn only_a_unit_question_and_a_conflict_list_candidates() {
    let questions = guided::questions(&every_kind());

    for question in &questions {
        let unit = matches!(
            question.kind,
            QuestionKind::UnitTarget | QuestionKind::SlotConflict
        );
        if !unit {
            assert_eq!(question.candidates, [], "{}", question.id);
            assert_eq!(question.chosen_slot, None, "{}", question.id);
        }
    }
}

#[test]
fn the_other_questions_map_to_their_switches_and_targets() {
    let questions = guided::questions(&every_kind());

    let other = question(&questions, "other-slots:%c0");
    assert_eq!(
        answer_of(other, "keep-other-slots").keep_other_slots,
        Some(true)
    );
    assert!(
        other
            .facts
            .contains(&"test skin3: own 2 models, 1 texture".to_string()),
        "{:?}",
        other.facts
    );
    let bank = question(&questions, "events-bank:%u0");
    assert_eq!(
        answer_of(bank, "cut-from-game-bank").game_events_only,
        Some(true)
    );
    let skins = question(&questions, "several-skins:%w0");
    assert_eq!(
        targets(&answer_of(skins, "one")),
        [given("test", 2, None), given("test", 3, None)]
    );
    let error = question(&questions, "blocking-error:target.infeasible:%u0");
    assert_eq!(answer_of(error, "allow"), Answer::default());
    assert_eq!(
        error.choices[1].label,
        ChoiceLabel::AllowError {
            code: "target.infeasible".to_string()
        }
    );
}

#[test]
fn an_answer_writes_the_fields_it_does_not_set_as_none() {
    let questions = guided::questions(&every_kind());
    let stale = question(&questions, "stale-files:Test.wad.client");

    let written = serde_json::to_value(answer_of(stale, "keep-all")).unwrap();

    assert_eq!(
        written,
        json!({
            "kept_files": null,
            "keep_stale": true,
            "unit_targets": null,
            "keep_other_slots": null,
            "game_events_only": null,
            "block": null,
        })
    );
    let read: Answer = serde_json::from_value(json!({})).unwrap();
    assert_eq!(read, Answer::default());
}

#[test]
fn a_view_with_no_finding_decided_by_a_default_asks_nothing() {
    let quiet = view(
        vec![finding(
            0,
            "history.absent",
            Severity::Warning,
            ("input", "history"),
            json!(null),
        )],
        vec![decision("%u0", TEST, 0, Some(0), vec![candidate(0, true)])],
    );
    assert_eq!(guided::questions(&quiet), []);

    // A unit.left of copies alone, and an events bank of the game's, ask nothing either.
    let left = finding(
        0,
        "unit.left",
        Severity::Note,
        ("character", "%c0"),
        json!({ "character": "%c0", "units": [{ "unit": "%u4", "why": "copy", "of": "%u0" }] }),
    );
    let carried = finding(
        1,
        "audio.carried",
        Severity::Note,
        ("unit", "%u0"),
        json!({ "unit": "%u0", "once": [], "banks": [
            { "source": "a", "name": "b", "clips": 1, "layer": "base", "replaces": null,
              "carry": { "shape": "overlay" } },
        ] }),
    );
    assert_eq!(
        guided::questions(&view(vec![left, carried], Vec::new())),
        []
    );

    let mut halted = every_kind();
    halted.halted = Some(0);
    assert_eq!(guided::questions(&halted), []);
}
