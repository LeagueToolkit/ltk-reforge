import { describe, expect, it } from "vitest";

import type {
  Answer,
  CandidateView,
  Choice,
  ChoiceLabel,
  Question,
  QuestionKind,
} from "@/lib/bindings.gen";

import { askAgain, mergeAnswers, placeCandidates, takenAnswers } from "./guided";
import { NOT_GUIDED, STANDARD } from "./repairStore";

const NONE: Answer = {
  kept_files: null,
  keep_stale: null,
  unit_targets: null,
  keep_other_slots: null,
  game_events_only: null,
  block: null,
};

function choice(id: string, answer: Partial<Answer> = {}): Choice {
  return { id, label: { kind: "leave-to-game" }, answer: { ...NONE, ...answer } };
}

function question(id: string, kind: QuestionKind, choices: Choice[], chosen = 0): Question {
  return {
    id,
    kind,
    finding: 0,
    subject: id,
    choices,
    chosen,
    facts: [],
    candidates: [],
    chosen_slot: null,
  };
}

function candidate(slot: number, feasible: boolean): CandidateView {
  return { slot, why: "origin", checks: [], feasible };
}

function labelled(id: string, label: ChoiceLabel): Choice {
  return { ...choice(id), label };
}

const stale = question("stale-files:a.wad.client", "stale-files", [
  choice("drop"),
  choice("keep-all", { keep_stale: true }),
  choice("keep:x.tex", { kept_files: [{ wad: "a.wad.client", path: "x.tex" }] }),
]);
const unit = question(
  "unit-target:%u0",
  "unit-target",
  [
    choice("target:3", { unit_targets: [{ character: "a", slot: 0, target: 3 }] }),
    choice("target:0"),
    choice("leave", { unit_targets: [{ character: "a", slot: 0, target: null }] }),
  ],
  1,
);
const other = question("other-slots:%c0", "other-slots", [
  choice("target-only"),
  choice("keep-other-slots", { keep_other_slots: true }),
]);
const error = question("blocking-error:target.infeasible:%u0", "blocking-error", [
  choice("block", { block: ["target.infeasible,subject=%u0"] }),
  choice("allow"),
]);
const otherError = question("blocking-error:ref.dangling:%o1", "blocking-error", [
  choice("block", { block: ["ref.dangling,subject=%o1"] }),
  choice("allow"),
]);

describe("the answers merged into the settings", () => {
  it("are the standard settings with each error blocked when every answer is the default", () => {
    const questions = [stale, unit, other, error, otherError];
    const settings = mergeAnswers(STANDARD, takenAnswers(questions, {}));
    expect(settings).toEqual({
      ...STANDARD,
      block: ["target.infeasible,subject=%u0", "ref.dangling,subject=%o1"],
    });
  });

  it("append the lists, set the switches, and block only the errors not allowed", () => {
    const questions = [stale, unit, other, error, otherError];
    const answers = {
      [stale.id]: "keep:x.tex",
      [unit.id]: "leave",
      [other.id]: "keep-other-slots",
      [error.id]: "allow",
    };
    const settings = mergeAnswers(STANDARD, takenAnswers(questions, answers));
    expect(settings.keptFiles).toEqual([{ wad: "a.wad.client", path: "x.tex" }]);
    expect(settings.unitTargets).toEqual([{ character: "a", slot: 0, target: null }]);
    expect(settings.keepOtherSlots).toBe(true);
    expect(settings.keepStale).toBe(false);
    expect(settings.block).toEqual(["ref.dangling,subject=%o1"]);
  });

  it("append to what the settings hold already", () => {
    const base = { ...STANDARD, unitTargets: [{ character: "b", slot: 2, target: null }] };
    const settings = mergeAnswers(base, takenAnswers([unit], { [unit.id]: "target:3" }));
    expect(settings.unitTargets).toEqual([
      { character: "b", slot: 2, target: null },
      { character: "a", slot: 0, target: 3 },
    ]);
  });

  it("take an answer whose choice the question lacks as the default", () => {
    const settings = mergeAnswers(STANDARD, takenAnswers([other], { [other.id]: "gone" }));
    expect(settings.keepOtherSlots).toBe(false);
  });
});

describe("a rerun's questions", () => {
  it("keep an earlier answer where the question's id and the choice's id match", () => {
    const earlier = {
      ...NOT_GUIDED,
      questions: [unit, other, error],
      answers: { [unit.id]: "target:3", [other.id]: "keep-other-slots", [error.id]: "allow" },
    };
    const fewerChoices = { ...unit, choices: unit.choices.slice(1), chosen: 0 };
    const again = askAgain(earlier, [fewerChoices, other, error]);
    expect(again.step).toBe("asking");
    expect(again.questions.map((q) => q.id)).toEqual([unit.id, other.id, error.id]);
    expect(again.answers).toEqual({ [other.id]: "keep-other-slots", [error.id]: "allow" });
  });

  it("keep a question no longer asked when its answer is not the default, in its kind's place", () => {
    const earlier = {
      ...NOT_GUIDED,
      questions: [stale, other, error],
      answers: { [stale.id]: "keep:x.tex" },
    };
    const again = askAgain(earlier, [error, otherError]);
    expect(again.questions.map((q) => q.id)).toEqual([stale.id, error.id, otherError.id]);
    expect(again.answers).toEqual({ [stale.id]: "keep:x.tex" });
  });

  it("drop a question no longer asked that was left at its default", () => {
    const earlier = { ...NOT_GUIDED, questions: [stale, other], answers: {} };
    expect(askAgain(earlier, []).questions).toEqual([]);
  });
});

describe("a question's candidates", () => {
  const fits = candidate(3, true);
  const unfit = candidate(0, false);
  const elsewhere = candidate(5, false);

  it("sit beside the unit's choice of their slot, those no choice targets apart", () => {
    const asked = {
      ...question("unit-target:%u0", "unit-target", [
        labelled("target:3", { kind: "target", slot: 3 }),
        labelled("keep-at:0", { kind: "keep-at-slot", slot: 0 }),
        labelled("leave", { kind: "leave-to-game" }),
      ]),
      candidates: [unfit, fits, elsewhere],
    };
    const placed = placeCandidates(asked);
    expect([...placed.byChoice]).toEqual([
      ["target:3", fits],
      ["keep-at:0", unfit],
    ]);
    expect(placed.apart).toEqual([elsewhere]);
  });

  it("sit beside a conflict's choices in order, one per unit given the slot", () => {
    const first = candidate(5, true);
    const second = candidate(5, false);
    const choices = [
      labelled("takes:%u2", { kind: "takes-slot", unit: "a skin6", slot: 5 }),
      labelled("takes:%u3", { kind: "takes-slot", unit: "a skin7", slot: 5 }),
      labelled("leave", { kind: "leave-to-game" }),
    ];
    const conflict = question("slot-conflict:%c1:5", "slot-conflict", choices, 2);
    const placed = placeCandidates({ ...conflict, candidates: [first, second] });
    expect([...placed.byChoice]).toEqual([
      ["takes:%u2", first],
      ["takes:%u3", second],
    ]);
    expect(placed.apart).toEqual([]);
    expect(placeCandidates(conflict).byChoice.size).toBe(0);
  });
});
