import { CheckIcon, XIcon } from "@phosphor-icons/react";

import { Button, SectionCard } from "@/components";
import { m } from "@/i18n";
import type { DecisionView, PolicyView, TargetsView } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

function slotLabel(slot: number | null): string {
  return slot == null ? m.decide_left_to_game_label() : m.decide_skin_label({ slot });
}

function policyLabel(policy: PolicyView): string {
  return `${policy.target} / ${policy.anim_reuse}`;
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
  return (
    <div className="flex flex-col gap-3 py-3" data-ui="DecisionsView:decision">
      <div>
        <p className="text-sm text-surface-50">
          {m.decide_unit_title({
            character: decision.character_name || decision.character,
            unit: decision.unit,
            source: decision.source,
            chosen: slotLabel(decision.chosen),
          })}
        </p>
        <p className="font-mono text-xs text-surface-400">
          {m.decide_reason_label({ reason: decision.reason, origin })}
        </p>
      </div>

      <table className="w-full text-left text-xs">
        <thead className="text-surface-400 select-none">
          <tr>
            <th className="w-20 py-1 font-normal">{m.decide_slot_column()}</th>
            <th className="py-1 font-normal">{m.decide_checks_column()}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-surface-700/50">
          {decision.candidates.map((candidate) => (
            <tr key={candidate.slot} className="align-top">
              <td className="py-1.5">
                <span
                  className={twMerge(
                    "flex items-center gap-1",
                    candidate.feasible ? "text-success-text" : "text-danger-text",
                  )}
                >
                  {candidate.feasible ? (
                    <CheckIcon weight="bold" className="h-3 w-3" />
                  ) : (
                    <XIcon weight="bold" className="h-3 w-3" />
                  )}
                  {candidate.slot}
                </span>
                <span className="text-surface-400">{candidate.why}</span>
              </td>
              <td className="py-1.5">
                <ul className="flex flex-col gap-0.5">
                  {candidate.checks.map((check) => (
                    <li key={check.aspect}>
                      <span
                        className={twMerge(
                          "font-mono",
                          check.fit === "fits" ? "text-surface-300" : "text-danger-text",
                        )}
                      >
                        {check.aspect}: {check.fit}
                      </span>
                      <span className="text-surface-400"> · {check.detail}</span>
                    </li>
                  ))}
                </ul>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {decision.alternatives.length > 0 && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-surface-400 select-none">{m.decide_alternatives_title()}</p>
          <ul className="flex flex-col gap-1">
            {decision.alternatives.map((alternative) => (
              <li
                key={policyLabel(alternative.policy)}
                className="flex items-center justify-between gap-3 text-xs"
              >
                <span className="font-mono text-surface-300">
                  {policyLabel(alternative.policy)}: {slotLabel(alternative.chosen)} (
                  {alternative.reason})
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
 * The decide stage of a port: per unit, the chosen skin and why, every slot weighed with its
 * checks, and what the compared policies choose, each one a run away.
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
      description={m.decide_policy_label({ policy: policyLabel(targets.policy) })}
    >
      <div className="divide-y divide-surface-700/50">
        {targets.decisions.map((decision) => (
          <Decision key={decision.unit} decision={decision} onPolicy={onPolicy} />
        ))}
      </div>
    </SectionCard>
  );
}
