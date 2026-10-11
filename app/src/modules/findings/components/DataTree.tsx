import { Fragment, type ReactNode, useMemo, useState } from "react";

import { m } from "@/i18n";

import { useRunRef, useSpelled } from "../api/run";
import { hashesIn, spellsAt } from "../hashes";

/** How many items of a list are drawn at first, and added by each "show more". */
const PAGE = 20;

/** A name for each id a finding's fields can hold: `%f12` is a file's path. */
export type IdNames = Partial<Record<string, string>>;

/**
 * A subject's name for a person: the name the run gave its id, else the one the finding
 * carries. A finding raised before a stage named its tables carries the id as its name.
 */
export function subjectName(subject: { id: string; name: string }, names: IdNames = {}): string {
  return names[subject.id] ?? subject.name;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Whether a finding's data holds a field to show. */
export function hasFields(data: unknown): boolean {
  return isRecord(data) && Object.keys(data).length > 0;
}

/** What a node of the tree draws with: the run's names of ids, and the hashes spelled. */
interface Spelling {
  names: IdNames;
  /** A name for each hash the run's names or the tables spell. */
  spelled: Record<string, string>;
}

/** Where a node is: the key it is under, and the key of the record holding it. */
interface At {
  key?: string;
  parent?: string;
}

function Leaf({ value, spelling, at }: { value: unknown; spelling: Spelling; at: At }) {
  if (typeof value === "string") {
    const hashed = spellsAt(value, at.key, at.parent) ? spelling.spelled[value] : undefined;
    const name = hashed ?? spelling.names[value];
    return name === undefined ? (
      <span className="break-all text-surface-200">{value}</span>
    ) : (
      <span
        className="break-all text-surface-200 underline decoration-surface-600 decoration-dotted underline-offset-2"
        title={value}
      >
        {name}
      </span>
    );
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return <span className="text-accent-300">{String(value)}</span>;
  }
  return <span className="text-surface-500">{"null"}</span>;
}

/** A list's first items, a page at a time. */
function Items({
  items,
  children,
}: {
  items: unknown[];
  children: (shown: unknown[]) => ReactNode;
}) {
  const [count, setCount] = useState(PAGE);
  const left = items.length - count;
  return (
    <>
      {children(items.slice(0, count))}
      {left > 0 && (
        <button
          type="button"
          onClick={() => setCount(count + PAGE)}
          className="cursor-pointer self-start font-sans text-surface-400 hover:text-surface-100"
        >
          {m.findings_more_action({ count: Math.min(PAGE, left) })}
        </button>
      )}
    </>
  );
}

function Node({ value, spelling, at }: { value: unknown; spelling: Spelling; at: At }) {
  if (Array.isArray(value)) {
    const items: unknown[] = value;
    if (items.length === 0) {
      return <span className="text-surface-500">{"[]"}</span>;
    }
    // A list's items are under the list's key.
    if (items.every((item) => !isRecord(item) && !Array.isArray(item))) {
      return (
        <span className="flex flex-col">
          <Items items={items}>
            {(shown) =>
              shown.map((item, index) => (
                <Leaf key={index} value={item} spelling={spelling} at={at} />
              ))
            }
          </Items>
        </span>
      );
    }
    return (
      <ol className="flex flex-col gap-1 border-l border-surface-700 pl-3">
        <Items items={items}>
          {(shown) =>
            shown.map((item, index) => (
              <li key={index} className="flex gap-2">
                <span className="text-surface-500">{index}</span>
                <Node value={item} spelling={spelling} at={at} />
              </li>
            ))
          }
        </Items>
      </ol>
    );
  }
  if (isRecord(value)) {
    return (
      <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-3 gap-y-0.5">
        {Object.entries(value).map(([key, field]) => (
          <Fragment key={key}>
            <dt className="text-surface-400">{key}</dt>
            <dd className="min-w-0">
              <Node value={field} spelling={spelling} at={{ key, parent: at.key }} />
            </dd>
          </Fragment>
        ))}
      </dl>
    );
  }
  return <Leaf value={value} spelling={spelling} at={at} />;
}

/**
 * A finding's fields as the library wrote them, each key beside its value or its fields. An id
 * the run names shows its name, and a hash its names or the tables spell shows that name
 * (#ADR-0103), each with the id or the hash as its title.
 */
export function DataTree({ value, names = {} }: { value: unknown; names?: IdNames }) {
  const run = useRunRef();
  const hashes = useMemo(() => hashesIn(value), [value]);
  const spelled = useSpelled(run, hashes);
  return (
    <div className="font-mono text-xs" data-ui="DataTree">
      <Node value={value} spelling={{ names, spelled }} at={{}} />
    </div>
  );
}
