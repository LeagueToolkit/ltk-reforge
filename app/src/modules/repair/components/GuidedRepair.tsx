import { ArrowCounterClockwiseIcon, CompassIcon, PlayIcon } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { match } from "ts-pattern";

import { Button, FailureNote, SectionCard } from "@/components";
import { m } from "@/i18n";
import {
  type ChoiceLabel,
  commands,
  type Question,
  type QuestionKind,
  type RunRequest,
  type RunView,
  type StageName,
} from "@/lib/bindings.gen";
import { useInputs, useInputsGate } from "@/modules/inputs";
import { mutationFn } from "@/utils";

import { useJobRunning, useRunMod, useRunRunning, useRunStages } from "../api/run";
import { askAgain, mergeAnswers, takenAnswers, takenChoice } from "../state/guided";
import {
  type AdvancedSettings,
  type GuidedAnswers,
  NOT_GUIDED,
  runSettings,
  STANDARD,
  useRepairStore,
} from "../state/repairStore";
import { LiveProgress } from "./LiveProgress";
import { useOutputFolder } from "./ModCard";
import { resultsOf, RunResults } from "./RunResults";
import { RunSummary } from "./RunSummary";
import { StopButton } from "./StopButton";

const kindTitle: Record<QuestionKind, () => string> = {
  "stale-files": m.guided_kind_stale_files_title,
  "unit-target": m.guided_kind_unit_target_title,
  "slot-conflict": m.guided_kind_slot_conflict_title,
  "other-slots": m.guided_kind_other_slots_title,
  "events-bank": m.guided_kind_events_bank_title,
  "several-skins": m.guided_kind_several_skins_title,
  "blocking-error": m.guided_kind_blocking_error_title,
};

/** A choice's words, from its label. */
function choiceText(label: ChoiceLabel): string {
  return match(label)
    .with({ kind: "drop-stale" }, () => m.guided_choice_drop_stale())
    .with({ kind: "keep-all-stale" }, () => m.guided_choice_keep_all_stale())
    .with({ kind: "keep-these" }, ({ files }) =>
      m.guided_choice_keep_these({ path: files.map((file) => file.path).join(", ") }),
    )
    .with({ kind: "target" }, ({ slot }) => m.guided_choice_target({ slot }))
    .with({ kind: "leave-to-game" }, () => m.guided_choice_leave_to_game())
    .with({ kind: "keep-at-slot" }, ({ slot }) => m.guided_choice_keep_at_slot({ slot }))
    .with({ kind: "takes-slot" }, ({ unit, slot }) => m.guided_choice_takes_slot({ unit, slot }))
    .with({ kind: "keep-other-slots" }, () => m.guided_choice_keep_other_slots())
    .with({ kind: "change-target-only" }, () => m.guided_choice_change_target_only())
    .with({ kind: "cut-from-game-bank" }, () => m.guided_choice_cut_from_game_bank())
    .with({ kind: "keep-mod-bank" }, () => m.guided_choice_keep_mod_bank())
    .with({ kind: "as-several-skins" }, () => m.guided_choice_as_several_skins())
    .with({ kind: "one-skin" }, () => m.guided_choice_one_skin())
    .with({ kind: "allow-error" }, ({ code }) => m.guided_choice_allow_error({ code }))
    .with({ kind: "block-error" }, ({ code }) => m.guided_choice_block_error({ code }))
    .exhaustive();
}

/** One question: its title, its subject, the mod's facts it names, and its choices. */
function QuestionCard({
  question,
  answers,
  onAnswer,
}: {
  question: Question;
  answers: GuidedAnswers;
  onAnswer: (choice: string) => void;
}) {
  const taken = takenChoice(question, answers).id;
  return (
    <SectionCard title={kindTitle[question.kind]()} description={question.subject}>
      {question.facts.length > 0 && (
        <ul
          className="flex list-disc flex-col gap-0.5 pl-5 text-xs break-words text-surface-300"
          aria-label={m.guided_facts_label()}
        >
          {question.facts.map((fact, at) => (
            <li key={at}>{fact}</li>
          ))}
        </ul>
      )}
      <fieldset className="flex flex-col gap-1.5">
        <legend className="sr-only">{m.guided_choices_label()}</legend>
        {question.choices.map((choice, at) => (
          <label
            key={choice.id}
            className="flex cursor-pointer items-start gap-2 text-sm text-surface-100 select-none"
          >
            <input
              type="radio"
              name={question.id}
              value={choice.id}
              checked={taken === choice.id}
              onChange={() => onAnswer(choice.id)}
              className="mt-1 accent-accent-500"
            />
            <span className="min-w-0 break-words">
              {choiceText(choice.label)}
              {at === question.chosen && (
                <span className="ml-1.5 text-xs text-surface-400">{m.guided_default_label()}</span>
              )}
            </span>
          </label>
        ))}
      </fieldset>
    </SectionCard>
  );
}

/**
 * The guided view (#ADR-0090): a port with the standard settings as far as the plan, then a
 * question for each choice the run made by a default, and a run from the answers through the
 * write, which resumes from the plan's snapshot or the load's (#ADR-0094). A run with nothing
 * to ask is written as the simple view's is. The questions and their settings are the
 * backend's (`guided_questions`); the view merges the answers into the standard settings.
 */
