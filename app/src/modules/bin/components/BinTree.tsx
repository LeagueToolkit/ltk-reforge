/**
 * A bin's rows as a tree, read-only, a window at a time.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/components/BinTree.tsx`. Cut: the
 * edits, the add lines, the dependencies rows, the context menu, the link and class contexts,
 * the object tab's roots and the bounded height of a class view's section.
 */
import {
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { BinRow } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import type { BinRef } from "../api";
import { createGuideStore, GuideStoreContext } from "../guides";
import { type TreeReveal, useReveal } from "../hooks/useReveal";
import { useRowWindow } from "../hooks/useRowWindow";
import { isCollapseAllKey, useTreeNavigation } from "../hooks/useTreeNavigation";
import { useNextPages, useTreeRows } from "../hooks/useTreeRows";
import { useValueMarks, ValueMarksContext } from "../hooks/useValueMarks";
import { rowTag } from "../kindTag";
import { lineParent, nameColumns, rowKey, sortedRoots, type VisibleRow } from "../rows";
import { BinRowLine, MoreRow } from "./BinRow";

export interface BinTreeProps {
  /** The bin the rows are read from. */
  bin: BinRef;
  /** The rows at depth zero, as the open answered them. */
  roots: readonly BinRow[];
  /** The tree's accessible name. */
  label: string;
  /** A row to open down to, focus and scroll to the top. */
  reveal?: TreeReveal | null;
  /** A count the header's collapse-all button raises, which collapses every open row. */
  collapseAllSignal?: number;
  /** Hears whether any row is open, which is when collapsing all does something. */
  onOpenChange?: (open: boolean) => void;
  /** The scroll box's own classes, such as its height. */
  className?: string;
}

/**
 * The rows of one bin as a tree, a window at a time.
 *
 * The tree stays in the backend. `useTreeRows` holds the expansion state and the lines it
 * produces, and `useRowWindow` draws the ones on screen. A bin of one object opens it, since
 * a collapsed single row is a bin that says nothing.
 */
export function BinTree({
  bin,
  roots: opened,
  label,
  reveal = null,
  collapseAllSignal = 0,
  onOpenChange,
  className,
}: BinTreeProps) {
  const roots = useMemo(() => sortedRoots(opened), [opened]);
  const [initialExpanded] = useState(() => (roots.length === 1 ? [rowKey(roots[0]!)] : []));
  const {
    visible,
    loaded,
    toggle: toggleRow,
    collapseAll: collapseRows,
    expand,
    requestMore,
  } = useTreeRows(bin, roots, initialExpanded);

  const scrollRef = useRef<HTMLDivElement>(null);
  const { items, lines, totalSize, measureElement, scrollToKey } = useRowWindow(scrollRef, visible);
  useNextPages(lines, requestMore);

  const { focused, clearFocus } = useReveal(reveal, {
    roots,
    visible,
    loaded,
    expand,
    requestMore,
    scrollToKey,
  });
  const toggle = useCallback(
    (key: string) => {
      clearFocus();
      toggleRow(key);
    },
    [clearFocus, toggleRow],
  );

  const collapseAll = useCallback(() => {
    clearFocus();
    collapseRows();
    scrollRef.current?.scrollTo({ top: 0, behavior: "instant" });
  }, [clearFocus, collapseRows]);

  const collapsedFor = useRef(collapseAllSignal);
  useEffect(() => {
    if (collapseAllSignal === collapsedFor.current) return;
    collapsedFor.current = collapseAllSignal;
    collapseAll();
  }, [collapseAllSignal, collapseAll]);

  const open = visible.some((line) => line.kind === "row" && line.expanded);
  useEffect(() => onOpenChange?.(open), [open, onOpenChange]);

  const navigation = useTreeNavigation({ visible, scrollRef, scrollToKey, drawn: items, toggle });

  function handleKeyDown(event: ReactKeyboardEvent<HTMLElement>) {
    if (navigation.keyDown(event)) {
      event.preventDefault();
      return;
    }
    if (!isCollapseAllKey(event)) return;
    event.preventDefault();
    collapseAll();
  }

  const inView = useMemo(
    () => lines.flatMap((line) => (line.kind === "row" ? [line.row] : [])),
    [lines],
  );
  const marks = useValueMarks(bin, inView);

  /* One width for the whole list, so the values stay in a column while no name elides that
     could have fitted. */
  const nameCols = useMemo(() => nameColumns(visible, rowTag), [visible]);

  /* Outside React state, so a pointer crossing the rows redraws the guides and nothing else. */
  const [guides] = useState(createGuideStore);
  function lineAt(target: EventTarget): VisibleRow | null {
    let wrapper: HTMLElement | null = target as HTMLElement;
    while (wrapper !== null && wrapper.dataset.index === undefined) wrapper = wrapper.parentElement;
    const index = Number(wrapper?.dataset.index);
    return Number.isInteger(index) ? (visible[index] ?? null) : null;
  }
  function showGuidesAt(target: EventTarget) {
    const line = lineAt(target);
    if (line !== null) guides.set({ active: lineParent(line) });
  }
  useEffect(() => {
    const line = visible.find((candidate) => candidate.key === focused);
    if (line !== undefined) guides.set({ active: lineParent(line) });
  }, [focused, guides, visible]);

  return (
    <GuideStoreContext value={guides}>
      <ValueMarksContext value={marks}>
        <div
          ref={scrollRef}
          role="tree"
          aria-label={label}
          data-ui="BinTree"
          className={twMerge(
            "overflow-auto px-1 py-1 font-mono outline-none select-none",
            className,
          )}
          style={{ "--bin-name-cols": nameCols } as CSSProperties}
          onKeyDown={handleKeyDown}
          onPointerDown={(event) => showGuidesAt(event.target)}
          onFocus={(event) => {
            showGuidesAt(event.target);
            navigation.focused(event.target);
          }}
          onMouseOver={(event) => {
            const line = lineAt(event.target);
            guides.set({ hover: line === null ? null : lineParent(line) });
          }}
          onMouseLeave={() => guides.set({ hover: null })}
        >
          <div className="relative w-full" style={{ height: totalSize }}>
            {items.map((item) => {
              const line = visible[item.index];
              if (!line) return null;
              return (
                <div
                  key={item.key}
                  ref={measureElement}
                  data-index={item.index}
                  className="absolute top-0 left-0 w-full"
                  style={{ transform: `translateY(${item.start}px)` }}
                >
                  {line.kind === "row" && (
                    <BinRowLine
                      line={line}
                      focused={line.key === focused}
                      tabStop={line.key === navigation.tabStop}
                      error={loaded.get(line.key)?.error}
                      onToggle={toggle}
                    />
                  )}
                  {line.kind === "more" && <MoreRow line={line} />}
                </div>
              );
            })}
          </div>
        </div>
      </ValueMarksContext>
    </GuideStoreContext>
  );
}
