import { SegmentedControl } from "@/components";
import { m } from "@/i18n";

import { useRunEvents } from "../api/run";
import { type RepairMode, useRepairStore } from "../state/repairStore";
import { AdvancedRepair } from "./AdvancedRepair";
import { ModCard } from "./ModCard";
import { SimpleRepair } from "./SimpleRepair";

/**
 * A run of one mod in the simple or the advanced view of the same run (#ADR-0050). Both port
 * by default; a repair is a preset of the advanced view (#ADR-0052).
 */
export function RepairPage() {
  const mode = useRepairStore((state) => state.mode);
  const setMode = useRepairStore((state) => state.setMode);
  useRunEvents();

  const modes: { value: RepairMode; label: string }[] = [
    { value: "simple", label: m.repair_mode_simple_label() },
    { value: "advanced", label: m.repair_mode_advanced_label() },
  ];

  return (
    <div className="flex flex-col gap-6" data-ui="RepairPage">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h1 className="font-display text-xl font-semibold text-surface-50">{m.run_page_title()}</h1>
        <SegmentedControl
          options={modes}
          value={mode}
          onChange={setMode}
          aria-label={m.repair_mode_label()}
        />
      </div>
      <ModCard />
      {mode === "simple" && <SimpleRepair />}
      {mode === "advanced" && <AdvancedRepair />}
    </div>
  );
}
