import type { Answer, CandidateView, Choice, Question, QuestionKind } from "@/lib/bindings.gen";

import type { AdvancedSettings, GuidedAnswers, GuidedState } from "./repairStore";

/** The order the backend asks the kinds in, the stale files first and the errors last. */
const KIND_ORDER: QuestionKind[] = [
  "stale-files",
  "unit-target",
  "slot-conflict",
  "other-slots",
  "events-bank",
  "several-skins",
  "blocking-error",
];

/** The choice taken for a question: the one answered, else its default, what the run did. */
export function takenChoice(question: Question, answers: GuidedAnswers): Choice {
  const answered = question.choices.find((choice) => choice.id === answers[question.id]);
  return answered ?? question.choices[question.chosen];
}

/** A question's candidates placed: beside the choice that targets each, or apart. */
export interface PlacedCandidates {
  /** The candidate each choice targets, by the choice's id. */
  byChoice: Map<string, CandidateView>;
  /** The candidates no choice targets: slots weighed that the unit does not fit. */
  apart: CandidateView[];
}

/**
 * Each candidate of a question beside the choice that targets it. A unit's choice targets the
 * candidate of its slot; a conflict's choices that give the slot to a unit take the
 * candidates in order, which the backend lists one per such choice or not at all.
 */
export function placeCandidates(question: Question): PlacedCandidates {
  const byChoice = new Map<string, CandidateView>();
  const { candidates } = question;
  if (question.kind === "slot-conflict") {
    const takers = question.choices.filter((choice) => choice.label.kind === "takes-slot");
    if (takers.length === candidates.length) {
      takers.forEach((choice, at) => byChoice.set(choice.id, candidates[at]));
    }
    return { byChoice, apart: [] };
  }
  for (const choice of question.choices) {
    const { label } = choice;
    if (label.kind === "target" || label.kind === "keep-at-slot") {
      const targeted = candidates.find((candidate) => candidate.slot === label.slot);
      if (targeted) {
        byChoice.set(choice.id, targeted);
      }
    }
  }
  const placed = new Set(byChoice.values());
  return { byChoice, apart: candidates.filter((candidate) => !placed.has(candidate)) };
}

/** Whether a question is answered other than what the run did. */
function changed(question: Question, answers: GuidedAnswers): boolean {
  return takenChoice(question, answers).id !== question.choices[question.chosen].id;
}

/** The answers of every question, in question order. */
export function takenAnswers(questions: Question[], answers: GuidedAnswers): Answer[] {
  return questions.map((question) => takenChoice(question, answers).answer);
}

/**
 * The settings with the answers merged in, in order: a list appended to, a switch set where an
 * answer sets it. The block list is built from the errors' answers alone.
 */
export function mergeAnswers(base: AdvancedSettings, answers: Answer[]): AdvancedSettings {
  return answers.reduce<AdvancedSettings>(
    (settings, answer) => ({
      ...settings,
      keptFiles: [...settings.keptFiles, ...(answer.kept_files ?? [])],
      unitTargets: [...settings.unitTargets, ...(answer.unit_targets ?? [])],
      block: [...settings.block, ...(answer.block ?? [])],
      keepStale: answer.keep_stale ?? settings.keepStale,
      keepOtherSlots: answer.keep_other_slots ?? settings.keepOtherSlots,
      gameEventsOnly: answer.game_events_only ?? settings.gameEventsOnly,
    }),
    base,
  );
}

/**
 * The questions a rerun asks, with the earlier answers kept where a question's id and the
 * choice's id match. A question the rerun no longer asks, answered other than its default,
 * stays with its answer: its answer is what made its finding go, and dropping it would undo
 * it.
 */
export function askAgain(earlier: GuidedState, fresh: Question[]): GuidedState {
  const freshIds = new Set(fresh.map((question) => question.id));
  const carried = earlier.questions.filter(
    (question) => !freshIds.has(question.id) && changed(question, earlier.answers),
  );
  const questions = [...fresh, ...carried]
    .map((question, at) => ({ question, at }))
    .sort(
      (a, b) =>
        KIND_ORDER.indexOf(a.question.kind) - KIND_ORDER.indexOf(b.question.kind) || a.at - b.at,
    )
    .map(({ question }) => question);
  const answers: GuidedAnswers = {};
  for (const question of questions) {
    const answer = earlier.answers[question.id];
    if (answer != null && question.choices.some((choice) => choice.id === answer)) {
      answers[question.id] = answer;
    }
  }
  return { step: "asking", questions, answers };
}
