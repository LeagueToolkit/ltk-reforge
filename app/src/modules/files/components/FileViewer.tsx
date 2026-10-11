import { LockSimpleIcon } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { match } from "ts-pattern";

import { FailureNote, Spinner, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { BinHeader, FileRow, FileView, RunRef } from "@/lib/bindings.gen";
import {
  type BinRef,
  binOpenQuery,
  BinTree,
  CollapseAllButton,
  type TreeReveal,
} from "@/modules/bin";

import { type BinPlace, type FileQuery, placeKey } from "../jump";
import { sizeLabel } from "../tree";
import { kindGroup } from "./FileTree";

// A box about the page's height less the page's padding and the badge row, so a jump that
// brings the viewer's top to the page's top is not stopped short by the page's end.
const boxClass =
  "max-h-[calc(100vh-8rem)] overflow-auto rounded-lg border border-surface-700/50 bg-surface-950/40 p-3 font-mono text-xs whitespace-pre text-surface-200";

const treeClass =
  "max-h-[calc(100vh-8rem)] rounded-lg border border-surface-700/50 bg-surface-950/40";

/** A run as its kind and number, which keep one identity across renders. */
function runOf(of: RunRef["of"], id: number | null): RunRef {
  return of === "kept" ? { of, id: id ?? 0 } : { of };
}

/** What a file other than a bin is shown as, by the kind its bytes have. */
function Shown({ view }: { view: Exclude<FileView, { as: "bin" }> }) {
  return match(view)
    .with({ as: "text" }, ({ text, cut }) => (
      <pre className={boxClass} data-ui="FileViewer:text">
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
      <pre className={boxClass} data-ui="FileViewer:hex">
        {head}
      </pre>
    ))
    .exhaustive();
}

/** What the header row says of a bin: its objects, and its version or that it is a patch. */
function BinFacts({ header }: { header: BinHeader }) {
  const facts =
    header.kind === "patch"
      ? [
          m.bin_patch_label(),
          m.bin_objects_label({ count: header.objects }),
          m.bin_records_label({ count: header.patches }),
          ...(header.deleted > 0 ? [m.bin_deleted_label({ count: header.deleted })] : []),
        ]
      : [
          m.bin_objects_label({ count: header.objects }),
          ...(header.version === null ? [] : [m.bin_version_label({ version: header.version })]),
          ...(header.dependencies > 0
            ? [m.bin_dependencies_label({ count: header.dependencies })]
            : []),
        ];
  return (
    <span className="flex flex-wrap items-center gap-x-3 text-surface-300" data-ui="FileViewer:bin">
      {facts.map((fact) => (
        <span key={fact} className="tabular-nums">
          {fact}
        </span>
      ))}
    </span>
  );
}

interface FileViewerProps {
  row: FileRow;
  /** The read of its bytes, and the run it is one of. */
  query: FileQuery;
  /** The place in a bin a jump asked for. */
  place?: BinPlace | null;
  /**
   * The number of the jump that asked for the file. Each new one brings the viewer's top to the
   * top of the page and the tree's row at the place to the top of the tree.
   */
  landing?: number;
}

/**
 * One file, read-only: what it is, and its bytes as the app shows them. A bin is a tree of its
 * rows (#ADR-0104), opened at the place a jump asked for.
 */
export function FileViewer({ row, query, place = null, landing }: FileViewerProps) {
  // A file's bytes under their id never change, and one no tier holds is not asked again.
  const view = useQuery({ ...query, staleTime: Infinity, retry: false });
  const isBin = view.data?.as === "bin";
  const kept = query.run.of === "kept" ? query.run.id : null;
  const bin = useMemo<BinRef>(
    () => ({ run: runOf(query.run.of, kept), content: row.content }),
    [query.run.of, kept, row.content],
  );
  const opened = useQuery({ ...binOpenQuery(bin), enabled: isBin });
  const [collapses, setCollapses] = useState(0);
  const [open, setOpen] = useState(false);
  const reveal = useMemo<TreeReveal | null>(
    () =>
      place === null || landing === undefined ? null : { key: placeKey(place), token: landing },
    [place, landing],
  );

  const root = useRef<HTMLDivElement>(null);
  // The viewer's top goes to the page's top on each jump; the tree scrolls its own row to its
  // top.
  const ready = view.data !== undefined && (!isBin || opened.data !== undefined);
  useEffect(() => {
    if (landing !== undefined && ready) {
      root.current?.scrollIntoView({ block: "start" });
    }
  }, [landing, ready]);
  return (
    <div ref={root} className="flex scroll-mt-8 flex-col gap-3" data-ui="FileViewer">
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
        {opened.data && <BinFacts header={opened.data.header} />}
        {opened.data && (
          <span className="ml-auto">
            <CollapseAllButton
              disabled={!open}
              onCollapse={() => setCollapses((count) => count + 1)}
            />
          </span>
        )}
      </div>
      {(view.isPending || (isBin && opened.isPending)) && <Spinner />}
      {view.isError && <FailureNote failure={view.error} />}
      {opened.isError && <FailureNote failure={opened.error} />}
      {view.data && view.data.as !== "bin" && <Shown view={view.data} />}
      {opened.data && (
        <BinTree
          // A file's tree opens as it does at first.
          key={row.content}
          bin={bin}
          roots={opened.data.rows}
          label={m.bin_tree_label()}
          reveal={reveal}
          collapseAllSignal={collapses}
          onOpenChange={setOpen}
          className={treeClass}
        />
      )}
    </div>
  );
}
