//! The guided view's questions (#ADR-0090): one for each finding of a kind the library decided
//! by a default, derived from a run's view so that what is asked is decided here and the front
//! end renders it. A question's choices come from the finding's data and the decision's
//! candidates, and each answer is a setting the library takes (#ADR-0098), never a rule of the
//! app's (#ADR-0046). The view's text is cleaned already, so a fact built from it is too.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::run::{CandidateView, DecisionView, KeptFileView, RunView, UnitTargetView};
use crate::view::{Finding, Severity};

/// The most files of a stale finding offered one by one; past it the two whole-mod choices
/// alone are offered.
const KEEP_EACH_BOUND: usize = 16;

/// What a question is about, which gives its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum QuestionKind {
    /// Files that are the game's older bytes (`file.stale`).
    StaleFiles,
    /// A unit the policy's default placed or left (`target.*`).
    UnitTarget,
    /// Units of one rank on one slot, each leaving it (`target.conflict`).
    SlotConflict,
    /// Units at slots the port does not change (`unit.left`).
    OtherSlots,
    /// Clips whose events are cut from the mod's own events bank (`audio.carried`).
    EventsBank,
    /// A mod made for several skins on purpose (`rework.restoration`).
    SeveralSkins,
    /// An error that refuses the write.
    BlockingError,
}

/// What a choice says, as the front end words it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChoiceLabel {
    /// The stale copies are not carried.
    DropStale,
    /// Every stale copy is carried.
    KeepAllStale,
    /// These stale copies are carried.
    KeepThese {
        /// The files.
        files: Vec<KeptFileView>,
    },
    /// The unit ships to this slot.
    Target {
        /// The slot.
        slot: u16,
    },
    /// The unit is left to the game.
    LeaveToGame,
    /// The unit stays at its own slot.
    KeepAtSlot {
        /// The slot.
        slot: u16,
    },
    /// This unit takes the slot, and the others of the conflict leave it.
    TakesSlot {
        /// The unit, as the view's names spell it.
        unit: String,
        /// The slot.
        slot: u16,
    },
    /// The units at other slots ship too, each at its own slot.
    KeepOtherSlots,
    /// The port changes its target slot only.
    ChangeTargetOnly,
    /// The events come from the game's bank only, and clips no event of it plays are not
    /// carried.
    CutFromGameBank,
    /// The events come from the mod's own events bank.
    KeepModBank,
    /// The mod ships as the several skins it is made for.
    AsSeveralSkins,
    /// The mod ships one skin: every other unit of the champion is left to the game.
    OneSkin,
    /// The write goes on with the error in the package.
    AllowError {
        /// The finding's code.
        code: String,
    },
    /// The error refuses the write.
    BlockError {
        /// The finding's code.
        code: String,
    },
}

/// The settings a choice gives, as a patch of the run's settings: the fields it sets, none for
/// the rest. The front end merges the patches in question order, a list appended to, a switch
/// set.
// No field is skipped when none: the bindings would then split the type in two, one for each
// direction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Answer {
    /// Files kept where the run drops them as stale copies.
    pub kept_files: Option<Vec<KeptFileView>>,
    /// Carry every stale copy.
    pub keep_stale: Option<bool>,
    /// Targets given to units alone.
    pub unit_targets: Option<Vec<UnitTargetView>>,
    /// Ship the units at slots other than the target too.
    pub keep_other_slots: Option<bool>,
    /// Cut events from the game's bank only.
    pub game_events_only: Option<bool>,
    /// Filters of the findings that refuse the write.
    pub block: Option<Vec<String>>,
}

/// One choice of a question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Choice {
    /// Its id within the question, the same across runs of the mod.
    pub id: String,
    /// What it says.
    pub label: ChoiceLabel,
    /// The settings it gives.
    pub answer: Answer,
}

/// One question of the guided view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Question {
    /// Its id, the same across runs of the mod: its kind and its subject, so that an answer
    /// is kept when a rerun asks again.
    pub id: String,
    /// What it is about.
    pub kind: QuestionKind,
    /// The finding it is asked from, by its id.
    pub finding: u32,
    /// The unit, character or WAD, as the view's names spell it.
    pub subject: String,
    /// The choices.
    pub choices: Vec<Choice>,
    /// The default's index among the choices: what the run did.
    pub chosen: usize,
    /// The mod's own facts the question names, the finding's message first.
    pub facts: Vec<String>,
}

