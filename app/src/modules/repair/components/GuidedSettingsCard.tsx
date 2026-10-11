import { Button, SectionCard } from "@/components";
import { m } from "@/i18n";

import { useRepairStore } from "../state/repairStore";
import { slotLabel } from "./DecisionsView";

/** One setting, read only, its values listed, with the button that clears it. */
function Row({
  label,
  values = [],
  onClear,
}: {
  label: string;
  values?: string[];
  onClear: () => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <p className="text-sm text-surface-100 select-none">{label}</p>
        {values.length > 0 && (
          <ul className="flex flex-col gap-0.5 font-mono text-xs break-all text-surface-300">
            {values.map((value, at) => (
              <li key={at}>{value}</li>
            ))}
          </ul>
        )}
      </div>
      <Button size="xs" variant="ghost" onClick={onClear}>
        {m.guided_advanced_clear_action()}
      </Button>
    </div>
  );
}

/**
 * The settings of one mod the guided view's answers gave (#ADR-0098): the targets given to
 * units, the stale copies kept and the units at other skins shipped. Read only, each with a
 * Clear button; none shows while the settings hold none.
 */
export function GuidedSettingsCard() {
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const { unitTargets, keptFiles, keepOtherSlots } = advanced;
  if (unitTargets.length === 0 && keptFiles.length === 0 && !keepOtherSlots) {
    return null;
  }

  return (
    <SectionCard title={m.guided_advanced_title()} description={m.guided_advanced_description()}>
      {unitTargets.length > 0 && (
        <Row
          label={m.guided_advanced_unit_targets_label()}
          values={unitTargets.map((given) =>
            m.guided_advanced_unit_target_value({
              character: given.character,
              slot: given.slot,
              target: slotLabel(given.target),
            }),
          )}
          onClear={() => setAdvanced({ unitTargets: [] })}
        />
      )}
      {keptFiles.length > 0 && (
        <Row
          label={m.guided_advanced_kept_files_label()}
          values={keptFiles.map((kept) => `${kept.wad}/${kept.path}`)}
          onClear={() => setAdvanced({ keptFiles: [] })}
        />
      )}
      {keepOtherSlots && (
        <Row
          label={m.guided_advanced_keep_other_slots_label()}
          onClear={() => setAdvanced({ keepOtherSlots: false })}
        />
      )}
    </SectionCard>
  );
}
