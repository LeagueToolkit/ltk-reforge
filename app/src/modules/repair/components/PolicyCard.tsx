import { SectionCard, SegmentedControl, Switch } from "@/components";
import { m } from "@/i18n";

import { useRepairStore } from "../state/repairStore";

const TARGETS = ["auto", "base", "origin", "slot"] as const;
const LEVELS = ["block", "same-joints", "own-rig", "allow"] as const;

/** A port's policy: how each unit's target is chosen, and the animation reuse accepted. */
export function PolicyCard() {
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const fixed = !TARGETS.includes(advanced.target as (typeof TARGETS)[number]);
  const targets = [
    ...TARGETS.map((value) => ({
      value: value as string,
      label: m[`policy.target.${value}.label`](),
    })),
    { value: "skin", label: m["policy.target.skin.label"]() },
  ];
  const levels = LEVELS.map((value) => ({
    value: value as string,
    label: m[`policy.reuse.${value}.label`](),
  }));

  return (
    <SectionCard title={m.policy_title()} description={m.policy_description()}>
      <div className="flex flex-col gap-2">
        <p className="text-sm text-surface-100 select-none">{m.policy_target_label()}</p>
        <div className="flex items-center gap-3">
          <SegmentedControl
            options={targets}
            value={fixed ? "skin" : advanced.target}
            onChange={(target) => setAdvanced({ target: target === "skin" ? "1" : target })}
            aria-label={m.policy_target_label()}
          />
          {fixed && (
            <input
              type="number"
              min={0}
              value={advanced.target}
              onChange={(event) => setAdvanced({ target: event.target.value })}
              aria-label={m["policy.target.skin.label"]()}
              className="w-20 rounded-md border border-surface-600 bg-surface-950/40 px-2 py-1 font-mono text-xs text-surface-100"
            />
          )}
        </div>
        <p className="text-xs text-surface-400 select-none">
          {fixed
            ? m["policy.target.skin.description"]()
            : m[`policy.target.${advanced.target as (typeof TARGETS)[number]}.description`]()}
        </p>
      </div>
      <div className="flex flex-col gap-2">
        <p className="text-sm text-surface-100 select-none">{m.policy_reuse_label()}</p>
        <SegmentedControl
          options={levels}
          value={advanced.animReuse}
          onChange={(animReuse) => setAdvanced({ animReuse })}
          aria-label={m.policy_reuse_label()}
        />
        <p className="text-xs text-surface-400 select-none">
          {m[`policy.reuse.${advanced.animReuse as (typeof LEVELS)[number]}.description`]()}
        </p>
      </div>
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-sm text-surface-100 select-none">{m.policy_events_label()}</p>
          <p className="text-xs text-surface-400 select-none">{m.policy_events_description()}</p>
        </div>
        <Switch
          checked={advanced.gameEventsOnly}
          onCheckedChange={(gameEventsOnly) => setAdvanced({ gameEventsOnly })}
          aria-label={m.policy_events_label()}
        />
      </div>
    </SectionCard>
  );
}
