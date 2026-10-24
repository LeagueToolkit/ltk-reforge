import { match } from "ts-pattern";

import type { BatchRow, BatchUnit } from "@/lib/bindings.gen";

/** A slot as the summary spells it: the base, else its number. */
function slot(number: number | null): string {
  if (number === null) {
    return "game";
  }
  return number === 0 ? "base" : `skin${number}`;
}

/** One unit's move: `Vex base→skin7`. */
function unitText(unit: BatchUnit): string {
  return `${unit.character} ${slot(unit.source)}→${slot(unit.chosen)}`;
}

/** How a row ended, in a word, or where each of a written port's units went. */
function outcomeText(row: BatchRow): string {
  return match(row.outcome)
    .with({ outcome: "written" }, () =>
      row.units.length > 0 ? row.units.map(unitText).join(", ") : "written",
    )
    .with({ outcome: "refused" }, () => "refused")
    .with({ outcome: "halted" }, () => "halted")
    .with({ outcome: "fault" }, () => "failed")
    .with({ outcome: "unreadable" }, () => "unreadable")
    .with({ outcome: "cancelled" }, () => "stopped")
    .exhaustive();
}

/**
 * The batch in a line per mod, for the clipboard: the mod's name, version and author, then
 * where each unit went (`base→base` ported at the base, `base→skin7` moved to a premium
 * slot) or how the run ended.
 */
export function summaryText(rows: readonly BatchRow[]): string {
  return rows
    .map((row) => {
      const name = row.name ?? row.archive;
      const version = row.version ? ` ${row.version}` : "";
      const author = row.author ? ` (${row.author})` : "";
      return `${name}${version}${author}: ${outcomeText(row)}`;
    })
    .join("\n");
}
