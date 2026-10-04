import { SegmentedControl } from "@/components";
import { m } from "@/i18n";
import type { RunKind } from "@/lib/bindings.gen";

import { useRunEvents } from "../api/run";
import { type RepairMode, useRepairStore } from "../state/repairStore";
import { AdvancedRepair } from "./AdvancedRepair";
import { ModCard } from "./ModCard";
import { SimpleRepair } from "./SimpleRepair";

/**
 * A run of one mod, a repair or a port, in the simple or the advanced view of the same run
 * (#ADR-0050).
 */
export function RepairPage() {
  const mode = useRepairStore((state) => state.mode);
  const setMode = useRepairStore((state) => state.setMode);
  const kind = useRepairStore((state) => state.kind);
  const setKind = useRepairStore((state) => state.setKind);
  useRunEvents();

  const modes: { value: RepairMode; label: string }[] = [
    { value: "simple", label: m.repair_mode_simple_label() },
    { value: "advanced", label: m.repair_mode_advanced_label() },
  ];
  const kinds: { value: RunKind; label: string }[] = [
    { value: "repair", label: m.run_kind_repair_label() },
    { value: "port", label: m.run_kind_port_label() },
  ];

  return (
    <div className="flex max-w-4xl flex-col gap-6" data-ui="RepairPage">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h1 className="font-display text-xl font-semibold text-surface-50">{m.run_page_title()}</h1>
        <div className="flex gap-3">
          <SegmentedControl
            options={kinds}
            value={kind}
            onChange={setKind}
            aria-label={m.run_kind_label()}
          />
          <SegmentedControl
            options={modes}
            value={mode}
            onChange={setMode}
            aria-label={m.repair_mode_label()}
          />
        </div>
      </div>
      <p className="-mt-4 text-sm text-surface-400 select-none">
        {kind === "port" ? m.run_kind_port_description() : m.run_kind_repair_description()}
      </p>
      <ModCard />
      {mode === "simple" && <SimpleRepair />}
      {mode === "advanced" && <AdvancedRepair />}
    </div>
  );
}
