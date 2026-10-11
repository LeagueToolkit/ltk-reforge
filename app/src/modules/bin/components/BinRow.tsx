/**
 * One line of a bin's tree: a row's name, its tag and its value, or the line under a node whose
 * rows have not all answered.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/components/BinRow.tsx`. Cut: every
 * edit action and mark, the declared, override and change marks, the field and class cards,
 * the mismatch mark of a declared kind, and the hover action that opens an object.
 */
import {
  CaretRightIcon,
  CubeIcon,
  CubeTransparentIcon,
  SpinnerGapIcon,
  WarningCircleIcon,
} from "@phosphor-icons/react";
import { useMemo } from "react";

import { Tooltip } from "@/components";
import { describeError, m } from "@/i18n";
import type { AppError, BinRow } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { useGuideLevels } from "../guides";
import { rowTag } from "../kindTag";
import {
  canExpand,
  guideBlocks,
  INDENT,
  lineParent,
  MAX_INDENT_DEPTH,
  type MoreLine,
  outsideColumn,
  repeatsKey,
  type RowLine,
} from "../rows";
import { CutText } from "./CutText";
import { ClassName, RowValue } from "./RowValue";

/** A row's value as wide as it draws. */
const HUG_VALUE_CLASS = "flex-initial";

/* The mark of the row a jump landed on: the accent bar and tint the viewer marks the object a
   jump asked for with. An inset shadow, so the bar takes no room from the row's columns. */
const FOCUSED_CLASS = "bg-accent-500/20 shadow-[inset_4px_0_0_var(--color-accent-500)]";

interface RowLineProps {
  line: RowLine;
  /** A reveal landed on this row. */
  focused: boolean;
  /** The fetch of the rows under this one failed. */
  error?: AppError;
  onToggle: (key: string) => void;
  /** The row is the tree's one tab stop. */
  tabStop?: boolean;
}

/** One node of the bin: its name, its kind as a tag, and its value. */
export function BinRowLine({ line, focused, error, onToggle, tabStop = false }: RowLineProps) {
  const { row, depth, expanded, loading } = line;
  const expandable = canExpand(row);

  return (
    <div
      data-ui="BinTree:row"
      data-tree-row={line.key}
      data-focused={focused || undefined}
      role="treeitem"
      aria-level={depth + 1}
      aria-expanded={expandable ? expanded : undefined}
      aria-selected={focused}
      tabIndex={tabStop ? 0 : -1}
      className={twMerge(
        /* DS-VEIL, DS-RADIUS. No transition: a fade under a pointer crossing a list of short
           rows reads as a flicker rather than as a highlight. */
        "group/row flex min-h-6 items-center gap-2 rounded-sm pr-2 text-mono-row outline-none hover:bg-surface-veil-soft focus-visible:bg-surface-veil",
        expandable && "cursor-pointer",
        focused && FOCUSED_CLASS,
      )}
      onClick={() => expandable && onToggle(line.key)}
    >
      <NameCell line={line} expandable={expandable} expanded={expanded} loading={loading} />
      {row.value.type === "records" ? (
        <span className="shrink-0 text-meta text-surface-400">{row.value.len}</span>
      ) : (
        <RowValue row={row} className={HUG_VALUE_CLASS} />
      )}
      {error && (
        <Tooltip content={describeError(error).title}>
          <WarningCircleIcon className="size-3.5 shrink-0 text-warning-text" />
        </Tooltip>
      )}
    </div>
  );
}

/** The line under a node whose rows have not all answered. */
export function MoreRow({ line }: { line: MoreLine }) {
  return (
    <div className="flex h-6 items-center gap-2 pr-2 text-meta text-surface-400">
      <Guides depth={line.depth} parent={lineParent(line)} />
      <span className="w-3 shrink-0" />
      <SpinnerGapIcon className="size-3 animate-spin" />
      <span>{m.bin_more_label({ loaded: line.loaded, total: line.total })}</span>
    </div>
  );
}

interface GuidesProps {
  depth: number;
  /** The key of the row the line hangs under. Null at depth zero. */
  parent: string | null;
}

/**
 * One guide per open level, each under the caret of the level it belongs to.
 *
 * A guide runs the line's full height, so a block's guides join into one edge. The block the
 * reader stands in takes the accent, and the block under the pointer lifts a rung.
 */
