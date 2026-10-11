import { Checkbox, Disclosure } from "@/components";
import { m, stageLabel } from "@/i18n";

import { AREA_GROUPS, AREAS, hasLine, SEVERITIES, severityLine, withLine } from "./blockLines";

export interface BlockPickerProps {
  /** The filter lines, the one source: a checkbox reads and writes its line in them. */
  block: string[];
  onChange: (block: string[]) => void;
}

/**
 * What refuses the write: a severity or an area picked by its box, and the raw lines under a
 * disclosure for a term no box offers. The lines stay the setting; the boxes are a view of them.
 */
export function BlockPicker({ block, onChange }: BlockPickerProps) {
  const other = block.filter(
    (line) =>
      line.trim() !== "" &&
      !SEVERITIES.some((severity) => hasLine([line], severityLine(severity))) &&
      !AREAS.some((area) => hasLine([line], area)),
  );
  return (
    <div className="flex flex-col gap-3" data-ui="BlockPicker">
      <div>
        <p className="text-sm text-surface-100 select-none">{m.advanced_block_label()}</p>
        <p className="text-xs text-surface-400 select-none">{m.advanced_block_description()}</p>
      </div>
      <div className="flex flex-col gap-1">
        <p className="text-xs text-surface-400 select-none">{m.advanced_block_severity_label()}</p>
        <div className="flex flex-wrap gap-x-4 gap-y-1">
          {SEVERITIES.map((severity) => (
            <Checkbox
              key={severity}
              checked={hasLine(block, severityLine(severity))}
              onCheckedChange={(on) => onChange(withLine(block, severityLine(severity), on))}
              label={m[`severity.${severity}.label`]()}
            />
          ))}
        </div>
      </div>
      <div className="flex flex-col gap-2">
        <p className="text-xs text-surface-400 select-none">{m.advanced_block_area_label()}</p>
        <div
          className="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-x-4 gap-y-3"
          data-ui="BlockPicker:areas"
        >
          {AREA_GROUPS.map((group) => (
            <div key={group.stage} className="flex flex-col gap-1">
              <p className="text-xs font-medium text-surface-300 select-none">
                {group.stage === "inputs"
                  ? m.advanced_block_inputs_label()
                  : stageLabel(group.stage)}
              </p>
              {group.areas.map((area) => (
                <Checkbox
                  key={area}
                  checked={hasLine(block, area)}
                  onCheckedChange={(on) => onChange(withLine(block, area, on))}
                  label={<span className="font-mono text-xs">{area}</span>}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
      <Disclosure
        summary={
          <span className="text-xs text-surface-400 select-none">
            {m.advanced_block_raw_title({ count: other.length })}
          </span>
        }
        panelClassName="pt-2"
      >
        <label className="flex flex-col gap-1">
          <span className="text-xs text-surface-400 select-none">
            {m.advanced_block_raw_description()}
          </span>
          <textarea
            rows={3}
            value={block.join("\n")}
            onChange={(event) => onChange(event.target.value.split("\n"))}
            aria-label={m.advanced_block_label()}
            className="rounded-md border border-surface-600 bg-surface-950/40 p-2 font-mono text-xs text-surface-100 focus-visible:outline-2 focus-visible:outline-accent-500"
          />
        </label>
      </Disclosure>
    </div>
  );
}
