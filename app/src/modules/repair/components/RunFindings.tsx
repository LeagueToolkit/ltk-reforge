import { type ReactNode, useState } from "react";

import { Button, SectionCard } from "@/components";
import { m } from "@/i18n";
import type { Finding, FixupChanges, ObjectView } from "@/lib/bindings.gen";
import { FindingChanges, isFixupFinding } from "@/modules/changes";
import { FindingList, type IdNames, worstFirst } from "@/modules/findings";

/** What a finding opens to: a fixup's finding, the changes it counts, once they came. */
export function fixupDetail(changes: FixupChanges[] | null, objects: ObjectView[]) {
  return function detailOf(finding: Finding): ReactNode {
    if (!changes || !isFixupFinding(finding)) {
      return null;
    }
    return <FindingChanges finding={finding} changes={changes} objects={objects} />;
  };
}

export interface RunFindingsProps {
  findings: Finding[];
  /** What the fixups changed. Null until `fix` ended. */
  changes: FixupChanges[] | null;
  objects: ObjectView[];
  /** A name for each id a finding's fields can hold. */
  names: IdNames;
}

/**
 * A run's findings, the worst first. Errors and warnings are always shown; the notes are
 * folded behind a button, since a run raises many and none of them loses anything. A fixup's
 * notes are under "What changed" with its changes, once they came.
 */
export function RunFindings({ findings, changes, objects, names }: RunFindingsProps) {
  const [notes, setNotes] = useState(false);
  const listed = worstFirst(
    findings.filter(
      (finding) => !(changes && isFixupFinding(finding) && finding.severity === "note"),
    ),
  );
  const problems = listed.filter((finding) => finding.severity !== "note");
  const noteCount = listed.length - problems.length;

  return (
    <SectionCard
      title={m.repair_findings_title()}
      action={
        noteCount > 0 && (
          <Button size="xs" variant="ghost" onClick={() => setNotes(!notes)}>
            {notes
              ? m.repair_notes_hide_action()
              : m.repair_notes_show_action({ count: noteCount })}
          </Button>
        )
      }
    >
      <FindingList
        findings={notes ? listed : problems}
        empty={m.repair_findings_empty()}
        detailOf={fixupDetail(changes, objects)}
        names={names}
      />
    </SectionCard>
  );
}
