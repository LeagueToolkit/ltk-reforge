import { CaretRightIcon, MagnifyingGlassIcon } from "@phosphor-icons/react";
import { Link, useParams } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";

import { m } from "@/i18n";
import type { RecordHead, Series } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { matches, SERIES, statusWord } from "../heads";
import { routeCode } from "../links";
import { RecordTitle } from "./RecordTitle";

/**
 * A record's line: its code and title, and its status when it is not accepted. The line of the
 * record opened is scrolled into the list's view, for a record opened from a link elsewhere.
 */
function RecordLine({ head, active }: { head: RecordHead; active: boolean }) {
  const status = statusWord(head);
  const line = useRef<HTMLAnchorElement>(null);
  useEffect(() => {
    // The list alone scrolls, to the line's middle: scrolling the page too would move the
    // sticky list under it, and the record opened scrolls the page back to its top.
    const shown = line.current;
    const list = shown?.closest("nav");
    if (!active || !shown || !list) {
      return;
    }
    const box = list.getBoundingClientRect();
    const at = shown.getBoundingClientRect();
    if (at.top < box.top || at.bottom > box.bottom) {
      list.scrollTop += at.top - box.top - (box.height - at.height) / 2;
    }
  }, [active]);
  return (
    <Link
      ref={line}
      to="/records/$code"
      params={{ code: routeCode(head.code) }}
      // In a narrow list the title goes under the code rather than a word a line beside it.
      className="flex w-full flex-wrap items-baseline gap-x-2 rounded-md px-3 py-1.5 text-left select-none hover:bg-surface-veil data-[status=active]:bg-accent-500/10 data-[status=active]:ring-1 data-[status=active]:ring-accent-500/35 data-[status=active]:ring-inset"
      data-ui="RecordList:row"
    >
      <span className="shrink-0 font-mono text-xs text-surface-400">{routeCode(head.code)}</span>
      <span className="min-w-40 flex-1 text-sm text-surface-100">
        <RecordTitle title={head.title} />
      </span>
      {status && <span className="shrink-0 text-xs text-surface-500">{status}</span>}
    </Link>
  );
}

interface RecordGroupProps {
  series: Series;
  heads: RecordHead[];
  /** The route's part of the code of the record opened. */
  selected: string | undefined;
  collapsed: boolean;
  onToggle: () => void;
}

/** A series' records under its heading, which folds them. */
function RecordGroup({ series, heads, selected, collapsed, onToggle }: RecordGroupProps) {
  return (
    <section className="flex flex-col gap-1" data-ui="RecordList:group">
      <button
        type="button"
        aria-expanded={!collapsed}
        onClick={onToggle}
        className="group flex cursor-pointer items-center gap-2 rounded-md px-3 py-1 text-xs font-medium tracking-wide text-surface-400 uppercase select-none hover:text-surface-200"
      >
        <CaretRightIcon
          weight="bold"
          className="h-3 w-3 shrink-0 transition-transform group-aria-expanded:rotate-90"
        />
        {m[`records_series.${series}.group`]()}
        <span className="ml-auto font-normal text-surface-500">{heads.length}</span>
      </button>
      {!collapsed && (
        <ul className="flex flex-col">
          {heads.map((head) => (
            <li key={head.code}>
              <RecordLine head={head} active={routeCode(head.code) === selected} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

interface RecordListProps {
  heads: RecordHead[];
  className?: string;
}

/**
 * The records, grouped by series, filtered by code and title. The record of the route is
 * marked; a line opens its record by the route. Which groups are folded lasts while the page
 * is open.
 */
export function RecordList({ heads, className }: RecordListProps) {
  const { code: selected } = useParams({ strict: false });
  const [filter, setFilter] = useState("");
  const [collapsed, setCollapsed] = useState<ReadonlySet<Series>>(new Set());
  const shown = heads.filter((head) => matches(head, filter));
  const toggle = (series: Series) => {
    const next = new Set(collapsed);
    if (!next.delete(series)) {
      next.add(series);
    }
    setCollapsed(next);
  };
  return (
    <div className={twMerge("flex flex-col gap-3", className)} data-ui="RecordList">
      <label className="flex items-center gap-2 rounded-md border border-surface-600 bg-surface-950/40 px-2 focus-within:outline-2 focus-within:outline-accent-500">
        <MagnifyingGlassIcon className="h-3.5 w-3.5 shrink-0 text-surface-400" />
        <input
          type="search"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder={m.records_filter_placeholder()}
          aria-label={m.records_filter_label()}
          className="h-8 min-w-0 flex-1 bg-transparent text-xs text-surface-100 outline-none placeholder:text-surface-500"
        />
      </label>
      {shown.length === 0 && (
        <p className="text-sm text-surface-400 select-none">{m.records_none_match()}</p>
      )}
      {/* The list scrolls under the filter, which stays in view. */}
      <nav
        aria-label={m.records_list_label()}
        className="flex min-h-0 flex-col gap-3 overflow-x-hidden overflow-y-auto"
      >
        {SERIES.map((series) => {
          const group = shown.filter((head) => head.series === series);
          return (
            group.length > 0 && (
              <RecordGroup
                key={series}
                series={series}
                heads={group}
                selected={selected}
                collapsed={collapsed.has(series)}
                onToggle={() => toggle(series)}
              />
            )
          );
        })}
      </nav>
    </div>
  );
}
