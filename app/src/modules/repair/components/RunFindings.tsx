import { useState } from "react";

import { Button, SectionCard } from "@/components";
import { m } from "@/i18n";
import type { Finding } from "@/lib/bindings.gen";
import { FindingList, worstFirst } from "@/modules/findings";

/**
 * A run's findings, the worst first. Errors and warnings are always shown; the notes are
 * folded behind a button, since a run raises many and none of them loses anything.
 */
export function RunFindings({ findings }: { findings: Finding[] }) {
  const [notes, setNotes] = useState(false);
  const sorted = worstFirst(findings);
  const shown = notes ? sorted : sorted.filter((finding) => finding.severity !== "note");
  const noteCount = findings.length - sorted.filter((f) => f.severity !== "note").length;

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
      <FindingList findings={shown} empty={m.repair_findings_empty()} />
    </SectionCard>
  );
}
