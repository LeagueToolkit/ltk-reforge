import { match } from "ts-pattern";

import type { BatchRow, Outcome } from "@/lib/bindings.gen";

/** The tone an outcome's badge takes. */
export type OutcomeTone = "success" | "warning" | "danger" | "muted";

/**
 * The tone of an outcome: a package written, no package for what the run found (a refusal or a
 * fatal finding), no package for a fault or an archive that does not read, a run stopped.
 */
export function outcomeTone(outcome: Outcome): OutcomeTone {
  return match(outcome)
    .with({ outcome: "written" }, () => "success" as const)
    .with({ outcome: "refused" }, { outcome: "halted" }, () => "warning" as const)
    .with({ outcome: "fault" }, { outcome: "unreadable" }, () => "danger" as const)
    .with({ outcome: "cancelled" }, () => "muted" as const)
    .exhaustive();
}

/** How many rows of a batch ended each way, by their tone: what the summary line counts. */
export interface Summary {
  written: number;
  refused: number;
  failed: number;
  stopped: number;
}

/** The rows of a batch counted for its summary line. */
export function summaryOf(rows: readonly BatchRow[]): Summary {
  const summary: Summary = { written: 0, refused: 0, failed: 0, stopped: 0 };
  for (const row of rows) {
    const tone = outcomeTone(row.outcome);
    if (tone === "success") {
      summary.written += 1;
    } else if (tone === "warning") {
      summary.refused += 1;
    } else if (tone === "danger") {
      summary.failed += 1;
    } else {
      summary.stopped += 1;
    }
  }
  return summary;
}

/** The paths of `added` not yet in `archives`, after them, each once. */
export function withArchives(archives: readonly string[], added: readonly string[]): string[] {
  const seen = new Set(archives);
  const next = [...archives];
  for (const path of added) {
    if (!seen.has(path)) {
      seen.add(path);
      next.push(path);
    }
  }
  return next;
}
