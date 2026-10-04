import { m } from "@/i18n";

import { FixupList } from "./FixupList";

/** A repair: the fixups a run makes. Picking a mod and running it follow. */
export function RepairPage() {
  return (
    <div className="flex max-w-3xl flex-col gap-6" data-ui="RepairPage">
      <h1 className="font-display text-xl font-semibold text-surface-50">
        {m.fixups_page_title()}
      </h1>
      <FixupList />
    </div>
  );
}