/// The questions a run's view asks, in the order asked: the stale files first, since that
/// answer reruns `load` (#ADR-0099), the errors that refuse the write last. None for a run
/// that halted, and none for a finding whose data does not give its choices.
pub fn questions(view: &RunView) -> Vec<Question> {
    if view.halted.is_some() {
        return Vec::new();
    }
    let ask = Asker { view };
    let findings = &view.findings;
    let mut asked = Vec::new();
    asked.extend(findings.iter().filter_map(|f| ask.stale_files(f)));
    asked.extend(findings.iter().filter_map(|f| ask.unit_target(f)));
    asked.extend(findings.iter().filter_map(|f| ask.slot_conflict(f)));
    asked.extend(findings.iter().filter_map(|f| ask.other_slots(f)));
    asked.extend(findings.iter().filter_map(|f| ask.events_bank(f)));
    asked.extend(findings.iter().filter_map(|f| ask.several_skins(f)));
    asked.extend(findings.iter().filter_map(blocking_error));
    asked
}

/// The data of a finding of `code`, read as `T`. None for another code, or data that does not
/// read.
fn data_of<T: DeserializeOwned>(finding: &Finding, code: &str) -> Option<T> {
    (finding.code == code)
        .then(|| serde_json::from_value(finding.data.clone()).ok())
        .flatten()
}

#[derive(Deserialize)]
struct StaleData {
    wad: String,
    files: Vec<String>,
}

#[derive(Deserialize)]
struct UnitData {
    unit: String,
}

#[derive(Deserialize)]
struct ConflictData {
    character: String,
    slot: u16,
    units: Vec<String>,
}

#[derive(Deserialize)]
struct LeftData {
    character: String,
    units: Vec<LeftRow>,
}

#[derive(Deserialize)]
struct LeftRow {
    unit: String,
    why: String,
}

#[derive(Deserialize)]
struct WorkData {
    character: String,
    units: Vec<UnitWork>,
}

#[derive(Deserialize)]
struct UnitWork {
    unit: String,
    models: usize,
    skeletons: usize,
    textures: usize,
    files: usize,
    objects: usize,
    clips: usize,
}

#[derive(Deserialize)]
struct AudioData {
    unit: String,
    banks: Vec<BankData>,
}

#[derive(Deserialize)]
struct BankData {
    source: String,
    clips: usize,
    carry: CarryData,
}

#[derive(Deserialize)]
struct CarryData {
    shape: String,
    #[serde(default)]
    events: usize,
    #[serde(default)]
    from_mod: bool,
    #[serde(default)]
    shared: usize,
}

#[derive(Deserialize)]
struct RestorationData {
    wad: String,
    champion: String,
}

/// The codes of a unit's target the policy's default decided: #ADR-0016, #ADR-0019, and a
/// unit with no rig facts or a sub-character left to the game.
const UNIT_TARGET_CODES: [&str; 4] = [
    "target.infeasible",
    "target.unjudged",
    "target.sub-left",
    "target.pre-rework",
];

/// The questions' derivation from one view.
struct Asker<'v> {
    view: &'v RunView,
}

