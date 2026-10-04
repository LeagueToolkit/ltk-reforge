import { SegmentedControl } from "@/components";
import { m } from "@/i18n";

import { type RepairMode, useRepairStore } from "../state/repairStore";
import { AdvancedRepair } from "./AdvancedRepair";
import { ModCard } from "./ModCard";
import { SimpleRepair } from "./SimpleRepair";

/** A repair of one mod, in the simple or the advanced view of the same run (#ADR-0050). */
export function RepairPage() {
  const mode = useRepairStore((state) => state.mode);
  const setMode = useRepairStore((state) => state.setMode);
  const options: { value: RepairMode; label: string }[] = [
    { value: "simple", label: m.repair_mode_simple_label() },
    { value: "advanced", label: m.repair_mode_advanced_label() },
  ];

  return (
    <div className="flex max-w-4xl flex-col gap-6" data-ui="RepairPage">
      <div className="flex items-center justify-between gap-4">
        <h1 className="font-display text-xl font-semibold text-surface-50">
          {m.fixups_page_title()}
        </h1>
        <SegmentedControl
          options={options}
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
