import { useEffect, useRef } from "react";

import { FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";
import type { Series } from "@/lib/bindings.gen";

import { useRecord, useRecords } from "../api/records";
import { routeCode } from "../links";
import { RecordLink } from "./RecordLink";
import { RecordMarkdown } from "./RecordMarkdown";

/** A record's series, in a word. */
function SeriesBadge({ series }: { series: Series }) {
  return (
    <span
      className="shrink-0 rounded-full bg-surface-veil px-2 py-0.5 text-[11px] font-medium text-surface-200 select-none"
      data-ui="SeriesBadge"
    >
      {m[`records_series.${series}.label`]()}
    </span>
  );
}

interface CodeLinksProps {
  title: string;
  codes: string[];
  known: (code: string) => boolean;
}

/** Codes under a heading, each a link when the bundle holds it. None draws nothing. */
function CodeLinks({ title, codes, known }: CodeLinksProps) {
  if (codes.length === 0) {
    return null;
  }
  return (
    <section className="flex flex-col gap-1.5" data-ui="RecordDetail:links">
      <h3 className="text-xs font-medium tracking-wide text-surface-400 uppercase select-none">
        {title}
      </h3>
      <ul className="flex flex-wrap gap-x-3 gap-y-1 text-xs">
        {codes.map((code) => (
          <li key={code}>
            {known(code) ? (
              <RecordLink code={code} />
            ) : (
              <span className="font-mono text-surface-400">{code}</span>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

/**
 * A record opened: its title, code and series, its text, and what it cites and is cited by.
 * Another record opens at its top.
 */
export function RecordDetail({ code }: { code: string }) {
  const top = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // The shell's main pane scrolls, not the window the router resets on a navigation.
    top.current?.closest("main")?.scrollTo({ top: 0 });
  }, [code]);
  return (
    <div ref={top}>
      <RecordBody code={code} />
    </div>
  );
}

function RecordBody({ code }: { code: string }) {
  const record = useRecord(code);
  const records = useRecords();
  const bundled = new Set(records.data?.map((head) => head.code));
  const known = (cited: string) => bundled.has(cited);

  if (record.isPending) {
    return <Spinner />;
  }
  if (record.isError) {
    return <FailureNote failure={record.error} />;
  }
  const view = record.data;
  if (!view) {
    return (
      <p className="text-sm text-surface-400 select-none">
        {m.records_unknown({ code: routeCode(code) })}
      </p>
    );
  }
  return (
    <article className="flex flex-col gap-6" data-ui="RecordDetail">
      <header className="flex flex-col gap-2">
        <h2 className="font-display text-xl font-semibold text-surface-50">{view.head.title}</h2>
        <p className="flex items-center gap-2 text-xs text-surface-400">
          <span className="font-mono">{routeCode(view.head.code)}</span>
          <SeriesBadge series={view.head.series} />
        </p>
      </header>
      <RecordMarkdown markdown={view.markdown} known={known} />
      <footer className="flex flex-col gap-4 border-t border-surface-700/50 pt-4">
        <CodeLinks title={m.records_cites_title()} codes={view.cites} known={known} />
        <CodeLinks title={m.records_cited_by_title()} codes={view.cited_by} known={known} />
      </footer>
    </article>
  );
}
