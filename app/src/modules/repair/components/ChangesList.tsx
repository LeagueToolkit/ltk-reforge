import { ArrowRightIcon } from "@phosphor-icons/react";
import { match } from "ts-pattern";

import { SectionCard } from "@/components";
import { m } from "@/i18n";
import type { AfterView, ChangeView, FixupChanges } from "@/lib/bindings.gen";
import { useFixupCatalog } from "@/modules/fixups";

function afterText(after: AfterView): string {
  return match(after)
    .with({ as: "value" }, ({ text }) => text)
    .with({ as: "moved" }, ({ to }) => m.repair_after_moved({ to }))
    .with({ as: "removed" }, () => m.repair_after_removed())
    .with({ as: "lost" }, () => m.repair_after_lost())
    .with({ as: "unconvertible" }, ({ wants }) => m.repair_after_unconvertible({ wants }))
    .exhaustive();
}

function ChangeRow({ change }: { change: ChangeView }) {
  return (
    <li className="py-1 font-mono text-xs" data-ui="ChangesList:change">
      <p className="break-all text-surface-200">
        <span className="text-surface-400">{change.entry}</span> {change.path}
      </p>
      <p className="break-all text-surface-400">
        {change.before && <span className="text-surface-300">{change.before.text}</span>}
        {change.before && <ArrowRightIcon className="mx-1 inline h-3 w-3" />}
        <span className="text-accent-300">{afterText(change.after)}</span>
        {!change.reached && <span className="font-sans"> · {m.repair_unreached_label()}</span>}
      </p>
    </li>
  );
}

function FixupRow({ fixup, title }: { fixup: FixupChanges; title: string | undefined }) {
  return (
    <details className="group py-2" data-ui="ChangesList:fixup">
      <summary className="flex cursor-pointer items-start justify-between gap-4 select-none">
        <span className="min-w-0">
          <span className="block text-sm text-surface-100">{title ?? fixup.fixup}</span>
          <span className="block font-mono text-xs text-surface-400">{fixup.fixup}</span>
        </span>
        <span className="shrink-0 text-xs text-surface-400">
          {m.repair_counts_label({
            changed: fixup.changed,
            removed: fixup.removed,
            lost: fixup.lost,
            unconvertible: fixup.unconvertible,
          })}
        </span>
      </summary>
      <ul className="mt-2 ml-2 border-l border-surface-700 pl-3">
        {fixup.changes.map((change, at) => (
          <ChangeRow key={at} change={change} />
        ))}
      </ul>
    </details>
  );
}

/** What each fixup changed, fixup by fixup, each opened to its values before and after. */
export function ChangesList({ changes }: { changes: FixupChanges[] }) {
  const catalog = useFixupCatalog();
  const titles = new Map(catalog.data?.map((entry) => [entry.id, entry.title]));
  return (
    <SectionCard title={m.repair_changes_title()}>
      {changes.length === 0 && (
        <p className="text-sm text-surface-400 select-none">{m.repair_changes_empty()}</p>
      )}
      {changes.length > 0 && (
        <div className="divide-y divide-surface-700/50">
          {changes.map((fixup) => (
            <FixupRow key={fixup.fixup} fixup={fixup} title={titles.get(fixup.fixup)} />
          ))}
        </div>
      )}
    </SectionCard>
  );
}
