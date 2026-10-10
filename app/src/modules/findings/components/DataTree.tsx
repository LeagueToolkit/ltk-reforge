import { Fragment, type ReactNode, useState } from "react";

import { m } from "@/i18n";

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

function Leaf({ value, names }: { value: unknown; names: IdNames }) {
  if (typeof value === "string") {
    const name = names[value];
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

function Node({ value, names }: { value: unknown; names: IdNames }) {
  if (Array.isArray(value)) {
    const items: unknown[] = value;
    if (items.length === 0) {
      return <span className="text-surface-500">{"[]"}</span>;
    }
    if (items.every((item) => !isRecord(item) && !Array.isArray(item))) {
      return (
        <span className="flex flex-col">
          <Items items={items}>
            {(shown) => shown.map((item, at) => <Leaf key={at} value={item} names={names} />)}
          </Items>
        </span>
      );
    }
    return (
      <ol className="flex flex-col gap-1 border-l border-surface-700 pl-3">
        <Items items={items}>
          {(shown) =>
            shown.map((item, at) => (
              <li key={at} className="flex gap-2">
                <span className="text-surface-500">{at}</span>
                <Node value={item} names={names} />
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
              <Node value={field} names={names} />
            </dd>
          </Fragment>
        ))}
      </dl>
    );
  }
  return <Leaf value={value} names={names} />;
}

/**
 * A finding's fields as the library wrote them, each key beside its value or its fields. An id
 * the run names shows its name, the id as its title.
 */
export function DataTree({ value, names = {} }: { value: unknown; names?: IdNames }) {
  return (
    <div className="font-mono text-xs" data-ui="DataTree">
      <Node value={value} names={names} />
    </div>
  );
}
