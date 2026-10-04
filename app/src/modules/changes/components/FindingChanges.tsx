import type { Finding, FixupChanges, ObjectView } from "@/lib/bindings.gen";

import { itemsOf, outcomeOfCode } from "../model";
import { ObjectGroups } from "./ObjectGroups";

/** Whether a finding counts a fixup's changes: a `fixup.*` finding a fixup raised. */
export function isFixupFinding(finding: Finding): boolean {
  return finding.by != null && outcomeOfCode(finding.code) !== null;
}

export interface FindingChangesProps {
  finding: Finding;
  changes: FixupChanges[];
  objects: ObjectView[];
}

/** The changes a fixup's finding counts, object by object. */
export function FindingChanges({ finding, changes, objects }: FindingChangesProps) {
  const outcome = outcomeOfCode(finding.code);
  const own = changes.filter((fixup) => fixup.fixup === finding.by);
  const items = itemsOf(own, objects).filter((item) => item.change.outcome === outcome);
  return <ObjectGroups items={items} />;
}
