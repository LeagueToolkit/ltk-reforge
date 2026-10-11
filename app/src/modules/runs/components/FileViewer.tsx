import { LockSimpleIcon } from "@phosphor-icons/react";
import { type QueryKey, useQuery } from "@tanstack/react-query";
import { match } from "ts-pattern";

import { FailureNote, Spinner, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { FileRow, FileView } from "@/lib/bindings.gen";

import { sizeLabel } from "../tree";
import { kindGroup } from "./FileTree";

/** The read of one file's bytes as the app shows them: a kept run's, or an explored mod's. */
export interface FileQuery {
  queryKey: QueryKey;
  queryFn: () => Promise<FileView>;
}

const preClass =
  "max-h-[60vh] overflow-auto rounded-lg border border-surface-700/50 bg-surface-950/40 p-3 font-mono text-xs whitespace-pre text-surface-200";

/** What a file is shown as, by the kind its bytes have. */
function Shown({ view }: { view: FileView }) {
  return match(view)
    .with({ as: "bin" }, ({ text, more }) => (
      <pre className={preClass} data-ui="FileViewer:text">
        {text}
        {more > 0 && `\n${m.runs_file_more({ count: more })}`}
      </pre>
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
}

/** One file, read-only: what it is, and its bytes as the app shows them. */
export function FileViewer({ row, query }: FileViewerProps) {
  // A file's bytes under their id never change, and one no tier holds is not asked again.
  const view = useQuery({ ...query, staleTime: Infinity, retry: false });
  return (
    <div className="flex flex-col gap-3" data-ui="FileViewer">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md bg-surface-veil-soft px-3 py-2 text-xs">
        <span className="inline-flex items-center gap-1 rounded-full bg-surface-veil px-2 py-0.5 font-medium text-surface-200 select-none">
          <LockSimpleIcon weight="bold" className="h-3 w-3" />
          {m.runs_file_read_only()}
        </span>
        <span className="min-w-0 font-mono break-all text-surface-100">{row.path ?? row.hash}</span>
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
