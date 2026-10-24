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
    .with({ outcome: "scanned" }, () =>
      row.units.length > 0 ? row.units.map(unitText).join(", ") : "scanned",
    )
    .with({ outcome: "refused" }, () => "refused")
    .with({ outcome: "halted" }, () => "halted")
    .with({ outcome: "fault" }, () => "failed")
    .with({ outcome: "unreadable" }, () => "unreadable")
    .with({ outcome: "cancelled" }, () => "stopped")
    .exhaustive();
}

/**
 * One mod's line: its name, version and author, then where each unit went (`base→base`
 * ported at the base, `base→skin7` moved to a premium slot) or how the run ended.
 */
export function rowText(row: BatchRow): string {
  const name = row.name ?? row.archive;
  const version = row.version ? ` ${row.version}` : "";
  const author = row.author ? ` (${row.author})` : "";
  return `${name}${version}${author}: ${outcomeText(row)}`;
}

/** The batch in a line per mod, for the clipboard. */
export function summaryText(rows: readonly BatchRow[]): string {
  return rows.map(rowText).join("\n");
}

/**
 * The written rows with a unit the mod ships at the base that the port placed at a premium
 * slot (#ADR-0013): a paste of a premium skin that does not fit the base becomes a mod for
 * that skin.
 */
export function movedToPremium(rows: readonly BatchRow[]): BatchRow[] {
  return rows.filter(
    (row) =>
      row.outcome.outcome === "written" &&
      row.units.some((unit) => unit.source === 0 && unit.chosen !== null && unit.chosen > 0),
  );
}
