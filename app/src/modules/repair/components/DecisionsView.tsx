import { CheckIcon, QuestionIcon, XIcon } from "@phosphor-icons/react";

import { Button, Disclosure, SectionCard } from "@/components";
import { m } from "@/i18n";
import type {
  CandidateView,
  CheckView,
  DecisionView,
  PolicyView,
  TargetsView,
} from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

const REASONS = [
  "base",
  "paste-cleaned",
  "slot",
  "fallback",
  "clips-origin",
  "skeleton-origin",
  "forced",
  "infeasible",
  "sub-follows",
  "sub-left-to-base",
  "unjudged",
  "superseded",
  "conflict",
  "pre-rework",
] as const;
type Reason = (typeof REASONS)[number];

const ASPECTS = ["mesh", "submeshes", "skeleton", "clips"] as const;
type Aspect = (typeof ASPECTS)[number];

const TARGETS = ["auto", "base", "origin", "slot"] as const;
const LEVELS = ["block", "same-joints", "own-rig", "allow"] as const;

function slotLabel(slot: number | null): string {
  return slot == null ? m.decide_left_to_game_label() : m.decide_skin_label({ slot });
}

function targetLabel(target: string): string {
  return TARGETS.includes(target as (typeof TARGETS)[number])
    ? m[`policy.target.${target as (typeof TARGETS)[number]}.label`]()
    : m.decide_skin_label({ slot: target });
}

function levelLabel(level: string): string {
  return LEVELS.includes(level as (typeof LEVELS)[number])
    ? m[`policy.reuse.${level as (typeof LEVELS)[number]}.label`]()
    : level;
}

function aspectLabel(aspect: string): string {
  return ASPECTS.includes(aspect as Aspect)
    ? m[`decide.aspect.${aspect as Aspect}.label`]()
    : aspect;
}

/** The decision as one sentence, from its reason; the bare move when the reason is unknown. */
function decisionTitle(decision: DecisionView): string {
  const character = decision.character_name || decision.character;
  const chosen = slotLabel(decision.chosen);
  if (REASONS.includes(decision.reason as Reason)) {
    return m[`decide.reason.${decision.reason as Reason}.title`]({ character, chosen });
  }
  return m.decide_unit_title({ character, source: decision.source, chosen });
}

/** A check's outcome, from the mark the library writes (`fits`, `fix`, `NO`, `?`). */
function fitOf(check: CheckView): { label: string; icon: React.ReactNode; className: string } {
  switch (check.fit) {
    case "fits":
      return {
        label: m.decide_fit_fits(),
        icon: <CheckIcon weight="bold" className="h-3 w-3" />,
        className: "text-success-text",
      };
    case "fix":
      return {
        label: m.decide_fit_fix(),
        icon: <CheckIcon weight="bold" className="h-3 w-3" />,
        className: "text-warning-text",
      };
    case "?":
      return {
        label: m.decide_fit_unknown(),
        icon: <QuestionIcon weight="bold" className="h-3 w-3" />,
        className: "text-surface-400",
      };
    default:
      return {
        label: check.fit === "NO" ? m.decide_fit_no() : check.fit,
        icon: <XIcon weight="bold" className="h-3 w-3" />,
        className: "text-danger-text",
      };
  }
}