export function GuidedRepair() {
  const archive = useRepairStore((state) => state.archive);
  const last = useRepairStore((state) => state.last);
  const live = useRepairStore((state) => state.live);
  const guided = useRepairStore((state) => state.guided);
  const setGuided = useRepairStore((state) => state.setGuided);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const out = useOutputFolder();
  const inputs = useInputs();
  const run = useRunMod();
  const running = useRunRunning();
  // A batch under way holds the run job.
  const busy = useJobRunning();
  const { gate, dialog } = useInputsGate();
  const ask = useMutation({ mutationFn: mutationFn(commands.guidedQuestions) });
  const stages = useRunStages(STANDARD.kind);

  const request = (settings: AdvancedSettings, through: StageName): RunRequest | null => {
    if (!archive || (through === "write" && !out)) {
      return null;
    }
    const kind = STANDARD.kind;
    return { kind, archive, through, settings: runSettings(settings), out };
  };

  /** Runs from these settings through the write, and shows how it ended. */
  const write = (settings: AdvancedSettings, questions: Question[], answers: GuidedAnswers) => {
    const asked = request(settings, "write");
    if (!asked) {
      return;
    }
    run.mutate(asked, {
      onSuccess: () => {
        // The advanced view shows what the answers set, which has no editor there.
        const { unitTargets, keptFiles, keepOtherSlots } = settings;
        setAdvanced({ unitTargets, keptFiles, keepOtherSlots });
        setGuided({ step: "done", questions, answers });
      },
    });
  };

  /** The questions a run through the plan asks; with none, the run goes on to the write. */
  const askOf = (view: RunView) => {
    if (view.halted != null || view.fault || view.cancelled) {
      return;
    }
    ask.mutate(view, {
      onSuccess: (questions) => {
        if (questions.length === 0) {
          write(STANDARD, [], {});
        } else {
          setGuided({ step: "asking", questions, answers: {} });
        }
      },
    });
  };

  const guide = () => {
    const asked = request(STANDARD, "plan");
    if (!asked) {
      return;
    }
    setGuided(NOT_GUIDED);
    gate(() => run.mutate(asked, { onSuccess: askOf }));
  };

  const runAnswers = () => {
    const answers = takenAnswers(guided.questions, guided.answers);
    const settings = mergeAnswers(STANDARD, answers);
    gate(() => write(settings, guided.questions, guided.answers));
  };

  const changeAnswers = () => {
    if (!last) {
      return;
    }
    ask.mutate(last, { onSuccess: (fresh) => setGuided(askAgain(guided, fresh)) });
  };

  const answer = (question: string, choice: string) =>
    setGuided({ answers: { ...guided.answers, [question]: choice } });

  const results = resultsOf(live, last);
  const disabled = !archive || !out || inputs.isPending || busy;

  return (
    <div className="flex flex-col gap-6" data-ui="GuidedRepair">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-surface-400 select-none">
          {guided.step === "asking"
            ? m.guided_questions_description()
            : m.guided_start_description()}
        </p>
        {running ? (
          <StopButton />
        ) : (
          guided.step === "start" && (
            <Button
              variant="filled"
              left={<CompassIcon weight="bold" />}
              disabled={disabled}
              // The start's description beside it would squeeze it onto two lines.
              className="shrink-0"
              onClick={guide}
            >
              {m.guided_start_action()}
            </Button>
          )
        )}
      </div>
      {dialog}
      {live && <LiveProgress live={live} stages={stages} />}
      {run.isError && <FailureNote failure={run.error} />}
      {ask.isError && <FailureNote failure={ask.error} />}
      {guided.step === "asking" && !live && (
        <div className="flex flex-col gap-6" data-ui="GuidedRepair:questions">
          {guided.questions.map((question) => (
            <QuestionCard
              key={question.id}
              question={question}
              answers={guided.answers}
              onAnswer={(choice) => answer(question.id, choice)}
            />
          ))}
          <div className="flex flex-wrap items-center gap-3">
            <Button
              variant="filled"
              left={<PlayIcon weight="bold" />}
              disabled={disabled || running}
              onClick={runAnswers}
            >
              {m.guided_run_action()}
            </Button>
            <Button variant="ghost" onClick={() => setGuided(NOT_GUIDED)}>
              {m.guided_back_action()}
            </Button>
          </div>
        </div>
      )}
      {guided.step !== "asking" && last && !live && <RunSummary view={last} />}
      {guided.step === "done" && !live && (
        <div className="flex flex-wrap items-center gap-3" data-ui="GuidedRepair:done">
          {guided.questions.length > 0 && (
            <Button
              variant="outline"
              left={<ArrowCounterClockwiseIcon weight="bold" />}
              loading={ask.isPending}
              disabled={running}
              onClick={changeAnswers}
            >
              {m.guided_change_action()}
            </Button>
          )}
          <Button variant="ghost" onClick={() => setGuided(NOT_GUIDED)}>
            {m.guided_back_action()}
          </Button>
        </div>
      )}
      {guided.step !== "asking" && results && <RunResults {...results} />}
    </div>
  );
}
