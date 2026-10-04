import { type ReactNode, useState } from "react";

import { Button, Disclosure } from "@/components";
import { m } from "@/i18n";
import type { ObjectView } from "@/lib/bindings.gen";

import { type ChangeItem, groupBy, splitEntry } from "../model";
import { ChangeRow, OutcomeCounts } from "./ChangeRow";

/** How many rows a list shows before it asks to show more. */
const PAGE = 50;

/** The first rows of a list, a page at a time, with a button for the next page. */
export function Paged<T>({ items, children }: { items: T[]; children: (shown: T[]) => ReactNode }) {
  const [count, setCount] = useState(PAGE);
  const left = items.length - count;
  return (
    <>
      {children(items.slice(0, count))}
      {left > 0 && (
        <Button
          size="xs"
          variant="ghost"
          className="self-start"
          onClick={() => setCount(count + PAGE)}
        >
          {m.changes_more_action({ count: Math.min(PAGE, left) })}
        </Button>
      )}
    </>
  );
}

function ClassChip({ name }: { name: string }) {
  return (
    <span className="rounded bg-surface-800 px-1.5 font-mono text-[11px] text-surface-300">
      {name}
    </span>
  );
}

function BinPath({ path }: { path: string }) {
  return <span className="font-mono text-[11px] break-all text-surface-500">{path}</span>;
}

/** The one value every object of a list has, else none. */
function shared(objects: (ObjectView | undefined)[], of: (object: ObjectView) => string) {
  const values = new Set(objects.map((object) => (object ? of(object) : undefined)));
  const [only] = values;
  return values.size === 1 ? only : undefined;
}

interface ObjectSummaryProps {
  object: ObjectView | undefined;
  entry: string;
  items: ChangeItem[];
  /** Whether the row names the object's class and bin, which the list does not name for all. */
  showClass: boolean;
  showBin: boolean;
}

function ObjectSummary({ object, entry, items, showClass, showBin }: ObjectSummaryProps) {
  const { folder, name } = splitEntry(object?.entry ?? entry);
  return (
    <span className="flex flex-col gap-0.5">
      <span className="flex flex-wrap items-baseline gap-x-2">
        <span className="font-mono text-xs break-all">
          <span className="text-surface-400">{folder}</span>
          <span className="text-surface-50">{name}</span>
        </span>
        {object && showClass && <ClassChip name={object.class} />}
        <OutcomeCounts items={items} />
      </span>
      {object && showBin && <BinPath path={object.bin} />}
    </span>
  );
}

export interface ObjectGroupsProps {
  items: ChangeItem[];
  /** The name of a fixup, for groups that mix fixups. */
  fixupLabel?: (fixup: string) => string;
}

/**
 * Changes grouped by the object they are in, each object opening to its values. A list of one
 * object is open from the start. A class or a bin every object of the list shares is named once,
 * above it.
 */
export function ObjectGroups({ items, fixupLabel }: ObjectGroupsProps) {
  const groups = groupBy(items, (item) => item.change.object);
  const objects = groups.map((group) => group.items[0].object);
  const many = groups.length > 1;
  const sharedClass = many ? shared(objects, (object) => object.class) : undefined;
  const sharedBin = many ? shared(objects, (object) => object.bin) : undefined;
  return (
    <div className="flex flex-col" data-ui="ObjectGroups">
      {(sharedClass ?? sharedBin) && (
        <p className="flex flex-wrap items-baseline gap-x-2 pb-1" data-ui="ObjectGroups:shared">
          {sharedClass && <ClassChip name={sharedClass} />}
          {sharedBin && <BinPath path={sharedBin} />}
        </p>
      )}
      <Paged items={groups}>
        {(shown) =>
          shown.map((group) => (
            <Disclosure
              // A filter that leaves one object opens it.
              key={many ? group.key : `${group.key}:only`}
              defaultOpen={!many}
              summary={
                <ObjectSummary
                  object={group.items[0].object}
                  entry={group.items[0].change.entry}
                  items={group.items}
                  showClass={sharedClass === undefined}
                  showBin={sharedBin === undefined}
                />
              }
              summaryClassName="py-1.5"
              panelClassName="flex flex-col pb-2 pl-5"
            >
              <Paged items={group.items}>
                {(rows) => (
                  <ul className="divide-y divide-surface-800">
                    {rows.map((item, at) => (
                      <ChangeRow key={at} change={item.change} fixup={fixupLabel?.(item.fixup)} />
                    ))}
                  </ul>
                )}
              </Paged>
            </Disclosure>
          ))
        }
      </Paged>
    </div>
  );
}