impl Asker<'_> {
    /// The name the view gives an id, else the id.
    fn name(&self, id: &str) -> String {
        self.view
            .names
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    }

    /// The finding's subject, as the view's names spell it.
    fn subject(&self, finding: &Finding) -> String {
        match self.view.names.get(&finding.subject.id) {
            Some(name) => name.clone(),
            None => finding.subject.name.clone(),
        }
    }

    /// The decision of a unit, by its id.
    fn decision(&self, unit: &str) -> Option<&DecisionView> {
        let targets = self.view.targets.as_ref()?;
        targets.decisions.iter().find(|d| d.unit == unit)
    }

    /// `file.stale`: drop, keep every stale copy, or keep one file (#ADR-0023, #ADR-0098).
    fn stale_files(&self, finding: &Finding) -> Option<Question> {
        let data: StaleData = data_of(finding, "file.stale")?;
        let wad = self.name(&data.wad);
        let paths: Vec<String> = data.files.iter().map(|id| self.name(id)).collect();
        let mut facts = vec![finding.message.clone()];
        facts.extend(paths.iter().take(KEEP_EACH_BOUND).cloned());
        if paths.len() > KEEP_EACH_BOUND {
            facts.push(format!("and {} more", paths.len() - KEEP_EACH_BOUND));
        }
        let mut choices = vec![
            Choice {
                id: "drop".to_string(),
                label: ChoiceLabel::DropStale,
                answer: Answer::default(),
            },
            Choice {
                id: "keep-all".to_string(),
                label: ChoiceLabel::KeepAllStale,
                answer: Answer {
                    keep_stale: Some(true),
                    ..Answer::default()
                },
            },
        ];
        if paths.len() <= KEEP_EACH_BOUND {
            choices.extend(paths.iter().map(|path| {
                let file = KeptFileView {
                    wad: wad.clone(),
                    path: path.clone(),
                };
                Choice {
                    id: format!("keep:{path}"),
                    label: ChoiceLabel::KeepThese {
                        files: vec![file.clone()],
                    },
                    answer: Answer {
                        kept_files: Some(vec![file]),
                        ..Answer::default()
                    },
                }
            }));
        }
        Some(Question {
            id: format!("stale-files:{wad}"),
            kind: QuestionKind::StaleFiles,
            finding: finding.id,
            subject: wad,
            choices,
            chosen: 0,
            facts,
        })
    }

    /// `target.infeasible`, `target.unjudged`, `target.sub-left`, `target.pre-rework`: each
    /// slot the unit fits, the slot the run kept it at, or the game's own skin. The run's own
    /// choice gives no setting: the policy's choice stands.
    fn unit_target(&self, finding: &Finding) -> Option<Question> {
        let code = UNIT_TARGET_CODES
            .into_iter()
            .find(|code| finding.code == *code)?;
        let data: UnitData = data_of(finding, code)?;
        let decision = self.decision(&data.unit)?;
        let mut facts = vec![finding.message.clone()];
        facts.extend(decision.candidates.iter().map(candidate_fact));

        let mut slots: Vec<u16> = Vec::new();
        for candidate in decision.candidates.iter().filter(|c| c.feasible) {
            if !slots.contains(&candidate.slot) {
                slots.push(candidate.slot);
            }
        }
        let mut labels: Vec<(String, ChoiceLabel, Option<u16>)> = slots
            .iter()
            .map(|&slot| {
                let label = ChoiceLabel::Target { slot };
                (format!("target:{slot}"), label, Some(slot))
            })
            .collect();
        if let Some(chosen) = decision.chosen.filter(|slot| !slots.contains(slot)) {
            labels.push(match code {
                "target.unjudged" => (
                    format!("keep-at:{chosen}"),
                    ChoiceLabel::KeepAtSlot { slot: chosen },
                    Some(chosen),
                ),
                _ => (
                    format!("target:{chosen}"),
                    ChoiceLabel::Target { slot: chosen },
                    Some(chosen),
                ),
            });
        }
        labels.push(("leave".to_string(), ChoiceLabel::LeaveToGame, None));

        let chosen = labels
            .iter()
            .position(|(_, _, target)| *target == decision.chosen)?;
        let choices = labels
            .into_iter()
            .enumerate()
            .map(|(at, (id, label, target))| {
                let answer = match at == chosen {
                    true => Answer::default(),
                    false => Answer {
                        unit_targets: Some(vec![given(decision, target)]),
                        ..Answer::default()
                    },
                };
                Choice { id, label, answer }
            });
        Some(Question {
            id: format!("unit-target:{}", data.unit),
            kind: QuestionKind::UnitTarget,
            finding: finding.id,
            subject: self.subject(finding),
            choices: choices.collect(),
            chosen,
            facts,
        })
    }

    /// `target.conflict`: one unit of the conflict takes the slot and the others leave it, or
    /// each leaves it as the run did (#ADR-0076).
    fn slot_conflict(&self, finding: &Finding) -> Option<Question> {
        let data: ConflictData = data_of(finding, "target.conflict")?;
        let slot = data.slot;
        let units: Vec<&DecisionView> = (data.units.iter())
            .filter_map(|unit| self.decision(unit))
            .collect();
        if units.is_empty() {
            return None;
        }
        let mut facts = vec![finding.message.clone()];
        facts.extend(units.iter().map(|d| self.name(&d.unit)));
        let mut choices: Vec<Choice> = units
            .iter()
            .map(|taker| {
                let targets = units.iter().map(|unit| {
                    let target = (unit.unit == taker.unit).then_some(slot);
                    given(unit, target)
                });
                Choice {
                    id: format!("takes:{}", taker.unit),
                    label: ChoiceLabel::TakesSlot {
                        unit: self.name(&taker.unit),
                        slot,
                    },
                    answer: Answer {
                        unit_targets: Some(targets.collect()),
                        ..Answer::default()
                    },
                }
            })
            .collect();
        choices.push(Choice {
            id: "leave".to_string(),
            label: ChoiceLabel::LeaveToGame,
            answer: Answer::default(),
        });
        Some(Question {
            id: format!("slot-conflict:{}:{slot}", data.character),
            kind: QuestionKind::SlotConflict,
            finding: finding.id,
            subject: self.name(&data.character),
            chosen: choices.len() - 1,
            choices,
            facts,
        })
    }

    /// `unit.left` with units at slots the port does not change (#ADR-0082): the target slot
    /// only, or every slot. The units' own work comes from the character's
    /// `unit.left-with-work`.
    fn other_slots(&self, finding: &Finding) -> Option<Question> {
        let data: LeftData = data_of(finding, "unit.left")?;
        let off: Vec<&LeftRow> = data
            .units
            .iter()
            .filter(|r| r.why == "off-target")
            .collect();
        if off.is_empty() {
            return None;
        }
        let work: Vec<UnitWork> = (self.view.findings.iter())
            .filter_map(|f| data_of::<WorkData>(f, "unit.left-with-work"))
            .filter(|w| w.character == data.character)
            .flat_map(|w| w.units)
            .collect();
        let mut facts = vec![finding.message.clone()];
        facts.extend(off.iter().map(|row| {
            let name = self.name(&row.unit);
            match work.iter().find(|w| w.unit == row.unit) {
                Some(work) => format!("{name}: {}", work_fact(work)),
                None => name,
            }
        }));
        let choices = vec![
            Choice {
                id: "target-only".to_string(),
                label: ChoiceLabel::ChangeTargetOnly,
                answer: Answer::default(),
            },
            Choice {
                id: "keep-other-slots".to_string(),
                label: ChoiceLabel::KeepOtherSlots,
                answer: Answer {
                    keep_other_slots: Some(true),
                    ..Answer::default()
                },
            },
        ];
        Some(Question {
            id: format!("other-slots:{}", data.character),
            kind: QuestionKind::OtherSlots,
            finding: finding.id,
            subject: self.name(&data.character),
            choices,
            chosen: 0,
            facts,
        })
    }

    /// `audio.carried` with banks whose events are cut from the mod's own events bank
    /// (#ADR-0030): that bank, as the run did, or the game's alone. The setting changes no
    /// bank whose events are the game's, an older bank's successor among them (#ADR-0084).
    fn events_bank(&self, finding: &Finding) -> Option<Question> {
        let data: AudioData = data_of(finding, "audio.carried")?;
        let own: Vec<&BankData> = (data.banks.iter())
            .filter(|bank| bank.carry.shape == "own" && bank.carry.from_mod)
            .collect();
        if own.is_empty() {
            return None;
        }
        let mut facts = vec![finding.message.clone()];
        facts.extend(own.iter().map(|bank| {
            let carry = &bank.carry;
            format!(
                "{}: {} own clips, {} events cut from the mod's own events bank, {} of them in the game's bank too",
                bank.source, bank.clips, carry.events, carry.shared
            )
        }));
        let choices = vec![
            Choice {
                id: "keep-mod-bank".to_string(),
                label: ChoiceLabel::KeepModBank,
                answer: Answer::default(),
            },
            Choice {
                id: "cut-from-game-bank".to_string(),
                label: ChoiceLabel::CutFromGameBank,
                answer: Answer {
                    game_events_only: Some(true),
                    ..Answer::default()
                },
            },
        ];
        Some(Question {
            id: format!("events-bank:{}", data.unit),
            kind: QuestionKind::EventsBank,
            finding: finding.id,
            subject: self.name(&data.unit),
            choices,
            chosen: 0,
            facts,
        })
    }

    /// `rework.restoration`, a mod made for several skins on purpose (#ADR-0086): as it is,
    /// or one skin, every unit of the champion but the first left to the game. None when the
    /// champion has one unit, which leaves nothing to choose.
    fn several_skins(&self, finding: &Finding) -> Option<Question> {
        let data: RestorationData = data_of(finding, "rework.restoration")?;
        let targets = self.view.targets.as_ref()?;
        let units: Vec<&DecisionView> = (targets.decisions.iter())
            .filter(|d| d.character_name.eq_ignore_ascii_case(&data.champion))
            .collect();
        let [_, rest @ ..] = units.as_slice() else {
            return None;
        };
        if rest.is_empty() {
            return None;
        }
        let mut facts = vec![finding.message.clone()];
        facts.extend(units.iter().map(|d| self.name(&d.unit)));
        let left = rest.iter().map(|unit| given(unit, None));
        let choices = vec![
            Choice {
                id: "several".to_string(),
                label: ChoiceLabel::AsSeveralSkins,
                answer: Answer::default(),
            },
            Choice {
                id: "one".to_string(),
                label: ChoiceLabel::OneSkin,
                answer: Answer {
                    unit_targets: Some(left.collect()),
                    ..Answer::default()
                },
            },
        ];
        Some(Question {
            id: format!("several-skins:{}", data.wad),
            kind: QuestionKind::SeveralSkins,
            finding: finding.id,
            subject: self.name(&data.wad),
            choices,
            chosen: 0,
            facts,
        })
    }
}

