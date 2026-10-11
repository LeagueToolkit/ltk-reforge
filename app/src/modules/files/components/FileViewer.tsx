import { LockSimpleIcon } from "@phosphor-icons/react";
import { type QueryKey, useQuery } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { match } from "ts-pattern";

import { FailureNote, Spinner, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { FileRow, FileView } from "@/lib/bindings.gen";

import { fencedObject } from "../fence";
import { sizeLabel } from "../tree";
import { kindGroup } from "./FileTree";

/**
 * The read of one file's bytes as the app shows them: a kept run's, the last run's, or an
 * explored mod's.
 */
export interface FileQuery {
  queryKey: QueryKey;
  queryFn: () => Promise<FileView>;
}

// A text about the page's height less the page's padding and the badge row, so a jump that
// brings the viewer's top to the page's top is not stopped short by the page's end.
const preClass =
  "max-h-[calc(100vh-8rem)] overflow-auto rounded-lg border border-surface-700/50 bg-surface-950/40 p-3 font-mono text-xs whitespace-pre text-surface-200";

/** The object asked for, marked. It is the text's first, so the text's own scroll is at it. */
function AskedObject({ text }: { text: string }) {
  return (
    <span
      // The bar's width comes out of the left padding, so the text keeps its column.
      className="-mx-3 block border-l-4 border-accent-500 bg-accent-500/20 pr-3 pl-2"
      data-ui="FileViewer:entry"
    >
      {text}
    </span>
  );
}

/** A bin's objects, the one asked for first and marked. */
function BinText({ text, more, found }: { text: string; more: number; found: boolean | null }) {
  const { asked, rest } = fencedObject(text);
  return (
    <>
      {found === false && (
        <p className="text-xs text-surface-400 select-none" data-ui="FileViewer:entry-missing">
          {m.runs_file_entry_missing()}
        </p>
      )}
      <pre className={preClass} data-ui="FileViewer:text">
        {asked !== null && <AskedObject text={asked} />}
        {rest}
        {more > 0 && `\n${m.runs_file_more({ count: more })}`}
      </pre>
    </>
  );
}

/** What a file is shown as, by the kind its bytes have. */
function Shown({ view }: { view: FileView }) {
  return match(view)
    .with({ as: "bin" }, ({ text, more, entry_found }) => (
      <BinText text={text} more={more} found={entry_found} />
    ))
    .with({ as: "text" }, ({ text, cut }) => (
      <pre className={preClass} data-ui="FileViewer:text">
        {text}
        {cut && `\n${m.runs_file_cut()}`}
      </pre>
    ))
    .with({ as: "image" }, ({ png, width, height, format }) => (
      <figure className="flex flex-col items-start gap-2" data-ui="FileViewer:image">
        <img
          src={png}
          alt={m.runs_file_image_alt()}
          className="max-h-[60vh] max-w-full rounded-md border border-surface-700 bg-surface-800 object-contain"
        />
        <figcaption className="text-xs text-surface-400 select-none">
          {m.runs_file_image_label({ width, height, format })}
        </figcaption>
      </figure>
    ))
    .with({ as: "hex" }, ({ head }) => (
      <pre className={preClass} data-ui="FileViewer:hex">
        {head}
      </pre>
    ))
    .exhaustive();
}

interface FileViewerProps {
  row: FileRow;
  /** The read of its bytes. */
  query: FileQuery;
  /** The entry of a bin's object asked for first, eight hex digits. */
  entry?: string | null;
  /** The entry's name, shown in place of its hash, which shows on hover. */
  entryName?: string;
  /**
   * The number of the jump that asked for the file. Each new one brings the viewer's top to the
   * top of the page and its text to its start, once the text shows.
   */
  landing?: number;
}

/**
 * One file, read-only: what it is, and its bytes as the app shows them. A bin's object asked
 * for comes first, marked, and the viewer scrolls to it.
 */
export function FileViewer({ row, query, entry = null, entryName, landing }: FileViewerProps) {
  // A file's bytes under their id never change, and one no tier holds is not asked again.
  const view = useQuery({ ...query, staleTime: Infinity, retry: false });
  const root = useRef<HTMLDivElement>(null);
  // The asked object is the text's first and may stand taller than the page, which a scroll
  // to its nearest edge leaves where it is: the viewer's top goes to the page's top, and the
  // text, which a person may have scrolled on a tab kept mounted, to its start.
  useEffect(() => {
    if (landing === undefined || !view.data) {
      return;
    }
    const text = root.current?.querySelector<HTMLElement>("[data-ui='FileViewer:text']");
    if (text) {
      text.scrollTop = 0;
    }
    root.current?.scrollIntoView({ block: "start" });
  }, [landing, view.data]);
  return (
    <div ref={root} className="flex scroll-mt-8 flex-col gap-3" data-ui="FileViewer">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md bg-surface-veil-soft px-3 py-2 text-xs">
        <span className="inline-flex items-center gap-1 rounded-full bg-surface-veil px-2 py-0.5 font-medium text-surface-200 select-none">
          <LockSimpleIcon weight="bold" className="h-3 w-3" />
          {m.runs_file_read_only()}
        </span>
        <span className="min-w-0 font-mono break-all text-surface-100">{row.path ?? row.hash}</span>
        {entry && (
          <span className="min-w-0 break-words text-surface-300" title={entryName && entry}>
            {m.runs_file_entry_label()} <span className="font-mono">{entryName ?? entry}</span>
          </span>
        )}
        <Tooltip content={<span className="font-mono text-xs">{row.kind}</span>}>
          <span className="text-surface-300">
            {m[`runs_file_kind.${kindGroup(row.kind)}.label`]()}
          </span>
        </Tooltip>
        <span className="text-surface-300 tabular-nums">{sizeLabel(row.size)}</span>
        <Tooltip content={<span className="font-mono text-xs">{row.content}</span>}>
          <span className="text-surface-400">
            {m.runs_file_content_label()}{" "}
            <span className="font-mono">{row.content.slice(0, 12)}</span>
          </span>
        </Tooltip>
      </div>
      {view.isPending && <Spinner />}
      {view.isError && <FailureNote failure={view.error} />}
      {view.data && <Shown view={view.data} />}
    </div>
  );
}