function Guides({ depth, parent }: GuidesProps) {
  const blocks = useMemo(() => guideBlocks(parent, depth), [parent, depth]);
  const { active, hover } = useGuideLevels(blocks);
  const indented = Math.min(depth, MAX_INDENT_DEPTH);
  const tone = (level: number) =>
    twMerge(
      "shrink-0 border-l border-surface-700/60",
      level === hover && "border-surface-600",
      level === active && "border-accent-500",
      level >= indented && "w-0.5",
    );
  return (
    <span className="flex shrink-0 translate-x-[6px] self-stretch" aria-hidden>
      {Array.from({ length: depth }, (_, level) => (
        <span
          key={level}
          className={tone(level)}
          style={level < indented ? { width: INDENT } : undefined}
        />
      ))}
    </span>
  );
}

interface CaretProps {
  expandable: boolean;
  expanded: boolean;
  loading: boolean;
}

function Caret({ expandable, expanded, loading }: CaretProps) {
  return (
    <span className="flex h-4 w-3 shrink-0 items-center justify-center text-surface-400">
      {loading && <SpinnerGapIcon className="size-3 animate-spin" />}
      {!loading && expandable && (
        <CaretRightIcon weight="bold" className={twMerge("size-3", expanded && "rotate-90")} />
      )}
    </span>
  );
}

interface NameCellProps {
  line: RowLine;
  expandable: boolean;
  expanded: boolean;
  loading: boolean;
}

/**
 * The row's name, and its tag after it.
 *
 * The indent is inside this cell rather than beside it, so the value column starts at one x
 * whatever the depth is and a run of rows reads as a column.
 */
function NameCell({ line, expandable, expanded, loading }: NameCellProps) {
  const { row, depth } = line;
  /* A target is an object of another file, drawn as the heading its records sit under. */
  const target = row.node === "target";
  const object = row.node === "object" || target;
  const element = row.node === "element";
  const held = element && row.value.type === "struct" ? row.value : null;
  const nameClass = twMerge(
    /* An element's index is what a reader counts rows by, so the class beside it elides first. */
    element ? "shrink-0" : "truncate",
    object ? "font-medium text-surface-100" : "text-surface-200",
    element && "text-surface-400",
    row.unnamed && "text-surface-400",
  );
  const Glyph = target ? CubeTransparentIcon : CubeIcon;

  return (
    <span
      className={twMerge(
        "flex min-w-0 shrink-0 items-center gap-1.5 self-stretch",
        /* An element sits outside the column: its value follows its index rather than starting
           where a property's value does. A target draws no class after its path, so the path
           takes the line and cuts in its middle, where the paths of one patch share folders. */
        target && "min-w-0 flex-1",
        !target && outsideColumn(row.node) && "max-w-[60%]",
        !outsideColumn(row.node) && "w-[min(calc(var(--bin-name-cols)*1ch+2rem),50%)]",
      )}
    >
      <Guides depth={depth} parent={lineParent(line)} />
      <Caret expandable={expandable} expanded={expanded} loading={loading} />
      {object && <Glyph className="size-3.5 shrink-0 text-surface-400" />}
      {target ? (
        <CutText text={row.name} className={nameClass} />
      ) : (
        <span className={nameClass} title={row.label || undefined}>
          {row.name}
        </span>
      )}
      {repeatsKey(row) && (
        <Tooltip content={m.bin_repeated_key_hint()}>
          <WarningCircleIcon
            aria-label={m.bin_repeated_key_hint()}
            className="size-3.5 shrink-0 text-warning-text"
          />
        </Tooltip>
      )}
      {held && <ClassName classHash={held.classHash} name={held.class} />}
      {!object && !element && <KindTag row={row} />}
    </span>
  );
}

/* DS-KIND-HUE, DS-TEXT. Dimmed only as far as AA contrast allows in both themes. */
const TAG_CLASS = "shrink-0 text-bin-kind-text/85";

/** The row's kind in ritobin's words. */
function KindTag({ row }: { row: BinRow }) {
  const tag = rowTag(row);
  if (tag === null) return null;
  return <span className={TAG_CLASS}>{tag}</span>;
}