/// An error, which the simple view's write refuses: refuse the write, or write with what the
/// finding says is lost (#ADR-0003).
fn blocking_error(finding: &Finding) -> Option<Question> {
    if finding.severity != Severity::Error {
        return None;
    }
    let code = &finding.code;
    let subject = &finding.subject;
    // A table's id, the run or an input names the one finding; an archive's entry or a path
    // may hold a comma, which ends a filter's term, so the code alone stands for it.
    let filter = match subject.id.starts_with('%') || matches!(&*subject.kind, "run" | "input") {
        true => format!("{code},subject={}", subject.id),
        false => code.clone(),
    };
    let choices = vec![
        Choice {
            id: "block".to_string(),
            label: ChoiceLabel::BlockError { code: code.clone() },
            answer: Answer {
                block: Some(vec![filter]),
                ..Answer::default()
            },
        },
        Choice {
            id: "allow".to_string(),
            label: ChoiceLabel::AllowError { code: code.clone() },
            answer: Answer::default(),
        },
    ];
    Some(Question {
        id: format!("blocking-error:{code}:{}", subject.id),
        kind: QuestionKind::BlockingError,
        finding: finding.id,
        subject: subject.name.clone(),
        choices,
        chosen: 0,
        facts: vec![finding.message.clone()],
    })
}

