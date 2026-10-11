import { useMemo } from "react";

import { Button, FailureNote, SectionCard, Spinner } from "@/components";
import { m } from "@/i18n";
import type { BuildView, HistoryBuilds, HistoryObjectView } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { useHistoryObject } from "../api/history";
import { type DiffLine, lineDiff } from "../diff";
import type { ObjectAsked } from "../state/historyStore";
import { BuildSelect, inputClass, lifeShort } from "./parts";

const preClass =
  "max-h-[70vh] overflow-auto rounded-lg border border-surface-700/50 bg-surface-950/40 p-3 font-mono text-xs whitespace-pre";

const lineClass: Record<DiffLine["kind"], string> = {
  same: "text-surface-500",
  added: "bg-success/10 text-success-text",
  removed: "bg-danger/10 text-danger-text",
};

const lineMark: Record<DiffLine["kind"], string> = { same: " ", added: "+", removed: "-" };

/** A unified diff of two texts, a line each, marked as removed, added or the same. */
function Diff({ before, after }: { before: string; after: string }) {
  const lines = useMemo(() => lineDiff(before, after), [before, after]);
  if (lines == null) {
    return <p className="text-sm text-warning-text select-none">{m.history_diff_too_long()}</p>;
  }
  if (lines.every((line) => line.kind === "same")) {
    return <p className="text-sm text-surface-400 select-none">{m.history_diff_equal()}</p>;
  }
  return (
    <pre className={preClass} data-ui="HistoryDiff">
      {lines.map((line, index) => (
        <div key={index} className={lineClass[line.kind]}>
          {lineMark[line.kind]} {line.text}
        </div>
      ))}
    </pre>
  );
}

interface CompareProps {
  asked: ObjectAsked;
  at: BuildView;
  object: HistoryObjectView;
  other: number;
  builds: HistoryBuilds;
}

/** The object at another build, as a diff against it at the build looked up. */
function Compare({ asked, at, object, other, builds }: CompareProps) {
  const found = useHistoryObject(asked.bin, asked.entry, other);
  const version = builds.builds[other]?.version ?? `#${other}`;
  if (found.isPending) {
    return <Spinner />;
  }
  if (found.isError) {
    return <FailureNote failure={found.error} />;
  }
  if (found.data == null) {
    return (
      <p className="text-sm text-surface-400 select-none">{m.history_diff_missing({ version })}</p>
    );
  }
  // The earlier build's text is the one lines are removed from.
  const otherFirst = other < at.position;
  const [before, after] = otherFirst
    ? [found.data.text, object.text]
    : [object.text, found.data.text];
  return (
    <div className="flex flex-col gap-2">
      <p className="text-xs font-medium text-surface-300 select-none">
        {m.history_diff_title({
          before: otherFirst ? version : at.version,
          after: otherFirst ? at.version : version,
        })}
      </p>
      <Diff before={before} after={after} />
    </div>
  );
}

interface ObjectSectionProps {
  /** The build the object is shown at: the one looked up. */
  at: BuildView;
  builds: HistoryBuilds;
  bin: string;
  entry: string;
  onBin: (bin: string) => void;
  onEntry: (entry: string) => void;
  asked: ObjectAsked | null;
  onShow: () => void;
  /** The build the object is compared at, by its position. */
  other: number | null;
  onOther: (other: number | null) => void;
}

/**
 * An object of a bin at the build looked up, as text with its names spelled, and compared
 * with the same object at another build.
 */
export function ObjectSection({
  at,
  builds,
  bin,
  entry,
  onBin,
  onEntry,
  asked,
  onShow,
  other,
  onOther,
}: ObjectSectionProps) {
  const object = useHistoryObject(asked?.bin ?? "", asked?.entry ?? "", asked ? at.position : null);
  return (
    <SectionCard
      title={m.history_object_title()}
      description={m.history_object_description({ version: at.version })}
    >
      <form
        className="flex flex-wrap items-end gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          onShow();
        }}
      >
        <label className="flex min-w-64 flex-[2] flex-col gap-1 text-xs text-surface-300">
          {m.history_object_bin_label()}
          <input
            value={bin}
            spellCheck={false}
            onChange={(event) => onBin(event.target.value)}
            className={inputClass}
          />
        </label>
        <label className="flex min-w-48 flex-1 flex-col gap-1 text-xs text-surface-300">
          {m.history_object_entry_label()}
          <input
            value={entry}
            spellCheck={false}
            onChange={(event) => onEntry(event.target.value)}
            className={inputClass}
          />
        </label>
        <Button type="submit" size="sm" disabled={bin.trim() === "" || entry.trim() === ""}>
          {m.history_object_show_action()}
        </Button>
      </form>
      {asked == null && (
        <p className="text-sm text-surface-400 select-none">{m.history_object_hint()}</p>
      )}
      {asked != null && object.isPending && <Spinner />}
      {object.isError && <FailureNote failure={object.error} />}
      {asked != null && object.isSuccess && object.data == null && (
        <p className="text-sm text-surface-400 select-none">
          {m.history_object_none({ version: at.version })}
        </p>
      )}
      {asked != null && object.data && (
        <div className="flex flex-col gap-3" data-ui="HistoryObject">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-surface-400">
            <span className="font-mono text-surface-100">{object.data.class}</span>
            <span>
              {m.history_object_wad_label()}{" "}
              <span className="font-mono break-all text-surface-200">{object.data.wad}</span>
            </span>
            <span>
              {m.history_object_life_label()}{" "}
              <span className="text-surface-200">{lifeShort(object.data.life)}</span>
            </span>
            <label className="ml-auto flex items-center gap-2 text-surface-300">
              {m.history_compare_label()}
              <BuildSelect
                builds={builds}
                value={other}
                onChange={onOther}
                label={m.history_compare_label()}
                none={m.history_compare_none()}
              />
            </label>
          </div>
          {other == null || other === at.position ? (
            <pre className={twMerge(preClass, "text-surface-200")}>
              {object.data.text}
              {object.data.cut && `\n${m.history_object_cut()}`}
            </pre>
          ) : (
            <Compare asked={asked} at={at} object={object.data} other={other} builds={builds} />
          )}
        </div>
      )}
    </SectionCard>
  );
}
