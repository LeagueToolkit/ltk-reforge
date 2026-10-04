import { CaretDownIcon, CaretRightIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button } from "@/components";
import { m } from "@/i18n";
import type { LogLine } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

function levelClass(level: string): string {
  if (level === "error") {
    return "text-danger-text";
  }
  if (level === "warn") {
    return "text-warning-text";
  }
  return "text-surface-300";
}

/** A run's details: the library's spans and events, folded until the user opens them. */
export function RunDetails({ lines }: { lines: LogLine[] }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col gap-2" data-ui="RunDetails">
      <div>
        <Button
          size="xs"
          variant="ghost"
          left={open ? <CaretDownIcon weight="bold" /> : <CaretRightIcon weight="bold" />}
          onClick={() => setOpen(!open)}
        >
          {m.run_details_action({ count: lines.length })}
        </Button>
      </div>
      {open && (
        <ol className="max-h-72 overflow-y-auto rounded-md border border-surface-700 bg-surface-950/60 p-2 font-mono text-xs">
          {lines.map((line, at) => (
            <li key={at} className="flex gap-3">
              <span className="w-16 shrink-0 text-right text-surface-500">
                {(line.at / 1000).toFixed(2)}
              </span>
              <span className={twMerge("break-all", levelClass(line.level))}>{line.text}</span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