/// A target given to the unit of a decision.
fn given(decision: &DecisionView, target: Option<u16>) -> UnitTargetView {
    UnitTargetView {
        character: decision.character_name.clone(),
        slot: decision.source,
        target,
    }
}

/// A candidate slot and its checks as one sentence.
fn candidate_fact(candidate: &CandidateView) -> String {
    let fits = match candidate.feasible {
        true => "fits",
        false => "does not fit",
    };
    let checks: Vec<String> = (candidate.checks.iter())
        .map(|check| format!("{} {}: {}", check.aspect, check.fit, check.detail))
        .collect();
    format!(
        "skin{} ({}) {fits}: {}",
        candidate.slot,
        candidate.why,
        checks.join("; ")
    )
}

/// What a unit that leaves holds of its own, its counts that are not zero.
fn work_fact(work: &UnitWork) -> String {
    let counts = [
        (work.models, "models"),
        (work.skeletons, "skeletons"),
        (work.textures, "textures"),
        (work.files, "other files"),
        (work.objects, "objects"),
        (work.clips, "clips"),
    ];
    let held: Vec<String> = (counts.iter())
        .filter(|(count, _)| *count > 0)
        .map(|(count, what)| format!("{count} {what}"))
        .collect();
    format!("own {}", held.join(", "))
}
