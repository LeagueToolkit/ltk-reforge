import { match } from "ts-pattern";

import { FailureNote, SectionCard, Spinner } from "@/components";
import { m } from "@/i18n";
import type { FixupEntry, FixupPhase } from "@/lib/bindings.gen";

import { useFixupCatalog } from "../api/catalog";

function phaseLabel(phase: FixupPhase): string {
  return match(phase)
    .with({ name: "files" }, () => m["phase.files.label"]())
    .with({ name: "undo" }, () => m["phase.undo.label"]())
    .with({ name: "layout" }, ({ build }) => m["phase.layout.label"]({ build }))
    .with({ name: "conform" }, () => m["phase.conform.label"]())
    .with({ name: "strip" }, () => m["phase.strip.label"]())
    .exhaustive();
}

function FixupRow({ fixup }: { fixup: FixupEntry }) {
  return (
    <li className="flex items-start justify-between gap-4 py-2" data-ui="FixupList:row">
      <div className="min-w-0">
        <p className="text-sm text-surface-100">{fixup.title}</p>
        <p className="font-mono text-xs text-surface-400">
          {fixup.id}
          {fixup.records.length > 0 && ` · ${fixup.records.join(" ")}`}
        </p>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-0.5 text-xs text-surface-400 select-none">
        <span>{phaseLabel(fixup.phase)}</span>
        {!fixup.default && (
          <span className="text-warning-text">{m.fixups_off_by_default_label()}</span>
        )}
      </div>
    </li>
  );
}

/** The library's fixups, in the order they run, each with its phase and records. */
export function FixupList() {
  const catalog = useFixupCatalog();

  return (
    <SectionCard title={m.fixups_list_title()} description={m.fixups_list_description()}>
      {catalog.isPending && <Spinner />}
      {catalog.isError && <FailureNote failure={catalog.error} />}
      {catalog.isSuccess && (
        <ul className="divide-y divide-surface-700/50">
          {catalog.data.map((fixup) => (
            <FixupRow key={fixup.id} fixup={fixup} />
          ))}
        </ul>
      )}
    </SectionCard>
  );
}
