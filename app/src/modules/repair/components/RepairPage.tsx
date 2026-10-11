import { SegmentedControl } from "@/components";
import { m } from "@/i18n";
import { LAST_RUN, RunRefContext } from "@/modules/findings";
import { StaleInputsLink } from "@/modules/inputs";

import { useRunEvents } from "../api/run";
import { type RepairMode, useRepairStore } from "../state/repairStore";
import { AdvancedRepair } from "./AdvancedRepair";
import { GuidedRepair } from "./GuidedRepair";
import { ModCard } from "./ModCard";
import { SimpleRepair } from "./SimpleRepair";

/**
 * A run of one mod in the simple, the guided or the advanced view of the same run (#ADR-0050,
 * #ADR-0090). Each ports by default; a repair is a preset of the advanced view (#ADR-0052).
 */
export function RepairPage() {
  const mode = useRepairStore((state) => state.mode);
  const setMode = useRepairStore((state) => state.setMode);
  // The backend holds the run that ended last; a run under way is not yet it.
  const ended = useRepairStore((state) => state.last != null && state.live == null);
  useRunEvents();

  const modes: { value: RepairMode; label: string }[] = [
    { value: "simple", label: m.repair_mode_simple_label() },
    { value: "guided", label: m.repair_mode_guided_label() },
    { value: "advanced", label: m.repair_mode_advanced_label() },
  ];

  return (
    <div className="flex flex-col gap-6" data-ui="RepairPage">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <h1 className="font-display text-xl font-semibold text-surface-50">
            {m.run_page_title()}
          </h1>
          <StaleInputsLink />
        </div>
        <SegmentedControl
          options={modes}
          value={mode}
          onChange={setMode}
          aria-label={m.repair_mode_label()}
        />
      </div>
      <ModCard />
      <RunRefContext value={ended ? LAST_RUN : null}>
        {mode === "simple" && <SimpleRepair />}
        {mode === "guided" && <GuidedRepair />}
        {mode === "advanced" && <AdvancedRepair />}
      </RunRefContext>
    </div>
  );
}