function Checks({ checks }: { checks: CheckView[] }) {
  return (
    <ul className="flex flex-col gap-0.5 text-xs">
      {checks.map((check) => {
        const fit = fitOf(check);
        return (
          <li key={check.aspect} className="flex items-start gap-1.5">
            <span className={twMerge("mt-0.5 shrink-0", fit.className)}>{fit.icon}</span>
            <span className="text-surface-200">
              <span className="text-surface-400 select-none">{aspectLabel(check.aspect)}: </span>
              {check.detail}
              {check.fit !== "fits" && (
                <span className={twMerge("ml-1", fit.className)}>({fit.label})</span>
              )}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

function Candidate({ candidate }: { candidate: CandidateView }) {
  return (
    <li className="flex flex-col gap-1 py-2" data-ui="DecisionsView:candidate">
      <p
        className={twMerge(
          "flex items-center gap-1 text-xs",
          candidate.feasible ? "text-success-text" : "text-danger-text",
        )}
      >
        {candidate.feasible ? (
          <CheckIcon weight="bold" className="h-3 w-3" />
        ) : (
          <XIcon weight="bold" className="h-3 w-3" />
        )}
        {m.decide_candidate_label({ slot: slotLabel(candidate.slot), why: candidate.why })}
      </p>
      <Checks checks={candidate.checks} />
    </li>
  );
}

function Decision({
  decision,
  onPolicy,
}: {
  decision: DecisionView;
  onPolicy?: (policy: PolicyView) => void;
}) {
  const origin =
    decision.origin.slot != null
      ? `${decision.origin.verdict} (${decision.origin.slot})`
      : decision.origin.verdict;
  const chosen = decision.candidates.find((candidate) => candidate.slot === decision.chosen);
  const others = decision.candidates.filter((candidate) => candidate !== chosen);
  const differ = decision.alternatives.filter(
    (alternative) => alternative.chosen !== decision.chosen,
  );

  return (
    <div className="flex flex-col gap-3 py-3" data-ui="DecisionsView:decision">
      <div>
        <p className="text-sm text-surface-50">{decisionTitle(decision)}</p>
        <p className="font-mono text-xs text-surface-400">
          {m.decide_unit_meta({
            character: decision.character_name || decision.character,
            source: decision.source,
            reason: decision.reason,
            origin,
          })}
        </p>
      </div>

      {chosen && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-surface-400 select-none">
            {m.decide_chosen_checks_title({ chosen: slotLabel(chosen.slot) })}
          </p>
          <Checks checks={chosen.checks} />
        </div>
      )}

      {others.length > 0 && (
        <Disclosure
          summary={
            <span className="text-xs text-surface-400 select-none">
              {others.length === 1
                ? m.decide_other_candidates_title_one()
                : m.decide_other_candidates_title({ count: others.length })}
            </span>
          }
        >
          <ul className="divide-y divide-surface-700/50 pl-5">
            {others.map((candidate) => (
              <Candidate key={candidate.slot} candidate={candidate} />
            ))}
          </ul>
        </Disclosure>
      )}

      {decision.alternatives.length > 0 && differ.length === 0 && (
        <p className="text-xs text-surface-400 select-none">{m.decide_alternatives_same()}</p>
      )}
      {differ.length > 0 && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-surface-400 select-none">{m.decide_alternatives_title()}</p>
          <ul className="flex flex-col gap-1">
            {differ.map((alternative) => (
              <li
                key={`${alternative.policy.target}/${alternative.policy.anim_reuse}`}
                className="flex items-center justify-between gap-3 text-xs"
              >
                <span className="text-surface-200">
                  {m.decide_alternative_label({
                    level: levelLabel(alternative.policy.anim_reuse),
                    chosen: slotLabel(alternative.chosen),
                  })}
                  <span className="ml-1 font-mono text-surface-400">({alternative.reason})</span>
                </span>
                {onPolicy && (
                  <Button size="xs" variant="ghost" onClick={() => onPolicy(alternative.policy)}>
                    {m.decide_use_policy_action()}
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/**
 * The decide stage of a port: per unit, where it ships and why in one sentence, what fits
 * there, the other slots weighed, and what the other levels of animation reuse choose, each
 * one a run away.
 */
export function DecisionsView({
  targets,
  onPolicy,
}: {
  targets: TargetsView;
  onPolicy?: (policy: PolicyView) => void;
}) {
  return (
    <SectionCard
      title={m.decide_title()}
      description={m.decide_policy_label({
        target: targetLabel(targets.policy.target),
        reuse: levelLabel(targets.policy.anim_reuse),
      })}
    >
      <div className="divide-y divide-surface-700/50">
        {targets.decisions.map((decision) => (
          <Decision key={decision.unit} decision={decision} onPolicy={onPolicy} />
        ))}
      </div>
    </SectionCard>
  );
}
