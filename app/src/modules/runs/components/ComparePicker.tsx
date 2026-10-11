import { Button, Dialog } from "@/components";
import { m } from "@/i18n";
import type { RunRow } from "@/lib/bindings.gen";

import { groupOthers } from "../compare";
import { RunLine, runButtonClass } from "./RunList";

interface ComparePickerProps {
  open: boolean;
  onClose: () => void;
  /** The run open, which the one picked is compared with. */
  row: RunRow;
  runs: RunRow[];
  onPick: (id: number) => void;
  /** The time the ages are counted to. */
  now: Date;
}

/** A group of the runs to pick from, under a small heading. None when it holds no run. */
function Group({
  title,
  runs,
  now,
  onPick,
}: {
  title: string;
  runs: RunRow[];
  now: Date;
  onPick: (id: number) => void;
}) {
  if (runs.length === 0) {
    return null;
  }
  return (
    <section className="flex flex-col gap-1">
      <h4 className="px-3 text-[11px] font-semibold tracking-wide text-surface-400 uppercase select-none">
        {title}
      </h4>
      <ul className="flex flex-col gap-1">
        {runs.map((run) => (
          <li key={run.id}>
            <button
              type="button"
              onClick={() => onPick(run.id)}
              className={runButtonClass}
              data-ui="ComparePicker:row"
            >
              <RunLine row={run} now={now} />
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** The other kept runs to compare the open one with: its mod's first, then the rest. */
export function ComparePicker({ open, onClose, row, runs, onPick, now }: ComparePickerProps) {
  const others = groupOthers(runs, row);
  const pick = (id: number) => {
    onPick(id);
    onClose();
  };
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={m.compare_pick_title()}
      description={m.compare_pick_description()}
      footer={
        <Button variant="ghost" onClick={onClose}>
          {m.compare_pick_cancel()}
        </Button>
      }
    >
      <div className="flex max-h-[60vh] flex-col gap-4 overflow-y-auto" data-ui="ComparePicker">
        <Group title={m.compare_pick_same_mod()} runs={others.sameMod} now={now} onPick={pick} />
        <Group title={m.compare_pick_other_mods()} runs={others.rest} now={now} onPick={pick} />
      </div>
    </Dialog>
  );
}
